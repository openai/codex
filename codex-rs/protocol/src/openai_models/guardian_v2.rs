use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

use super::ReasoningEffort;

/// Optional model-owned defaults for Guardian v2 classification experiments.
#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq, Eq, TS, JsonSchema)]
pub struct GuardianV2ModelConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classifier_instructions: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_threshold_basis_points: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<ReasoningEffort>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcript: Option<GuardianV2TranscriptModelConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_action_tokens: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_classifier_instruction_tokens: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_parent_compaction_tokens: Option<usize>,
}

/// Optional model-owned defaults for selecting and bounding Guardian v2 history.
#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq, Eq, TS, JsonSchema)]
pub struct GuardianV2TranscriptModelConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sources: Option<Vec<String>>,
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
