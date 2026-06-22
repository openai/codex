use std::fmt;
use std::future::Future;
use std::time::Duration;

use serde_json::Value as JsonValue;
use tokio_util::sync::CancellationToken;

/// Identifies one execution cell within a session runtime.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct CellId(String);

impl CellId {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CellId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Selects the next observable frontier for a running cell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ObserveMode {
    YieldAfter(Duration),
    PendingFrontier,
}

/// An observable cell lifecycle event.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum CellEvent {
    Yielded {
        content_items: Vec<OutputItem>,
    },
    Pending {
        content_items: Vec<OutputItem>,
        pending_tool_call_ids: Vec<String>,
    },
    Completed {
        content_items: Vec<OutputItem>,
        error_text: Option<String>,
    },
    Terminated {
        content_items: Vec<OutputItem>,
    },
}

/// Output emitted by a cell since its preceding observation.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum OutputItem {
    Text {
        text: String,
    },
    Image {
        image_url: String,
        detail: Option<ImageDetail>,
    },
}

/// Requested image fidelity for an output image.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ImageDetail {
    Auto,
    Low,
    High,
    Original,
}

/// Transport-neutral input for creating a cell.
///
/// The owning session assigns the cell ID when it admits the request.
pub(crate) struct CreateCellRequest {
    pub(crate) tool_call_id: String,
    pub(crate) enabled_tools: Vec<ToolDefinition>,
    pub(crate) source: String,
}

/// Tool metadata exposed to code running inside a cell.
pub(crate) struct ToolDefinition {
    pub(crate) name: String,
    pub(crate) tool_name: ToolName,
    pub(crate) description: String,
    pub(crate) kind: ToolKind,
}

/// A tool name with an optional namespace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ToolName {
    pub(crate) name: String,
    pub(crate) namespace: Option<String>,
}

/// The JavaScript calling convention for a tool.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ToolKind {
    Function,
    Freeform,
}

/// A nested tool request emitted by a running cell.
pub(crate) struct NestedToolCall {
    pub(crate) cell_id: CellId,
    pub(crate) runtime_tool_call_id: String,
    pub(crate) tool_name: ToolName,
    pub(crate) tool_kind: ToolKind,
    pub(crate) input: Option<JsonValue>,
}

/// Host callbacks used by cells owned by a [`super::SessionRuntime`].
///
/// Implementations must honor cancellation tokens. `cell_closed` is called
/// after the runtime has stopped routing requests to the cell.
pub(crate) trait SessionRuntimeDelegate: Send + Sync + 'static {
    fn invoke_tool(
        &self,
        invocation: NestedToolCall,
        cancellation_token: CancellationToken,
    ) -> impl Future<Output = Result<JsonValue, String>> + Send;

    fn notify(
        &self,
        call_id: String,
        cell_id: CellId,
        text: String,
        cancellation_token: CancellationToken,
    ) -> impl Future<Output = Result<(), String>> + Send;

    fn cell_closed(&self, cell_id: &CellId);
}

/// A failure reported by a session runtime operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Error {
    ShuttingDown,
    DuplicateCell(CellId),
    MissingCell(CellId),
    BusyObserver(CellId),
    AlreadyTerminating(CellId),
    ClosedCell(CellId),
    Runtime(String),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ShuttingDown => formatter.write_str("code mode session is shutting down"),
            Self::DuplicateCell(cell_id) => write!(formatter, "exec cell {cell_id} already exists"),
            Self::MissingCell(cell_id) => write!(formatter, "exec cell {cell_id} not found"),
            Self::BusyObserver(cell_id) => {
                write!(
                    formatter,
                    "exec cell {cell_id} already has an active observer"
                )
            }
            Self::AlreadyTerminating(cell_id) => {
                write!(formatter, "exec cell {cell_id} is already terminating")
            }
            Self::ClosedCell(cell_id) => {
                write!(formatter, "exec cell {cell_id} closed unexpectedly")
            }
            Self::Runtime(error_text) => formatter.write_str(error_text),
        }
    }
}

impl std::error::Error for Error {}
