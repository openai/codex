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
    pub tool_call_id: String,
    pub enabled_tools: Vec<ToolDefinition>,
    pub source: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ObserveRequest {
    pub cell_id: CellId,
    pub yield_time_ms: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ObserveToPendingRequest {
    pub cell_id: CellId,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub enum CellOutcome {
    LiveCell(RuntimeResponse),
    MissingCell(RuntimeResponse),
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub enum PendingOutcome {
    Pending {
        cell_id: CellId,
        content_items: Vec<FunctionCallOutputContentItem>,
        pending_tool_call_ids: Vec<String>,
    },
    Completed(RuntimeResponse),
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub enum ObserveToPendingOutcome {
    LiveCell(PendingOutcome),
    MissingCell(RuntimeResponse),
}

impl From<CellOutcome> for RuntimeResponse {
    fn from(outcome: CellOutcome) -> Self {
        match outcome {
            CellOutcome::LiveCell(response) | CellOutcome::MissingCell(response) => response,
        }
    }
}

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
