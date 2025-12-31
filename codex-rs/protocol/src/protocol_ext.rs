//! Extension event types for protocol.
//!
//! This module contains event types that extend the core protocol without
//! modifying the upstream EventMsg enum directly. This minimizes merge
//! conflicts when syncing with upstream.

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

/// Extension event message wrapper.
///
/// All custom events are wrapped in this enum and added to EventMsg as a single
/// `Ext(ExtEventMsg)` variant, minimizing changes to the upstream protocol.rs.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
#[serde(tag = "ext_type", rename_all = "snake_case")]
pub enum ExtEventMsg {
    /// Activity event from a subagent (Task tool execution).
    SubagentActivity(SubagentActivityEvent),

    /// Full compact completed successfully.
    CompactCompleted(CompactCompletedEvent),

    /// Micro-compact completed successfully.
    MicroCompactCompleted(MicroCompactCompletedEvent),

    /// Compact operation failed.
    CompactFailed(CompactFailedEvent),

    /// Context usage exceeded auto-compact threshold.
    CompactThresholdExceeded(CompactThresholdExceededEvent),

    /// Plan Mode has been entered.
    PlanModeEntered(PlanModeEnteredEvent),

    /// Plan Mode entry requested (waiting for user approval).
    PlanModeEntryRequest(PlanModeEntryRequestEvent),

    /// Plan Mode exit requested (waiting for user approval).
    PlanModeExitRequest(PlanModeExitRequestEvent),

    /// Plan Mode has been exited.
    PlanModeExited(PlanModeExitedEvent),

    /// LLM requests to ask the user questions.
    UserQuestionRequest(UserQuestionRequestEvent),
}

// ============================================================================
// Subagent Events
// ============================================================================

/// Event type for subagent activity.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum SubagentEventType {
    /// Subagent execution started.
    Started,
    /// Subagent execution completed successfully.
    Completed,
    /// Subagent execution encountered an error.
    Error,
    /// Subagent turn started.
    TurnStart,
    /// Subagent turn completed.
    TurnComplete,
    /// Tool call started within subagent.
    ToolCallStart,
    /// Tool call ended within subagent.
    ToolCallEnd,
    /// Grace period started (timeout/max_turns recovery).
    GracePeriodStart,
    /// Grace period ended.
    GracePeriodEnd,
}

/// Activity event from a subagent execution.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct SubagentActivityEvent {
    /// Unique identifier for this agent instance.
    pub agent_id: String,
    /// Type of agent (e.g., "Explore", "Plan").
    pub agent_type: String,
    /// Type of activity event.
    pub event_type: SubagentEventType,
    /// Optional additional data (turn number, duration, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub data: Option<serde_json::Value>,
}

// ============================================================================
// Compact V2 Events
// ============================================================================

/// Event emitted when full compact completes successfully.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct CompactCompletedEvent {
    /// Tokens before compaction.
    #[ts(type = "number")]
    pub pre_compact_tokens: i64,
    /// Tokens after compaction.
    #[ts(type = "number")]
    pub post_compact_tokens: i64,
    /// Tokens used for summarization input.
    #[ts(type = "number")]
    #[serde(default)]
    pub compaction_input_tokens: i64,
    /// Tokens generated in summary output.
    #[ts(type = "number")]
    #[serde(default)]
    pub compaction_output_tokens: i64,
    /// Number of files restored after compaction.
    pub files_restored: i32,
    /// Duration of compact operation in milliseconds.
    #[ts(type = "number")]
    #[serde(default)]
    pub duration_ms: i64,
    /// Whether this was an auto-compact or manual.
    pub is_auto: bool,
}

/// Event emitted when micro-compact completes successfully.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct MicroCompactCompletedEvent {
    /// Number of tool results that were compacted.
    pub tools_compacted: i32,
    /// Estimated tokens saved by compaction.
    #[ts(type = "number")]
    pub tokens_saved: i64,
}

/// Event emitted when compact operation fails.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct CompactFailedEvent {
    /// Error message describing the failure.
    pub message: String,
    /// Whether this was an auto-compact or manual.
    pub is_auto: bool,
}

/// Event emitted when context usage exceeds auto-compact threshold.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct CompactThresholdExceededEvent {
    /// Current token usage.
    #[ts(type = "number")]
    pub current_tokens: i64,
    /// Auto-compact threshold that was exceeded.
    #[ts(type = "number")]
    pub threshold_tokens: i64,
    /// Percentage of context window used.
    pub usage_percent: f64,
}

// ============================================================================
// Plan Mode Events
// ============================================================================

/// Permission mode for post-plan execution.
///
/// Aligned with Claude Code's plan exit options (chunks.88.mjs).
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum PlanExitPermissionMode {
    /// Auto-approve all tools (no permission prompts).
    BypassPermissions,
    /// Auto-approve file edits only (write_file, smart_edit).
    AcceptEdits,
    /// Manual approval for everything (default behavior).
    #[default]
    Default,
}

/// Event emitted when Plan Mode is entered.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct PlanModeEnteredEvent {
    /// Path to the plan file.
    pub plan_file_path: String,
}

/// Event emitted when Plan Mode entry is requested (waiting for user approval).
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct PlanModeEntryRequestEvent {
    // No fields needed - just a request for user approval
}

/// Event emitted when Plan Mode exit is requested (waiting for user approval).
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct PlanModeExitRequestEvent {
    /// Content of the plan file.
    pub plan_content: String,
    /// Path to the plan file.
    pub plan_file_path: String,
}

/// Event emitted when Plan Mode is exited.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct PlanModeExitedEvent {
    /// Whether the user approved the plan.
    pub approved: bool,
}

// ============================================================================
// User Question Events
// ============================================================================

/// Event emitted when LLM requests to ask the user questions.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct UserQuestionRequestEvent {
    /// The tool call ID for this question request.
    pub tool_call_id: String,
    /// Questions to ask the user (1-4 questions).
    pub questions: Vec<UserQuestion>,
}

/// A single question to ask the user.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct UserQuestion {
    /// The complete question text.
    pub question: String,
    /// Short header/label for the question (max 12 chars).
    pub header: String,
    /// Available options for this question (2-4 options).
    pub options: Vec<QuestionOption>,
    /// Whether multiple answers can be selected.
    pub multi_select: bool,
}

/// An option for a user question.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct QuestionOption {
    /// Display text for this option (1-5 words).
    pub label: String,
    /// Explanation of what this option means.
    pub description: String,
}
