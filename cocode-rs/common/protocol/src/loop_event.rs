//! Event types emitted by the core loop.
//!
//! These events allow consumers to observe the progress of the agent's
//! execution without being coupled to implementation details.

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ApprovalRequest;

/// Events emitted during loop execution.
///
/// These events provide a complete view of what the agent is doing,
/// enabling UI updates, logging, and debugging.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LoopEvent {
    // ========== Stream Lifecycle ==========
    /// A stream request has started.
    StreamRequestStart,
    /// A stream request has completed.
    StreamRequestEnd {
        /// Token usage for this request.
        usage: TokenUsage,
    },

    // ========== Turn Lifecycle ==========
    /// A new turn has started.
    TurnStarted {
        /// Unique identifier for this turn.
        turn_id: String,
        /// Turn number (1-indexed).
        turn_number: i32,
    },
    /// A turn has completed.
    TurnCompleted {
        /// Unique identifier for this turn.
        turn_id: String,
        /// Token usage for this turn.
        usage: TokenUsage,
    },

    // ========== Content Streaming ==========
    /// Text content delta from the model.
    TextDelta {
        /// Turn identifier.
        turn_id: String,
        /// The text delta.
        delta: String,
    },
    /// Thinking content delta (for models that support thinking).
    ThinkingDelta {
        /// Turn identifier.
        turn_id: String,
        /// The thinking delta.
        delta: String,
    },
    /// Tool call delta (partial tool call JSON).
    ToolCallDelta {
        /// Call identifier.
        call_id: String,
        /// The tool call delta.
        delta: String,
    },
    /// Raw SSE event passthrough.
    StreamEvent {
        /// The raw event data.
        event: RawStreamEvent,
    },

    // ========== Tool Execution ==========
    /// A tool use has been queued for execution.
    ToolUseQueued {
        /// Call identifier.
        call_id: String,
        /// Tool name.
        name: String,
        /// Tool input (JSON).
        input: Value,
    },
    /// A tool has started executing.
    ToolUseStarted {
        /// Call identifier.
        call_id: String,
        /// Tool name.
        name: String,
    },
    /// Progress update from a tool.
    ToolProgress {
        /// Call identifier.
        call_id: String,
        /// Progress information.
        progress: ToolProgressInfo,
    },
    /// A tool has completed execution.
    ToolUseCompleted {
        /// Call identifier.
        call_id: String,
        /// Tool output.
        output: ToolResultContent,
        /// Whether the tool returned an error.
        is_error: bool,
    },
    /// Tool execution was aborted.
    ToolExecutionAborted {
        /// Reason for abortion.
        reason: AbortReason,
    },

    // ========== Permission ==========
    /// User approval is required to proceed.
    ApprovalRequired {
        /// The approval request.
        request: ApprovalRequest,
    },
    /// User has responded to an approval request.
    ApprovalResponse {
        /// Request identifier.
        request_id: String,
        /// Whether the user approved.
        approved: bool,
    },

    // ========== Agent Events ==========
    /// A sub-agent has been spawned.
    SubagentSpawned {
        /// Agent identifier.
        agent_id: String,
        /// Type of agent.
        agent_type: String,
        /// Description of what the agent will do.
        description: String,
    },
    /// Progress update from a sub-agent.
    SubagentProgress {
        /// Agent identifier.
        agent_id: String,
        /// Progress information.
        progress: AgentProgress,
    },
    /// A sub-agent has completed.
    SubagentCompleted {
        /// Agent identifier.
        agent_id: String,
        /// Result from the agent.
        result: String,
    },
    /// A sub-agent has been moved to background.
    SubagentBackgrounded {
        /// Agent identifier.
        agent_id: String,
        /// Path to output file for monitoring.
        output_file: PathBuf,
    },

    // ========== Background Tasks ==========
    /// A background task has started.
    BackgroundTaskStarted {
        /// Task identifier.
        task_id: String,
        /// Type of task.
        task_type: TaskType,
    },
    /// Progress update from a background task.
    BackgroundTaskProgress {
        /// Task identifier.
        task_id: String,
        /// Progress information.
        progress: TaskProgress,
    },
    /// A background task has completed.
    BackgroundTaskCompleted {
        /// Task identifier.
        task_id: String,
        /// Result from the task.
        result: String,
    },

    // ========== Compaction ==========
    /// Compaction has started.
    CompactionStarted,
    /// Compaction has completed.
    CompactionCompleted {
        /// Number of messages removed.
        removed_messages: i32,
        /// Tokens in the summary.
        summary_tokens: i32,
    },
    /// Micro-compaction was applied to tool results.
    MicroCompactionApplied {
        /// Number of results compacted.
        removed_results: i32,
    },
    /// Session memory compaction was applied.
    SessionMemoryCompactApplied {
        /// Tokens saved.
        saved_tokens: i32,
        /// Tokens in the summary.
        summary_tokens: i32,
    },

    // ========== Model Fallback ==========
    /// Model fallback has started.
    ModelFallbackStarted {
        /// Original model.
        from: String,
        /// Fallback model.
        to: String,
        /// Reason for fallback.
        reason: String,
    },
    /// Model fallback has completed.
    ModelFallbackCompleted,
    /// A message has been tombstoned (marked for removal).
    Tombstone {
        /// The tombstoned message.
        message: TombstonedMessage,
    },

    // ========== Retry Events ==========
    /// A retry is being attempted.
    Retry {
        /// Current attempt number.
        attempt: i32,
        /// Maximum attempts allowed.
        max_attempts: i32,
        /// Delay before retry (milliseconds).
        delay_ms: i32,
    },

    // ========== API Errors ==========
    /// An API error occurred.
    ApiError {
        /// The error.
        error: ApiErrorInfo,
        /// Retry information if retriable.
        retry_info: Option<RetryInfo>,
    },

    // ========== MCP Events ==========
    /// An MCP tool call has begun.
    McpToolCallBegin {
        /// Server name.
        server: String,
        /// Tool name.
        tool: String,
        /// Call identifier.
        call_id: String,
    },
    /// An MCP tool call has ended.
    McpToolCallEnd {
        /// Server name.
        server: String,
        /// Tool name.
        tool: String,
        /// Call identifier.
        call_id: String,
        /// Whether it was an error.
        is_error: bool,
    },
    /// MCP server startup status update.
    McpStartupUpdate {
        /// Server name.
        server: String,
        /// Startup status.
        status: McpStartupStatus,
    },
    /// MCP startup has completed.
    McpStartupComplete {
        /// Successfully started servers.
        servers: Vec<McpServerInfo>,
        /// Failed servers (name, error message).
        failed: Vec<(String, String)>,
    },

    // ========== Plan Mode ==========
    /// Plan mode has been entered.
    PlanModeEntered {
        /// Path to the plan file.
        plan_file: PathBuf,
    },
    /// Plan mode has been exited.
    PlanModeExited {
        /// Whether the plan was approved.
        approved: bool,
    },

    // ========== Hooks ==========
    /// A hook has been executed.
    HookExecuted {
        /// Type of hook event.
        hook_type: HookEventType,
        /// Name of the hook.
        hook_name: String,
    },

    // ========== Stream Stall Detection ==========
    /// A stream stall has been detected.
    StreamStallDetected {
        /// Turn identifier.
        turn_id: String,
        /// Timeout duration that was exceeded.
        #[serde(with = "humantime_serde")]
        timeout: Duration,
    },

    // ========== Prompt Caching ==========
    /// Prompt cache hit.
    PromptCacheHit {
        /// Number of tokens served from cache.
        cached_tokens: i32,
    },
    /// Prompt cache miss.
    PromptCacheMiss,

    // ========== Errors & Control ==========
    /// An error occurred in the loop.
    Error {
        /// The error.
        error: LoopError,
    },
    /// The loop was interrupted by the user.
    Interrupted,
    /// Maximum turns reached.
    MaxTurnsReached,
}

/// Raw SSE event from the stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawStreamEvent {
    /// Event type.
    pub event_type: String,
    /// Event data (JSON).
    pub data: Value,
}

/// Token usage information.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Input tokens used.
    #[serde(default)]
    pub input_tokens: i64,
    /// Output tokens used.
    #[serde(default)]
    pub output_tokens: i64,
    /// Cache read tokens (if applicable).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read_tokens: Option<i64>,
    /// Cache creation tokens (if applicable).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_creation_tokens: Option<i64>,
    /// Reasoning tokens (if applicable).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<i64>,
}

impl TokenUsage {
    /// Create a new TokenUsage.
    pub fn new(input_tokens: i64, output_tokens: i64) -> Self {
        Self {
            input_tokens,
            output_tokens,
            cache_read_tokens: None,
            cache_creation_tokens: None,
            reasoning_tokens: None,
        }
    }

    /// Get total tokens used.
    pub fn total(&self) -> i64 {
        self.input_tokens + self.output_tokens
    }
}

/// Progress information from a tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolProgressInfo {
    /// Progress message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Progress percentage (0-100).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percentage: Option<i32>,
    /// Bytes processed (for file operations).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes_processed: Option<i64>,
    /// Total bytes (for file operations).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_bytes: Option<i64>,
}

/// Content of a tool result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolResultContent {
    /// Text content.
    Text(String),
    /// Structured content (JSON).
    Structured(Value),
}

impl Default for ToolResultContent {
    fn default() -> Self {
        ToolResultContent::Text(String::new())
    }
}

/// Reason for aborting tool execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AbortReason {
    /// Fallback to non-streaming due to streaming error.
    StreamingFallback,
    /// A sibling tool call encountered an error.
    SiblingError,
    /// User interrupted the operation.
    UserInterrupted,
}

impl AbortReason {
    /// Get the reason as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            AbortReason::StreamingFallback => "streaming_fallback",
            AbortReason::SiblingError => "sibling_error",
            AbortReason::UserInterrupted => "user_interrupted",
        }
    }
}

impl std::fmt::Display for AbortReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Progress information from a sub-agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProgress {
    /// Progress message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Current step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_step: Option<i32>,
    /// Total steps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_steps: Option<i32>,
}

/// Type of background task.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    /// Shell command execution.
    Shell,
    /// Agent execution.
    Agent,
    /// File operation.
    FileOp,
    /// Other task type.
    Other(String),
}

/// Progress information from a background task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskProgress {
    /// Progress message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Output produced so far.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    /// Exit code (if completed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
}

/// A tombstoned message (marked for removal).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TombstonedMessage {
    /// Message role.
    pub role: String,
    /// Message content (summary or placeholder).
    pub content: String,
}

/// Retry information.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryInfo {
    /// Current attempt number.
    pub attempt: i32,
    /// Maximum attempts allowed.
    pub max_attempts: i32,
    /// Delay before retry (milliseconds).
    pub delay_ms: i32,
    /// Whether the error is retriable.
    pub retriable: bool,
}

/// API error information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorInfo {
    /// Error code.
    pub code: String,
    /// Error message.
    pub message: String,
    /// HTTP status code (if applicable).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<i32>,
}

/// MCP server startup status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpStartupStatus {
    /// Starting the server.
    Starting,
    /// Connecting to the server.
    Connecting,
    /// Server is ready.
    Ready,
    /// Server failed to start.
    Failed,
}

/// Information about an MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerInfo {
    /// Server name.
    pub name: String,
    /// Number of tools provided.
    pub tool_count: i32,
    /// Tool names.
    #[serde(default)]
    pub tools: Vec<String>,
}

/// Type of hook event.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookEventType {
    /// Before a tool call.
    PreToolCall,
    /// After a successful tool call.
    PostToolCall,
    /// After a failed tool call.
    PostToolCallFailure,
    /// On session start.
    SessionStart,
    /// On session end.
    SessionEnd,
    /// On user prompt submit.
    PromptSubmit,
}

impl HookEventType {
    /// Get the hook type as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            HookEventType::PreToolCall => "pre_tool_call",
            HookEventType::PostToolCall => "post_tool_call",
            HookEventType::PostToolCallFailure => "post_tool_call_failure",
            HookEventType::SessionStart => "session_start",
            HookEventType::SessionEnd => "session_end",
            HookEventType::PromptSubmit => "prompt_submit",
        }
    }
}

impl std::fmt::Display for HookEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// An error that occurred in the loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopError {
    /// Error code.
    pub code: String,
    /// Error message.
    pub message: String,
    /// Whether this error is recoverable.
    #[serde(default)]
    pub recoverable: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_usage() {
        let usage = TokenUsage::new(100, 50);
        assert_eq!(usage.input_tokens, 100i64);
        assert_eq!(usage.output_tokens, 50i64);
        assert_eq!(usage.total(), 150i64);
    }

    #[test]
    fn test_abort_reason() {
        assert_eq!(
            AbortReason::StreamingFallback.as_str(),
            "streaming_fallback"
        );
        assert_eq!(AbortReason::SiblingError.as_str(), "sibling_error");
        assert_eq!(AbortReason::UserInterrupted.as_str(), "user_interrupted");
    }

    #[test]
    fn test_hook_event_type() {
        assert_eq!(HookEventType::PreToolCall.as_str(), "pre_tool_call");
        assert_eq!(HookEventType::PostToolCall.as_str(), "post_tool_call");
        assert_eq!(HookEventType::SessionStart.as_str(), "session_start");
    }

    #[test]
    fn test_loop_event_serde() {
        let event = LoopEvent::TurnStarted {
            turn_id: "turn-1".to_string(),
            turn_number: 1,
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("turn_started"));

        let parsed: LoopEvent = serde_json::from_str(&json).unwrap();
        if let LoopEvent::TurnStarted {
            turn_id,
            turn_number,
        } = parsed
        {
            assert_eq!(turn_id, "turn-1");
            assert_eq!(turn_number, 1);
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_retry_info() {
        let info = RetryInfo {
            attempt: 1,
            max_attempts: 3,
            delay_ms: 1000,
            retriable: true,
        };

        let json = serde_json::to_string(&info).unwrap();
        let parsed: RetryInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, info);
    }

    #[test]
    fn test_tool_result_content() {
        let text = ToolResultContent::Text("Hello".to_string());
        let json = serde_json::to_string(&text).unwrap();
        assert_eq!(json, "\"Hello\"");

        let structured = ToolResultContent::Structured(serde_json::json!({"key": "value"}));
        let json = serde_json::to_string(&structured).unwrap();
        assert!(json.contains("key"));
    }

    #[test]
    fn test_mcp_startup_status() {
        let status = McpStartupStatus::Ready;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"ready\"");

        let parsed: McpStartupStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, McpStartupStatus::Ready);
    }
}
