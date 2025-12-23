use std::collections::HashMap;

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use strum::IntoEnumIterator;
use strum_macros::Display;
use strum_macros::EnumIter;
use ts_rs::TS;

use crate::config_types::Verbosity;

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
#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema, PartialEq, Eq)]
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
    pub model_link: Option<String>,
    pub upgrade_copy: Option<String>,
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
    /// whether this model is supported in the api
    pub supported_in_api: bool,
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

/// Shell execution capability for a model.
#[derive(
    Debug,
    Serialize,
    Deserialize,
    Clone,
    Copy,
    PartialEq,
    Eq,
    TS,
    JsonSchema,
    EnumIter,
    Display,
    Hash,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum ConfigShellToolType {
    Default,
    Local,
    UnifiedExec,
    Disabled,
    ShellCommand,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApplyPatchToolType {
    Freeform,
    Function,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq, Default, Hash, TS, JsonSchema, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningSummaryFormat {
    #[default]
    None,
    Experimental,
}

/// Server-provided truncation policy metadata for a model.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TruncationMode {
    Bytes,
    Tokens,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, TS, JsonSchema)]
pub struct TruncationPolicyConfig {
    pub mode: TruncationMode,
    pub limit: i64,
}

impl TruncationPolicyConfig {
    pub const fn bytes(limit: i64) -> Self {
        Self {
            mode: TruncationMode::Bytes,
            limit,
        }
    }

    pub const fn tokens(limit: i64) -> Self {
        Self {
            mode: TruncationMode::Tokens,
            limit,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Copy, PartialEq, Eq, Hash, JsonSchema, TS)]
pub enum TruncationPolicy {
    Bytes(usize),
    Tokens(usize),
}

impl From<TruncationPolicyConfig> for TruncationPolicy {
    fn from(config: TruncationPolicyConfig) -> Self {
        match config.mode {
            TruncationMode::Bytes => Self::Bytes(config.limit as usize),
            TruncationMode::Tokens => Self::Tokens(config.limit as usize),
        }
    }
}

impl std::ops::Mul<f64> for TruncationPolicy {
    type Output = Self;

    /// Scale the underlying budget by `multiplier`, rounding up to avoid under-budgeting.
    fn mul(self, multiplier: f64) -> Self::Output {
        match self {
            TruncationPolicy::Bytes(bytes) => {
                TruncationPolicy::Bytes((bytes as f64 * multiplier).ceil() as usize)
            }
            TruncationPolicy::Tokens(tokens) => {
                TruncationPolicy::Tokens((tokens as f64 * multiplier).ceil() as usize)
            }
        }
    }
}

/// A model family is a group of models that share certain characteristics.
#[derive(Debug, Clone, Deserialize, Serialize, Hash, JsonSchema, TS)]
pub struct ModelFamily {
    /// The full model slug used to derive this model family, e.g.
    /// "gpt-4.1-2025-04-14".
    pub slug: String,

    /// The model family name, e.g. "gpt-4.1". This string is used when deriving
    /// default metadata for the family, such as context windows.
    pub family: String,

    /// True if the model needs additional instructions on how to use the
    /// "virtual" `apply_patch` CLI.
    pub needs_special_apply_patch_instructions: bool,

    /// Maximum supported context window, if known.
    pub context_window: Option<i64>,

    /// Token threshold for automatic compaction if config does not override it.
    pub auto_compact_token_limit: Option<i64>,

    // Whether the `reasoning` field can be set when making a request to this
    // model family. Note it has `effort` and `summary` subfields (though
    // `summary` is optional).
    pub supports_reasoning_summaries: bool,

    // The reasoning effort to use for this model family when none is explicitly chosen.
    pub default_reasoning_effort: Option<ReasoningEffort>,

    // Define if we need a special handling of reasoning summary
    pub reasoning_summary_format: ReasoningSummaryFormat,

    /// Whether this model supports parallel tool calls when using the
    /// Responses API.
    pub supports_parallel_tool_calls: bool,

    /// Present if the model performs better when `apply_patch` is provided as
    /// a tool call instead of just a bash command
    pub apply_patch_tool_type: Option<ApplyPatchToolType>,

    // Instructions to use for querying the model
    pub base_instructions: String,

    /// Names of beta tools that should be exposed to this model family.
    pub experimental_supported_tools: Vec<String>,

    /// Percentage of the context window considered usable for inputs, after
    /// reserving headroom for system prompts, tool overhead, and model output.
    /// This is applied when computing the effective context window seen by
    /// consumers.
    pub effective_context_window_percent: i64,

    /// If the model family supports setting the verbosity level when using Responses API.
    pub support_verbosity: bool,

    // The default verbosity level for this model family when using Responses API.
    pub default_verbosity: Option<Verbosity>,

    /// Preferred shell tool type for this model family when features do not override it.
    pub shell_type: ConfigShellToolType,

    pub truncation_policy: TruncationPolicy,
}

impl ModelFamily {
    /// Convert a `ModelFamily` into the protocol's `ModelInfo` shape for inclusion in events.
    ///
    /// This intentionally omits fields that are not needed for session bootstrapping
    /// (e.g. `priority`, `visibility`, and `base_instructions`).
    pub fn to_session_configured_model_info(&self) -> ModelInfo {
        let default_reasoning_level = self.default_reasoning_effort.unwrap_or_default();
        let truncation_policy = match self.truncation_policy {
            TruncationPolicy::Bytes(limit) => TruncationPolicyConfig::bytes(limit as i64),
            TruncationPolicy::Tokens(limit) => TruncationPolicyConfig::tokens(limit as i64),
        };

        ModelInfo {
            slug: self.slug.clone(),
            display_name: self.slug.clone(),
            description: None,
            default_reasoning_level,
            supported_reasoning_levels: vec![ReasoningEffortPreset {
                effort: default_reasoning_level,
                description: default_reasoning_level.to_string(),
            }],
            shell_type: self.shell_type,
            visibility: ModelVisibility::None,
            supported_in_api: true,
            priority: 0,
            upgrade: None,
            base_instructions: None,
            supports_reasoning_summaries: self.supports_reasoning_summaries,
            support_verbosity: self.support_verbosity,
            default_verbosity: self.default_verbosity,
            apply_patch_tool_type: self.apply_patch_tool_type.clone(),
            truncation_policy,
            supports_parallel_tool_calls: self.supports_parallel_tool_calls,
            context_window: self.context_window,
            reasoning_summary_format: self.reasoning_summary_format.clone(),
            experimental_supported_tools: self.experimental_supported_tools.clone(),
        }
    }

    pub fn auto_compact_token_limit(&self) -> Option<i64> {
        self.auto_compact_token_limit
            .or(self.context_window.map(|cw| (cw * 9) / 10))
    }

    pub fn get_model_slug(&self) -> &str {
        &self.slug
    }

    pub fn with_remote_overrides(mut self, remote_models: Vec<ModelInfo>) -> Self {
        for model in remote_models {
            if model.slug == self.slug {
                self.apply_remote_overrides(model);
            }
        }
        self
    }

    fn apply_remote_overrides(&mut self, model: ModelInfo) {
        let ModelInfo {
            slug: _,
            display_name: _,
            description: _,
            default_reasoning_level,
            supported_reasoning_levels: _,
            shell_type,
            visibility: _,
            supported_in_api: _,
            priority: _,
            upgrade: _,
            base_instructions,
            supports_reasoning_summaries,
            support_verbosity,
            default_verbosity,
            apply_patch_tool_type,
            truncation_policy,
            supports_parallel_tool_calls,
            context_window,
            reasoning_summary_format,
            experimental_supported_tools,
        } = model;

        self.default_reasoning_effort = Some(default_reasoning_level);
        self.shell_type = shell_type;
        if let Some(base) = base_instructions {
            self.base_instructions = base;
        }
        self.supports_reasoning_summaries = supports_reasoning_summaries;
        self.support_verbosity = support_verbosity;
        self.default_verbosity = default_verbosity;
        self.apply_patch_tool_type = apply_patch_tool_type;
        self.truncation_policy = truncation_policy.into();
        self.supports_parallel_tool_calls = supports_parallel_tool_calls;
        self.context_window = context_window;
        self.reasoning_summary_format = reasoning_summary_format;
        self.experimental_supported_tools = experimental_supported_tools;
    }
}

/// Semantic version triple encoded as an array in JSON (e.g. [0, 62, 0]).
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, TS, JsonSchema)]
pub struct ClientVersion(pub i32, pub i32, pub i32);

/// Model metadata returned by the Codex backend `/models` endpoint.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, TS, JsonSchema)]
pub struct ModelInfo {
    pub slug: String,
    pub display_name: String,
    pub description: Option<String>,
    pub default_reasoning_level: ReasoningEffort,
    pub supported_reasoning_levels: Vec<ReasoningEffortPreset>,
    pub shell_type: ConfigShellToolType,
    pub visibility: ModelVisibility,
    pub supported_in_api: bool,
    pub priority: i32,
    pub upgrade: Option<String>,
    pub base_instructions: Option<String>,
    pub supports_reasoning_summaries: bool,
    pub support_verbosity: bool,
    pub default_verbosity: Option<Verbosity>,
    pub apply_patch_tool_type: Option<ApplyPatchToolType>,
    pub truncation_policy: TruncationPolicyConfig,
    pub supports_parallel_tool_calls: bool,
    pub context_window: Option<i64>,
    pub reasoning_summary_format: ReasoningSummaryFormat,
    pub experimental_supported_tools: Vec<String>,
}

/// Response wrapper for `/models`.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, TS, JsonSchema, Default)]
pub struct ModelsResponse {
    pub models: Vec<ModelInfo>,
    #[serde(default)]
    pub etag: String,
}

// convert ModelInfo to ModelPreset
impl From<ModelInfo> for ModelPreset {
    fn from(info: ModelInfo) -> Self {
        ModelPreset {
            id: info.slug.clone(),
            model: info.slug.clone(),
            display_name: info.display_name,
            description: info.description.unwrap_or_default(),
            default_reasoning_effort: info.default_reasoning_level,
            supported_reasoning_efforts: info.supported_reasoning_levels.clone(),
            is_default: false, // default is the highest priority available model
            upgrade: info.upgrade.as_ref().map(|upgrade_slug| ModelUpgrade {
                id: upgrade_slug.clone(),
                reasoning_effort_mapping: reasoning_effort_mapping_from_presets(
                    &info.supported_reasoning_levels,
                ),
                migration_config_key: info.slug.clone(),
                // todo(aibrahim): add the model link here.
                model_link: None,
                upgrade_copy: None,
            }),
            show_in_picker: info.visibility == ModelVisibility::List,
            supported_in_api: info.supported_in_api,
        }
    }
}

fn reasoning_effort_mapping_from_presets(
    presets: &[ReasoningEffortPreset],
) -> Option<HashMap<ReasoningEffort, ReasoningEffort>> {
    if presets.is_empty() {
        return None;
    }

    // Map every canonical effort to the closest supported effort for the new model.
    let supported: Vec<ReasoningEffort> = presets.iter().map(|p| p.effort).collect();
    let mut map = HashMap::new();
    for effort in ReasoningEffort::iter() {
        let nearest = nearest_effort(effort, &supported);
        map.insert(effort, nearest);
    }
    Some(map)
}

fn effort_rank(effort: ReasoningEffort) -> i32 {
    match effort {
        ReasoningEffort::None => 0,
        ReasoningEffort::Minimal => 1,
        ReasoningEffort::Low => 2,
        ReasoningEffort::Medium => 3,
        ReasoningEffort::High => 4,
        ReasoningEffort::XHigh => 5,
    }
}

fn nearest_effort(target: ReasoningEffort, supported: &[ReasoningEffort]) -> ReasoningEffort {
    let target_rank = effort_rank(target);
    supported
        .iter()
        .copied()
        .min_by_key(|candidate| (effort_rank(*candidate) - target_rank).abs())
        .unwrap_or(target)
}
