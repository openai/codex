//! Provider trait for AI service providers.

use crate::capability::ModelInfo;
use crate::error::HyperError;
use crate::model::Model;
use async_trait::async_trait;
use std::fmt::Debug;
use std::sync::Arc;

/// A provider for AI models.
///
/// Providers are responsible for creating model instances and listing
/// available models. Each provider represents a specific AI service
/// (OpenAI, Anthropic, Google, etc.).
#[async_trait]
pub trait Provider: Send + Sync + Debug {
    /// Get the provider name (e.g., "openai", "anthropic", "gemini").
    fn name(&self) -> &str;

    /// Get a model instance by ID.
    ///
    /// Returns an error if the model is not found or not supported.
    #[must_use = "this returns a Result that must be handled"]
    fn model(&self, model_id: &str) -> Result<Arc<dyn Model>, HyperError>;

    /// List available models from this provider.
    ///
    /// The default implementation returns an empty list.
    /// Providers should override this to return their available models.
    #[must_use = "this returns a Result that must be handled"]
    async fn list_models(&self) -> Result<Vec<ModelInfo>, HyperError> {
        Ok(vec![])
    }

    /// Check if this provider supports a specific model.
    fn supports_model(&self, model_id: &str) -> bool {
        self.model(model_id).is_ok()
    }
}

/// Configuration for creating a provider.
#[derive(Debug, Clone, Default)]
pub struct ProviderConfig {
    /// API key for authentication.
    pub api_key: Option<String>,
    /// Base URL override.
    pub base_url: Option<String>,
    /// Organization ID (for providers that support it).
    pub organization_id: Option<String>,
    /// Default model to use.
    pub default_model: Option<String>,
    /// Request timeout in seconds.
    pub timeout_secs: Option<i64>,
    /// Additional provider-specific configuration.
    pub extra: Option<serde_json::Value>,
}

impl ProviderConfig {
    /// Create a new provider config with an API key.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: Some(api_key.into()),
            ..Default::default()
        }
    }

    /// Set the base URL.
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Set the organization ID.
    pub fn with_organization_id(mut self, org_id: impl Into<String>) -> Self {
        self.organization_id = Some(org_id.into());
        self
    }

    /// Set the default model.
    pub fn with_default_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = Some(model.into());
        self
    }

    /// Set the request timeout.
    pub fn with_timeout(mut self, secs: i64) -> Self {
        self.timeout_secs = Some(secs);
        self
    }

    /// Set extra configuration.
    pub fn with_extra(mut self, extra: serde_json::Value) -> Self {
        self.extra = Some(extra);
        self
    }

    /// Get the API key, returning an error if not set.
    #[must_use = "this returns a Result that must be handled"]
    pub fn require_api_key(&self) -> Result<&str, HyperError> {
        self.api_key
            .as_deref()
            .ok_or_else(|| HyperError::ConfigError("API key is required".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_config_builder() {
        let config = ProviderConfig::new("sk-test-key")
            .with_base_url("https://api.example.com")
            .with_organization_id("org-123")
            .with_default_model("gpt-4o")
            .with_timeout(30);

        assert_eq!(config.api_key, Some("sk-test-key".to_string()));
        assert_eq!(config.base_url, Some("https://api.example.com".to_string()));
        assert_eq!(config.organization_id, Some("org-123".to_string()));
        assert_eq!(config.default_model, Some("gpt-4o".to_string()));
        assert_eq!(config.timeout_secs, Some(30));
    }

    #[test]
    fn test_require_api_key() {
        let config = ProviderConfig::new("sk-test");
        assert!(config.require_api_key().is_ok());

        let config = ProviderConfig::default();
        assert!(config.require_api_key().is_err());
    }
}
