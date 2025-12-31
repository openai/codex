//! Subagent execution result types.

use serde::Deserialize;
use serde::Serialize;
use std::time::Duration;

/// Status of a completed subagent execution.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SubagentStatus {
    /// Successfully completed.
    Goal,
    /// Execution timed out.
    Timeout,
    /// Maximum turns exceeded.
    MaxTurns,
    /// Execution was cancelled.
    Aborted,
    /// Execution error occurred.
    Error,
}

/// Result of a subagent execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentResult {
    /// Final status.
    pub status: SubagentStatus,
    /// Result content.
    pub result: String,
    /// Number of conversation turns used.
    pub turns_used: i32,
    /// Total execution duration.
    pub duration: Duration,
    /// Agent instance ID.
    pub agent_id: String,
    /// Total number of tool calls made.
    pub total_tool_use_count: i32,
    /// Total execution time in milliseconds.
    pub total_duration_ms: i64,
    /// Total tokens used (input + output).
    pub total_tokens: i32,
    /// Detailed token usage.
    pub usage: Option<TokenUsage>,
}

/// Token usage details.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: i32,
    pub output_tokens: i32,
    #[serde(default)]
    pub cache_creation_input_tokens: Option<i32>,
    #[serde(default)]
    pub cache_read_input_tokens: Option<i32>,
}
