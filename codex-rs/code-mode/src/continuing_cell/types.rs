use std::fmt;
use std::future::Future;
use std::pin::Pin;

use codex_code_mode_protocol::CodeModeToolKind;
use codex_code_mode_protocol::ExecuteRequest;
use codex_code_mode_protocol::ImageDetail;
use codex_code_mode_protocol::ToolDefinition;
use codex_protocol::ToolName;
use serde_json::Value as JsonValue;
use tokio::sync::mpsc;

use crate::runtime::RuntimeThread;

/// Owned future that runs one prepared continuing cell.
pub type CellTask = Pin<Box<dyn Future<Output = Result<(), ActorError>> + Send + 'static>>;

/// Handle, event stream, and unspawned actor task for one admitted cell.
pub type PreparedCell = (CellHandle, mpsc::UnboundedReceiver<CellEvent>, CellTask);

/// Input required to start a callback-only continuing cell.
pub struct CellRequest {
    tool_call_id: String,
    enabled_tools: Vec<ToolDefinition>,
    source: String,
}

impl CellRequest {
    pub fn new(
        tool_call_id: impl Into<String>,
        enabled_tools: Vec<ToolDefinition>,
        source: impl Into<String>,
    ) -> Result<Self, InvalidCellRequest> {
        let tool_call_id = tool_call_id.into();
        if tool_call_id.trim().is_empty() {
            return Err(InvalidCellRequest::EmptyToolCallId);
        }
        let source = source.into();
        if source.trim().is_empty() {
            return Err(InvalidCellRequest::EmptySource);
        }
        Ok(Self {
            tool_call_id,
            enabled_tools,
            source,
        })
    }

    pub(super) fn into_runtime_request(self) -> ExecuteRequest {
        ExecuteRequest {
            tool_call_id: self.tool_call_id,
            enabled_tools: self.enabled_tools,
            source: self.source,
            yield_time_ms: None,
            max_output_tokens: None,
        }
    }
}

/// Why a continuing cell request could not be constructed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvalidCellRequest {
    EmptyToolCallId,
    EmptySource,
}

impl fmt::Display for InvalidCellRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyToolCallId => formatter.write_str("cell tool call ID must not be empty"),
            Self::EmptySource => formatter.write_str("cell source must not be empty"),
        }
    }
}

impl std::error::Error for InvalidCellRequest {}

/// Failure to initialize the V8 runtime for a continuing cell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StartError(String);

impl StartError {
    pub(super) fn new(message: String) -> Self {
        Self(message)
    }
}

impl fmt::Display for StartError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for StartError {}

/// A cell-actor fault that cannot be represented as a normal cell terminal event.
pub struct ActorError {
    kind: ActorErrorKind,
    runtime_thread: Option<RuntimeThread>,
}

impl ActorError {
    pub fn kind(&self) -> ActorErrorKind {
        self.kind
    }

    pub(super) fn new(kind: ActorErrorKind) -> Self {
        Self {
            kind,
            runtime_thread: None,
        }
    }

    pub(super) fn cleanup_timeout(runtime_thread: RuntimeThread) -> Self {
        Self::retained_runtime(ActorErrorKind::RuntimeCleanupTimedOut, runtime_thread)
    }

    fn retained_runtime(kind: ActorErrorKind, runtime_thread: RuntimeThread) -> Self {
        Self {
            kind,
            runtime_thread: Some(runtime_thread),
        }
    }
}

impl fmt::Debug for ActorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ActorError")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

impl fmt::Display for ActorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            ActorErrorKind::RuntimeClosedUnexpectedly => {
                formatter.write_str("continuing cell runtime closed without a terminal event")
            }
            ActorErrorKind::RuntimeThreadPanicked => {
                formatter.write_str("continuing cell runtime thread panicked")
            }
            ActorErrorKind::RuntimeCleanupTimedOut => {
                formatter.write_str("continuing cell runtime cleanup timed out")
            }
        }
    }
}

impl std::error::Error for ActorError {}

impl Drop for ActorError {
    fn drop(&mut self) {
        drop(self.runtime_thread.take());
    }
}

/// Actor-task faults for the session supervisor; these are not wire reasons.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActorErrorKind {
    RuntimeClosedUnexpectedly,
    RuntimeThreadPanicked,
    RuntimeCleanupTimedOut,
}

/// One ordered event emitted by a continuing cell actor.
#[derive(Clone, Debug, PartialEq)]
pub enum CellEvent {
    Started,
    OutputText {
        text: String,
    },
    OutputImage {
        image_url: String,
        detail: ImageDetail,
    },
    Notification {
        call_id: String,
        text: String,
    },
    YieldRequested,
    ToolCallRequested {
        id: String,
        name: ToolName,
        kind: CodeModeToolKind,
        input: Option<JsonValue>,
    },
    Completed {
        stored_value_writes: std::collections::HashMap<String, JsonValue>,
        error_text: Option<String>,
    },
    Terminated,
}

/// Result supplied for a previously emitted tool callback request.
#[derive(Debug)]
pub enum ToolCallOutcome {
    Result(JsonValue),
    Error(String),
}

/// A handle for sending callback results or termination to a cell actor.
#[derive(Clone)]
pub struct CellHandle {
    pub(super) command_tx: mpsc::UnboundedSender<CellCommand>,
}

impl CellHandle {
    pub fn finish_tool_call(
        &self,
        id: impl Into<String>,
        outcome: ToolCallOutcome,
    ) -> Result<(), CellClosed> {
        self.command_tx
            .send(CellCommand::FinishToolCall {
                id: id.into(),
                outcome,
            })
            .map_err(|_| CellClosed)
    }

    pub fn terminate(&self) -> Result<(), CellClosed> {
        self.command_tx
            .send(CellCommand::Terminate)
            .map_err(|_| CellClosed)
    }
}

/// Returned when a command targets an actor that has already closed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CellClosed;

impl fmt::Display for CellClosed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("continuing cell is closed")
    }
}

impl std::error::Error for CellClosed {}

pub(super) enum CellCommand {
    FinishToolCall {
        id: String,
        outcome: ToolCallOutcome,
    },
    Terminate,
}
