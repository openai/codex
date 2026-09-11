use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::Weak;

use codex_code_mode::CellId;
use codex_code_mode::CodeModeNestedToolCall;
use codex_code_mode::CodeModeSessionDelegate;
use codex_code_mode::NotificationFuture;
use codex_code_mode::ToolInvocationFuture;
use codex_history::CodexHarnessMetadata;
use codex_history::ResponseItemEnvelope;
use codex_protocol::ResponseItemId;
use codex_protocol::ThreadId;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseItem;
use codex_utils_output_truncation::with_serialization_allowance;
use serde_json::Value as JsonValue;
use tokio::sync::oneshot;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use tracing::Instrument;

use super::PUBLIC_TOOL_NAME;
use super::submit_nested_tool;
use super::telemetry::DispatchInterruption;
use super::telemetry::NestedToolDispatchTrace;
use super::telemetry::trace_id;
use crate::session::session::Session;
use crate::session::step_context::StepContext;
use crate::tools::ExecutedToolCalls;
use crate::tools::call_trace;
use crate::tools::context::SharedTurnDiffTracker;
use crate::tools::parallel::ToolCallRuntime;

pub(super) struct CodeModeDispatchBroker {
    thread_id: ThreadId,
    dispatch_tx: async_channel::Sender<DispatchMessage>,
    dispatch_rx: async_channel::Receiver<DispatchMessage>,
    dispatch_gates: Arc<Mutex<HashMap<CellId, CellDispatchGate>>>,
    executed_tool_calls: ExecutedToolCalls,
}

/// Retains the step advertised to one execution, including callbacks after it yields.
pub(super) struct CodeModeCellDelegate {
    pub(super) broker: Arc<CodeModeDispatchBroker>,
    pub(super) step_context: Arc<StepContext>,
}

struct CellDispatchGate {
    ready: watch::Sender<bool>,
    // Keep the original exec item when later waits resume this cell.
    originating_item_id: Option<ResponseItemId>,
}

impl CodeModeDispatchBroker {
    pub(super) fn new(thread_id: ThreadId, executed_tool_calls: ExecutedToolCalls) -> Self {
        let (dispatch_tx, dispatch_rx) = async_channel::unbounded();
        Self {
            thread_id,
            dispatch_tx,
            dispatch_rx,
            dispatch_gates: Arc::new(Mutex::new(HashMap::new())),
            executed_tool_calls,
        }
    }

    pub(super) fn mark_cell_ready_for_dispatch(
        &self,
        cell_id: &CellId,
        originating_item_id: Option<ResponseItemId>,
    ) {
        let ready = {
            let mut dispatch_gates = self
                .dispatch_gates
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let gate = dispatch_gates
                .entry(cell_id.clone())
                .or_insert_with(|| CellDispatchGate {
                    ready: watch::channel(false).0,
                    originating_item_id: None,
                });
            gate.originating_item_id = originating_item_id;
            gate.ready.clone()
        };
        ready.send_replace(true);
    }

    pub(super) fn cell_originating_item_id(&self, cell_id: &CellId) -> Option<ResponseItemId> {
        self.dispatch_gates
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(cell_id)
            .and_then(|gate| gate.originating_item_id.clone())
    }

    pub(super) fn close_cell(&self, cell_id: &CellId) {
        let mut dispatch_gates = self
            .dispatch_gates
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        dispatch_gates.remove(cell_id);
        self.executed_tool_calls.finish_cell_recording(cell_id);
    }

    pub(super) fn active_cell_ids(&self) -> Vec<CellId> {
        self.dispatch_gates
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .keys()
            .cloned()
            .collect()
    }

    pub(super) fn start_turn_worker(
        &self,
        session: Arc<Session>,
        step_context: Arc<StepContext>,
        tracker: SharedTurnDiffTracker,
    ) -> CodeModeDispatchWorker {
        let tool_runtime = ToolCallRuntime::new(Arc::clone(&session), step_context, tracker);
        let host = Arc::new(CoreTurnHost {
            session,
            tool_runtime,
        });
        let dispatch_rx = self.dispatch_rx.clone();
        let dispatch_gates = Arc::clone(&self.dispatch_gates);
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
        tokio::spawn(async move {
            loop {
                let message = tokio::select! {
                    _ = &mut shutdown_rx => break,
                    message = dispatch_rx.recv() => message.ok(),
                };
                let Some(message) = message else {
                    break;
                };
                match message {
                    DispatchMessage::Notify {
                        call_id,
                        cell_id,
                        text,
                        output_token_limit,
                        cancellation_token,
                        response_tx,
                    } => {
                        let response = if wait_until_cell_ready_for_dispatch(
                            &dispatch_gates,
                            &cell_id,
                            &cancellation_token,
                        )
                        .await
                        {
                            host.notify(call_id, cell_id, text, output_token_limit)
                                .await
                        } else {
                            remove_dispatch_gate(&dispatch_gates, &cell_id);
                            Err("code mode notification cancelled".to_string())
                        };
                        let _ = response_tx.send(response);
                    }
                    DispatchMessage::InvokeTool {
                        invocation,
                        step_context,
                        mut dispatch_trace,
                        cancellation_token,
                        response_tx,
                    } => {
                        let cell_id = invocation.cell_id.clone();
                        if !wait_until_cell_ready_for_dispatch(
                            &dispatch_gates,
                            &cell_id,
                            &cancellation_token,
                        )
                        .await
                        {
                            let outcome = if cancellation_token.is_cancelled() {
                                DispatchInterruption::Cancelled
                            } else {
                                DispatchInterruption::CellClosed
                            };
                            dispatch_trace.interruption = Some(outcome);
                            remove_dispatch_gate(&dispatch_gates, &cell_id);
                            continue;
                        }
                        let Some(step_context) = step_context
                            .upgrade()
                            .filter(|_| !cancellation_token.is_cancelled())
                        else {
                            let _ = response_tx
                                .send(Err("code mode nested tool call cancelled".to_string()));
                            continue;
                        };
                        let track_completeness =
                            ExecutedToolCalls::is_enabled(&step_context.turn.config.features);
                        let host = Arc::clone(&host);
                        let dispatch_gates = Arc::clone(&dispatch_gates);
                        let span = dispatch_trace.span.clone();
                        tokio::spawn(async move {
                            let invocation = {
                                let dispatch_gate = track_completeness.then(|| {
                                    dispatch_gates
                                        .lock()
                                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                                });
                                if dispatch_gate.as_ref().is_some_and(|gates| {
                                    cancellation_token.is_cancelled()
                                        || !gates.contains_key(&cell_id)
                                }) {
                                    let outcome = if cancellation_token.is_cancelled() {
                                        DispatchInterruption::Cancelled
                                    } else {
                                        DispatchInterruption::CellClosed
                                    };
                                    dispatch_trace.interruption = Some(outcome);
                                    return;
                                }
                                // Submission and cell closure share this gate.
                                span.in_scope(|| {
                                    dispatch_trace.interruption = None;
                                    host.submit_tool(
                                        invocation,
                                        step_context,
                                        dispatch_trace.call_id.clone(),
                                        cancellation_token.clone(),
                                    )
                                })
                                .instrument(span)
                            };
                            tokio::pin!(invocation);
                            let response = tokio::select! {
                                biased;
                                _ = cancellation_token.cancelled() => invocation.await,
                                response = &mut invocation => response,
                            };
                            let _ = response_tx.send(response);
                        });
                    }
                }
            }
        });
        CodeModeDispatchWorker {
            shutdown_tx: Some(shutdown_tx),
        }
    }
}

fn dispatch_gate(
    dispatch_gates: &Mutex<HashMap<CellId, CellDispatchGate>>,
    cell_id: &CellId,
) -> watch::Sender<bool> {
    let mut dispatch_gates = match dispatch_gates.lock() {
        Ok(dispatch_gates) => dispatch_gates,
        Err(poisoned) => poisoned.into_inner(),
    };
    dispatch_gates
        .entry(cell_id.clone())
        .or_insert_with(|| CellDispatchGate {
            ready: watch::channel(false).0,
            originating_item_id: None,
        })
        .ready
        .clone()
}

fn remove_dispatch_gate(
    dispatch_gates: &Mutex<HashMap<CellId, CellDispatchGate>>,
    cell_id: &CellId,
) {
    let mut dispatch_gates = match dispatch_gates.lock() {
        Ok(dispatch_gates) => dispatch_gates,
        Err(poisoned) => poisoned.into_inner(),
    };
    dispatch_gates.remove(cell_id);
}

async fn wait_until_cell_ready_for_dispatch(
    dispatch_gates: &Mutex<HashMap<CellId, CellDispatchGate>>,
    cell_id: &CellId,
    cancellation_token: &CancellationToken,
) -> bool {
    if cancellation_token.is_cancelled() {
        return false;
    }
    let mut ready_rx = dispatch_gate(dispatch_gates, cell_id).subscribe();
    loop {
        if *ready_rx.borrow_and_update() {
            return true;
        }
        tokio::select! {
            changed = ready_rx.changed() => {
                if changed.is_err() {
                    return false;
                }
            }
            _ = cancellation_token.cancelled() => return false,
        }
    }
}

impl CodeModeSessionDelegate for CodeModeCellDelegate {
    #[tracing::instrument(
        name = "code_mode.broker.invoke_tool",
        level = "info",
        skip_all,
        fields(
            conversation.id = %self.broker.thread_id,
            cell.id = %invocation.cell_id,
            runtime_tool_call_id = invocation.runtime_tool_call_id.as_str(),
            tool_name = invocation.tool_name.name.as_str(),
            tool_namespace = invocation.tool_name.namespace.as_deref(),
        )
    )]
    fn invoke_tool<'a>(
        &'a self,
        invocation: CodeModeNestedToolCall,
        cancellation_token: CancellationToken,
    ) -> ToolInvocationFuture<'a> {
        Box::pin(async move {
            let call_id = format!("{PUBLIC_TOOL_NAME}-{}", uuid::Uuid::new_v4());
            call_trace::received(
                self.broker.thread_id,
                &invocation.tool_name,
                &call_id,
                call_trace::Receipt::CodeModeBroker {
                    cell_id: trace_id(invocation.cell_id.as_str()),
                    runtime_tool_call_id: trace_id(&invocation.runtime_tool_call_id),
                },
            );
            let mut dispatch_trace =
                Box::new(NestedToolDispatchTrace::new(self.broker.thread_id, call_id));
            if cancellation_token.is_cancelled() {
                dispatch_trace.interruption = Some(DispatchInterruption::Cancelled);
                return Err("code mode nested tool call cancelled".to_string());
            }
            let (response_tx, response_rx) = oneshot::channel();
            // Only the worker can tell whether dispatch beats cancellation once the call is queued.
            self.broker
                .dispatch_tx
                .send(DispatchMessage::InvokeTool {
                    invocation,
                    step_context: Arc::downgrade(&self.step_context),
                    dispatch_trace,
                    cancellation_token: cancellation_token.clone(),
                    response_tx,
                })
                .await
                .map_err(|_| "code mode nested tool dispatcher is unavailable".to_string())?;
            tokio::select! {
                response = response_rx => response
                    .map_err(|_| "code mode nested tool dispatcher stopped".to_string())?,
                _ = cancellation_token.cancelled() => {
                    Err("code mode nested tool call cancelled".to_string())
                }
            }
        })
    }

    fn notify<'a>(
        &'a self,
        call_id: String,
        cell_id: CellId,
        text: String,
        cancellation_token: CancellationToken,
    ) -> NotificationFuture<'a> {
        Box::pin(async move {
            if cancellation_token.is_cancelled() {
                return Err("code mode notification cancelled".to_string());
            }
            let (response_tx, response_rx) = oneshot::channel();
            self.broker
                .dispatch_tx
                .send(DispatchMessage::Notify {
                    call_id,
                    cell_id,
                    text,
                    output_token_limit: with_serialization_allowance(
                        self.step_context
                            .settings
                            .model_info
                            .truncation_policy
                            .into(),
                    )
                    .token_budget(),
                    cancellation_token: cancellation_token.clone(),
                    response_tx,
                })
                .await
                .map_err(|_| "code mode notification dispatcher is unavailable".to_string())?;
            tokio::select! {
                response = response_rx => response
                    .map_err(|_| "code mode notification dispatcher stopped".to_string())?,
                _ = cancellation_token.cancelled() => {
                    Err("code mode notification cancelled".to_string())
                }
            }
        })
    }

    fn cell_closed(&self, cell_id: &CellId) {
        self.broker.close_cell(cell_id);
    }
}

enum DispatchMessage {
    InvokeTool {
        invocation: CodeModeNestedToolCall,
        // The delegate owns the step while the callback is live; stale queued work must not.
        step_context: Weak<StepContext>,
        dispatch_trace: Box<NestedToolDispatchTrace>,
        cancellation_token: CancellationToken,
        response_tx: oneshot::Sender<Result<JsonValue, String>>,
    },
    Notify {
        call_id: String,
        cell_id: CellId,
        text: String,
        output_token_limit: usize,
        cancellation_token: CancellationToken,
        response_tx: oneshot::Sender<Result<(), String>>,
    },
}

pub(crate) struct CodeModeDispatchWorker {
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl Drop for CodeModeDispatchWorker {
    fn drop(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
    }
}

struct CoreTurnHost {
    session: Arc<Session>,
    tool_runtime: ToolCallRuntime,
}

impl CoreTurnHost {
    fn submit_tool(
        &self,
        invocation: CodeModeNestedToolCall,
        step_context: Arc<StepContext>,
        call_id: String,
        cancellation_token: CancellationToken,
    ) -> impl std::future::Future<Output = Result<JsonValue, String>> + Send + 'static {
        let invocation = submit_nested_tool(
            Arc::clone(&self.session),
            step_context,
            self.tool_runtime.clone(),
            invocation,
            call_id,
            cancellation_token,
        )
        .map_err(|error| error.to_string());
        async move { invocation?.await.map_err(|error| error.to_string()) }
    }

    async fn notify(
        &self,
        call_id: String,
        cell_id: CellId,
        text: String,
        output_token_limit: usize,
    ) -> Result<(), String> {
        if text.trim().is_empty() {
            return Ok(());
        }
        self.session
            .inject_if_running(vec![ResponseItemEnvelope {
                item: ResponseItem::CustomToolCallOutput {
                    id: None,
                    call_id,
                    name: Some(PUBLIC_TOOL_NAME.to_string()),
                    output: FunctionCallOutputPayload::from_text(text),
                    internal_chat_message_metadata_passthrough: None,
                },
                metadata: Some(CodexHarnessMetadata {
                    history_truncation_token_limit: Some(output_token_limit),
                    ..Default::default()
                }),
            }])
            .await
            .map_err(|_| {
                format!("failed to inject exec notify message for cell {cell_id}: no active turn")
            })
    }
}

#[cfg(test)]
#[path = "delegate_tests.rs"]
mod tests;
