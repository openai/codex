use std::collections::HashMap;

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use strum_macros::Display;
use strum_macros::EnumIter;
use ts_rs::TS;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ModelName {
    Gpt51CodexMax,
    Gpt51Codex,
    Gpt51CodexMini,
    Gpt51,
    Gpt5Codex,
    Gpt5CodexMini,
    Gpt5,
}

impl ModelName {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Gpt51CodexMax => "gpt-5.1-codex-max",
            Self::Gpt51Codex => "gpt-5.1-codex",
            Self::Gpt51CodexMini => "gpt-5.1-codex-mini",
            Self::Gpt51 => "gpt-5.1",
            Self::Gpt5Codex => "gpt-5-codex",
            Self::Gpt5CodexMini => "gpt-5-codex-mini",
            Self::Gpt5 => "gpt-5",
        }
    }
}

/// See https://platform.openai.com/docs/guides/reasoning?api-mode=responses#get-started-with-reasoning
#[derive(
    Debug,
    Serialize,
    Deserialize,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Display,
    JsonSchema,
    TS,
    EnumIter,
    Hash,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum ReasoningEffort {
    None,
    Minimal,
    Low,
    #[default]
    Medium,
    High,
    XHigh,
}

/// A reasoning effort option that can be surfaced for a model.
#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema, PartialEq)]
pub struct ReasoningEffortPreset {
    /// Effort level that the model supports.
    pub effort: ReasoningEffort,
    /// Short human description shown next to the effort in UIs.
    pub description: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema, PartialEq)]
pub struct ModelUpgrade {
    pub id: String,
    pub reasoning_effort_mapping: Option<HashMap<ReasoningEffort, ReasoningEffort>>,
    pub migration_config_key: String,
}

/// Metadata describing a Codex-supported model.
#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema, PartialEq)]
pub struct ModelPreset {
    /// Stable identifier for the preset.
    pub id: String,
    /// Model slug (e.g., "gpt-5").
    pub model: String,
    /// Display name shown in UIs.
    pub display_name: String,
    /// Short human description shown in UIs.
    pub description: String,
    /// Reasoning effort applied when none is explicitly chosen.
    pub default_reasoning_effort: ReasoningEffort,
    /// Supported reasoning effort options.
    pub supported_reasoning_efforts: Vec<ReasoningEffortPreset>,
    /// Whether this is the default model for new users.
    pub is_default: bool,
    /// recommended upgrade model
    pub upgrade: Option<ModelUpgrade>,
    /// Whether this preset should appear in the picker UI.
    pub show_in_picker: bool,
}
