use codex_protocol::ToolName;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;

use crate::CellId;
use crate::CodeModeToolKind;
use crate::FunctionCallOutputContentItem;
use crate::ToolDefinition;

pub const DEFAULT_EXEC_YIELD_TIME_MS: u64 = 10_000;
pub const DEFAULT_WAIT_YIELD_TIME_MS: u64 = 10_000;
pub const DEFAULT_MAX_OUTPUT_TOKENS_PER_EXEC_CALL: usize = 10_000;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CreateCellRequest {
    pub cell_id: CellId,
    pub tool_call_id: String,
    pub enabled_tools: Vec<ToolDefinition>,
    pub source: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ObserveRequest {
    pub cell_id: CellId,
    pub generation: ObservationGeneration,
    pub yield_time_ms: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ReleaseObservationRequest {
    pub cell_id: CellId,
    pub generation: ObservationGeneration,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ObservationGeneration(u64);

impl ObservationGeneration {
    pub const INITIAL: Self = Self(0);

    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    pub const fn next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}

/// A valid result of observing a cell.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum ObserveOutcome {
    #[serde(rename = "yielded")]
    Yielded {
        cell_id: CellId,
        content_items: Vec<FunctionCallOutputContentItem>,
    },
    #[serde(rename = "completed")]
    Completed {
        cell_id: CellId,
        content_items: Vec<FunctionCallOutputContentItem>,
        error_text: Option<String>,
    },
    #[serde(rename = "terminated")]
    Terminated {
        cell_id: CellId,
        content_items: Vec<FunctionCallOutputContentItem>,
    },
    #[serde(rename = "missing")]
    Missing { cell_id: CellId },
}

/// A valid result of requesting cell termination.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum TerminateOutcome {
    #[serde(rename = "completed")]
    Completed {
        cell_id: CellId,
        content_items: Vec<FunctionCallOutputContentItem>,
        error_text: Option<String>,
    },
    #[serde(rename = "terminated")]
    Terminated {
        cell_id: CellId,
        content_items: Vec<FunctionCallOutputContentItem>,
    },
    #[serde(rename = "missing")]
    Missing { cell_id: CellId },
}

impl From<ObserveOutcome> for RuntimeResponse {
    fn from(outcome: ObserveOutcome) -> Self {
        match outcome {
            ObserveOutcome::Yielded {
                cell_id,
                content_items,
            } => Self::Yielded {
                cell_id,
                content_items,
            },
            ObserveOutcome::Completed {
                cell_id,
                content_items,
                error_text,
            } => Self::Result {
                cell_id,
                content_items,
                error_text,
            },
            ObserveOutcome::Terminated {
                cell_id,
                content_items,
            } => Self::Terminated {
                cell_id,
                content_items,
            },
            ObserveOutcome::Missing { cell_id } => missing_cell_response(cell_id),
        }
    }
}

impl From<TerminateOutcome> for RuntimeResponse {
    fn from(outcome: TerminateOutcome) -> Self {
        match outcome {
            TerminateOutcome::Completed {
                cell_id,
                content_items,
                error_text,
            } => Self::Result {
                cell_id,
                content_items,
                error_text,
            },
            TerminateOutcome::Terminated {
                cell_id,
                content_items,
            } => Self::Terminated {
                cell_id,
                content_items,
            },
            TerminateOutcome::Missing { cell_id } => missing_cell_response(cell_id),
        }
    }
}

/// Core's model-facing representation after decoding a session outcome.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum RuntimeResponse {
    Yielded {
        cell_id: CellId,
        content_items: Vec<FunctionCallOutputContentItem>,
    },
    Terminated {
        cell_id: CellId,
        content_items: Vec<FunctionCallOutputContentItem>,
    },
    Result {
        cell_id: CellId,
        content_items: Vec<FunctionCallOutputContentItem>,
        error_text: Option<String>,
    },
}

fn missing_cell_response(cell_id: CellId) -> RuntimeResponse {
    RuntimeResponse::Result {
        error_text: Some(format!("exec cell {cell_id} not found")),
        cell_id,
        content_items: Vec::new(),
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CodeModeNestedToolCall {
    pub cell_id: CellId,
    pub runtime_tool_call_id: String,
    pub tool_name: ToolName,
    pub tool_kind: CodeModeToolKind,
    pub input: Option<JsonValue>,
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
