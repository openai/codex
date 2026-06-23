mod types;

use std::collections::HashMap;
use std::time::Duration;

use codex_code_mode_protocol::DEFAULT_IMAGE_DETAIL;
use codex_code_mode_protocol::FunctionCallOutputContentItem;
use serde_json::Value as JsonValue;
use tokio::sync::mpsc;

pub use self::types::ActorError;
pub use self::types::ActorErrorKind;
pub use self::types::CellClosed;
use self::types::CellCommand;
pub use self::types::CellEvent;
pub use self::types::CellHandle;
pub use self::types::CellRequest;
pub use self::types::CellTask;
pub use self::types::InvalidCellRequest;
pub use self::types::PreparedCell;
pub use self::types::StartError;
pub use self::types::ToolCallOutcome;
use crate::runtime::PendingRuntimeMode;
use crate::runtime::RuntimeCommand;
use crate::runtime::RuntimeControlCommand;
use crate::runtime::RuntimeEvent;
use crate::runtime::spawn_owned_runtime;

const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

/// A callback-only cell actor that continues through runtime checkpoints.
pub struct CellActor;

impl CellActor {
    /// Prepares a cell without spawning its actor task.
    ///
    /// The returned future must be polled by the owning session.
    pub fn prepare(
        request: CellRequest,
        stored_values: HashMap<String, JsonValue>,
    ) -> Result<PreparedCell, StartError> {
        let (runtime_event_tx, runtime_event_rx) = mpsc::unbounded_channel();
        let (runtime_tx, runtime_control_tx, runtime_terminate_handle, runtime_thread) =
            spawn_owned_runtime(
                stored_values,
                request.into_runtime_request(),
                runtime_event_tx,
                PendingRuntimeMode::Continue,
            )
            .map_err(StartError::new)?;
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let task = Box::pin(run_cell(
            RuntimeHandle {
                command_tx: runtime_tx,
                control_tx: runtime_control_tx,
                terminator: RuntimeTerminator::Isolate(runtime_terminate_handle),
                thread: Some(runtime_thread),
            },
            runtime_event_rx,
            command_rx,
            event_tx,
            SHUTDOWN_TIMEOUT,
        ));
        Ok((CellHandle { command_tx }, event_rx, task))
    }
}

struct RuntimeHandle {
    command_tx: std::sync::mpsc::Sender<RuntimeCommand>,
    control_tx: std::sync::mpsc::Sender<RuntimeControlCommand>,
    terminator: RuntimeTerminator,
    thread: Option<crate::runtime::RuntimeThread>,
}

enum RuntimeTerminator {
    Isolate(v8::IsolateHandle),
    #[cfg(test)]
    Noop,
}

impl RuntimeHandle {
    fn terminate(&self) {
        let _ = self.control_tx.send(RuntimeControlCommand::Terminate);
        let _ = self.command_tx.send(RuntimeCommand::Terminate);
        match &self.terminator {
            RuntimeTerminator::Isolate(terminate_handle) => {
                terminate_handle.terminate_execution();
            }
            #[cfg(test)]
            RuntimeTerminator::Noop => {}
        }
    }
}

impl Drop for RuntimeHandle {
    fn drop(&mut self) {
        if self
            .thread
            .as_ref()
            .is_some_and(crate::runtime::RuntimeThread::join_pending)
        {
            self.terminate();
        }
    }
}

async fn run_cell(
    runtime: RuntimeHandle,
    mut runtime_event_rx: mpsc::UnboundedReceiver<RuntimeEvent>,
    mut command_rx: mpsc::UnboundedReceiver<CellCommand>,
    event_tx: mpsc::UnboundedSender<CellEvent>,
    shutdown_timeout: Duration,
) -> Result<(), ActorError> {
    let mut started = false;
    let mut command_channel_open = true;
    let outcome = loop {
        tokio::select! {
            biased;
            command = command_rx.recv(), if command_channel_open => {
                match command {
                    Some(CellCommand::FinishToolCall { id, outcome }) => {
                        let command = tool_call_command(id, outcome);
                        // The runtime may have already emitted its terminal event and
                        // closed this receiver. Preserve that outcome instead of
                        // synthesizing explicit termination.
                        let _ = runtime.command_tx.send(command);
                    }
                    Some(CellCommand::Terminate) => {
                        runtime.terminate();
                        break ShutdownOutcome::Terminal(CellEvent::Terminated);
                    }
                    None => command_channel_open = false,
                }
            }
            event = runtime_event_rx.recv() => {
                let Some(event) = event else {
                    break ShutdownOutcome::Fault(ActorErrorKind::RuntimeClosedUnexpectedly);
                };
                match event {
                    RuntimeEvent::Started => {
                        if started {
                            runtime.terminate();
                            break ShutdownOutcome::Fault(
                                ActorErrorKind::RuntimeClosedUnexpectedly,
                            );
                        }
                        started = true;
                        let _ = event_tx.send(CellEvent::Started);
                    }
                    RuntimeEvent::Result {
                        stored_value_writes,
                        error_text,
                    } => {
                        break ShutdownOutcome::Terminal(CellEvent::Completed {
                            stored_value_writes,
                            error_text,
                        });
                    }
                    _event if !started => {
                        runtime.terminate();
                        break ShutdownOutcome::Fault(ActorErrorKind::RuntimeClosedUnexpectedly);
                    }
                    RuntimeEvent::Pending => {}
                    RuntimeEvent::ContentItem(item) => send_output(&event_tx, item),
                    RuntimeEvent::YieldRequested => {
                        let _ = event_tx.send(CellEvent::YieldRequested);
                    }
                    RuntimeEvent::Notify { call_id, text } => {
                        let _ = event_tx.send(CellEvent::Notification { call_id, text });
                    }
                    RuntimeEvent::ToolCall {
                        id,
                        name,
                        kind,
                        input,
                    } => {
                        let _ = event_tx.send(CellEvent::ToolCallRequested {
                            id,
                            name,
                            kind,
                            input,
                        });
                    }
                }
            }
        }
    };
    drop(command_rx);
    drop(runtime_event_rx);
    finish_shutdown(
        runtime,
        outcome,
        &event_tx,
        tokio::time::Instant::now() + shutdown_timeout,
    )
    .await
}

enum ShutdownOutcome {
    Terminal(CellEvent),
    Fault(ActorErrorKind),
}

async fn finish_shutdown(
    mut runtime: RuntimeHandle,
    outcome: ShutdownOutcome,
    event_tx: &mpsc::UnboundedSender<CellEvent>,
    shutdown_deadline: tokio::time::Instant,
) -> Result<(), ActorError> {
    let Some(runtime_thread) = runtime.thread.as_mut() else {
        return Err(ActorError::new(ActorErrorKind::RuntimeClosedUnexpectedly));
    };
    let runtime_finished = tokio::select! {
        biased;
        _ = tokio::time::sleep_until(shutdown_deadline) => false,
        _ = runtime_thread.wait() => true,
    };
    if !runtime_finished {
        runtime.terminate();
        let Some(runtime_thread) = runtime.thread.take() else {
            return Err(ActorError::new(ActorErrorKind::RuntimeClosedUnexpectedly));
        };
        return Err(ActorError::cleanup_timeout(runtime_thread));
    }
    let Some(runtime_thread) = runtime.thread.as_mut() else {
        return Err(ActorError::new(ActorErrorKind::RuntimeClosedUnexpectedly));
    };
    if runtime_thread.join_finished().is_err() {
        return Err(ActorError::new(ActorErrorKind::RuntimeThreadPanicked));
    }

    match outcome {
        ShutdownOutcome::Terminal(terminal) => {
            let _ = event_tx.send(terminal);
            Ok(())
        }
        ShutdownOutcome::Fault(kind) => Err(ActorError::new(kind)),
    }
}

fn tool_call_command(id: String, outcome: ToolCallOutcome) -> RuntimeCommand {
    match outcome {
        ToolCallOutcome::Result(result) => RuntimeCommand::ToolResponse { id, result },
        ToolCallOutcome::Error(error_text) => RuntimeCommand::ToolError { id, error_text },
    }
}

fn send_output(event_tx: &mpsc::UnboundedSender<CellEvent>, item: FunctionCallOutputContentItem) {
    let _ = event_tx.send(match item {
        FunctionCallOutputContentItem::InputText { text } => CellEvent::OutputText { text },
        FunctionCallOutputContentItem::InputImage { image_url, detail } => CellEvent::OutputImage {
            image_url,
            detail: detail.unwrap_or(DEFAULT_IMAGE_DETAIL),
        },
    });
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
