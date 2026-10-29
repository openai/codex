use std::sync::Arc;
use std::time::Duration;

use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::CodeModeNestedToolCall;
use codex_code_mode_protocol::CodeModeOutputTruncation;
use codex_code_mode_protocol::CodeModeSession;
use codex_code_mode_protocol::CodeModeSessionCellExecutionLimits;
use codex_code_mode_protocol::CodeModeSessionDelegate;
use codex_code_mode_protocol::CodeModeSessionOptions;
use codex_code_mode_protocol::CodeModeSessionResultFuture;
use codex_code_mode_protocol::CodeModeToolKind;
use codex_code_mode_protocol::DEFAULT_EXEC_YIELD_TIME_MS;
use codex_code_mode_protocol::ExecuteRequest;
use codex_code_mode_protocol::ExecuteToPendingOutcome;
use codex_code_mode_protocol::FunctionCallOutputContentItem;
use codex_code_mode_protocol::RuntimeResponse;
use codex_code_mode_protocol::StartedCell;
use codex_code_mode_protocol::TerminateRequest;
use codex_code_mode_protocol::WaitOutcome;
use codex_code_mode_protocol::WaitRequest;
use codex_code_mode_protocol::WaitToPendingOutcome;
use codex_code_mode_protocol::WaitToPendingRequest;
use serde_json::Value as JsonValue;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use crate::TaskFailureHandler;
use crate::cell_actor::take_completed_event;
use crate::output::OutputCollector;
use crate::session_runtime as runtime;
use crate::session_runtime::SessionRuntime;

const YIELD_GRACE_PERIOD: Duration = Duration::from_secs(1);
const MIN_YIELD_TIME_FOR_GRACE: Duration = Duration::from_secs(10);

pub struct InProcessCodeModeSession {
    runtime: SessionRuntime,
    options: CodeModeSessionOptions,
}

impl InProcessCodeModeSession {
    pub fn new() -> Self {
        Self::with_limits(CodeModeSessionCellExecutionLimits::default())
    }

    pub fn with_limits(cell_execution_limits: CodeModeSessionCellExecutionLimits) -> Self {
        Self::with_options(
            CodeModeSessionOptions {
                cell_execution_limits,
                ..CodeModeSessionOptions::default()
            },
            /*task_failure_handler*/ None,
        )
    }

    /// Select output handling at creation. A session's mode must remain fixed
    /// for all of its cells and observations.
    pub fn with_options(
        options: CodeModeSessionOptions,
        task_failure_handler: Option<TaskFailureHandler>,
    ) -> Self {
        Self {
            runtime: SessionRuntime::new_with_task_failure_handler(task_failure_handler),
            options: CodeModeSessionOptions {
                cell_execution_limits: CodeModeSessionCellExecutionLimits {
                    max_heap_size_bytes: None,
                    ..options.cell_execution_limits
                },
                ..options
            },
        }
    }

    pub async fn execute(
        &self,
        request: ExecuteRequest,
        delegate: Arc<dyn CodeModeSessionDelegate>,
        preempt: Option<CancellationToken>,
    ) -> Result<StartedCell, String> {
        let yield_time_ms = request.yield_time_ms.unwrap_or(DEFAULT_EXEC_YIELD_TIME_MS);
        let max_output_tokens = request.max_output_tokens;
        let output_truncation = self.options.output_truncation;
        let started = self
            .runtime
            .execute(
                runtime_request(request, self.options.output_truncation),
                runtime::ObserveMode::YieldAfter(self.resolve_yield_timeout(yield_time_ms))
                    .with_yield_signal(preempt.unwrap_or_default()),
                Arc::new(ProtocolDelegate { delegate }),
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
                .and_then(|event| {
                    runtime_response(
                        &response_cell_id,
                        event,
                        output_truncation,
                        max_output_tokens,
                    )
                });
            let _ = response_tx.send(response);
        });
        Ok(StartedCell::from_result_receiver(cell_id, response_rx))
    }

    pub async fn execute_to_pending(
        &self,
        request: ExecuteRequest,
        delegate: Arc<dyn CodeModeSessionDelegate>,
    ) -> Result<ExecuteToPendingOutcome, String> {
        let max_output_tokens = request.max_output_tokens;
        let started = self
            .runtime
            .execute(
                runtime_request(request, self.options.output_truncation),
                runtime::ObserveMode::PendingFrontier,
                Arc::new(ProtocolDelegate { delegate }),
            )
            .await
            .map_err(|error| error.to_string())?;
        let cell_id = protocol_cell_id(&started.cell_id);
        let event = started
            .initial_event()
            .await
            .map_err(|error| error.to_string())?;
        pending_outcome(
            &cell_id,
            event,
            self.options.output_truncation,
            max_output_tokens,
        )
    }

    pub async fn wait(
        &self,
        request: WaitRequest,
        preempt: Option<CancellationToken>,
    ) -> Result<WaitOutcome, String> {
        self.begin_wait(request, preempt).await.await
    }

    async fn begin_wait(
        &self,
        request: WaitRequest,
        preempt: Option<CancellationToken>,
    ) -> CodeModeSessionResultFuture<'static, WaitOutcome> {
        let WaitRequest {
            cell_id,
            yield_time_ms,
            max_output_tokens,
        } = request;
        let output_truncation = self.options.output_truncation;
        let runtime_cell_id = runtime_cell_id(&cell_id);
        match self
            .runtime
            .begin_observe(
                &runtime_cell_id,
                runtime::ObserveMode::YieldAfter(self.resolve_yield_timeout(yield_time_ms))
                    .with_yield_signal(preempt.unwrap_or_default()),
            )
            .await
        {
            Ok(pending_event) => Box::pin(async move {
                match pending_event.event().await {
                    Ok(event) => Ok(WaitOutcome::LiveCell(runtime_response(
                        &cell_id,
                        event,
                        output_truncation,
                        max_output_tokens,
                    )?)),
                    Err(runtime::Error::MissingCell(_) | runtime::Error::ClosedCell(_)) => {
                        Ok(WaitOutcome::MissingCell(missing_cell_response(
                            cell_id,
                            output_truncation,
                            max_output_tokens,
                        )?))
                    }
                    Err(error) => Err(error.to_string()),
                }
            }),
            Err(runtime::Error::MissingCell(_) | runtime::Error::ClosedCell(_)) => {
                missing_wait(cell_id, output_truncation, max_output_tokens)
            }
            Err(error) => Box::pin(async move { Err(error.to_string()) }),
        }
    }

    pub async fn terminate(&self, request: TerminateRequest) -> Result<WaitOutcome, String> {
        let TerminateRequest {
            cell_id,
            max_output_tokens,
        } = request;
        match self.runtime.terminate(&runtime_cell_id(&cell_id)).await {
            Ok(event) => Ok(WaitOutcome::LiveCell(runtime_response(
                &cell_id,
                event,
                self.options.output_truncation,
                max_output_tokens,
            )?)),
            Err(runtime::Error::MissingCell(_) | runtime::Error::ClosedCell(_)) => {
                Ok(WaitOutcome::MissingCell(missing_cell_response(
                    cell_id,
                    self.options.output_truncation,
                    max_output_tokens,
                )?))
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
                &cell_id,
                event,
                self.options.output_truncation,
                /*max_output_tokens*/ None,
            )?)),
            Err(runtime::Error::MissingCell(_) | runtime::Error::ClosedCell(_)) => {
                Ok(WaitToPendingOutcome::MissingCell(missing_cell_response(
                    cell_id,
                    self.options.output_truncation,
                    /*max_output_tokens*/ None,
                )?))
            }
            Err(error) => Err(error.to_string()),
        }
    }

    pub async fn shutdown(&self) -> Result<(), String> {
        self.runtime
            .shutdown()
            .await
            .map_err(|error| error.to_string())
    }

    fn resolve_yield_timeout(&self, yield_time_ms: u64) -> Duration {
        let yield_time = Duration::from_millis(yield_time_ms);
        let timeout = if yield_time >= MIN_YIELD_TIME_FOR_GRACE {
            yield_time.saturating_add(YIELD_GRACE_PERIOD)
        } else {
            yield_time
        };

        self.options
            .cell_execution_limits
            .max_yield_time_ms
            .map(Duration::from_millis)
            .map_or(timeout, |limit| timeout.min(limit))
    }
}

impl Default for InProcessCodeModeSession {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeModeSession for InProcessCodeModeSession {
    fn output_truncation(&self) -> CodeModeOutputTruncation {
        self.options.output_truncation
    }

    fn execute<'a>(
        &'a self,
        request: ExecuteRequest,
        delegate: Arc<dyn CodeModeSessionDelegate>,
        preempt: Option<CancellationToken>,
    ) -> CodeModeSessionResultFuture<'a, StartedCell> {
        Box::pin(InProcessCodeModeSession::execute(
            self, request, delegate, preempt,
        ))
    }

    fn wait<'a>(
        &'a self,
        request: WaitRequest,
        preempt: Option<CancellationToken>,
    ) -> CodeModeSessionResultFuture<'a, WaitOutcome> {
        Box::pin(InProcessCodeModeSession::wait(self, request, preempt))
    }

    fn terminate<'a>(
        &'a self,
        request: TerminateRequest,
    ) -> CodeModeSessionResultFuture<'a, WaitOutcome> {
        Box::pin(InProcessCodeModeSession::terminate(self, request))
    }

    fn shutdown<'a>(&'a self) -> CodeModeSessionResultFuture<'a, ()> {
        Box::pin(InProcessCodeModeSession::shutdown(self))
    }
}

struct ProtocolDelegate {
    delegate: Arc<dyn CodeModeSessionDelegate>,
}

impl runtime::SessionRuntimeDelegate for ProtocolDelegate {
    #[tracing::instrument(
        name = "code_mode.runtime.invoke_tool",
        level = "info",
        skip_all,
        fields(
            cell.id = %invocation.cell_id,
            runtime_tool_call_id = invocation.runtime_tool_call_id.as_str(),
            tool_name = invocation.tool_name.name.as_str(),
            tool_namespace = invocation.tool_name.namespace.as_deref(),
        )
    )]
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

fn runtime_request(
    request: ExecuteRequest,
    output_truncation: CodeModeOutputTruncation,
) -> runtime::CreateCellRequest {
    runtime::CreateCellRequest {
        output_truncation,
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
    output_truncation: CodeModeOutputTruncation,
    max_output_tokens: Option<usize>,
) -> Result<ExecuteToPendingOutcome, String> {
    match event {
        runtime::CellEvent::Pending {
            content_items,
            pending_tool_call_ids,
        } => Ok(ExecuteToPendingOutcome::Pending {
            cell_id: cell_id.clone(),
            content_items: rendered_items(content_items, output_truncation, max_output_tokens),
            pending_tool_call_ids,
        }),
        event => Ok(ExecuteToPendingOutcome::Completed(runtime_response(
            cell_id,
            event,
            output_truncation,
            max_output_tokens,
        )?)),
    }
}

fn runtime_response(
    cell_id: &CellId,
    event: runtime::CellEvent,
    output_truncation: CodeModeOutputTruncation,
    max_output_tokens: Option<usize>,
) -> Result<RuntimeResponse, String> {
    match event {
        runtime::CellEvent::Yielded { content_items } => Ok(RuntimeResponse::Yielded {
            cell_id: cell_id.clone(),
            content_items: rendered_items(content_items, output_truncation, max_output_tokens),
            code_mode_host_duration: None,
        }),
        runtime::CellEvent::Completed {
            content_items,
            error_text,
            result_buffered_for,
        } => Ok(RuntimeResponse::Result {
            cell_id: cell_id.clone(),
            content_items: rendered_items(content_items, output_truncation, max_output_tokens),
            error_text,
            result_buffered_for,
            code_mode_host_duration: None,
        }),
        runtime::CellEvent::Terminated { content_items } => Ok(RuntimeResponse::Terminated {
            cell_id: cell_id.clone(),
            content_items: rendered_items(content_items, output_truncation, max_output_tokens),
            code_mode_host_duration: None,
        }),
        runtime::CellEvent::Pending { .. } => {
            Err("cell returned a pending frontier unexpectedly".to_string())
        }
    }
}

fn rendered_items(
    items: OutputCollector,
    mode: CodeModeOutputTruncation,
    max_output_tokens: Option<usize>,
) -> Vec<FunctionCallOutputContentItem> {
    let items = items.into_collected_output();
    match mode {
        CodeModeOutputTruncation::Client => items.into_untruncated_items(),
        CodeModeOutputTruncation::Server => crate::output::render_output(items, max_output_tokens),
    }
}

fn missing_cell_response(
    cell_id: CellId,
    mode: CodeModeOutputTruncation,
    max_output_tokens: Option<usize>,
) -> Result<RuntimeResponse, String> {
    let error_text = format!("exec cell {cell_id} not found");
    let mut output = OutputCollector::new(mode);
    let event = take_completed_event(&mut output, Some(error_text));
    runtime_response(&cell_id, event, mode, max_output_tokens)
}

fn missing_wait(
    cell_id: CellId,
    mode: CodeModeOutputTruncation,
    max_output_tokens: Option<usize>,
) -> CodeModeSessionResultFuture<'static, WaitOutcome> {
    Box::pin(async move {
        Ok(WaitOutcome::MissingCell(missing_cell_response(
            cell_id,
            mode,
            max_output_tokens,
        )?))
    })
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "service_contract_tests.rs"]
mod contract_tests;

#[cfg(test)]
#[path = "service_output_collection_tests.rs"]
mod output_collection_tests;
