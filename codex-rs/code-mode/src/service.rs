use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::CodeModeNestedToolCall;
use codex_code_mode_protocol::CodeModeSession;
use codex_code_mode_protocol::CodeModeSessionDelegate;
use codex_code_mode_protocol::CodeModeSessionProvider;
use codex_code_mode_protocol::CodeModeSessionProviderFuture;
use codex_code_mode_protocol::CodeModeSessionResultFuture;
use codex_code_mode_protocol::CodeModeToolKind;
use codex_code_mode_protocol::DEFAULT_EXEC_YIELD_TIME_MS;
use codex_code_mode_protocol::ExecuteRequest;
use codex_code_mode_protocol::ExecuteToPendingOutcome;
use codex_code_mode_protocol::FunctionCallOutputContentItem;
use codex_code_mode_protocol::ImageDetail;
use codex_code_mode_protocol::NotificationFuture;
use codex_code_mode_protocol::RuntimeResponse;
use codex_code_mode_protocol::StartedCell;
use codex_code_mode_protocol::ToolInvocationFuture;
use codex_code_mode_protocol::WaitOutcome;
use codex_code_mode_protocol::WaitRequest;
use codex_code_mode_protocol::WaitToPendingOutcome;
use codex_code_mode_protocol::WaitToPendingRequest;
use serde_json::Value as JsonValue;
use tokio::sync::Mutex;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use crate::cell_actor::CellActor;
use crate::cell_actor::CellError;
use crate::cell_actor::CellEvent;
use crate::cell_actor::CellEventFuture;
use crate::cell_actor::CellHandle;
use crate::cell_actor::CellHost;
use crate::cell_actor::CellImageDetail;
use crate::cell_actor::CellOutputItem;
use crate::cell_actor::CellRequest;
use crate::cell_actor::CellToolCall;
use crate::cell_actor::CellToolDefinition;
use crate::cell_actor::CellToolKind;
use crate::cell_actor::CellToolName;
use crate::cell_actor::ObserveMode;

pub struct NoopCodeModeSessionDelegate;

impl CodeModeSessionDelegate for NoopCodeModeSessionDelegate {
    fn invoke_tool<'a>(
        &'a self,
        _invocation: CodeModeNestedToolCall,
        cancellation_token: CancellationToken,
    ) -> ToolInvocationFuture<'a> {
        Box::pin(async move {
            cancellation_token.cancelled().await;
            Err("code mode nested tools are unavailable".to_string())
        })
    }

    fn notify<'a>(
        &'a self,
        _call_id: String,
        _cell_id: CellId,
        _text: String,
        _cancellation_token: CancellationToken,
    ) -> NotificationFuture<'a> {
        Box::pin(async { Ok(()) })
    }

    fn cell_closed(&self, _cell_id: &CellId) {}
}

#[derive(Default)]
pub struct InProcessCodeModeSessionProvider;

impl CodeModeSessionProvider for InProcessCodeModeSessionProvider {
    fn create_session<'a>(
        &'a self,
        delegate: Arc<dyn CodeModeSessionDelegate>,
    ) -> CodeModeSessionProviderFuture<'a> {
        Box::pin(async move {
            let session: Arc<dyn CodeModeSession> =
                Arc::new(CodeModeService::with_delegate(delegate));
            Ok(session)
        })
    }
}

struct Inner {
    stored_values: Mutex<HashMap<String, JsonValue>>,
    cells: Mutex<HashMap<CellId, CellHandle>>,
    delegate: Arc<dyn CodeModeSessionDelegate>,
    shutting_down: AtomicBool,
    next_cell_id: AtomicU64,
}

pub struct CodeModeService {
    inner: Arc<Inner>,
}

impl CodeModeService {
    pub fn new() -> Self {
        Self::with_delegate(Arc::new(NoopCodeModeSessionDelegate))
    }

    pub fn with_delegate(delegate: Arc<dyn CodeModeSessionDelegate>) -> Self {
        Self {
            inner: Arc::new(Inner {
                stored_values: Mutex::new(HashMap::new()),
                cells: Mutex::new(HashMap::new()),
                delegate,
                shutting_down: AtomicBool::new(false),
                next_cell_id: AtomicU64::new(1),
            }),
        }
    }

    fn allocate_cell_id(&self) -> CellId {
        CellId::new(
            self.inner
                .next_cell_id
                .fetch_add(1, Ordering::Relaxed)
                .to_string(),
        )
    }

    pub async fn execute(&self, request: ExecuteRequest) -> Result<StartedCell, String> {
        let yield_time_ms = request.yield_time_ms.unwrap_or(DEFAULT_EXEC_YIELD_TIME_MS);
        let cell_id = self.allocate_cell_id();
        let initial_event = self
            .start_cell(
                cell_id.clone(),
                request,
                ObserveMode::YieldAfter(Duration::from_millis(yield_time_ms)),
            )
            .await?;
        let response_cell_id = cell_id.clone();
        let (response_tx, response_rx) = oneshot::channel();
        tokio::spawn(async move {
            let response = initial_event
                .await
                .map_err(|error| cell_error_text(&response_cell_id, error))
                .and_then(|event| runtime_response(&response_cell_id, event));
            let _ = response_tx.send(response);
        });

        Ok(StartedCell::from_result_receiver(cell_id, response_rx))
    }

    pub async fn execute_to_pending(
        &self,
        request: ExecuteRequest,
    ) -> Result<ExecuteToPendingOutcome, String> {
        let cell_id = self.allocate_cell_id();
        let event = self
            .start_cell(cell_id.clone(), request, ObserveMode::PendingFrontier)
            .await?
            .await
            .map_err(|error| cell_error_text(&cell_id, error))?;
        pending_outcome(&cell_id, event)
    }

    async fn start_cell(
        &self,
        cell_id: CellId,
        request: ExecuteRequest,
        initial_observe_mode: ObserveMode,
    ) -> Result<CellEventFuture, String> {
        let stored_values = self.inner.stored_values.lock().await.clone();
        let host = Arc::new(ServiceCellHost {
            cell_id: cell_id.clone(),
            inner: Arc::clone(&self.inner),
        });
        let mut cells = self.inner.cells.lock().await;
        if self.inner.shutting_down.load(Ordering::Acquire) {
            return Err("code mode session is shutting down".to_string());
        }
        if cells.contains_key(&cell_id) {
            return Err(format!("exec cell {cell_id} already exists"));
        }
        let (handle, initial_event, task) = CellActor::prepare(
            cell_request(request),
            stored_values,
            host,
            initial_observe_mode,
        )?;
        cells.insert(cell_id, handle);
        drop(cells);
        tokio::spawn(task);
        Ok(initial_event)
    }

    pub async fn wait(&self, request: WaitRequest) -> Result<WaitOutcome, String> {
        self.begin_wait(request).await.await
    }

    async fn begin_wait(
        &self,
        request: WaitRequest,
    ) -> CodeModeSessionResultFuture<'static, WaitOutcome> {
        let WaitRequest {
            cell_id,
            yield_time_ms,
        } = request;
        let handle = self.inner.cells.lock().await.get(&cell_id).cloned();
        let Some(handle) = handle else {
            return missing_wait(cell_id);
        };
        wait_for_event(
            cell_id,
            handle.observe(ObserveMode::YieldAfter(Duration::from_millis(
                yield_time_ms,
            ))),
        )
    }

    pub async fn terminate(&self, cell_id: CellId) -> Result<WaitOutcome, String> {
        let handle = self.inner.cells.lock().await.get(&cell_id).cloned();
        let Some(handle) = handle else {
            return Ok(WaitOutcome::MissingCell(missing_cell_response(cell_id)));
        };
        wait_for_event(cell_id, handle.terminate()).await
    }

    pub async fn wait_to_pending(
        &self,
        request: WaitToPendingRequest,
    ) -> Result<WaitToPendingOutcome, String> {
        let cell_id = request.cell_id;
        let handle = self.inner.cells.lock().await.get(&cell_id).cloned();
        let Some(handle) = handle else {
            return Ok(WaitToPendingOutcome::MissingCell(missing_cell_response(
                cell_id,
            )));
        };
        match handle.observe(ObserveMode::PendingFrontier).await {
            Ok(event) => Ok(WaitToPendingOutcome::LiveCell(pending_outcome(
                &cell_id, event,
            )?)),
            Err(CellError::Closed) => Ok(WaitToPendingOutcome::MissingCell(missing_cell_response(
                cell_id,
            ))),
            Err(error) => Err(cell_error_text(&cell_id, error)),
        }
    }

    pub async fn shutdown(&self) -> Result<(), String> {
        self.inner.shutting_down.store(true, Ordering::Release);
        let handles = self
            .inner
            .cells
            .lock()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for handle in handles {
            handle.shutdown();
        }
        while !self.inner.cells.lock().await.is_empty() {
            tokio::task::yield_now().await;
        }
        Ok(())
    }
}

impl Default for CodeModeService {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for CodeModeService {
    fn drop(&mut self) {
        self.inner.shutting_down.store(true, Ordering::Release);
        if let Ok(cells) = self.inner.cells.try_lock() {
            for handle in cells.values() {
                handle.shutdown();
            }
        }
    }
}

impl CodeModeSession for CodeModeService {
    fn is_alive(&self) -> bool {
        !self.inner.shutting_down.load(Ordering::Acquire)
    }

    fn execute<'a>(
        &'a self,
        request: ExecuteRequest,
    ) -> CodeModeSessionResultFuture<'a, StartedCell> {
        Box::pin(CodeModeService::execute(self, request))
    }

    fn wait<'a>(&'a self, request: WaitRequest) -> CodeModeSessionResultFuture<'a, WaitOutcome> {
        Box::pin(CodeModeService::wait(self, request))
    }

    fn terminate<'a>(&'a self, cell_id: CellId) -> CodeModeSessionResultFuture<'a, WaitOutcome> {
        Box::pin(CodeModeService::terminate(self, cell_id))
    }

    fn shutdown<'a>(&'a self) -> CodeModeSessionResultFuture<'a, ()> {
        Box::pin(CodeModeService::shutdown(self))
    }
}

struct ServiceCellHost {
    cell_id: CellId,
    inner: Arc<Inner>,
}

impl CellHost for ServiceCellHost {
    async fn invoke_tool(
        &self,
        invocation: CellToolCall,
        cancellation_token: CancellationToken,
    ) -> Result<JsonValue, String> {
        self.inner
            .delegate
            .invoke_tool(
                CodeModeNestedToolCall {
                    cell_id: self.cell_id.clone(),
                    runtime_tool_call_id: invocation.id,
                    tool_name: codex_protocol::ToolName {
                        name: invocation.name.name,
                        namespace: invocation.name.namespace,
                    },
                    tool_kind: match invocation.kind {
                        CellToolKind::Function => CodeModeToolKind::Function,
                        CellToolKind::Freeform => CodeModeToolKind::Freeform,
                    },
                    input: invocation.input,
                },
                cancellation_token,
            )
            .await
    }

    async fn notify(
        &self,
        call_id: String,
        text: String,
        cancellation_token: CancellationToken,
    ) -> Result<(), String> {
        self.inner
            .delegate
            .notify(call_id, self.cell_id.clone(), text, cancellation_token)
            .await
    }

    async fn commit_stored_values(&self, stored_value_writes: HashMap<String, JsonValue>) {
        self.inner
            .stored_values
            .lock()
            .await
            .extend(stored_value_writes);
    }

    async fn closed(&self) {
        self.inner.cells.lock().await.remove(&self.cell_id);
        self.inner.delegate.cell_closed(&self.cell_id);
    }
}

fn cell_request(request: ExecuteRequest) -> CellRequest {
    CellRequest {
        tool_call_id: request.tool_call_id,
        enabled_tools: request
            .enabled_tools
            .into_iter()
            .map(|definition| CellToolDefinition {
                name: definition.name,
                tool_name: CellToolName {
                    name: definition.tool_name.name,
                    namespace: definition.tool_name.namespace,
                },
                description: definition.description,
                kind: match definition.kind {
                    CodeModeToolKind::Function => CellToolKind::Function,
                    CodeModeToolKind::Freeform => CellToolKind::Freeform,
                },
            })
            .collect(),
        source: request.source,
    }
}

fn wait_for_event(
    cell_id: CellId,
    event: CellEventFuture,
) -> CodeModeSessionResultFuture<'static, WaitOutcome> {
    Box::pin(async move {
        match event.await {
            Ok(event) => Ok(WaitOutcome::LiveCell(runtime_response(&cell_id, event)?)),
            Err(CellError::Closed) => Ok(WaitOutcome::MissingCell(missing_cell_response(cell_id))),
            Err(error) => Err(cell_error_text(&cell_id, error)),
        }
    })
}

fn pending_outcome(cell_id: &CellId, event: CellEvent) -> Result<ExecuteToPendingOutcome, String> {
    match event {
        CellEvent::Pending {
            content_items,
            pending_tool_call_ids,
        } => Ok(ExecuteToPendingOutcome::Pending {
            cell_id: cell_id.clone(),
            content_items: content_items.into_iter().map(output_item).collect(),
            pending_tool_call_ids,
        }),
        event => Ok(ExecuteToPendingOutcome::Completed(runtime_response(
            cell_id, event,
        )?)),
    }
}

fn runtime_response(cell_id: &CellId, event: CellEvent) -> Result<RuntimeResponse, String> {
    match event {
        CellEvent::Yielded { content_items } => Ok(RuntimeResponse::Yielded {
            cell_id: cell_id.clone(),
            content_items: content_items.into_iter().map(output_item).collect(),
        }),
        CellEvent::Completed {
            content_items,
            error_text,
        } => Ok(RuntimeResponse::Result {
            cell_id: cell_id.clone(),
            content_items: content_items.into_iter().map(output_item).collect(),
            error_text,
        }),
        CellEvent::Terminated { content_items } => Ok(RuntimeResponse::Terminated {
            cell_id: cell_id.clone(),
            content_items: content_items.into_iter().map(output_item).collect(),
        }),
        CellEvent::Pending { .. } => {
            Err("cell returned a pending frontier unexpectedly".to_string())
        }
    }
}

fn output_item(item: CellOutputItem) -> FunctionCallOutputContentItem {
    match item {
        CellOutputItem::Text { text } => FunctionCallOutputContentItem::InputText { text },
        CellOutputItem::Image { image_url, detail } => FunctionCallOutputContentItem::InputImage {
            image_url,
            detail: detail.map(|detail| match detail {
                CellImageDetail::Auto => ImageDetail::Auto,
                CellImageDetail::Low => ImageDetail::Low,
                CellImageDetail::High => ImageDetail::High,
                CellImageDetail::Original => ImageDetail::Original,
            }),
        },
    }
}

fn cell_error_text(cell_id: &CellId, error: CellError) -> String {
    match error {
        CellError::Busy => format!("exec cell {cell_id} already has an active observer"),
        CellError::AlreadyTerminating => format!("exec cell {cell_id} is already terminating"),
        CellError::Closed => format!("exec cell {cell_id} closed unexpectedly"),
    }
}

fn missing_cell_response(cell_id: CellId) -> RuntimeResponse {
    RuntimeResponse::Result {
        error_text: Some(format!("exec cell {cell_id} not found")),
        cell_id,
        content_items: Vec::new(),
    }
}

fn missing_wait(cell_id: CellId) -> CodeModeSessionResultFuture<'static, WaitOutcome> {
    Box::pin(async move { Ok(WaitOutcome::MissingCell(missing_cell_response(cell_id))) })
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "service_contract_tests.rs"]
mod contract_tests;
