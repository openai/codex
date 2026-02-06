//! Configuration types for multi-provider management.
//!
//! This module defines the types used to configure models and providers
//! from JSON/TOML files. The configuration follows a layered approach:
//!
//! - `models.json`: Provider-independent model metadata
//! - `providers.json` / `config.toml`: Provider configuration with model entries
//!
//! For resolved runtime types, see `ProviderInfo` in cocode_protocol.

use crate::error::config_error::ConfigValidationSnafu;
use cocode_protocol::Capability;
use cocode_protocol::ModelInfo;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;

// Re-export from protocol for backwards compatibility.
pub use cocode_protocol::ProviderInfo;
pub use cocode_protocol::ProviderType;
pub use cocode_protocol::WireApi;

/// Internal storage for model configurations.
///
/// **Important**: External config files use **array format**:
/// ```json
/// [{"slug": "gpt-5", "display_name": "GPT-5", ...}]
/// ```
///
/// This struct is populated by `ConfigLoader` which deserializes the array
/// and converts it to a HashMap keyed by `slug`.
///
/// Do NOT deserialize config files directly into this type.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelsFile {
    /// Map of model slug to model configuration.
    #[serde(default)]
    pub models: HashMap<String, ModelInfo>,
}

impl ModelsFile {
    /// Add models from a list, error on duplicate slug.
    ///
    /// Each model in the list is keyed by its `slug` field.
    /// Returns an error if a model with the same slug already exists.
    pub fn add_models(
        &mut self,
        models: Vec<ModelInfo>,
        source: impl std::fmt::Display,
    ) -> Result<(), crate::error::ConfigError> {
        for model in models {
            if self.models.contains_key(&model.slug) {
                return ConfigValidationSnafu {
                    file: source.to_string(),
                    message: format!("duplicate model slug: {}", model.slug),
                }
                .fail();
            }
            self.models.insert(model.slug.clone(), model);
        }
        Ok(())
    }
}

/// Internal storage for provider configurations.
///
/// **Important**: External config files use **array format**:
/// ```json
/// [{"name": "openai", "type": "openai", "base_url": "...", ...}]
/// ```
///
/// This struct is populated by `ConfigLoader` which deserializes the array
/// and converts it to a HashMap keyed by `name`.
///
/// Do NOT deserialize config files directly into this type.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProvidersFile {
    /// Map of provider name (identifier) to provider configuration.
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
}

impl ProvidersFile {
    /// Add providers from a list, error on duplicate name.
    ///
    /// Each provider in the list is keyed by its `name` field.
    /// Returns an error if a provider with the same name already exists.
    pub fn add_providers(
        &mut self,
        providers: Vec<ProviderConfig>,
        source: impl std::fmt::Display,
    ) -> Result<(), crate::error::ConfigError> {
        for provider in providers {
            if self.providers.contains_key(&provider.name) {
                return ConfigValidationSnafu {
                    file: source.to_string(),
                    message: format!("duplicate provider name: {}", provider.name),
                }
                .fail();
            }
            self.providers.insert(provider.name.clone(), provider);
        }
        Ok(())
    }
}

fn default_timeout() -> i64 {
    600
}

fn default_true() -> bool {
    true
}

/// Provider configuration from JSON/TOML.
///
/// Example TOML:
/// ```toml
/// [providers.openai]
/// name = "OpenAI"
/// type = "openai"
/// base_url = "https://api.openai.com/v1"
/// env_key = "OPENAI_API_KEY"
/// streaming = true
/// wire_api = "responses"
///
/// [[providers.openai.models]]
/// slug = "gpt-5"
///
/// [[providers.openai.models]]
/// slug = "gpt-4o"
/// timeout_secs = 120
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Provider identifier (used as map key, e.g., "openai", "anthropic").
    pub name: String,

    /// Provider type for selecting the implementation.
    #[serde(rename = "type")]
    pub provider_type: ProviderType,

    /// Base URL for API endpoint.
    pub base_url: String,

    /// Request timeout in seconds (default: 600).
    /// Note: Can be overridden per-model via ModelInfo.timeout_secs.
    #[serde(default = "default_timeout")]
    pub timeout_secs: i64,

    /// Environment variable name for API key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env_key: Option<String>,

    /// API key (prefer env_key for security).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    /// Enable streaming mode (default: true).
    #[serde(default = "default_true")]
    pub streaming: bool,

    /// Wire protocol (responses or chat, default: responses).
    #[serde(default)]
    pub wire_api: WireApi,

    /// Models this provider serves.
    #[serde(default)]
    pub models: Vec<ProviderModelEntry>,

    /// Provider-specific SDK client options (e.g., organization_id, use_zhipuai).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<serde_json::Value>,

    /// HTTP interceptors to apply to requests.
    ///
    /// Interceptors are applied in order of their priority (lower = earlier).
    /// Available built-in interceptors:
    /// - `byted_model_hub`: Adds session_id to "extra" header for ByteDance ModelHub
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub interceptors: Vec<String>,
}

impl ProviderConfig {
    /// Validate required fields.
    pub fn validate(&self) -> Result<(), String> {
        if self.name.is_empty() {
            return Err("provider name is required".to_string());
        }
        if self.base_url.is_empty() {
            return Err("provider base_url is required".to_string());
        }
        Ok(())
    }

    /// Convert to domain type (partial, without resolved API key or models).
    ///
    /// Use `ConfigResolver::resolve_provider()` to get a fully resolved `ProviderInfo`.
    pub fn to_provider_info(&self) -> cocode_protocol::ProviderInfo {
        cocode_protocol::ProviderInfo::new(&self.name, self.provider_type, &self.base_url)
            .with_timeout(self.timeout_secs)
            .with_streaming(self.streaming)
            .with_wire_api(self.wire_api)
    }

    /// Find a model entry by slug.
    pub fn find_model(&self, slug: &str) -> Option<&ProviderModelEntry> {
        self.models.iter().find(|m| m.model_info.slug == slug)
    }

    /// List all model slugs in this provider.
    pub fn list_model_slugs(&self) -> Vec<&str> {
        self.models
            .iter()
            .map(|m| m.model_info.slug.as_str())
            .collect()
    }
}

/// Per-model configuration within a provider.
///
/// Uses `#[serde(flatten)]` to allow inline ModelInfo fields in config files.
///
/// Example TOML:
/// ```toml
/// [[providers.volcengine.models]]
/// slug = "deepseek-r1"
/// model_id = "ep-20250101-xxxxx"  # API endpoint ID
/// timeout_secs = 300
/// max_output_tokens = 16384
/// thinking_budget = 32000
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderModelEntry {
    /// Model info (slug required, other fields optional for overrides).
    #[serde(flatten)]
    pub model_info: ModelInfo,

    /// API model name if different from slug (e.g., "ep-xxx" endpoint ID).
    /// In config files, can use `model_id` or `model_alias`.
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "model_id")]
    pub model_alias: Option<String>,

    /// Model-specific options (temperature, seed, etc.).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub model_options: HashMap<String, serde_json::Value>,
}

impl ProviderModelEntry {
    /// Create a new entry with just a slug.
    pub fn new(slug: impl Into<String>) -> Self {
        Self {
            model_info: ModelInfo {
                slug: slug.into(),
                ..Default::default()
            },
            model_alias: None,
            model_options: HashMap::new(),
        }
    }

    /// Create a new entry with a slug and alias.
    pub fn with_alias(slug: impl Into<String>, alias: impl Into<String>) -> Self {
        Self {
            model_info: ModelInfo {
                slug: slug.into(),
                ..Default::default()
            },
            model_alias: Some(alias.into()),
            model_options: HashMap::new(),
        }
    }

    /// Get the slug (model identifier).
    pub fn slug(&self) -> &str {
        &self.model_info.slug
    }

    /// Get the API model name (alias if set and non-empty, otherwise slug).
    pub fn api_model_name(&self) -> &str {
        self.model_alias
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(&self.model_info.slug)
    }
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
    fn test_models_file_from_vec() {
        // External config files use Vec format (array of models).
        // This tests the real config file format and add_models() method.
        let json = r#"[
            {
                "slug": "gpt-4o",
                "display_name": "GPT-4o",
                "context_window": 128000,
                "max_output_tokens": 16384,
                "capabilities": ["text_generation", "streaming", "vision"]
            }
        ]"#;

        let models: Vec<ModelInfo> = serde_json::from_str(json).expect("deserialize");
        let mut file = ModelsFile::default();
        file.add_models(models, "test.json").expect("add models");

        assert!(file.models.contains_key("gpt-4o"));
        let model = file.models.get("gpt-4o").expect("model exists");
        assert_eq!(model.display_name, Some("GPT-4o".to_string()));
        assert_eq!(model.context_window, Some(128000));
    }

    #[test]
    fn test_providers_file_from_vec() {
        // External config files use Vec format (array of providers).
        // This tests the real config file format and add_providers() method.
        let json = r#"[
            {
                "name": "openai",
                "type": "openai",
                "env_key": "OPENAI_API_KEY",
                "base_url": "https://api.openai.com/v1",
                "models": []
            }
        ]"#;

        let providers: Vec<ProviderConfig> = serde_json::from_str(json).expect("deserialize");
        let mut file = ProvidersFile::default();
        file.add_providers(providers, "test.json")
            .expect("add providers");

        let provider = file.providers.get("openai").expect("provider exists");
        assert_eq!(provider.name, "openai");
        assert_eq!(provider.provider_type, ProviderType::Openai);
    }

    #[test]
    fn test_provider_model_entry_serde() {
        let json = r#"{
            "slug": "deepseek-r1",
            "model_id": "ep-20250101-xxxxx",
            "timeout_secs": 300,
            "max_output_tokens": 16384,
            "default_thinking_level": {"effort": "high", "budget_tokens": 32000}
        }"#;

        let entry: ProviderModelEntry = serde_json::from_str(json).expect("deserialize");
        assert_eq!(entry.slug(), "deepseek-r1");
        assert_eq!(entry.model_alias, Some("ep-20250101-xxxxx".to_string()));
        assert_eq!(entry.model_info.timeout_secs, Some(300));
        assert_eq!(entry.model_info.max_output_tokens, Some(16384));
        let level = entry.model_info.default_thinking_level.unwrap();
        assert_eq!(level.budget_tokens, Some(32000));
    }

    #[test]
    fn test_provider_model_entry_api_model_name() {
        let entry1 = ProviderModelEntry::new("gpt-5");
        assert_eq!(entry1.api_model_name(), "gpt-5");

        let entry2 = ProviderModelEntry::with_alias("deepseek-r1", "ep-xxxxx");
        assert_eq!(entry2.api_model_name(), "ep-xxxxx");
    }

    #[test]
    fn test_provider_model_entry_empty_alias_falls_back() {
        let entry = ProviderModelEntry {
            model_info: ModelInfo {
                slug: "test-model".to_string(),
                ..Default::default()
            },
            model_alias: Some("".to_string()),
            model_options: HashMap::new(),
        };
        // Empty alias should fall back to slug
        assert_eq!(entry.api_model_name(), "test-model");
    }

    #[test]
    fn test_wire_api_serde() {
        let api1 = WireApi::Responses;
        let json1 = serde_json::to_string(&api1).unwrap();
        assert_eq!(json1, "\"responses\"");

        let api2 = WireApi::Chat;
        let json2 = serde_json::to_string(&api2).unwrap();
        assert_eq!(json2, "\"chat\"");
    }

    #[test]
    fn test_provider_config_with_models() {
        let json = r#"{
            "name": "Custom OpenAI",
            "type": "openai",
            "base_url": "https://api.openai.com/v1",
            "env_key": "OPENAI_API_KEY",
            "streaming": true,
            "wire_api": "chat",
            "models": [
                {"slug": "gpt-5"},
                {"slug": "gpt-4o", "timeout_secs": 120}
            ]
        }"#;

        let config: ProviderConfig = serde_json::from_str(json).expect("deserialize");
        assert_eq!(config.name, "Custom OpenAI");
        assert!(config.streaming);
        assert_eq!(config.wire_api, WireApi::Chat);
        assert_eq!(config.models.len(), 2);

        // Check model lookup
        let gpt5 = config.find_model("gpt-5").expect("gpt-5 exists");
        assert_eq!(gpt5.slug(), "gpt-5");

        let gpt4o = config.find_model("gpt-4o").expect("gpt-4o exists");
        assert_eq!(gpt4o.model_info.timeout_secs, Some(120));
    }
}
