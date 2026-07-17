use std::sync::Arc;
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
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use crate::session_runtime as runtime;
use crate::session_runtime::SessionRuntime;

const YIELD_GRACE_PERIOD: Duration = Duration::from_secs(1);
const MIN_YIELD_TIME_FOR_GRACE: Duration = Duration::from_secs(10);

fn yield_timeout(yield_time_ms: u64) -> Duration {
    let yield_time = Duration::from_millis(yield_time_ms);
    if yield_time >= MIN_YIELD_TIME_FOR_GRACE {
        yield_time.saturating_add(YIELD_GRACE_PERIOD)
    } else {
        yield_time
    }
}

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
                Arc::new(InProcessCodeModeSession::with_delegate(delegate));
            Ok(session)
        })
    }
}

pub struct InProcessCodeModeSession {
    runtime: SessionRuntime<ProtocolDelegate>,
}

impl InProcessCodeModeSession {
    pub fn new() -> Self {
        Self::with_delegate(Arc::new(NoopCodeModeSessionDelegate))
    }

    pub fn with_delegate(delegate: Arc<dyn CodeModeSessionDelegate>) -> Self {
        Self {
            runtime: SessionRuntime::new(Arc::new(ProtocolDelegate { delegate })),
        }
    }

    pub fn with_delegate_and_task_failure_handler(
        delegate: Arc<dyn CodeModeSessionDelegate>,
        task_failure_handler: Arc<dyn Fn(String) + Send + Sync>,
    ) -> Self {
        Self {
            runtime: SessionRuntime::new_with_task_failure_handler(
                Arc::new(ProtocolDelegate { delegate }),
                Some(task_failure_handler),
            ),
        }
    }

    pub async fn execute(&self, request: ExecuteRequest) -> Result<StartedCell, String> {
        let yield_time_ms = request.yield_time_ms.unwrap_or(DEFAULT_EXEC_YIELD_TIME_MS);
        let started = self
            .runtime
            .execute(
                runtime_request(request),
                runtime::ObserveMode::YieldAfter(yield_timeout(yield_time_ms)),
            )
            .await
            .map_err(|error| error.to_string())?;
        let cell_id = protocol_cell_id(&started.cell_id);
        let response_cell_id = cell_id.clone();
        let (response_tx, response_rx) = oneshot::channel();
        tokio::spawn(async move {
            let response = started
                .initial_event()
                .await
                .map_err(|error| error.to_string())
                .and_then(|event| runtime_response(&response_cell_id, event));
            let _ = response_tx.send(response);
        });
        Ok(StartedCell::from_result_receiver(cell_id, response_rx))
    }

    pub async fn execute_to_pending(
        &self,
        request: ExecuteRequest,
    ) -> Result<ExecuteToPendingOutcome, String> {
        let started = self
            .runtime
            .execute(
                runtime_request(request),
                runtime::ObserveMode::PendingFrontier,
            )
            .await
            .map_err(|error| error.to_string())?;
        let cell_id = protocol_cell_id(&started.cell_id);
        let event = started
            .initial_event()
            .await
            .map_err(|error| error.to_string())?;
        pending_outcome(&cell_id, event)
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
        let runtime_cell_id = runtime_cell_id(&cell_id);
        match self
            .runtime
            .begin_observe(
                &runtime_cell_id,
                runtime::ObserveMode::YieldAfter(yield_timeout(yield_time_ms)),
            )
            .await
        {
            Ok(pending_event) => Box::pin(async move {
                match pending_event.event().await {
                    Ok(event) => Ok(WaitOutcome::LiveCell(runtime_response(&cell_id, event)?)),
                    Err(runtime::Error::MissingCell(_) | runtime::Error::ClosedCell(_)) => {
                        Ok(WaitOutcome::MissingCell(missing_cell_response(cell_id)))
                    }
                    Err(error) => Err(error.to_string()),
                }
            }),
            Err(runtime::Error::MissingCell(_) | runtime::Error::ClosedCell(_)) => {
                missing_wait(cell_id)
            }
            Err(error) => Box::pin(async move { Err(error.to_string()) }),
        }
    }

    pub async fn terminate(&self, cell_id: CellId) -> Result<WaitOutcome, String> {
        match self.runtime.terminate(&runtime_cell_id(&cell_id)).await {
            Ok(event) => Ok(WaitOutcome::LiveCell(runtime_response(&cell_id, event)?)),
            Err(runtime::Error::MissingCell(_) | runtime::Error::ClosedCell(_)) => {
                Ok(WaitOutcome::MissingCell(missing_cell_response(cell_id)))
            }
            Err(error) => Err(error.to_string()),
        }
    }

    pub async fn wait_to_pending(
        &self,
        request: WaitToPendingRequest,
    ) -> Result<WaitToPendingOutcome, String> {
        let cell_id = request.cell_id;
        match self
            .runtime
            .observe(
                &runtime_cell_id(&cell_id),
                runtime::ObserveMode::PendingFrontier,
            )
            .await
        {
            Ok(event) => Ok(WaitToPendingOutcome::LiveCell(pending_outcome(
                &cell_id, event,
            )?)),
            Err(runtime::Error::MissingCell(_) | runtime::Error::ClosedCell(_)) => Ok(
                WaitToPendingOutcome::MissingCell(missing_cell_response(cell_id)),
            ),
            Err(error) => Err(error.to_string()),
        }
    }

    pub async fn shutdown(&self) -> Result<(), String> {
        self.runtime
            .shutdown()
            .await
            .map_err(|error| error.to_string())
    }
}

impl Default for InProcessCodeModeSession {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeModeSession for InProcessCodeModeSession {
    fn execute<'a>(
        &'a self,
        request: ExecuteRequest,
    ) -> CodeModeSessionResultFuture<'a, StartedCell> {
        Box::pin(InProcessCodeModeSession::execute(self, request))
    }

    fn wait<'a>(&'a self, request: WaitRequest) -> CodeModeSessionResultFuture<'a, WaitOutcome> {
        Box::pin(InProcessCodeModeSession::wait(self, request))
    }

    fn terminate<'a>(&'a self, cell_id: CellId) -> CodeModeSessionResultFuture<'a, WaitOutcome> {
        Box::pin(InProcessCodeModeSession::terminate(self, cell_id))
    }

    fn shutdown<'a>(&'a self) -> CodeModeSessionResultFuture<'a, ()> {
        Box::pin(InProcessCodeModeSession::shutdown(self))
    }
}

struct ProtocolDelegate {
    delegate: Arc<dyn CodeModeSessionDelegate>,
}

impl runtime::SessionRuntimeDelegate for ProtocolDelegate {
    async fn invoke_tool(
        &self,
        invocation: runtime::NestedToolCall,
        cancellation_token: CancellationToken,
    ) -> Result<JsonValue, String> {
        self.delegate
            .invoke_tool(
                CodeModeNestedToolCall {
                    cell_id: protocol_cell_id(&invocation.cell_id),
                    runtime_tool_call_id: invocation.runtime_tool_call_id,
                    tool_name: codex_protocol::ToolName {
                        name: invocation.tool_name.name,
                        namespace: invocation.tool_name.namespace,
                    },
                    tool_kind: match invocation.tool_kind {
                        runtime::ToolKind::Function => CodeModeToolKind::Function,
                        runtime::ToolKind::Freeform => CodeModeToolKind::Freeform,
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
        cell_id: runtime::CellId,
        text: String,
        cancellation_token: CancellationToken,
    ) -> Result<(), String> {
        self.delegate
            .notify(
                call_id,
                protocol_cell_id(&cell_id),
                text,
                cancellation_token,
            )
            .await
    }

    fn cell_closed(&self, cell_id: &runtime::CellId) {
        self.delegate.cell_closed(&protocol_cell_id(cell_id));
    }
}

fn runtime_request(request: ExecuteRequest) -> runtime::CreateCellRequest {
    runtime::CreateCellRequest {
        tool_call_id: request.tool_call_id,
        enabled_tools: request
            .enabled_tools
            .into_iter()
            .map(|definition| runtime::ToolDefinition {
                name: definition.name,
                tool_name: runtime::ToolName {
                    name: definition.tool_name.name,
                    namespace: definition.tool_name.namespace,
                },
                description: definition.description,
                kind: match definition.kind {
                    CodeModeToolKind::Function => runtime::ToolKind::Function,
                    CodeModeToolKind::Freeform => runtime::ToolKind::Freeform,
                },
            })
            .collect(),
        source: request.source,
    }
}

fn runtime_cell_id(cell_id: &CellId) -> runtime::CellId {
    runtime::CellId::new(cell_id.as_str())
}

fn protocol_cell_id(cell_id: &runtime::CellId) -> CellId {
    CellId::new(cell_id.as_str().to_string())
}

fn pending_outcome(
    cell_id: &CellId,
    event: runtime::CellEvent,
) -> Result<ExecuteToPendingOutcome, String> {
    match event {
        runtime::CellEvent::Pending {
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

fn runtime_response(
    cell_id: &CellId,
    event: runtime::CellEvent,
) -> Result<RuntimeResponse, String> {
    match event {
        runtime::CellEvent::Yielded { content_items } => Ok(RuntimeResponse::Yielded {
            cell_id: cell_id.clone(),
            content_items: content_items.into_iter().map(output_item).collect(),
        }),
        runtime::CellEvent::Completed {
            content_items,
            error_text,
        } => Ok(RuntimeResponse::Result {
            cell_id: cell_id.clone(),
            content_items: content_items.into_iter().map(output_item).collect(),
            error_text,
        }),
        runtime::CellEvent::Terminated { content_items } => Ok(RuntimeResponse::Terminated {
            cell_id: cell_id.clone(),
            content_items: content_items.into_iter().map(output_item).collect(),
        }),
        runtime::CellEvent::Pending { .. } => {
            Err("cell returned a pending frontier unexpectedly".to_string())
        }
    }
}

fn output_item(item: runtime::OutputItem) -> FunctionCallOutputContentItem {
    match item {
        runtime::OutputItem::Text { text } => FunctionCallOutputContentItem::InputText { text },
        runtime::OutputItem::Image { image_url, detail } => {
            FunctionCallOutputContentItem::InputImage {
                image_url,
                detail: detail.map(|detail| match detail {
                    runtime::ImageDetail::Auto => ImageDetail::Auto,
                    runtime::ImageDetail::Low => ImageDetail::Low,
                    runtime::ImageDetail::High => ImageDetail::High,
                    runtime::ImageDetail::Original => ImageDetail::Original,
                }),
            }
        }
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
