use std::error::Error;
use std::fmt;
use std::time::Duration;

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
pub struct ExecuteRequest {
    pub tool_call_id: String,
    pub enabled_tools: Vec<ToolDefinition>,
    pub source: String,
    pub yield_time_ms: Option<u64>,
    pub max_output_tokens: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WaitRequest {
    pub cell_id: CellId,
    pub yield_time_ms: u64,
    /// Limit for this observation in Server mode; ignored in Client mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TerminateRequest {
    pub cell_id: CellId,
    /// Limit for the final observation in Server mode; ignored in Client mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<usize>,
}

impl From<CellId> for TerminateRequest {
    fn from(cell_id: CellId) -> Self {
        Self {
            cell_id,
            max_output_tokens: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WaitToPendingRequest {
    pub cell_id: CellId,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub enum WaitOutcome {
    LiveCell(RuntimeResponse),
    MissingCell(RuntimeResponse),
}

impl WaitOutcome {
    /// How long a completed result waited for retrieval. Zero if it was delivered immediately,
    /// or `None` if this wait did not retrieve a completed result.
    pub fn result_buffered_for(&self) -> Option<Duration> {
        match self {
            Self::LiveCell(RuntimeResponse::Result {
                result_buffered_for,
                ..
            }) => Some(*result_buffered_for),
            Self::LiveCell(_) | Self::MissingCell(_) => None,
        }
    }

    /// Returns timing for this wait or termination request, when supplied by its host.
    pub fn code_mode_host_duration(&self) -> Option<Duration> {
        match self {
            Self::LiveCell(response) | Self::MissingCell(response) => {
                response.code_mode_host_duration()
            }
        }
    }

    /// Records the enclosing host request's duration before wire conversion.
    pub fn with_code_mode_host_duration(self, code_mode_host_duration: Duration) -> Self {
        match self {
            Self::LiveCell(response) => {
                Self::LiveCell(response.with_code_mode_host_duration(code_mode_host_duration))
            }
            Self::MissingCell(response) => {
                Self::MissingCell(response.with_code_mode_host_duration(code_mode_host_duration))
            }
        }
    }
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub enum ExecuteToPendingOutcome {
    Pending {
        cell_id: CellId,
        content_items: Vec<FunctionCallOutputContentItem>,
        pending_tool_call_ids: Vec<String>,
    },
    Completed(RuntimeResponse),
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub enum WaitToPendingOutcome {
    LiveCell(ExecuteToPendingOutcome),
    MissingCell(RuntimeResponse),
}

impl From<WaitOutcome> for RuntimeResponse {
    fn from(outcome: WaitOutcome) -> Self {
        match outcome {
            WaitOutcome::LiveCell(response) | WaitOutcome::MissingCell(response) => response,
        }
    }
}

/// Runtime output with optional timing for the host request that observed it.
///
/// The JavaScript session returns untimed output. The host handler records its
/// complete request duration before conversion; wire conversions preserve that
/// field and reject untimed output. Decoded host responses always contain timing,
/// including measured zero. Raw traces also retain timing when present.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum RuntimeResponse {
    Yielded {
        cell_id: CellId,
        content_items: Vec<FunctionCallOutputContentItem>,
        #[serde(skip_serializing_if = "Option::is_none")]
        code_mode_host_duration: Option<Duration>,
    },
    Terminated {
        cell_id: CellId,
        content_items: Vec<FunctionCallOutputContentItem>,
        #[serde(skip_serializing_if = "Option::is_none")]
        code_mode_host_duration: Option<Duration>,
    },
    Result {
        cell_id: CellId,
        content_items: Vec<FunctionCallOutputContentItem>,
        error_text: Option<String>,
        /// Time between completion and retrieval when the result was buffered.
        #[serde(default, skip_serializing_if = "Duration::is_zero")]
        result_buffered_for: Duration,
        #[serde(skip_serializing_if = "Option::is_none")]
        code_mode_host_duration: Option<Duration>,
    },
}

impl RuntimeResponse {
    /// Returns timing for this observation, excluding background work between requests.
    pub fn code_mode_host_duration(&self) -> Option<Duration> {
        match self {
            Self::Yielded {
                code_mode_host_duration,
                ..
            }
            | Self::Terminated {
                code_mode_host_duration,
                ..
            }
            | Self::Result {
                code_mode_host_duration,
                ..
            } => *code_mode_host_duration,
        }
    }

    /// Records the enclosing host request's duration before wire conversion.
    pub fn with_code_mode_host_duration(mut self, code_mode_host_duration: Duration) -> Self {
        match &mut self {
            Self::Yielded {
                code_mode_host_duration: value,
                ..
            }
            | Self::Terminated {
                code_mode_host_duration: value,
                ..
            }
            | Self::Result {
                code_mode_host_duration: value,
                ..
            } => *value = Some(code_mode_host_duration),
        }
        self
    }
}

/// An untimed runtime response cannot be encoded for delivery to a host client.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MissingCodeModeHostDuration;

impl fmt::Display for MissingCodeModeHostDuration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("code-mode response is missing host duration")
    }
}

impl Error for MissingCodeModeHostDuration {}

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
