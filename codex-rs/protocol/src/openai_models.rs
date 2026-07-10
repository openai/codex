//! Shared model metadata types exchanged between Codex services and clients.
//!
//! These types are serialized across core, TUI, app-server, and SDK boundaries, so field defaults
//! are used to preserve compatibility when older payloads omit newly introduced attributes.

use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use schemars::r#gen::SchemaGenerator;
use schemars::schema::InstanceType;
use schemars::schema::Metadata;
use schemars::schema::Schema;
use schemars::schema::SchemaObject;
use schemars::schema::StringValidation;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;
use serde::de::DeserializeOwned;
use serde::de::Error;
use strum_macros::Display;
use strum_macros::EnumIter;
use tracing::warn;
use ts_rs::TS;

use crate::config_types::Personality;
use crate::config_types::ReasoningSummary;
use crate::config_types::SERVICE_TIER_DEFAULT_REQUEST_VALUE;
use crate::config_types::ServiceTier;
use crate::config_types::Verbosity;
use crate::protocol::MultiAgentVersion;

const PERSONALITY_PLACEHOLDER: &str = "{{ personality }}";
pub const SPEED_TIER_FAST: &str = "fast";

/// See https://platform.openai.com/docs/guides/reasoning?api-mode=responses#get-started-with-reasoning
#[derive(Debug, Default, Clone, PartialEq, Eq, TS, Hash)]
#[ts(type = "string")]
pub enum ReasoningEffort {
    None,
    Minimal,
    Low,
    #[default]
    Medium,
    High,
    XHigh,
    Max,
    Ultra,
    /// A model-defined effort value that this client does not know yet.
    Custom(String),
}

impl ReasoningEffort {
    /// Returns the exact value used on the wire.
    pub fn as_str(&self) -> &str {
        match self {
            Self::None => "none",
            Self::Minimal => "minimal",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::XHigh => "xhigh",
            Self::Max => "max",
            Self::Ultra => "ultra",
            Self::Custom(effort) => effort,
        }
    }
}

impl fmt::Display for ReasoningEffort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl JsonSchema for ReasoningEffort {
    fn schema_name() -> String {
        "ReasoningEffort".to_string()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        Schema::Object(SchemaObject {
            instance_type: Some(InstanceType::String.into()),
            metadata: Some(Box::new(Metadata {
                description: Some(
                    "A non-empty reasoning effort value advertised by the model.".to_string(),
                ),
                ..Default::default()
            })),
            string: Some(Box::new(StringValidation {
                min_length: Some(1),
                ..Default::default()
            })),
            ..Default::default()
        })
    }
}

impl Serialize for ReasoningEffort {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ReasoningEffort {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let effort = String::deserialize(deserializer)?;
        effort.parse().map_err(D::Error::custom)
    }
}

impl FromStr for ReasoningEffort {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(Self::None),
            "minimal" => Ok(Self::Minimal),
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            "xhigh" => Ok(Self::XHigh),
            "max" => Ok(Self::Max),
            "ultra" => Ok(Self::Ultra),
            "" => Err("reasoning_effort must not be empty".to_string()),
            effort => Ok(Self::Custom(effort.to_string())),
        }
    }
}

/// Canonical user-input modality tags advertised by a model.
#[derive(
    Debug,
    Serialize,
    Deserialize,
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
pub enum InputModality {
    /// Plain text turns and tool payloads.
    Text,
    /// Image attachments included in user turns.
    Image,
}

/// Backward-compatible default when `input_modalities` is omitted on the wire.
///
/// Legacy payloads predate modality metadata, so we conservatively assume both text and images are
/// accepted unless a preset explicitly narrows support.
pub fn default_input_modalities() -> Vec<InputModality> {
    vec![InputModality::Text, InputModality::Image]
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
    pub migration_config_key: String,
    pub model_link: Option<String>,
    pub upgrade_copy: Option<String>,
    pub migration_markdown: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema, PartialEq, Eq)]
pub struct ModelAvailabilityNux {
    pub message: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema, PartialEq, Eq)]
pub struct ModelServiceTier {
    pub id: String,
    pub name: String,
    pub description: String,
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
    /// Whether this model supports personality-specific instructions.
    #[serde(default)]
    pub supports_personality: bool,
    /// Deprecated: use `service_tiers` instead.
    #[serde(default)]
    pub additional_speed_tiers: Vec<String>,
    /// Service tiers this model can run with.
    #[serde(default)]
    pub service_tiers: Vec<ModelServiceTier>,
    /// Catalog default service tier id for this model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_service_tier: Option<String>,
    /// Whether this is the default model for new users.
    pub is_default: bool,
    /// recommended upgrade model
    pub upgrade: Option<ModelUpgrade>,
    /// Whether this preset should appear in the picker UI.
    pub show_in_picker: bool,
    /// Availability NUX shown when this preset becomes accessible to the user.
    pub availability_nux: Option<ModelAvailabilityNux>,
    /// whether this model is supported in the api
    pub supported_in_api: bool,
    /// Input modalities accepted when composing user turns for this preset.
    #[serde(default = "default_input_modalities")]
    pub input_modalities: Vec<InputModality>,
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
}

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, TS, JsonSchema, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum WebSearchToolType {
    #[default]
    Text,
    TextAndImage,
}

/// Server-provided truncation policy metadata for a model.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TruncationMode {
    Bytes,
    Tokens,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ToolMode {
    Direct,
    CodeMode,
    CodeModeOnly,
}

fn deserialize_optional_model_selector<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let Some(value) = Option::<String>::deserialize(deserializer)? else {
        return Ok(None);
    };
    Ok(serde_json::from_value(serde_json::Value::String(value)).ok())
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

/// Semantic version triple encoded as an array in JSON (e.g. [0, 62, 0]).
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, TS, JsonSchema)]
pub struct ClientVersion(pub i32, pub i32, pub i32);

const fn default_effective_context_window_percent() -> i64 {
    95
}

const fn default_true() -> bool {
    true
}

#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_true(value: &bool) -> bool {
    *value
}

/// Model metadata returned by the Codex backend `/models` endpoint.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, TS, JsonSchema)]
pub struct ModelInfo {
    pub slug: String,
    pub display_name: String,
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_reasoning_level: Option<ReasoningEffort>,
    pub supported_reasoning_levels: Vec<ReasoningEffortPreset>,
    pub shell_type: ConfigShellToolType,
    pub visibility: ModelVisibility,
    pub supported_in_api: bool,
    pub priority: i32,
    #[serde(default)]
    pub additional_speed_tiers: Vec<String>,
    #[serde(default)]
    pub service_tiers: Vec<ModelServiceTier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_service_tier: Option<String>,
    pub availability_nux: Option<ModelAvailabilityNux>,
    pub upgrade: Option<ModelInfoUpgrade>,
    pub base_instructions: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_messages: Option<ModelMessages>,
    #[serde(default)]
    pub include_skills_usage_instructions: bool,
    /// Whether the model accepts the Responses API `reasoning.summary` parameter.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub supports_reasoning_summary_parameter: bool,
    #[serde(default)]
    pub default_reasoning_summary: ReasoningSummary,
    pub support_verbosity: bool,
    pub default_verbosity: Option<Verbosity>,
    pub apply_patch_tool_type: Option<ApplyPatchToolType>,
    #[serde(default)]
    pub web_search_tool_type: WebSearchToolType,
    pub truncation_policy: TruncationPolicyConfig,
    pub supports_parallel_tool_calls: bool,
    #[serde(default)]
    pub supports_image_detail_original: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<i64>,
    /// Maximum context window allowed for config overrides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_context_window: Option<i64>,
    /// Token threshold for automatic compaction. When omitted, core derives it
    /// from `context_window` (90%). When provided, core clamps it to 90% of the
    /// context window when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_compact_token_limit: Option<i64>,
    /// Opaque identifier for compaction-compatible model configurations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comp_hash: Option<String>,
    /// Percentage of the context window considered usable for inputs, after
    /// reserving headroom for system prompts, tool overhead, and model output.
    #[serde(default = "default_effective_context_window_percent")]
    pub effective_context_window_percent: i64,
    pub experimental_supported_tools: Vec<String>,
    /// Input modalities accepted by the backend for this model.
    #[serde(default = "default_input_modalities")]
    pub input_modalities: Vec<InputModality>,
    /// Internal-only marker set by core when a model slug resolved to fallback metadata.
    #[serde(default, skip_serializing, skip_deserializing)]
    #[schemars(skip)]
    #[ts(skip)]
    pub used_fallback_model_metadata: bool,
    #[serde(default)]
    pub supports_search_tool: bool,
    #[serde(default)]
    pub use_responses_lite: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_review_model_override: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_model_selector"
    )]
    pub tool_mode: Option<ToolMode>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_model_selector"
    )]
    pub multi_agent_version: Option<MultiAgentVersion>,
}

impl ModelInfo {
    pub fn resolved_context_window(&self) -> Option<i64> {
        self.context_window.or(self.max_context_window)
    }

    pub fn auto_compact_token_limit(&self) -> Option<i64> {
        let context_limit = self
            .resolved_context_window()
            .map(|context_window| (context_window * 9) / 10);
        let config_limit = self.auto_compact_token_limit;
        if let Some(context_limit) = context_limit {
            return Some(
                config_limit.map_or(context_limit, |limit| std::cmp::min(limit, context_limit)),
            );
        }
        config_limit
    }

    pub fn supports_personality(&self) -> bool {
        self.model_messages
            .as_ref()
            .is_some_and(ModelMessages::supports_personality)
    }

    pub fn get_model_instructions(&self, personality: Option<Personality>) -> String {
        if let Some(model_messages) = &self.model_messages
            && let Some(template) = &model_messages.instructions_template
        {
            // if we have a template, always use it
            let personality_message = model_messages
                .get_personality_message(personality)
                .unwrap_or_default();
            template.replace(PERSONALITY_PLACEHOLDER, personality_message.as_str())
        } else {
            match personality {
                Some(personality @ (Personality::Friendly | Personality::Pragmatic)) => {
                    warn!(
                        model = %self.slug,
                        %personality,
                        "Model personality requested but model_messages is missing, falling back to base instructions."
                    );
                }
                Some(Personality::None) | None => {}
            }
            self.base_instructions.clone()
        }
    }
}

/// A strongly-typed template for assembling model instructions and developer messages. If
/// instructions_* is populated and valid, it will override base_instructions.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, TS, JsonSchema)]
pub struct ModelMessages {
    pub instructions_template: Option<String>,
    pub instructions_variables: Option<ModelInstructionsVariables>,
    pub approvals: Option<ApprovalMessages>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, TS, JsonSchema)]
pub struct ApprovalMessages {
    pub on_request: Option<String>,
    pub on_request_auto_review: Option<String>,
}

impl ModelMessages {
    fn has_personality_placeholder(&self) -> bool {
        self.instructions_template
            .as_ref()
            .map(|spec| spec.contains(PERSONALITY_PLACEHOLDER))
            .unwrap_or(false)
    }

    fn supports_personality(&self) -> bool {
        self.has_personality_placeholder()
            && self
                .instructions_variables
                .as_ref()
                .is_some_and(ModelInstructionsVariables::is_complete)
    }

    pub fn get_personality_message(&self, personality: Option<Personality>) -> Option<String> {
        self.instructions_variables
            .as_ref()
            .and_then(|variables| variables.get_personality_message(personality))
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, TS, JsonSchema)]
pub struct ModelInstructionsVariables {
    pub personality_default: Option<String>,
    pub personality_friendly: Option<String>,
    pub personality_pragmatic: Option<String>,
}

impl ModelInstructionsVariables {
    pub fn is_complete(&self) -> bool {
        self.personality_default.is_some()
            && self.personality_friendly.is_some()
            && self.personality_pragmatic.is_some()
    }

    pub fn get_personality_message(&self, personality: Option<Personality>) -> Option<String> {
        if let Some(personality) = personality {
            match personality {
                Personality::None => Some(String::new()),
                Personality::Friendly => self.personality_friendly.clone(),
                Personality::Pragmatic => self.personality_pragmatic.clone(),
            }
        } else {
            self.personality_default.clone()
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, TS, JsonSchema)]
pub struct ModelInfoUpgrade {
    pub model: String,
    pub migration_markdown: String,
}

impl From<&ModelUpgrade> for ModelInfoUpgrade {
    fn from(upgrade: &ModelUpgrade) -> Self {
        ModelInfoUpgrade {
            model: upgrade.id.clone(),
            migration_markdown: upgrade.migration_markdown.clone().unwrap_or_default(),
        }
    }
}

/// Response wrapper for `/models`.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, TS, JsonSchema, Default)]
pub struct ModelsResponse {
    pub models: Vec<ModelInfo>,
}

// convert ModelInfo to ModelPreset
impl From<ModelInfo> for ModelPreset {
    fn from(info: ModelInfo) -> Self {
        let supports_personality = info.supports_personality();
        ModelPreset {
            id: info.slug.clone(),
            model: info.slug.clone(),
            display_name: info.display_name,
            description: info.description.unwrap_or_default(),
            default_reasoning_effort: info
                .default_reasoning_level
                .unwrap_or(ReasoningEffort::None),
            supported_reasoning_efforts: info.supported_reasoning_levels.clone(),
            supports_personality,
            additional_speed_tiers: info.additional_speed_tiers,
            service_tiers: info.service_tiers,
            default_service_tier: info.default_service_tier,
            is_default: false, // default is the highest priority available model
            upgrade: info.upgrade.as_ref().map(|upgrade| ModelUpgrade {
                id: upgrade.model.clone(),
                migration_config_key: info.slug.clone(),
                // todo(aibrahim): add the model link here.
                model_link: None,
                upgrade_copy: None,
                migration_markdown: Some(upgrade.migration_markdown.clone()),
            }),
            show_in_picker: info.visibility == ModelVisibility::List,
            availability_nux: info.availability_nux,
            supported_in_api: info.supported_in_api,
            input_modalities: info.input_modalities,
        }
    }
}

impl ModelPreset {
    pub fn supports_fast_mode(&self) -> bool {
        self.service_tiers
            .iter()
            .any(|tier| tier.id == ServiceTier::Fast.request_value())
            || self
                .additional_speed_tiers
                .iter()
                .any(|tier| tier == SPEED_TIER_FAST)
    }
}

impl ModelInfo {
    pub fn supports_service_tier(&self, service_tier: &str) -> bool {
        self.service_tiers
            .iter()
            .any(|tier| tier.id == service_tier)
    }

    pub fn service_tier_for_request(&self, service_tier: Option<String>) -> Option<String> {
        service_tier.filter(|service_tier| {
            service_tier != SERVICE_TIER_DEFAULT_REQUEST_VALUE
                && self.supports_service_tier(service_tier)
        })
    }
}

impl ModelPreset {
    /// Filter models based on authentication mode.
    ///
    /// In ChatGPT mode, all models are visible. Otherwise, only API-supported models are shown.
    pub fn filter_by_auth(models: Vec<ModelPreset>, chatgpt_mode: bool) -> Vec<ModelPreset> {
        models
            .into_iter()
            .filter(|model| chatgpt_mode || model.supported_in_api)
            .collect()
    }

    /// Recompute the single default preset using picker visibility.
    ///
    /// The first picker-visible model wins; if none are picker-visible, the first model wins.
    pub fn mark_default_by_picker_visibility(models: &mut [ModelPreset]) {
        for preset in models.iter_mut() {
            preset.is_default = false;
        }
        if let Some(default) = models.iter_mut().find(|preset| preset.show_in_picker) {
            default.is_default = true;
        } else if let Some(default) = models.first_mut() {
            default.is_default = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::from_str;
    use serde_json::to_string;

    fn test_model(spec: Option<ModelMessages>) -> ModelInfo {
        ModelInfo {
            slug: "test-model".to_string(),
            display_name: "Test Model".to_string(),
            description: None,
            default_reasoning_level: None,
            supported_reasoning_levels: vec![],
            shell_type: ConfigShellToolType::ShellCommand,
            visibility: ModelVisibility::List,
            supported_in_api: true,
            priority: 1,
            additional_speed_tiers: Vec::new(),
            service_tiers: Vec::new(),
            default_service_tier: None,
            availability_nux: None,
            upgrade: None,
            base_instructions: "base".to_string(),
            model_messages: spec,
            include_skills_usage_instructions: false,
            supports_reasoning_summary_parameter: true,
            default_reasoning_summary: ReasoningSummary::Auto,
            support_verbosity: false,
            default_verbosity: None,
            apply_patch_tool_type: None,
            web_search_tool_type: WebSearchToolType::Text,
            truncation_policy: TruncationPolicyConfig::bytes(/*limit*/ 10_000),
            supports_parallel_tool_calls: false,
            supports_image_detail_original: false,
            context_window: None,
            max_context_window: None,
            auto_compact_token_limit: None,
            comp_hash: None,
            effective_context_window_percent: 95,
            experimental_supported_tools: vec![],
            input_modalities: default_input_modalities(),
            used_fallback_model_metadata: false,
            supports_search_tool: false,
            use_responses_lite: false,
            auto_review_model_override: None,
            tool_mode: None,
            multi_agent_version: None,
        }
    }

    fn personality_variables() -> ModelInstructionsVariables {
        ModelInstructionsVariables {
            personality_default: Some("default".to_string()),
            personality_friendly: Some("friendly".to_string()),
            personality_pragmatic: Some("pragmatic".to_string()),
        }
    }

    #[test]
    fn model_messages_deserialize_without_approvals() {
        let messages: ModelMessages =
            from_str(r#"{"instructions_template":null,"instructions_variables":null}"#)
                .expect("model messages should deserialize");

        assert_eq!(messages.approvals, None);
    }

    #[test]
    fn approval_messages_preserve_missing_and_empty_values() {
        let messages: ModelMessages = from_str(
            r#"{
                "instructions_template": null,
                "instructions_variables": null,
                "approvals": {
                    "on_request": ""
                }
            }"#,
        )
        .expect("approval messages should deserialize");

        assert_eq!(
            messages.approvals,
            Some(ApprovalMessages {
                on_request: Some(String::new()),
                on_request_auto_review: None,
            })
        );
    }

    #[test]
    fn reasoning_effort_accepts_known_and_custom_values() {
        let custom = ReasoningEffort::Custom("future".to_string());
        let deserialized = from_str::<ReasoningEffort>(r#""future""#)
            .expect("custom reasoning effort should deserialize");
        let serialized = to_string(&custom).expect("custom reasoning effort should serialize");
        let serialized_max = to_string(&ReasoningEffort::Max).expect("Max should serialize");
        let serialized_ultra = to_string(&ReasoningEffort::Ultra).expect("Ultra should serialize");

        assert_eq!(
            (
                "high".parse(),
                "max".parse(),
                "ultra".parse(),
                "future".parse(),
                deserialized,
                serialized,
                serialized_max,
                serialized_ultra,
                custom.to_string(),
            ),
            (
                Ok(ReasoningEffort::High),
                Ok(ReasoningEffort::Max),
                Ok(ReasoningEffort::Ultra),
                Ok(custom.clone()),
                custom,
                r#""future""#.to_string(),
                r#""max""#.to_string(),
                r#""ultra""#.to_string(),
                "future".to_string(),
            )
        );
    }

    #[test]
    fn reasoning_effort_rejects_empty_values() {
        assert_eq!(
            "".parse::<ReasoningEffort>(),
            Err("reasoning_effort must not be empty".to_string())
        );
    }

    #[test]
    fn reasoning_effort_json_schema_is_an_open_string() {
        let mut effort_generator = SchemaGenerator::default();

        assert_eq!(
            ReasoningEffort::json_schema(&mut effort_generator),
            Schema::Object(SchemaObject {
                instance_type: Some(InstanceType::String.into()),
                metadata: Some(Box::new(Metadata {
                    description: Some(
                        "A non-empty reasoning effort value advertised by the model.".to_string(),
                    ),
                    ..Default::default()
                })),
                string: Some(Box::new(StringValidation {
                    min_length: Some(1),
                    ..Default::default()
                })),
                ..Default::default()
            })
        );
    }

    #[test]
    fn get_model_instructions_uses_template_when_placeholder_present() {
        let model = test_model(Some(ModelMessages {
            instructions_template: Some("Hello {{ personality }}".to_string()),
            instructions_variables: Some(personality_variables()),
            approvals: None,
        }));

        let instructions = model.get_model_instructions(Some(Personality::Friendly));

        assert_eq!(instructions, "Hello friendly");
    }

    #[test]
    fn get_model_instructions_always_strips_placeholder() {
        let model = test_model(Some(ModelMessages {
            instructions_template: Some("Hello\n{{ personality }}".to_string()),
            instructions_variables: Some(ModelInstructionsVariables {
                personality_default: None,
                personality_friendly: Some("friendly".to_string()),
                personality_pragmatic: None,
            }),
            approvals: None,
        }));
        assert_eq!(
            model.get_model_instructions(Some(Personality::Friendly)),
            "Hello\nfriendly"
        );
        assert_eq!(
            model.get_model_instructions(Some(Personality::Pragmatic)),
            "Hello\n"
        );
        assert_eq!(
            model.get_model_instructions(Some(Personality::None)),
            "Hello\n"
        );
        assert_eq!(
            model.get_model_instructions(/*personality*/ None),
            "Hello\n"
        );

        let model_no_personality = test_model(Some(ModelMessages {
            instructions_template: Some("Hello\n{{ personality }}".to_string()),
            instructions_variables: Some(ModelInstructionsVariables {
                personality_default: None,
                personality_friendly: None,
                personality_pragmatic: None,
            }),
            approvals: None,
        }));
        assert_eq!(
            model_no_personality.get_model_instructions(Some(Personality::Friendly)),
            "Hello\n"
        );
        assert_eq!(
            model_no_personality.get_model_instructions(Some(Personality::Pragmatic)),
            "Hello\n"
        );
        assert_eq!(
            model_no_personality.get_model_instructions(Some(Personality::None)),
            "Hello\n"
        );
        assert_eq!(
            model_no_personality.get_model_instructions(/*personality*/ None),
            "Hello\n"
        );
    }

    #[test]
    fn get_model_instructions_falls_back_when_template_is_missing() {
        let model = test_model(Some(ModelMessages {
            instructions_template: None,
            instructions_variables: Some(ModelInstructionsVariables {
                personality_default: None,
                personality_friendly: None,
                personality_pragmatic: None,
            }),
            approvals: None,
        }));

        let instructions = model.get_model_instructions(Some(Personality::Friendly));

        assert_eq!(instructions, "base");
    }

    #[test]
    fn get_personality_message_returns_default_when_personality_is_none() {
        let personality_template = personality_variables();
        assert_eq!(
            personality_template.get_personality_message(/*personality*/ None),
            Some("default".to_string())
        );
    }

    #[test]
    fn get_personality_message() {
        let personality_variables = personality_variables();
        assert_eq!(
            personality_variables.get_personality_message(Some(Personality::Friendly)),
            Some("friendly".to_string())
        );
        assert_eq!(
            personality_variables.get_personality_message(Some(Personality::Pragmatic)),
            Some("pragmatic".to_string())
        );
        assert_eq!(
            personality_variables.get_personality_message(Some(Personality::None)),
            Some(String::new())
        );
        assert_eq!(
            personality_variables.get_personality_message(/*personality*/ None),
            Some("default".to_string())
        );

        let personality_variables = ModelInstructionsVariables {
            personality_default: Some("default".to_string()),
            personality_friendly: None,
            personality_pragmatic: None,
        };
        assert_eq!(
            personality_variables.get_personality_message(Some(Personality::Friendly)),
            None
        );
        assert_eq!(
            personality_variables.get_personality_message(Some(Personality::Pragmatic)),
            None
        );
        assert_eq!(
            personality_variables.get_personality_message(Some(Personality::None)),
            Some(String::new())
        );
        assert_eq!(
            personality_variables.get_personality_message(/*personality*/ None),
            Some("default".to_string())
        );

        let personality_variables = ModelInstructionsVariables {
            personality_default: None,
            personality_friendly: Some("friendly".to_string()),
            personality_pragmatic: Some("pragmatic".to_string()),
        };
        assert_eq!(
            personality_variables.get_personality_message(Some(Personality::Friendly)),
            Some("friendly".to_string())
        );
        assert_eq!(
            personality_variables.get_personality_message(Some(Personality::Pragmatic)),
            Some("pragmatic".to_string())
        );
        assert_eq!(
            personality_variables.get_personality_message(Some(Personality::None)),
            Some(String::new())
        );
        assert_eq!(
            personality_variables.get_personality_message(/*personality*/ None),
            None
        );
    }

    #[test]
    fn model_info_defaults_availability_nux_to_none_when_omitted() {
        let model: ModelInfo = serde_json::from_value(serde_json::json!({
            "slug": "test-model",
            "display_name": "Test Model",
            "description": null,
            "supported_reasoning_levels": [],
            "shell_type": "shell_command",
            "visibility": "list",
            "supported_in_api": true,
            "priority": 1,
            "upgrade": null,
            "base_instructions": "base",
            "model_messages": null,
            "default_reasoning_summary": "auto",
            "support_verbosity": false,
            "default_verbosity": null,
            "apply_patch_tool_type": null,
            "truncation_policy": {
                "mode": "bytes",
                "limit": 10000
            },
            "supports_parallel_tool_calls": false,
            "supports_image_detail_original": false,
            "context_window": null,
            "auto_compact_token_limit": null,
            "effective_context_window_percent": 95,
            "experimental_supported_tools": [],
            "input_modalities": ["text", "image"]
        }))
        .expect("deserialize model info");

        assert_eq!(model.availability_nux, None);
        assert!(!model.include_skills_usage_instructions);
        assert!(model.supports_reasoning_summary_parameter);
        assert!(!model.supports_image_detail_original);
        assert_eq!(model.web_search_tool_type, WebSearchToolType::Text);
        assert!(!model.supports_search_tool);
        assert!(!model.use_responses_lite);
        assert_eq!(model.comp_hash, None);
        assert_eq!(model.auto_review_model_override, None);
        assert_eq!(model.tool_mode, None);
    }

    #[test]
    fn model_info_deserializes_known_tool_mode() {
        let mut value =
            serde_json::to_value(test_model(/*spec*/ None)).expect("serialize test model");
        let object = value
            .as_object_mut()
            .expect("model info should be an object");
        object.insert(
            "tool_mode".to_string(),
            serde_json::Value::String("code_mode_only".to_string()),
        );
        let model = serde_json::from_value::<ModelInfo>(value).expect("deserialize model info");

        assert_eq!(model.tool_mode, Some(ToolMode::CodeModeOnly));
    }

    #[test]
    fn model_info_treats_unknown_tool_mode_as_omitted() {
        let mut value =
            serde_json::to_value(test_model(/*spec*/ None)).expect("serialize test model");
        let object = value
            .as_object_mut()
            .expect("model info should be an object");
        object.insert(
            "tool_mode".to_string(),
            serde_json::Value::String("future_tool_mode".to_string()),
        );
        let model = serde_json::from_value::<ModelInfo>(value).expect("deserialize model info");

        assert_eq!(model.tool_mode, None);
        let serialized = serde_json::to_value(model).expect("serialize model info");
        let object = serialized
            .as_object()
            .expect("model info should be an object");
        assert!(!object.contains_key("tool_mode"));
    }

    #[test]
    fn model_info_treats_unknown_multi_agent_version_as_omitted() {
        let mut value =
            serde_json::to_value(test_model(/*spec*/ None)).expect("serialize test model");
        let object = value
            .as_object_mut()
            .expect("model info should be an object");
        object.insert(
            "multi_agent_version".to_string(),
            serde_json::Value::String("future_multi_agent_version".to_string()),
        );
        let model = serde_json::from_value::<ModelInfo>(value).expect("deserialize model info");

        assert_eq!(model.multi_agent_version, None);
    }

    #[test]
    fn resolved_context_window_prefers_context_window() {
        let model = ModelInfo {
            context_window: Some(273_000),
            max_context_window: Some(400_000),
            ..test_model(/*spec*/ None)
        };

        assert_eq!(model.resolved_context_window(), Some(273_000));
    }

    #[test]
    fn resolved_context_window_falls_back_to_max_context_window() {
        let model = ModelInfo {
            context_window: None,
            max_context_window: Some(400_000),
            ..test_model(/*spec*/ None)
        };

        assert_eq!(model.resolved_context_window(), Some(400_000));
        assert_eq!(model.auto_compact_token_limit(), Some(360_000));
    }

    #[test]
    fn model_preset_preserves_availability_nux() {
        let preset = ModelPreset::from(ModelInfo {
            availability_nux: Some(ModelAvailabilityNux {
                message: "Try Spark.".to_string(),
            }),
            additional_speed_tiers: vec![SPEED_TIER_FAST.to_string()],
            default_service_tier: Some(ServiceTier::Fast.request_value().to_string()),
            service_tiers: Vec::new(),
            ..test_model(/*spec*/ None)
        });

        assert_eq!(
            preset.availability_nux,
            Some(ModelAvailabilityNux {
                message: "Try Spark.".to_string(),
            })
        );
        assert!(preset.supports_fast_mode());
        assert_eq!(
            preset.default_service_tier,
            Some(ServiceTier::Fast.request_value().to_string())
        );
    }

    #[test]
    fn model_preset_supports_fast_mode_from_service_tiers() {
        let preset = ModelPreset::from(ModelInfo {
            service_tiers: vec![ModelServiceTier {
                id: ServiceTier::Fast.request_value().to_string(),
                name: "Fast".to_string(),
                description: "Priority processing.".to_string(),
            }],
            ..test_model(/*spec*/ None)
        });

        assert!(preset.supports_fast_mode());
    }

    #[test]
    fn service_tier_for_request_omits_explicit_default_tier() {
        let model = ModelInfo {
            default_service_tier: Some(ServiceTier::Fast.request_value().to_string()),
            service_tiers: vec![ModelServiceTier {
                id: ServiceTier::Fast.request_value().to_string(),
                name: "Fast".to_string(),
                description: "Priority processing.".to_string(),
            }],
            ..test_model(/*spec*/ None)
        };

        assert_eq!(
            model.service_tier_for_request(Some(SERVICE_TIER_DEFAULT_REQUEST_VALUE.to_string())),
            None
        );
    }

    #[test]
    fn service_tier_for_request_filters_unsupported_tiers() {
        let model = ModelInfo {
            default_service_tier: Some(ServiceTier::Fast.request_value().to_string()),
            service_tiers: vec![ModelServiceTier {
                id: ServiceTier::Fast.request_value().to_string(),
                name: "Fast".to_string(),
                description: "Priority processing.".to_string(),
            }],
            ..test_model(/*spec*/ None)
        };

        assert_eq!(
            model.service_tier_for_request(Some(ServiceTier::Fast.request_value().to_string())),
            Some(ServiceTier::Fast.request_value().to_string())
        );
        assert_eq!(
            model.service_tier_for_request(Some("unsupported".to_string())),
            None
        );
        assert_eq!(model.service_tier_for_request(/*service_tier*/ None), None);
    }

    #[test]
    fn service_tier_for_request_does_not_apply_catalog_default() {
        let model = ModelInfo {
            default_service_tier: Some(ServiceTier::Fast.request_value().to_string()),
            service_tiers: vec![ModelServiceTier {
                id: ServiceTier::Fast.request_value().to_string(),
                name: "Fast".to_string(),
                description: "Priority processing.".to_string(),
            }],
            ..test_model(/*spec*/ None)
        };

        assert_eq!(model.service_tier_for_request(/*service_tier*/ None), None);
    }
}
