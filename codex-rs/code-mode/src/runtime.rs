use codex_protocol::ToolName;
use serde::Serialize;
use serde_json::Value as JsonValue;

use crate::CellId;
use crate::CodeModeToolKind;
use crate::FunctionCallOutputContentItem;
use crate::ToolDefinition;

pub const DEFAULT_EXEC_YIELD_TIME_MS: u64 = 10_000;
pub const DEFAULT_WAIT_YIELD_TIME_MS: u64 = 10_000;
pub const DEFAULT_MAX_OUTPUT_TOKENS_PER_EXEC_CALL: usize = 10_000;

#[derive(Clone, Debug)]
pub struct ExecuteRequest {
    pub tool_call_id: String,
    pub enabled_tools: Vec<ToolDefinition>,
    pub source: String,
    pub yield_time_ms: Option<u64>,
    pub max_output_tokens: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct WaitRequest {
    pub cell_id: CellId,
    pub yield_time_ms: u64,
}

#[derive(Clone, Debug)]
pub struct WaitToPendingRequest {
    pub cell_id: CellId,
}

/// Result of waiting on a code-mode cell.
///
/// The wrapped `RuntimeResponse` is the model-facing wait result. The enum
/// variant carries the extra lifecycle provenance that `RuntimeResponse` cannot:
/// a failed real cell and a missing-cell wait both use
/// `RuntimeResponse::Result { error_text: Some(..), .. }`, but only the former
/// should be treated as a code-cell lifecycle event.
#[derive(Debug, PartialEq)]
pub enum WaitOutcome {
    /// The requested code cell was live when the wait command was accepted.
    LiveCell(RuntimeResponse),
    /// The requested code cell was not live.
    MissingCell(RuntimeResponse),
}

/// Result of executing a code-mode cell until it either completes or reaches a
/// quiescent pending state.
#[derive(Debug, PartialEq)]
pub enum ExecuteToPendingOutcome {
    /// The cell is waiting for more runtime input after draining the runtime
    /// input queue that was ready at the pending boundary.
    Pending {
        cell_id: CellId,
        content_items: Vec<FunctionCallOutputContentItem>,
        /// Runtime tool-call ids emitted before this paused execution frontier
        /// sealed. Hosts can use these ids to drain their tool-call transport
        /// before surfacing the pending boundary to callers.
        pending_tool_call_ids: Vec<String>,
    },
    /// The cell reached a terminal runtime response before going pending.
    Completed(RuntimeResponse),
}

/// Result of resuming a live code-mode cell until it completes or becomes
/// quiescent again.
#[derive(Debug, PartialEq)]
pub enum WaitToPendingOutcome {
    /// The requested code cell was live when the wait command was accepted.
    LiveCell(ExecuteToPendingOutcome),
    /// The requested code cell was not live.
    MissingCell(RuntimeResponse),
}

impl From<WaitOutcome> for RuntimeResponse {
    fn from(outcome: WaitOutcome) -> Self {
        match outcome {
            WaitOutcome::LiveCell(response) | WaitOutcome::MissingCell(response) => response,
        }
    }
}

#[derive(Debug, PartialEq, Serialize)]
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

/// Nested tool request emitted by one code-mode cell.
///
/// Code mode owns the per-cell runtime id. Hosts should preserve it for
/// provenance/debugging, but should still assign their own runtime tool call id
/// if their tool-call graph requires globally unique ids.
#[derive(Debug)]
pub struct CodeModeNestedToolCall {
    pub cell_id: CellId,
    pub runtime_tool_call_id: String,
    pub tool_name: ToolName,
    pub tool_kind: CodeModeToolKind,
    pub input: Option<JsonValue>,
}
