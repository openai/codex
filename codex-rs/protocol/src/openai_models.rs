use std::collections::HashMap;

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use strum_macros::Display;
use strum_macros::EnumIter;
use ts_rs::TS;

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

/// Visibility of a model in the picker or APIs.
#[derive(
    Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, TS, JsonSchema, EnumIter, Display,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum ModelVisibility {
    List,
    Hide,
    None,
}

/// Reasoning support level reported by the backend.
#[derive(
    Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, TS, JsonSchema, EnumIter, Display,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum ReasoningLevel {
    None,
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
}

/// Shell execution capability for a model.
#[derive(
    Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, TS, JsonSchema, EnumIter, Display,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum ShellType {
    Default,
    Local,
    UnifiedExec,
    Disabled,
    ShellCommand,
}

/// Semantic version triple encoded as an array in JSON (e.g. [0, 62, 0]).
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, TS, JsonSchema)]
pub struct ClientVersion(pub i32, pub i32, pub i32);

/// Model metadata returned by the Codex backend `/models` endpoint.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, TS, JsonSchema)]
pub struct ModelInfo {
    pub slug: String,
    pub display_name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub default_reasoning_level: ReasoningLevel,
    pub supported_reasoning_levels: Vec<ReasoningLevel>,
    pub shell_type: ShellType,
    #[serde(default = "default_visibility")]
    pub visibility: ModelVisibility,
    pub minimal_client_version: ClientVersion,
    #[serde(default)]
    pub supported_in_api: bool,
    #[serde(default)]
    pub priority: i32,
}

/// Response wrapper for `/models`.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, TS, JsonSchema, Default)]
pub struct ModelsResponse {
    pub models: Vec<ModelInfo>,
}

fn default_visibility() -> ModelVisibility {
    ModelVisibility::None
}
