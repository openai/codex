use std::fmt;
use std::future::Future;

use serde_json::Value as JsonValue;
use tokio_util::sync::CancellationToken;

/// Identifies one execution cell within a session runtime.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CellId(String);

impl CellId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CellId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Controls how a cell advances when its runtime is waiting for external input.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum CellExecutionPolicy {
    /// Process tool and timer results even when no observation is attached.
    #[default]
    ContinueWhenUnblocked,
    /// Remain paused at a pending frontier until an explicit resume advances it.
    PauseAtPendingFrontier,
}

/// A cell that continues whenever external input unblocks its runtime.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Cell {
    id: CellId,
}

impl Cell {
    pub(super) fn new(id: CellId) -> Self {
        Self { id }
    }

    pub fn id(&self) -> &CellId {
        &self.id
    }
}

/// A cell that remains paused at each pending frontier until explicitly resumed.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PausableCell {
    id: CellId,
}

impl PausableCell {
    pub(super) fn new(id: CellId) -> Self {
        Self { id }
    }

    pub fn id(&self) -> &CellId {
        &self.id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CellKind {
    Continuing,
    Pausable,
}

impl fmt::Display for CellKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Continuing => formatter.write_str("continuing"),
            Self::Pausable => formatter.write_str("pausable"),
        }
    }
}

/// Identifies one durable pending frontier of a pausable cell.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PendingGeneration(u64);

impl PendingGeneration {
    pub(crate) fn new(value: u64) -> Self {
        Self(value)
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

/// A repeatable snapshot of one paused runtime frontier.
#[derive(Clone, Debug, PartialEq)]
pub struct PendingFrontier {
    pub generation: PendingGeneration,
    pub content_items: Vec<OutputItem>,
    pub pending_tool_call_ids: Vec<String>,
}

/// An observable cell lifecycle event.
#[derive(Clone, Debug, PartialEq)]
pub enum CellEvent {
    Yielded {
        content_items: Vec<OutputItem>,
    },
    Completed {
        content_items: Vec<OutputItem>,
        error_text: Option<String>,
    },
    Terminated {
        content_items: Vec<OutputItem>,
    },
}

/// An observable lifecycle event for a pausable cell.
#[derive(Clone, Debug, PartialEq)]
pub enum PausableCellEvent {
    Pending(PendingFrontier),
    Completed {
        content_items: Vec<OutputItem>,
        error_text: Option<String>,
    },
    Terminated {
        content_items: Vec<OutputItem>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResumeOutcome {
    Resumed,
    AlreadyRunning,
}

/// Output emitted by a cell since its preceding observation.
#[derive(Clone, Debug, PartialEq)]
pub enum OutputItem {
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
pub enum ImageDetail {
    Auto,
    Low,
    High,
    Original,
}

/// Transport-neutral input for creating a cell.
///
/// The owning session assigns the cell ID when it admits the request.
#[derive(Debug, PartialEq)]
pub struct CreateCellRequest {
    pub idempotency_key: String,
    pub tool_call_id: String,
    pub enabled_tools: Vec<ToolDefinition>,
    pub source: String,
}

/// Tool metadata exposed to code running inside a cell.
#[derive(Debug, PartialEq)]
pub struct ToolDefinition {
    pub name: String,
    pub tool_name: ToolName,
    pub description: String,
    pub kind: ToolKind,
}

/// A tool name with an optional namespace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolName {
    pub name: String,
    pub namespace: Option<String>,
}

/// The JavaScript calling convention for a tool.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolKind {
    Function,
    Freeform,
}

/// A nested tool request emitted by a running cell.
#[derive(Debug, PartialEq)]
pub struct NestedToolCall {
    pub cell_id: CellId,
    pub runtime_tool_call_id: String,
    pub tool_name: ToolName,
    pub tool_kind: ToolKind,
    pub input: Option<JsonValue>,
}

/// Host callbacks used by cells owned by a [`super::SessionRuntime`].
///
/// Implementations should forward callback cancellation tokens to downstream
/// work. After cancellation begins, the runtime allows callbacks a bounded
/// grace period to finish, then aborts their local tasks.
pub trait SessionRuntimeDelegate: Send + Sync + 'static {
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
}

/// A failure reported by a session runtime operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    ShuttingDown,
    DuplicateCell(CellId),
    MissingCell(CellId),
    BusyObserver(CellId),
    AlreadyTerminating(CellId),
    ClosedCell(CellId),
    WrongCellKind {
        cell_id: CellId,
        expected: CellKind,
        actual: CellKind,
    },
    InvalidGeneration {
        cell_id: CellId,
        requested: PendingGeneration,
        latest: Option<PendingGeneration>,
    },
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
            Self::WrongCellKind {
                cell_id,
                expected,
                actual,
            } => write!(
                formatter,
                "exec cell {cell_id} is {actual}, expected {expected}"
            ),
            Self::InvalidGeneration {
                cell_id,
                requested,
                latest,
            } => write!(
                formatter,
                "exec cell {cell_id} cannot resume generation {}; latest generation is {}",
                requested.get(),
                latest.map_or_else(
                    || "none".to_string(),
                    |generation| generation.get().to_string()
                )
            ),
            Self::Runtime(error_text) => formatter.write_str(error_text),
        }
    }
}

impl std::error::Error for Error {}
