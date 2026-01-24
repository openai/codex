//! Hook types for SDK callback system.
//!
//! Hooks allow SDK clients to intercept and modify agent behavior at various points.

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use ts_rs::TS;

/// Hook event types.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS, JsonSchema)]
#[serde(rename_all = "PascalCase")]
pub enum HookEvent {
    /// Before a tool is used.
    PreToolUse,
    /// After a tool is used.
    PostToolUse,
    /// When user submits a prompt.
    UserPromptSubmit,
    /// When the agent is about to stop.
    Stop,
    /// When a subagent is about to stop.
    SubagentStop,
    /// Before compacting conversation history.
    PreCompact,
}

/// Input data for hook callbacks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct HookInput {
    /// The hook event type.
    pub hook_event_name: HookEvent,
    /// Tool-specific input (for PreToolUse/PostToolUse).
    #[serde(flatten)]
    pub data: HookInputData,
}

/// Hook-specific input data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
#[serde(untagged)]
pub enum HookInputData {
    /// PreToolUse input.
    PreToolUse(PreToolUseInput),
    /// PostToolUse input.
    PostToolUse(PostToolUseInput),
    /// UserPromptSubmit input.
    UserPromptSubmit(UserPromptSubmitInput),
    /// Stop input.
    Stop(StopInput),
    /// SubagentStop input.
    SubagentStop(SubagentStopInput),
    /// PreCompact input.
    PreCompact(PreCompactInput),
}

/// PreToolUse hook input.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct PreToolUseInput {
    /// Name of the tool being invoked.
    pub tool_name: String,
    /// Tool input parameters.
    pub tool_input: JsonValue,
    /// Agent ID making the call.
    pub agent_id: Option<String>,
}

/// PostToolUse hook input.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct PostToolUseInput {
    /// Name of the tool that was invoked.
    pub tool_name: String,
    /// Tool input parameters.
    pub tool_input: JsonValue,
    /// Tool output.
    pub tool_output: JsonValue,
    /// Agent ID that made the call.
    pub agent_id: Option<String>,
}

/// UserPromptSubmit hook input.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct UserPromptSubmitInput {
    /// The user's prompt text.
    pub prompt: String,
}

/// Stop hook input.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct StopInput {
    /// Reason for stopping.
    pub reason: Option<String>,
}

/// SubagentStop hook input.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct SubagentStopInput {
    /// Subagent ID.
    pub agent_id: String,
    /// Reason for stopping.
    pub reason: Option<String>,
}

/// PreCompact hook input.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct PreCompactInput {
    /// Current conversation token count.
    pub token_count: i64,
    /// Maximum allowed tokens.
    pub max_tokens: i64,
}

/// Output from hook callbacks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default, TS, JsonSchema)]
pub struct HookOutput {
    /// Whether to continue execution.
    #[serde(rename = "continue")]
    pub continue_: Option<bool>,
    /// Whether to suppress output.
    pub suppress_output: Option<bool>,
    /// Reason for stopping (if stopping).
    pub stop_reason: Option<String>,
    /// Permission decision (for PreToolUse).
    pub decision: Option<HookPermissionDecision>,
    /// System message to inject.
    pub system_message: Option<String>,
    /// Reason for the decision.
    pub reason: Option<String>,
    /// Hook-specific output.
    pub hook_specific_output: Option<HookSpecificOutput>,
}

/// Hook permission decision.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum HookPermissionDecision {
    /// Block the operation.
    Block,
}

/// Hook-specific output data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct HookSpecificOutput {
    /// Hook event name.
    pub hook_event_name: HookEvent,
    /// Permission decision (for PreToolUse).
    pub permission_decision: Option<String>,
    /// Reason for the permission decision.
    pub permission_decision_reason: Option<String>,
    /// Modified tool input (for PreToolUse).
    pub modified_tool_input: Option<JsonValue>,
    /// Modified tool output (for PostToolUse).
    pub modified_tool_output: Option<JsonValue>,
}
