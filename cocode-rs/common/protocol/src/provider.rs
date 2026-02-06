//! Provider type definitions.
//!
//! This module defines the complete runtime types for providers:
//! - `ProviderType`: Provider type enumeration
//! - `WireApi`: Wire protocol (responses/chat)
//! - `ProviderInfo`: Complete runtime type with resolved API key and models
//!
//! For file loading types (with env_key, etc.), see `ProviderConfig` in config crate.

use crate::ModelInfo;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;

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

/// Wire protocol for API communication.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WireApi {
    /// OpenAI-style responses API.
    #[default]
    Responses,
    /// OpenAI-style chat completions API.
    Chat,
}

impl std::fmt::Display for WireApi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Responses => write!(f, "responses"),
            Self::Chat => write!(f, "chat"),
        }
    }
}

/// Default timeout in seconds.
fn default_timeout() -> i64 {
    600
}

fn default_true() -> bool {
    true
}

/// Model within a provider with deployment-specific info.
///
/// This wraps `ModelInfo` with provider-specific deployment information
/// like `model_alias` (e.g., endpoint IDs for Volcengine).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderModel {
    /// Resolved model info with all layers merged.
    #[serde(flatten)]
    pub info: ModelInfo,

    /// API model name if different from slug (e.g., endpoint ID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_alias: Option<String>,
}

impl ProviderModel {
    /// Create from ModelInfo (no alias).
    pub fn new(info: ModelInfo) -> Self {
        Self {
            info,
            model_alias: None,
        }
    }

    /// Create with an alias.
    pub fn with_alias(info: ModelInfo, alias: impl Into<String>) -> Self {
        Self {
            info,
            model_alias: Some(alias.into()),
        }
    }

    /// Get the API model name (alias if set and non-empty, otherwise slug).
    pub fn api_model_name(&self) -> &str {
        self.model_alias
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(&self.info.slug)
    }

    /// Get the slug (model identifier).
    pub fn slug(&self) -> &str {
        &self.info.slug
    }
}

/// Complete runtime provider type with resolved configuration.
///
/// This is the fully resolved provider with:
/// - Resolved API key (from env or config)
/// - All connection settings
/// - Map of resolved model configurations
///
/// Use `ProviderConfig` (in config crate) for file loading with `env_key`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    // === Identity ===
    /// Human-readable provider name.
    pub name: String,

    /// Provider type for selecting implementation.
    #[serde(rename = "type")]
    pub provider_type: ProviderType,

    // === Connection ===
    /// Base URL for API endpoint.
    pub base_url: String,

    /// Resolved API key (required for communication).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub api_key: String,

    // === Request Options ===
    /// Default request timeout in seconds (can be overridden per-model).
    #[serde(default = "default_timeout")]
    pub timeout_secs: i64,

    /// Enable streaming mode.
    #[serde(default = "default_true")]
    pub streaming: bool,

    /// Wire protocol (responses or chat).
    #[serde(default)]
    pub wire_api: WireApi,

    // === Models ===
    /// Models this provider serves (slug -> ProviderModel with resolved info).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub models: HashMap<String, ProviderModel>,

    // === Options ===
    /// Provider-specific SDK client configuration (e.g., organization_id, use_zhipuai).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<serde_json::Value>,
}

impl ProviderInfo {
    /// Create a new ProviderInfo with required fields.
    pub fn new(
        name: impl Into<String>,
        provider_type: ProviderType,
        base_url: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            provider_type,
            base_url: base_url.into(),
            api_key: String::new(),
            timeout_secs: default_timeout(),
            streaming: true,
            wire_api: WireApi::default(),
            models: HashMap::new(),
            options: None,
        }
    }

    /// Builder: set API key.
    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = key.into();
        self
    }

    /// Builder: set timeout.
    pub fn with_timeout(mut self, secs: i64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Builder: set streaming mode.
    pub fn with_streaming(mut self, streaming: bool) -> Self {
        self.streaming = streaming;
        self
    }

    /// Builder: set wire API.
    pub fn with_wire_api(mut self, wire_api: WireApi) -> Self {
        self.wire_api = wire_api;
        self
    }

    /// Builder: add a model (wraps in ProviderModel without alias).
    pub fn with_model(mut self, slug: impl Into<String>, model: ModelInfo) -> Self {
        self.models.insert(slug.into(), ProviderModel::new(model));
        self
    }

    /// Builder: add a model with alias.
    pub fn with_model_aliased(
        mut self,
        slug: impl Into<String>,
        model: ModelInfo,
        alias: impl Into<String>,
    ) -> Self {
        self.models
            .insert(slug.into(), ProviderModel::with_alias(model, alias));
        self
    }

    /// Builder: add a ProviderModel directly.
    pub fn with_provider_model(mut self, slug: impl Into<String>, model: ProviderModel) -> Self {
        self.models.insert(slug.into(), model);
        self
    }

    /// Builder: set models.
    pub fn with_models(mut self, models: HashMap<String, ProviderModel>) -> Self {
        self.models = models;
        self
    }

    /// Builder: set provider-specific options.
    pub fn with_options(mut self, options: serde_json::Value) -> Self {
        self.options = Some(options);
        self
    }

    /// Find a model by slug.
    pub fn get_model(&self, slug: &str) -> Option<&ProviderModel> {
        self.models.get(slug)
    }

    /// Get the API model name for a slug (alias if set and non-empty, otherwise slug).
    pub fn api_model_name(&self, slug: &str) -> Option<&str> {
        self.models.get(slug).map(|m| m.api_model_name())
    }

    /// List all model slugs.
    pub fn model_slugs(&self) -> Vec<&str> {
        self.models.keys().map(String::as_str).collect()
    }

    /// Get effective timeout for a model (model timeout or provider default).
    pub fn effective_timeout(&self, slug: &str) -> i64 {
        self.models
            .get(slug)
            .and_then(|m| m.info.timeout_secs)
            .unwrap_or(self.timeout_secs)
    }

    /// Check if API key is configured.
    pub fn has_api_key(&self) -> bool {
        !self.api_key.is_empty()
    }
}

impl PartialEq for ProviderInfo {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.provider_type == other.provider_type
            && self.base_url == other.base_url
            && self.api_key == other.api_key
            && self.timeout_secs == other.timeout_secs
            && self.streaming == other.streaming
            && self.wire_api == other.wire_api
            && self.models == other.models
        // Note: options is not compared for equality
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_type_default() {
        assert_eq!(ProviderType::default(), ProviderType::Openai);
    }

    #[test]
    fn test_provider_type_display() {
        assert_eq!(ProviderType::Openai.to_string(), "openai");
        assert_eq!(ProviderType::Anthropic.to_string(), "anthropic");
        assert_eq!(ProviderType::Gemini.to_string(), "gemini");
        assert_eq!(ProviderType::Volcengine.to_string(), "volcengine");
        assert_eq!(ProviderType::Zai.to_string(), "zai");
        assert_eq!(ProviderType::OpenaiCompat.to_string(), "openai_compat");
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
    fn test_wire_api_default() {
        assert_eq!(WireApi::default(), WireApi::Responses);
    }

    #[test]
    fn test_wire_api_display() {
        assert_eq!(WireApi::Responses.to_string(), "responses");
        assert_eq!(WireApi::Chat.to_string(), "chat");
    }

    #[test]
    fn test_wire_api_serde() {
        let api = WireApi::Chat;
        let json = serde_json::to_string(&api).expect("serialize");
        assert_eq!(json, "\"chat\"");

        let parsed: WireApi = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, WireApi::Chat);
    }

    #[test]
    fn test_provider_info_new() {
        let info = ProviderInfo::new("OpenAI", ProviderType::Openai, "https://api.openai.com/v1");
        assert_eq!(info.name, "OpenAI");
        assert_eq!(info.provider_type, ProviderType::Openai);
        assert_eq!(info.base_url, "https://api.openai.com/v1");
        assert_eq!(info.timeout_secs, 600);
        assert!(info.streaming);
        assert_eq!(info.wire_api, WireApi::Responses);
        assert!(info.models.is_empty());
        assert!(!info.has_api_key());
    }

    #[test]
    fn test_provider_info_builders() {
        let model = ModelInfo {
            slug: "gpt-4".to_string(),
            timeout_secs: Some(120),
            ..Default::default()
        };

        let info = ProviderInfo::new("Test", ProviderType::Openai, "https://api.test.com")
            .with_api_key("sk-test-key")
            .with_timeout(300)
            .with_streaming(false)
            .with_wire_api(WireApi::Chat)
            .with_model("gpt-4", model);

        assert_eq!(info.api_key, "sk-test-key");
        assert_eq!(info.timeout_secs, 300);
        assert!(!info.streaming);
        assert_eq!(info.wire_api, WireApi::Chat);
        assert!(info.has_api_key());
        assert!(info.get_model("gpt-4").is_some());
    }

    #[test]
    fn test_provider_info_model_methods() {
        let model1 = ModelInfo {
            slug: "model-1".to_string(),
            timeout_secs: Some(120),
            ..Default::default()
        };
        let model2 = ModelInfo {
            slug: "model-2".to_string(),
            ..Default::default()
        };

        let info = ProviderInfo::new("Test", ProviderType::Openai, "https://api.test.com")
            .with_timeout(600)
            .with_model("model-1", model1)
            .with_model("model-2", model2);

        // get_model
        assert!(info.get_model("model-1").is_some());
        assert!(info.get_model("model-2").is_some());
        assert!(info.get_model("nonexistent").is_none());

        // model_slugs
        let slugs = info.model_slugs();
        assert_eq!(slugs.len(), 2);
        assert!(slugs.contains(&"model-1"));
        assert!(slugs.contains(&"model-2"));

        // effective_timeout
        assert_eq!(info.effective_timeout("model-1"), 120); // Model-specific
        assert_eq!(info.effective_timeout("model-2"), 600); // Provider default
        assert_eq!(info.effective_timeout("nonexistent"), 600); // Provider default
    }

    #[test]
    fn test_provider_info_serde() {
        let info = ProviderInfo::new("Test", ProviderType::Anthropic, "https://api.anthropic.com")
            .with_api_key("test-key")
            .with_streaming(true)
            .with_wire_api(WireApi::Chat);

        let json = serde_json::to_string(&info).expect("serialize");
        assert!(json.contains("\"name\":\"Test\""));
        assert!(json.contains("\"type\":\"anthropic\""));
        assert!(json.contains("\"base_url\":\"https://api.anthropic.com\""));
        assert!(json.contains("\"streaming\":true"));
        assert!(json.contains("\"wire_api\":\"chat\""));

        let parsed: ProviderInfo = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, info);
    }

    #[test]
    fn test_provider_info_serde_defaults() {
        let json = r#"{
            "name": "Test",
            "type": "openai",
            "base_url": "https://api.test.com"
        }"#;
        let info: ProviderInfo = serde_json::from_str(json).expect("deserialize");
        assert_eq!(info.timeout_secs, 600);
        assert!(info.streaming);
        assert_eq!(info.wire_api, WireApi::Responses);
        assert!(info.models.is_empty());
    }

    #[test]
    fn test_provider_info_equality() {
        let info1 = ProviderInfo::new("Test", ProviderType::Openai, "https://api.test.com")
            .with_api_key("key1");
        let info2 = ProviderInfo::new("Test", ProviderType::Openai, "https://api.test.com")
            .with_api_key("key1");
        let info3 = ProviderInfo::new("Test", ProviderType::Openai, "https://api.test.com")
            .with_api_key("key2");

        assert_eq!(info1, info2);
        assert_ne!(info1, info3);
    }

    #[test]
    fn test_provider_model_new() {
        let model_info = ModelInfo {
            slug: "gpt-4".to_string(),
            timeout_secs: Some(120),
            ..Default::default()
        };
        let pm = ProviderModel::new(model_info.clone());

        assert_eq!(pm.slug(), "gpt-4");
        assert_eq!(pm.api_model_name(), "gpt-4"); // No alias, returns slug
        assert!(pm.model_alias.is_none());
        assert_eq!(pm.info.timeout_secs, Some(120));
    }

    #[test]
    fn test_provider_model_with_alias() {
        let model_info = ModelInfo {
            slug: "deepseek-r1".to_string(),
            ..Default::default()
        };
        let pm = ProviderModel::with_alias(model_info, "ep-20250101-xxxxx");

        assert_eq!(pm.slug(), "deepseek-r1");
        assert_eq!(pm.api_model_name(), "ep-20250101-xxxxx"); // Returns alias
        assert_eq!(pm.model_alias, Some("ep-20250101-xxxxx".to_string()));
    }

    #[test]
    fn test_provider_info_api_model_name() {
        let model1 = ModelInfo {
            slug: "model-1".to_string(),
            ..Default::default()
        };
        let model2 = ModelInfo {
            slug: "model-2".to_string(),
            ..Default::default()
        };

        let info = ProviderInfo::new("Test", ProviderType::Openai, "https://api.test.com")
            .with_model("model-1", model1)
            .with_model_aliased("model-2", model2, "endpoint-xxx");

        // api_model_name returns slug if no alias
        assert_eq!(info.api_model_name("model-1"), Some("model-1"));
        // api_model_name returns alias if set
        assert_eq!(info.api_model_name("model-2"), Some("endpoint-xxx"));
        // api_model_name returns None for unknown model
        assert_eq!(info.api_model_name("unknown"), None);
    }

    #[test]
    fn test_provider_model_empty_alias_falls_back_to_slug() {
        let model_info = ModelInfo {
            slug: "gpt-4".to_string(),
            ..Default::default()
        };
        // Create with empty string alias
        let pm = ProviderModel {
            info: model_info,
            model_alias: Some("".to_string()),
        };
        // Should fall back to slug, not return empty string
        assert_eq!(pm.api_model_name(), "gpt-4");
    }

    #[test]
    fn test_provider_model_serde() {
        let model_info = ModelInfo {
            slug: "test-model".to_string(),
            timeout_secs: Some(120),
            ..Default::default()
        };
        let pm = ProviderModel::with_alias(model_info, "ep-xxx");

        let json = serde_json::to_string(&pm).expect("serialize");
        assert!(json.contains("\"slug\":\"test-model\""));
        assert!(json.contains("\"model_alias\":\"ep-xxx\""));

        let parsed: ProviderModel = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.slug(), "test-model");
        assert_eq!(parsed.api_model_name(), "ep-xxx");
    }
}
