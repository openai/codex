use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

use super::ReasoningEffort;

/// Async classifier experiment, resolved once for each parent task.
#[derive(Debug, Default, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum AsyncClassifierMode {
    #[default]
    Snapshot,
    Conversation,
}

/// Optional model-owned defaults for Guardian v2 classification experiments.
#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq, Eq, TS, JsonSchema)]
pub struct GuardianV2ModelConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub async_classifier_mode: Option<AsyncClassifierMode>,
    /// Reset retained async history when the next request exceeds this token estimate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub async_classifier_conversation_token_limit: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classifier_instructions: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_threshold_basis_points: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tool_call_lag: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<ReasoningEffort>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcript: Option<GuardianV2TranscriptModelConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_action_tokens: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_classifier_instruction_tokens: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reuse_parent_compaction: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_parent_compaction_tokens: Option<usize>,
}

/// Optional model-owned defaults for selecting and bounding Guardian v2 history.
#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq, Eq, TS, JsonSchema)]
pub struct GuardianV2TranscriptModelConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sources: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_images: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_message_entry_tokens: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tool_entry_tokens: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_message_transcript_tokens: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tool_transcript_tokens: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_recent_non_user_entries: Option<usize>,
}
