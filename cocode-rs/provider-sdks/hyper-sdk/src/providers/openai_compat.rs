//! OpenAI-compatible provider for third-party services.
//!
//! This provider can be used with any service that implements the OpenAI API,
//! such as Azure OpenAI, local LLM servers, or other cloud providers.

use crate::error::HyperError;
use crate::model::Model;
use crate::provider::Provider;
use async_trait::async_trait;
use std::sync::Arc;

/// Configuration for an OpenAI-compatible provider.
#[derive(Debug, Clone)]
pub struct OpenAICompatConfig {
    /// Provider name (used for identification).
    pub name: String,
    /// API key.
    pub api_key: String,
    /// Base URL (required).
    pub base_url: String,
    /// API version (for Azure).
    pub api_version: Option<String>,
    /// Request timeout in seconds.
    pub timeout_secs: i64,
}

impl Default for OpenAICompatConfig {
    fn default() -> Self {
        Self {
            name: "openai_compat".to_string(),
            api_key: String::new(),
            base_url: String::new(),
            api_version: None,
            timeout_secs: 600,
        }
    }
}

/// OpenAI-compatible provider.
#[derive(Debug)]
pub struct OpenAICompatProvider {
    config: OpenAICompatConfig,
    client: reqwest::Client,
}

impl OpenAICompatProvider {
    /// Create a new OpenAI-compatible provider.
    pub fn new(config: OpenAICompatConfig) -> Result<Self, HyperError> {
        if config.base_url.is_empty() {
            return Err(HyperError::ConfigError(
                "Base URL is required for OpenAI-compatible providers".to_string(),
            ));
        }

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs as u64))
            .build()
            .map_err(|e| HyperError::ConfigError(format!("Failed to create HTTP client: {e}")))?;

        Ok(Self { config, client })
    }

    /// Create a builder for configuring the provider.
    pub fn builder(name: impl Into<String>) -> OpenAICompatProviderBuilder {
        OpenAICompatProviderBuilder::new(name)
    }

    /// Get a reference to the HTTP client.
    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    /// Get the API key.
    pub fn api_key(&self) -> &str {
        &self.config.api_key
    }

    /// Get the base URL.
    pub fn base_url(&self) -> &str {
        &self.config.base_url
    }
}

#[async_trait]
impl Provider for OpenAICompatProvider {
    fn name(&self) -> &str {
        &self.config.name
    }

    fn model(&self, model_id: &str) -> Result<Arc<dyn Model>, HyperError> {
        Ok(Arc::new(OpenAICompatModel {
            model_id: model_id.to_string(),
            config: self.config.clone(),
            client: self.client.clone(),
        }))
    }
}

/// Builder for OpenAI-compatible provider.
#[derive(Debug)]
pub struct OpenAICompatProviderBuilder {
    config: OpenAICompatConfig,
}

impl OpenAICompatProviderBuilder {
    /// Create a new builder with a provider name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            config: OpenAICompatConfig {
                name: name.into(),
                ..Default::default()
            },
        }
    }

    /// Set the API key.
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.config.api_key = key.into();
        self
    }

    /// Set the base URL.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.config.base_url = url.into();
        self
    }

    /// Set the API version (for Azure OpenAI).
    pub fn api_version(mut self, version: impl Into<String>) -> Self {
        self.config.api_version = Some(version.into());
        self
    }

    /// Set the request timeout.
    pub fn timeout_secs(mut self, secs: i64) -> Self {
        self.config.timeout_secs = secs;
        self
    }

    /// Build the provider.
    pub fn build(self) -> Result<OpenAICompatProvider, HyperError> {
        OpenAICompatProvider::new(self.config)
    }
}

/// OpenAI-compatible model implementation.
#[derive(Debug)]
struct OpenAICompatModel {
    model_id: String,
    config: OpenAICompatConfig,
    client: reqwest::Client,
}

#[async_trait]
impl Model for OpenAICompatModel {
    fn model_name(&self) -> &str {
        &self.model_id
    }

    fn provider(&self) -> &str {
        &self.config.name
    }

    async fn generate(
        &self,
        request: crate::request::GenerateRequest,
    ) -> Result<crate::response::GenerateResponse, HyperError> {
        // This is a placeholder - real implementation would make HTTP requests
        let _ = (&self.config, &self.client, request);
        Err(HyperError::Internal(
            "OpenAI-compatible generate not yet implemented".to_string(),
        ))
    }

    async fn stream(
        &self,
        request: crate::request::GenerateRequest,
    ) -> Result<crate::stream::StreamResponse, HyperError> {
        let _ = request;
        Err(HyperError::Internal(
            "OpenAI-compatible streaming not yet implemented".to_string(),
        ))
    }
}

// Pre-defined configurations for common providers

impl OpenAICompatProvider {
    /// Create a provider for Azure OpenAI.
    ///
    /// # Arguments
    /// * `endpoint` - Azure endpoint URL (e.g., https://your-resource.openai.azure.com)
    /// * `api_key` - Azure API key
    /// * `api_version` - API version (e.g., "2024-02-15-preview")
    pub fn azure(
        endpoint: impl Into<String>,
        api_key: impl Into<String>,
        api_version: impl Into<String>,
    ) -> Result<Self, HyperError> {
        OpenAICompatProviderBuilder::new("azure")
            .base_url(endpoint)
            .api_key(api_key)
            .api_version(api_version)
            .build()
    }

    /// Create a provider for a local LLM server (e.g., LM Studio, Ollama with OpenAI compat).
    ///
    /// # Arguments
    /// * `base_url` - Local server URL (e.g., http://localhost:1234/v1)
    pub fn local(base_url: impl Into<String>) -> Result<Self, HyperError> {
        OpenAICompatProviderBuilder::new("local")
            .base_url(base_url)
            .api_key("not-needed")
            .build()
    }

    /// Create a provider for Groq.
    ///
    /// # Arguments
    /// * `api_key` - Groq API key
    pub fn groq(api_key: impl Into<String>) -> Result<Self, HyperError> {
        OpenAICompatProviderBuilder::new("groq")
            .base_url("https://api.groq.com/openai/v1")
            .api_key(api_key)
            .build()
    }

    /// Create a provider for Together AI.
    ///
    /// # Arguments
    /// * `api_key` - Together API key
    pub fn together(api_key: impl Into<String>) -> Result<Self, HyperError> {
        OpenAICompatProviderBuilder::new("together")
            .base_url("https://api.together.xyz/v1")
            .api_key(api_key)
            .build()
    }

    /// Create a provider for Fireworks AI.
    ///
    /// # Arguments
    /// * `api_key` - Fireworks API key
    pub fn fireworks(api_key: impl Into<String>) -> Result<Self, HyperError> {
        OpenAICompatProviderBuilder::new("fireworks")
            .base_url("https://api.fireworks.ai/inference/v1")
            .api_key(api_key)
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder() {
        let result = OpenAICompatProvider::builder("custom")
            .api_key("test-key")
            .base_url("https://custom-llm.example.com/v1")
            .timeout_secs(120)
            .build();

        assert!(result.is_ok());
        let provider = result.unwrap();
        assert_eq!(provider.name(), "custom");
        assert_eq!(provider.api_key(), "test-key");
        assert_eq!(provider.base_url(), "https://custom-llm.example.com/v1");
    }

    #[test]
    fn test_builder_missing_url() {
        let result = OpenAICompatProvider::builder("custom")
            .api_key("test-key")
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_azure_constructor() {
        let result = OpenAICompatProvider::azure(
            "https://my-resource.openai.azure.com",
            "azure-key",
            "2024-02-15-preview",
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name(), "azure");
    }

    #[test]
    fn test_local_constructor() {
        let result = OpenAICompatProvider::local("http://localhost:1234/v1");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name(), "local");
    }

    #[test]
    fn test_groq_constructor() {
        let result = OpenAICompatProvider::groq("groq-key");
        assert!(result.is_ok());
        let provider = result.unwrap();
        assert_eq!(provider.name(), "groq");
        assert_eq!(provider.base_url(), "https://api.groq.com/openai/v1");
    }
}
