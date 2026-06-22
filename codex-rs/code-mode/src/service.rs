use std::collections::HashMap;
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
use codex_code_mode_protocol::CreateCellRequest;
use codex_code_mode_protocol::FunctionCallOutputContentItem;
use codex_code_mode_protocol::ImageDetail;
use codex_code_mode_protocol::NotificationFuture;
use codex_code_mode_protocol::ObservationGeneration;
use codex_code_mode_protocol::ObserveOutcome;
use codex_code_mode_protocol::ObserveRequest;
use codex_code_mode_protocol::ReleaseObservationRequest;
#[cfg(test)]
use codex_code_mode_protocol::RuntimeResponse;
use codex_code_mode_protocol::TerminateOutcome;
use codex_code_mode_protocol::ToolInvocationFuture;
use serde_json::Value as JsonValue;
use tokio::sync::Mutex;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use crate::session_runtime as runtime;
use crate::session_runtime::SessionRuntime;

#[cfg(test)]
struct ObserveToPendingRequest {
    cell_id: CellId,
}

#[cfg(test)]
#[derive(Debug, PartialEq)]
enum PendingOutcome {
    Pending {
        cell_id: CellId,
        content_items: Vec<FunctionCallOutputContentItem>,
        pending_tool_call_ids: Vec<String>,
    },
    Completed(RuntimeResponse),
}

#[cfg(test)]
#[derive(Debug, PartialEq)]
enum ObserveToPendingOutcome {
    LiveCell(PendingOutcome),
    MissingCell(RuntimeResponse),
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
                Arc::new(CodeModeService::with_delegate(delegate));
            Ok(session)
        })
    }
}

pub struct CodeModeService {
    runtime: Arc<SessionRuntime<ProtocolDelegate>>,
    observations: Mutex<HashMap<CellId, ObservationRecord>>,
    #[cfg(test)]
    pending_generations: Mutex<HashMap<runtime::CellId, runtime::PendingGeneration>>,
}

enum ObservationRecord {
    Retained {
        request: ObserveRequest,
        result_rx: watch::Receiver<Option<Result<ObserveOutcome, String>>>,
    },
    Released {
        generation: ObservationGeneration,
    },
}

impl CodeModeService {
    pub fn new() -> Self {
        Self::with_delegate(Arc::new(NoopCodeModeSessionDelegate))
    }

    pub fn with_delegate(delegate: Arc<dyn CodeModeSessionDelegate>) -> Self {
        #[cfg(not(test))]
        let runtime = Arc::new(SessionRuntime::new(Arc::new(ProtocolDelegate { delegate })));
        #[cfg(test)]
        let runtime = Arc::new(SessionRuntime::new_for_test(Arc::new(ProtocolDelegate {
            delegate,
        })));
        Self {
            runtime,
            observations: Mutex::new(HashMap::new()),
            #[cfg(test)]
            pending_generations: Mutex::new(HashMap::new()),
        }
    }

    pub async fn create_cell(&self, request: CreateCellRequest) -> Result<CellId, String> {
        let cell = self
            .runtime
            .create_cell(runtime_request(request))
            .await
            .map_err(|error| error.to_string())?;
        Ok(protocol_cell_id(cell.id()))
    }

    #[cfg(test)]
    async fn create_pausable_cell(&self, request: CreateCellRequest) -> Result<CellId, String> {
        let cell = self
            .runtime
            .create_pausable_cell(runtime_request(request))
            .await
            .map_err(|error| error.to_string())?;
        Ok(protocol_cell_id(cell.id()))
    }

    pub async fn observe(&self, request: ObserveRequest) -> Result<ObserveOutcome, String> {
        let cell_id = request.cell_id.clone();
        let mut result_rx = {
            let mut observations = self.observations.lock().await;
            let replay = match observations.get(&cell_id) {
                Some(ObservationRecord::Retained {
                    request: existing_request,
                    result_rx,
                }) if existing_request.generation == request.generation => {
                    if *existing_request != request {
                        return Err(format!(
                            "observation generation {} for cell {cell_id} was reused for a different request",
                            request.generation.get()
                        ));
                    }
                    Some(result_rx.clone())
                }
                Some(ObservationRecord::Retained {
                    request: existing_request,
                    result_rx,
                }) => {
                    let Some(next_generation) = existing_request.generation.next() else {
                        return Err(format!(
                            "observation generation exhausted for cell {cell_id}"
                        ));
                    };
                    if request.generation != next_generation {
                        return Err(format!(
                            "expected observation generation {} for cell {cell_id}, got {}",
                            next_generation.get(),
                            request.generation.get()
                        ));
                    }
                    if result_rx.borrow().is_none() {
                        return Err(format!(
                            "observation generation {} for cell {cell_id} is still pending",
                            existing_request.generation.get()
                        ));
                    }
                    None
                }
                Some(ObservationRecord::Released { generation }) => {
                    let Some(next_generation) = generation.next() else {
                        return Err(format!(
                            "observation generation exhausted for cell {cell_id}"
                        ));
                    };
                    if request.generation != next_generation {
                        return Err(format!(
                            "expected observation generation {} for cell {cell_id}, got {}",
                            next_generation.get(),
                            request.generation.get()
                        ));
                    }
                    None
                }
                None if request.generation != ObservationGeneration::INITIAL => {
                    return Err(format!(
                        "first observation for cell {cell_id} must use generation 0"
                    ));
                }
                None => None,
            };
            if let Some(result_rx) = replay {
                result_rx
            } else {
                let (result_tx, result_rx) = watch::channel(None);
                observations.insert(
                    cell_id,
                    ObservationRecord::Retained {
                        request: request.clone(),
                        result_rx: result_rx.clone(),
                    },
                );
                let runtime = Arc::clone(&self.runtime);
                tokio::spawn(async move {
                    let result = begin_observe_runtime(runtime, request).await.await;
                    result_tx.send_replace(Some(result));
                });
                result_rx
            }
        };

        result_rx
            .wait_for(Option::is_some)
            .await
            .map_err(|_| "observation ended before producing a result".to_string())?;

        result_rx
            .borrow()
            .clone()
            .ok_or_else(|| "observation ended before producing a result".to_string())?
    }

    pub async fn release_observation(
        &self,
        request: ReleaseObservationRequest,
    ) -> Result<(), String> {
        let mut observations = self.observations.lock().await;
        let Some(record) = observations.get(&request.cell_id) else {
            return Ok(());
        };
        match record {
            ObservationRecord::Released { generation } => {
                if request.generation <= *generation {
                    return Ok(());
                }
                return Err(format!(
                    "cannot release future observation generation {} for cell {}",
                    request.generation.get(),
                    request.cell_id
                ));
            }
            ObservationRecord::Retained {
                request: retained_request,
                result_rx,
            } => {
                if request.generation < retained_request.generation {
                    return Ok(());
                }
                if request.generation > retained_request.generation {
                    return Err(format!(
                        "cannot release future observation generation {} for cell {}; latest generation is {}",
                        request.generation.get(),
                        request.cell_id,
                        retained_request.generation.get()
                    ));
                }
                let result = result_rx.borrow();
                let result = result.as_ref().ok_or_else(|| {
                    format!(
                        "observation generation {} for cell {} is still pending",
                        request.generation.get(),
                        request.cell_id
                    )
                })?;
                let outcome = result.as_ref().map_err(|error| {
                    format!(
                        "observation generation {} for cell {} failed and cannot be released: {error}",
                        request.generation.get(),
                        request.cell_id
                    )
                })?;
                if !matches!(
                    outcome,
                    ObserveOutcome::Completed { .. } | ObserveOutcome::Terminated { .. }
                ) {
                    return Err(format!(
                        "observation generation {} for cell {} is not terminal",
                        request.generation.get(),
                        request.cell_id
                    ));
                }
            }
        }
        observations.insert(
            request.cell_id,
            ObservationRecord::Released {
                generation: request.generation,
            },
        );
        Ok(())
    }

    #[cfg(test)]
    async fn begin_observe(
        &self,
        request: ObserveRequest,
    ) -> CodeModeSessionResultFuture<'static, ObserveOutcome> {
        begin_observe_runtime(Arc::clone(&self.runtime), request).await
    }

    pub async fn terminate(&self, cell_id: CellId) -> Result<TerminateOutcome, String> {
        let runtime_cell_id = runtime_cell_id(&cell_id);
        let outcome = match self.runtime.terminate(&runtime_cell_id).await {
            Ok(event) => terminate_outcome(&cell_id, event),
            Err(runtime::Error::MissingCell(_) | runtime::Error::ClosedCell(_)) => {
                Ok(TerminateOutcome::Missing { cell_id })
            }
            Err(error) => Err(error.to_string()),
        };
        #[cfg(test)]
        self.pending_generations
            .lock()
            .await
            .remove(&runtime_cell_id);
        outcome
    }

    #[cfg(test)]
    async fn observe_to_pending(
        &self,
        request: ObserveToPendingRequest,
    ) -> Result<ObserveToPendingOutcome, String> {
        let cell_id = request.cell_id;
        let runtime_cell_id = runtime_cell_id(&cell_id);
        let cell = match self.runtime.pausable_cell(&runtime_cell_id).await {
            Ok(cell) => cell,
            Err(runtime::Error::MissingCell(_) | runtime::Error::ClosedCell(_)) => {
                return Ok(ObserveToPendingOutcome::MissingCell(missing_cell_response(
                    cell_id,
                )));
            }
            Err(error) => return Err(error.to_string()),
        };
        let generation = {
            self.pending_generations
                .lock()
                .await
                .get(&runtime_cell_id)
                .copied()
        };
        if let Some(generation) = generation {
            self.runtime
                .resume(&cell, generation)
                .await
                .map_err(|error| error.to_string())?;
        }
        match self.runtime.wait_to_pending(&cell).await {
            Ok(event) => {
                self.record_pending_generation(&runtime_cell_id, &event)
                    .await;
                Ok(ObserveToPendingOutcome::LiveCell(pending_outcome(
                    &cell_id, event,
                )?))
            }
            Err(runtime::Error::MissingCell(_) | runtime::Error::ClosedCell(_)) => Ok(
                ObserveToPendingOutcome::MissingCell(missing_cell_response(cell_id)),
            ),
            Err(error) => Err(error.to_string()),
        }
    }

    #[cfg(test)]
    async fn record_pending_generation(
        &self,
        cell_id: &runtime::CellId,
        event: &runtime::PausableCellEvent,
    ) {
        let mut generations = self.pending_generations.lock().await;
        match event {
            runtime::PausableCellEvent::Pending(frontier) => {
                generations.insert(cell_id.clone(), frontier.generation);
            }
            runtime::PausableCellEvent::Completed { .. }
            | runtime::PausableCellEvent::Terminated { .. } => {
                generations.remove(cell_id);
            }
        }
    }

    pub async fn shutdown(&self) -> Result<(), String> {
        let result = self
            .runtime
            .shutdown()
            .await
            .map_err(|error| error.to_string());
        #[cfg(test)]
        self.pending_generations.lock().await.clear();
        result
    }
}

async fn begin_observe_runtime(
    runtime: Arc<SessionRuntime<ProtocolDelegate>>,
    request: ObserveRequest,
) -> CodeModeSessionResultFuture<'static, ObserveOutcome> {
    let ObserveRequest {
        cell_id,
        generation: _,
        yield_time_ms,
    } = request;
    let runtime_cell_id = runtime_cell_id(&cell_id);
    let cell = match runtime.cell(&runtime_cell_id).await {
        Ok(cell) => cell,
        Err(runtime::Error::MissingCell(_) | runtime::Error::ClosedCell(_)) => {
            return missing_observation(cell_id);
        }
        Err(error) => return Box::pin(async move { Err(error.to_string()) }),
    };
    match runtime
        .begin_wait(&cell, Duration::from_millis(yield_time_ms))
        .await
    {
        Ok(pending_event) => Box::pin(async move {
            match pending_event.event().await {
                Ok(event) => Ok(observe_outcome(&cell_id, event)),
                Err(runtime::Error::MissingCell(_) | runtime::Error::ClosedCell(_)) => {
                    Ok(ObserveOutcome::Missing { cell_id })
                }
                Err(error) => Err(error.to_string()),
            }
        }),
        Err(runtime::Error::MissingCell(_) | runtime::Error::ClosedCell(_)) => {
            missing_observation(cell_id)
        }
        Err(error) => Box::pin(async move { Err(error.to_string()) }),
    }
}

impl Default for CodeModeService {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeModeSession for CodeModeService {
    fn is_alive(&self) -> bool {
        self.runtime.is_alive()
    }

    fn create_cell<'a>(
        &'a self,
        request: CreateCellRequest,
    ) -> CodeModeSessionResultFuture<'a, CellId> {
        Box::pin(CodeModeService::create_cell(self, request))
    }

    fn observe<'a>(
        &'a self,
        request: ObserveRequest,
    ) -> CodeModeSessionResultFuture<'a, ObserveOutcome> {
        Box::pin(CodeModeService::observe(self, request))
    }

    fn release_observation<'a>(
        &'a self,
        request: ReleaseObservationRequest,
    ) -> CodeModeSessionResultFuture<'a, ()> {
        Box::pin(CodeModeService::release_observation(self, request))
    }

    fn terminate<'a>(
        &'a self,
        cell_id: CellId,
    ) -> CodeModeSessionResultFuture<'a, TerminateOutcome> {
        Box::pin(CodeModeService::terminate(self, cell_id))
    }

    fn shutdown<'a>(&'a self) -> CodeModeSessionResultFuture<'a, ()> {
        Box::pin(CodeModeService::shutdown(self))
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

fn runtime_request(request: CreateCellRequest) -> runtime::CreateCellRequest {
    runtime::CreateCellRequest {
        cell_id: runtime_cell_id(&request.cell_id),
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

#[cfg(test)]
fn pending_outcome(
    cell_id: &CellId,
    event: runtime::PausableCellEvent,
) -> Result<PendingOutcome, String> {
    match event {
        runtime::PausableCellEvent::Pending(runtime::PendingFrontier {
            content_items,
            pending_tool_call_ids,
            ..
        }) => Ok(PendingOutcome::Pending {
            cell_id: cell_id.clone(),
            content_items: content_items.into_iter().map(output_item).collect(),
            pending_tool_call_ids,
        }),
        runtime::PausableCellEvent::Completed {
            content_items,
            error_text,
        } => Ok(PendingOutcome::Completed(RuntimeResponse::Result {
            cell_id: cell_id.clone(),
            content_items: content_items.into_iter().map(output_item).collect(),
            error_text,
        })),
        runtime::PausableCellEvent::Terminated { content_items } => {
            Ok(PendingOutcome::Completed(RuntimeResponse::Terminated {
                cell_id: cell_id.clone(),
                content_items: content_items.into_iter().map(output_item).collect(),
            }))
        }
    }
}

fn observe_outcome(cell_id: &CellId, event: runtime::CellEvent) -> ObserveOutcome {
    match event {
        runtime::CellEvent::Yielded { content_items } => ObserveOutcome::Yielded {
            cell_id: cell_id.clone(),
            content_items: content_items.into_iter().map(output_item).collect(),
        },
        runtime::CellEvent::Completed {
            content_items,
            error_text,
        } => ObserveOutcome::Completed {
            cell_id: cell_id.clone(),
            content_items: content_items.into_iter().map(output_item).collect(),
            error_text,
        },
        runtime::CellEvent::Terminated { content_items } => ObserveOutcome::Terminated {
            cell_id: cell_id.clone(),
            content_items: content_items.into_iter().map(output_item).collect(),
        },
    }
}

fn terminate_outcome(
    cell_id: &CellId,
    event: runtime::CellEvent,
) -> Result<TerminateOutcome, String> {
    match event {
        runtime::CellEvent::Yielded { .. } => Err(format!(
            "termination of code-mode cell {cell_id} unexpectedly yielded"
        )),
        runtime::CellEvent::Completed {
            content_items,
            error_text,
        } => Ok(TerminateOutcome::Completed {
            cell_id: cell_id.clone(),
            content_items: content_items.into_iter().map(output_item).collect(),
            error_text,
        }),
        runtime::CellEvent::Terminated { content_items } => Ok(TerminateOutcome::Terminated {
            cell_id: cell_id.clone(),
            content_items: content_items.into_iter().map(output_item).collect(),
        }),
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

#[cfg(test)]
fn missing_cell_response(cell_id: CellId) -> RuntimeResponse {
    RuntimeResponse::Result {
        error_text: Some(format!("exec cell {cell_id} not found")),
        cell_id,
        content_items: Vec::new(),
    }
}

fn missing_observation(cell_id: CellId) -> CodeModeSessionResultFuture<'static, ObserveOutcome> {
    Box::pin(async move { Ok(ObserveOutcome::Missing { cell_id }) })
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "service_contract_tests.rs"]
mod contract_tests;
