//! Configuration types for multi-provider management.
//!
//! This module defines the types used to configure models and providers
//! from JSON files. The configuration follows a layered approach:
//!
//! - `models.json`: Provider-independent model metadata
//! - `providers.json`: Provider access configuration with optional model overrides
//! - `profiles.json`: Named configuration bundles for quick switching

use cocode_protocol::Capability;
use cocode_protocol::ModelInfo;
use cocode_protocol::ReasoningEffort;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;

/// Root structure for models.json file.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelsFile {
    /// Schema version for forward compatibility.
    #[serde(default = "default_version")]
    pub version: String,
    /// Map of model ID to model configuration.
    #[serde(default)]
    pub models: HashMap<String, ModelInfo>,
}

/// Root structure for providers.json file.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProvidersFile {
    /// Schema version for forward compatibility.
    #[serde(default = "default_version")]
    pub version: String,
    /// Map of provider name to provider configuration.
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
}

/// Root structure for profiles.json file.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProfilesFile {
    /// Schema version for forward compatibility.
    #[serde(default = "default_version")]
    pub version: String,
    /// Default profile to use when none is specified.
    #[serde(default)]
    pub default_profile: Option<String>,
    /// Map of profile name to profile configuration.
    #[serde(default)]
    pub profiles: HashMap<String, ProfileConfig>,
}

/// Runtime state stored in active.json.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ActiveState {
    /// Currently active provider name.
    #[serde(default)]
    pub provider: Option<String>,
    /// Currently active model ID.
    #[serde(default)]
    pub model: Option<String>,
    /// Currently active profile name.
    #[serde(default)]
    pub profile: Option<String>,
    /// Runtime overrides for session config.
    #[serde(default)]
    pub session_overrides: Option<SessionConfigJson>,
    /// Timestamp of last update.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<String>,
}

fn default_version() -> String {
    "1.0".to_string()
}

/// Provider type enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderType {
    /// OpenAI API compatible.
    Openai,
    /// Anthropic Claude API.
    Anthropic,
    /// Google Gemini API.
    Gemini,
    /// Volcengine Ark API.
    Volcengine,
    /// Z.AI / ZhipuAI API.
    Zai,
    /// Generic OpenAI-compatible API.
    OpenaiCompat,
}

impl Default for ProviderType {
    fn default() -> Self {
        Self::Openai
    }
}

impl std::fmt::Display for ProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Openai => write!(f, "openai"),
            Self::Anthropic => write!(f, "anthropic"),
            Self::Gemini => write!(f, "gemini"),
            Self::Volcengine => write!(f, "volcengine"),
            Self::Zai => write!(f, "zai"),
            Self::OpenaiCompat => write!(f, "openai_compat"),
        }
    }
}

/// Provider configuration from JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Human-readable provider name.
    pub name: String,

    /// Provider type for selecting the implementation.
    #[serde(rename = "type")]
    pub provider_type: ProviderType,

    /// Environment variable name for API key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env_key: Option<String>,

    /// API key (prefer env_key for security).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    /// Base URL override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,

    /// Default model for this provider.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_model: Option<String>,

    /// Request timeout in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<i64>,

    /// Organization ID (for providers that support it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub organization_id: Option<String>,

    /// Model configurations specific to this provider.
    #[serde(default)]
    pub models: HashMap<String, ProviderModelConfig>,

    /// Extra provider-specific configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra: Option<serde_json::Value>,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            provider_type: ProviderType::default(),
            env_key: None,
            api_key: None,
            base_url: None,
            default_model: None,
            timeout_secs: None,
            organization_id: None,
            models: HashMap::new(),
            extra: None,
        }
    }
}

/// Model config within a provider (with override capability).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderModelConfig {
    /// Model ID alias (e.g., "ep-xxx" -> "deepseek-r1").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,

    /// Override model info for this provider-model combination.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_info_override: Option<ModelInfo>,
}

/// Profile for quick provider/model switching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileConfig {
    /// Provider name to use.
    pub provider: String,
    /// Model ID to use.
    pub model: String,
    /// Session configuration for this profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_config: Option<SessionConfigJson>,
}

/// Thinking configuration in JSON format.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ThinkingConfigJson {
    /// Thinking budget in tokens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<i32>,

    /// Whether to enable thinking summaries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_summary: Option<bool>,
}

/// Session configuration in JSON format.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionConfigJson {
    /// Sampling temperature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,

    /// Maximum tokens to generate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i32>,

    /// Top-p nucleus sampling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,

    /// Thinking budget in tokens (shorthand for thinking_config.budget).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_budget: Option<i32>,

    /// Reasoning effort level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<ReasoningEffort>,

    /// Full thinking configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_config: Option<ThinkingConfigJson>,
}

impl SessionConfigJson {
    /// Create a new empty session config.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the temperature.
    pub fn with_temperature(mut self, t: f64) -> Self {
        self.temperature = Some(t);
        self
    }

    /// Set the max tokens.
    pub fn with_max_tokens(mut self, n: i32) -> Self {
        self.max_tokens = Some(n);
        self
    }

    /// Set the top-p.
    pub fn with_top_p(mut self, p: f64) -> Self {
        self.top_p = Some(p);
        self
    }

    /// Set the thinking budget.
    pub fn with_thinking_budget(mut self, budget: i32) -> Self {
        self.thinking_budget = Some(budget);
        self
    }

    /// Set the reasoning effort.
    pub fn with_reasoning_effort(mut self, effort: ReasoningEffort) -> Self {
        self.reasoning_effort = Some(effort);
        self
    }
}

/// Resolved model info with all layers merged.
#[derive(Debug, Clone)]
pub struct ResolvedModelInfo {
    /// The model identifier.
    pub id: String,
    /// Human-readable name.
    pub display_name: String,
    /// Model description.
    pub description: Option<String>,
    /// Provider name.
    pub provider: String,
    /// Maximum context window in tokens.
    pub context_window: i64,
    /// Maximum output tokens.
    pub max_output_tokens: i64,
    /// Capabilities this model supports.
    pub capabilities: Vec<Capability>,
    /// Token limit before auto-compaction triggers.
    pub auto_compact_token_limit: Option<i64>,
    /// Effective context window as percentage.
    pub effective_context_window_percent: Option<i32>,
    /// Default reasoning effort level.
    pub default_reasoning_effort: Option<ReasoningEffort>,
    /// Supported reasoning effort levels.
    pub supported_reasoning_levels: Option<Vec<ReasoningEffort>>,
    /// Default thinking budget in tokens.
    pub thinking_budget_default: Option<i32>,
    /// Base system instructions for this model.
    pub base_instructions: Option<String>,
}

impl ResolvedModelInfo {
    /// Check if model has a specific capability.
    pub fn has_capability(&self, cap: Capability) -> bool {
        self.capabilities.contains(&cap)
    }

    /// Check if model supports reasoning summaries.
    pub fn supports_reasoning_summaries(&self) -> bool {
        self.capabilities.contains(&Capability::ReasoningSummaries)
    }

    /// Check if model supports parallel tool calls.
    pub fn supports_parallel_tool_calls(&self) -> bool {
        self.capabilities.contains(&Capability::ParallelToolCalls)
    }
}

/// Resolved provider config ready for use.
#[derive(Debug, Clone)]
pub struct ResolvedProviderConfig {
    /// Provider name.
    pub name: String,
    /// Provider type.
    pub provider_type: ProviderType,
    /// API key (resolved from env or config).
    pub api_key: String,
    /// Base URL.
    pub base_url: Option<String>,
    /// Default model.
    pub default_model: Option<String>,
    /// Request timeout in seconds.
    pub timeout_secs: i64,
    /// Organization ID.
    pub organization_id: Option<String>,
    /// Extra provider-specific configuration.
    pub extra: Option<serde_json::Value>,
}

/// Summary of a provider for listing.
#[derive(Debug, Clone, Serialize)]
pub struct ProviderSummary {
    /// Provider key/name.
    pub name: String,
    /// Human-readable display name.
    pub display_name: String,
    /// Provider type.
    pub provider_type: ProviderType,
    /// Whether API key is configured.
    pub has_api_key: bool,
    /// Number of models configured.
    pub model_count: i32,
}

/// Summary of a model for listing.
#[derive(Debug, Clone, Serialize)]
pub struct ModelSummary {
    /// Model ID.
    pub id: String,
    /// Human-readable display name.
    pub display_name: String,
    /// Context window size.
    pub context_window: Option<i64>,
    /// Capabilities summary.
    pub capabilities: Vec<Capability>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_info_config_merge() {
        let mut base = ModelInfo {
            display_name: Some("Base Model".to_string()),
            context_window: Some(4096),
            max_output_tokens: Some(1024),
            capabilities: Some(vec![Capability::TextGeneration]),
            ..Default::default()
        };

        let override_cfg = ModelInfo {
            context_window: Some(8192),
            capabilities: Some(vec![
                Capability::TextGeneration,
                Capability::ParallelToolCalls,
            ]),
            ..Default::default()
        };

        base.merge_from(&override_cfg);

        assert_eq!(base.display_name, Some("Base Model".to_string())); // Not overridden
        assert_eq!(base.context_window, Some(8192)); // Overridden
        assert_eq!(base.max_output_tokens, Some(1024)); // Not overridden
        assert!(base.has_capability(Capability::ParallelToolCalls)); // New value
    }

    #[test]
    fn test_provider_type_serde() {
        let pt = ProviderType::Anthropic;
        let json = serde_json::to_string(&pt).expect("serialize");
        assert_eq!(json, "\"anthropic\"");

        let parsed: ProviderType = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, ProviderType::Anthropic);
    }

    #[test]
    fn test_models_file_serde() {
        let json = r#"{
            "version": "1.0",
            "models": {
                "gpt-4o": {
                    "display_name": "GPT-4o",
                    "context_window": 128000,
                    "max_output_tokens": 16384,
                    "capabilities": ["text_generation", "streaming", "vision"]
                }
            }
        }"#;

        let file: ModelsFile = serde_json::from_str(json).expect("deserialize");
        assert_eq!(file.version, "1.0");
        assert!(file.models.contains_key("gpt-4o"));

        let model = file.models.get("gpt-4o").expect("model exists");
        assert_eq!(model.display_name, Some("GPT-4o".to_string()));
        assert_eq!(model.context_window, Some(128000));
    }

    #[test]
    fn test_providers_file_serde() {
        let json = r#"{
            "version": "1.0",
            "providers": {
                "openai": {
                    "name": "OpenAI",
                    "type": "openai",
                    "env_key": "OPENAI_API_KEY",
                    "base_url": "https://api.openai.com/v1",
                    "models": {}
                }
            }
        }"#;

        let file: ProvidersFile = serde_json::from_str(json).expect("deserialize");
        assert_eq!(file.version, "1.0");

        let provider = file.providers.get("openai").expect("provider exists");
        assert_eq!(provider.name, "OpenAI");
        assert_eq!(provider.provider_type, ProviderType::Openai);
    }

    #[test]
    fn test_profile_config_serde() {
        let json = r#"{
            "provider": "anthropic",
            "model": "claude-sonnet-4-20250514",
            "session_config": {
                "temperature": 0.3,
                "thinking_budget": 10000
            }
        }"#;

        let profile: ProfileConfig = serde_json::from_str(json).expect("deserialize");
        assert_eq!(profile.provider, "anthropic");
        assert_eq!(profile.model, "claude-sonnet-4-20250514");

        let session = profile.session_config.expect("session config exists");
        assert_eq!(session.temperature, Some(0.3));
        assert_eq!(session.thinking_budget, Some(10000));
    }

    #[test]
    fn test_session_config_json_builder() {
        let json_config = SessionConfigJson::new()
            .with_temperature(0.7)
            .with_max_tokens(4096)
            .with_thinking_budget(5000)
            .with_reasoning_effort(ReasoningEffort::Medium);

        assert_eq!(json_config.temperature, Some(0.7));
        assert_eq!(json_config.max_tokens, Some(4096));
        assert_eq!(json_config.thinking_budget, Some(5000));
        assert_eq!(json_config.reasoning_effort, Some(ReasoningEffort::Medium));
    }

    #[test]
    fn test_resolved_model_info_capability_methods() {
        let info = ResolvedModelInfo {
            id: "test".to_string(),
            display_name: "Test".to_string(),
            description: None,
            provider: "test".to_string(),
            context_window: 4096,
            max_output_tokens: 1024,
            capabilities: vec![
                Capability::TextGeneration,
                Capability::ReasoningSummaries,
                Capability::ParallelToolCalls,
            ],
            auto_compact_token_limit: None,
            effective_context_window_percent: None,
            default_reasoning_effort: None,
            supported_reasoning_levels: None,
            thinking_budget_default: None,
            base_instructions: None,
        };

        assert!(info.supports_reasoning_summaries());
        assert!(info.supports_parallel_tool_calls());
        assert!(info.has_capability(Capability::TextGeneration));
        assert!(!info.has_capability(Capability::Vision));
    }
}
