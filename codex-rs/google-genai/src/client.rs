//! Google Generative AI (Gemini) API Client.
//!
//! This module provides the main client for interacting with the Gemini API.

use crate::error::GenAiError;
use crate::error::Result;
use crate::stream::ContentStream;
use crate::stream::parse_sse_stream;
use crate::types::Content;
use crate::types::ErrorResponse;
use crate::types::GenerateContentConfig;
use crate::types::GenerateContentRequest;
use crate::types::GenerateContentResponse;
use crate::types::GenerationConfig;
use crate::types::RequestExtensions;
use crate::types::Tool;
use reqwest::header::CONTENT_TYPE;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;
use std::env;
use std::str::FromStr;
use tracing::debug;
use tracing::instrument;

/// Base URL for the Gemini API.
const GEMINI_API_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

/// Default API version.
const DEFAULT_API_VERSION: &str = "v1beta";

/// Client configuration.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// API key for authentication.
    pub api_key: Option<String>,

    /// Base URL for the API.
    pub base_url: Option<String>,

    /// API version.
    pub api_version: Option<String>,

    /// Request timeout in seconds.
    pub timeout_secs: Option<u64>,

    /// Default extensions for all requests.
    pub extensions: Option<RequestExtensions>,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            base_url: None,
            api_version: None,
            timeout_secs: Some(600), // 10 minutes default
            extensions: None,
        }
    }
}

impl ClientConfig {
    /// Create a new config with API key.
    pub fn with_api_key(api_key: impl Into<String>) -> Self {
        Self {
            api_key: Some(api_key.into()),
            ..Default::default()
        }
    }

    /// Set the base URL.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Set the API version.
    pub fn api_version(mut self, version: impl Into<String>) -> Self {
        self.api_version = Some(version.into());
        self
    }

    /// Set the request timeout.
    pub fn timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = Some(secs);
        self
    }

    /// Set default extensions for all requests.
    pub fn extensions(mut self, ext: RequestExtensions) -> Self {
        self.extensions = Some(ext);
        self
    }
}

/// Google Generative AI (Gemini) API Client.
#[derive(Debug, Clone)]
pub struct Client {
    /// HTTP client.
    http_client: reqwest::Client,

    /// API key.
    api_key: String,

    /// Base URL.
    base_url: String,

    /// API version (reserved for future use).
    #[allow(dead_code)]
    api_version: String,

    /// Default extensions for all requests.
    default_extensions: Option<RequestExtensions>,
}

impl Client {
    /// Create a new client with the given configuration.
    pub fn new(config: ClientConfig) -> Result<Self> {
        // Get API key from config or environment
        let api_key = config
            .api_key
            .or_else(|| env::var("GOOGLE_API_KEY").ok())
            .or_else(|| env::var("GEMINI_API_KEY").ok())
            .ok_or_else(|| {
                GenAiError::Configuration(
                    "API key not provided. Set GOOGLE_API_KEY or GEMINI_API_KEY environment variable, or pass api_key in config.".to_string()
                )
            })?;

        let base_url = config
            .base_url
            .unwrap_or_else(|| GEMINI_API_BASE_URL.to_string());

        let api_version = config
            .api_version
            .unwrap_or_else(|| DEFAULT_API_VERSION.to_string());

        // Build HTTP client
        let mut builder = reqwest::Client::builder();
        if let Some(timeout) = config.timeout_secs {
            builder = builder.timeout(std::time::Duration::from_secs(timeout));
        }
        let http_client = builder
            .build()
            .map_err(|e| GenAiError::Configuration(format!("Failed to create HTTP client: {e}")))?;

        Ok(Self {
            http_client,
            api_key,
            base_url,
            api_version,
            default_extensions: config.extensions,
        })
    }

    /// Create a new client from environment variables.
    pub fn from_env() -> Result<Self> {
        Self::new(ClientConfig::default())
    }

    /// Create a new client with an API key.
    pub fn with_api_key(api_key: impl Into<String>) -> Result<Self> {
        Self::new(ClientConfig::with_api_key(api_key))
    }

    /// Get the base URL with API version.
    fn get_base_url(&self) -> String {
        self.base_url.trim_end_matches('/').to_string()
    }

    /// Build the URL for a model endpoint.
    fn model_url(&self, model: &str, endpoint: &str) -> String {
        let base = self.get_base_url();
        let model_path = if model.starts_with("models/") {
            model.to_string()
        } else {
            format!("models/{model}")
        };
        format!("{base}/{model_path}:{endpoint}")
    }

    /// Build default headers.
    fn build_headers(&self) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            "x-goog-api-key",
            HeaderValue::from_str(&self.api_key)
                .map_err(|e| GenAiError::Configuration(format!("Invalid API key format: {e}")))?,
        );
        Ok(headers)
    }

    /// Merge client and config extensions.
    /// Order: client.ext → config.ext (config takes precedence)
    fn merge_extensions(
        &self,
        config_ext: Option<&RequestExtensions>,
    ) -> Option<RequestExtensions> {
        match (&self.default_extensions, config_ext) {
            (None, None) => None,
            (Some(c), None) => Some(c.clone()),
            (None, Some(r)) => Some(r.clone()),
            (Some(c), Some(r)) => Some(c.merge(r)),
        }
    }

    /// Build URL with optional query params from extensions.
    fn build_url_with_params(
        &self,
        base_url: &str,
        config_ext: Option<&RequestExtensions>,
    ) -> String {
        let merged = self.merge_extensions(config_ext);
        if let Some(params) = merged.as_ref().and_then(|e| e.params.as_ref()) {
            if !params.is_empty() {
                let query: String = params
                    .iter()
                    .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
                    .collect::<Vec<_>>()
                    .join("&");
                return if base_url.contains('?') {
                    format!("{base_url}&{query}")
                } else {
                    format!("{base_url}?{query}")
                };
            }
        }
        base_url.to_string()
    }

    /// Build headers with extensions applied.
    fn build_headers_with_ext(&self, config_ext: Option<&RequestExtensions>) -> Result<HeaderMap> {
        let mut headers = self.build_headers()?;

        // Apply merged extensions
        let merged = self.merge_extensions(config_ext);
        if let Some(ext_headers) = merged.as_ref().and_then(|e| e.headers.as_ref()) {
            for (k, v) in ext_headers {
                headers.insert(
                    HeaderName::from_str(k).map_err(|e| {
                        GenAiError::Configuration(format!("Invalid header name: {e}"))
                    })?,
                    HeaderValue::from_str(v).map_err(|e| {
                        GenAiError::Configuration(format!("Invalid header value: {e}"))
                    })?,
                );
            }
        }

        Ok(headers)
    }

    /// Merge ext_body into request body (3-layer merge).
    /// Order: original_body → client.ext.body → config.ext.body
    fn merge_body(
        &self,
        mut body: serde_json::Value,
        config_ext: Option<&RequestExtensions>,
    ) -> serde_json::Value {
        // Merge extensions first, then apply to body
        let merged = self.merge_extensions(config_ext);
        if let Some(ext_body) = merged.as_ref().and_then(|e| e.body.as_ref()) {
            if let (Some(body_obj), Some(ext_obj)) = (body.as_object_mut(), ext_body.as_object()) {
                for (k, v) in ext_obj {
                    body_obj.insert(k.clone(), v.clone());
                }
            }
        }
        body
    }

    /// Generate content using the specified model.
    #[instrument(skip(self, contents, config), fields(model = %model))]
    pub async fn generate_content(
        &self,
        model: &str,
        contents: Vec<Content>,
        config: Option<GenerateContentConfig>,
    ) -> Result<GenerateContentResponse> {
        // Extract extensions before consuming config
        let ext = config.as_ref().and_then(|c| c.extensions.as_ref());

        // Build URL with extension params
        let base_url = self.model_url(model, "generateContent");
        let url = self.build_url_with_params(&base_url, ext);
        debug!(url = %url, "Sending generate content request");

        // Build request body
        let config = config.unwrap_or_default();

        // Build generation config only if there are any generation parameters
        let generation_config = if config.has_generation_params() {
            Some(GenerationConfig {
                temperature: config.temperature,
                top_p: config.top_p,
                top_k: config.top_k,
                max_output_tokens: config.max_output_tokens,
                candidate_count: config.candidate_count,
                stop_sequences: config.stop_sequences,
                response_logprobs: config.response_logprobs,
                logprobs: config.logprobs,
                response_mime_type: config.response_mime_type,
                response_schema: config.response_schema,
                presence_penalty: config.presence_penalty,
                frequency_penalty: config.frequency_penalty,
                seed: config.seed,
                response_modalities: config.response_modalities,
                thinking_config: config.thinking_config,
            })
        } else {
            None
        };

        let request = GenerateContentRequest {
            contents,
            system_instruction: config.system_instruction,
            generation_config,
            safety_settings: config.safety_settings,
            tools: config.tools,
            tool_config: config.tool_config,
        };

        // Build headers with extensions
        let headers = self.build_headers_with_ext(config.extensions.as_ref())?;

        // Serialize request and merge ext_body
        let mut body = serde_json::to_value(&request)
            .map_err(|e| GenAiError::Parse(format!("Failed to serialize request: {e}")))?;
        body = self.merge_body(body, config.extensions.as_ref());

        let response = self
            .http_client
            .post(&url)
            .headers(headers)
            .body(body.to_string())
            .send()
            .await
            .map_err(|e| GenAiError::Network(e.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| GenAiError::Network(e.to_string()))?;

        debug!(status = %status, "Received response");

        if !status.is_success() {
            // Try to parse error response
            if let Ok(error_response) = serde_json::from_str::<ErrorResponse>(&body) {
                return Err(GenAiError::Api {
                    code: error_response.error.code,
                    message: error_response.error.message,
                    status: error_response.error.status,
                });
            }
            return Err(GenAiError::Api {
                code: status.as_u16() as i32,
                message: body,
                status: status.to_string(),
            });
        }

        serde_json::from_str(&body)
            .map_err(|e| GenAiError::Parse(format!("Failed to parse response: {e}\nBody: {body}")))
    }

    /// Generate content with a simple text prompt.
    pub async fn generate_content_text(
        &self,
        model: &str,
        prompt: &str,
        config: Option<GenerateContentConfig>,
    ) -> Result<GenerateContentResponse> {
        self.generate_content(model, vec![Content::user(prompt)], config)
            .await
    }

    /// Generate content with tools (function calling).
    pub async fn generate_content_with_tools(
        &self,
        model: &str,
        contents: Vec<Content>,
        tools: Vec<Tool>,
        config: Option<GenerateContentConfig>,
    ) -> Result<GenerateContentResponse> {
        let mut config = config.unwrap_or_default();
        config.tools = Some(tools);
        self.generate_content(model, contents, Some(config)).await
    }

    /// Generate content with streaming response.
    ///
    /// Returns a stream of `GenerateContentResponse` chunks. Each chunk contains
    /// partial content that should be accumulated by the caller.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use google_genai::Client;
    /// use futures::StreamExt;
    ///
    /// # async fn example() -> anyhow::Result<()> {
    /// let client = Client::from_env()?;
    /// let mut stream = client
    ///     .generate_content_stream("gemini-2.0-flash", vec![], None)
    ///     .await?;
    ///
    /// while let Some(chunk) = stream.next().await {
    ///     match chunk {
    ///         Ok(response) => {
    ///             if let Some(text) = response.text() {
    ///                 print!("{}", text);
    ///             }
    ///         }
    ///         Err(e) => eprintln!("Error: {}", e),
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(skip(self, contents, config), fields(model = %model))]
    pub async fn generate_content_stream(
        &self,
        model: &str,
        contents: Vec<Content>,
        config: Option<GenerateContentConfig>,
    ) -> Result<ContentStream> {
        // Extract extensions before consuming config
        let ext = config.as_ref().and_then(|c| c.extensions.as_ref());

        // Note: ?alt=sse is required for SSE streaming format (matches Python SDK)
        let base_url = format!("{}?alt=sse", self.model_url(model, "streamGenerateContent"));
        let url = self.build_url_with_params(&base_url, ext);
        debug!(url = %url, "Sending streaming generate content request");

        // Build request body (same as non-streaming)
        let (request, config_ext) = self.build_generate_request_with_ext(contents, config);

        // Build headers with extensions
        let headers = self.build_headers_with_ext(config_ext.as_ref())?;

        // Serialize request and merge ext_body
        let mut body = serde_json::to_value(&request)
            .map_err(|e| GenAiError::Parse(format!("Failed to serialize request: {e}")))?;
        body = self.merge_body(body, config_ext.as_ref());

        let response = self
            .http_client
            .post(&url)
            .headers(headers)
            .body(body.to_string())
            .send()
            .await
            .map_err(|e| GenAiError::Network(e.to_string()))?;

        let status = response.status();

        if !status.is_success() {
            // For error responses, read the body and parse error
            let body = response
                .text()
                .await
                .map_err(|e| GenAiError::Network(e.to_string()))?;

            if let Ok(error_response) = serde_json::from_str::<ErrorResponse>(&body) {
                return Err(GenAiError::Api {
                    code: error_response.error.code,
                    message: error_response.error.message,
                    status: error_response.error.status,
                });
            }
            return Err(GenAiError::Api {
                code: status.as_u16() as i32,
                message: body,
                status: status.to_string(),
            });
        }

        // Return SSE stream
        Ok(parse_sse_stream(response.bytes_stream()))
    }

    /// Generate content with streaming and simple text prompt.
    pub async fn generate_content_stream_text(
        &self,
        model: &str,
        prompt: &str,
        config: Option<GenerateContentConfig>,
    ) -> Result<ContentStream> {
        self.generate_content_stream(model, vec![Content::user(prompt)], config)
            .await
    }

    /// Generate content with streaming and tools.
    pub async fn generate_content_stream_with_tools(
        &self,
        model: &str,
        contents: Vec<Content>,
        tools: Vec<Tool>,
        config: Option<GenerateContentConfig>,
    ) -> Result<ContentStream> {
        let mut config = config.unwrap_or_default();
        config.tools = Some(tools);
        self.generate_content_stream(model, contents, Some(config))
            .await
    }

    /// Build generate content request body with extensions (shared between streaming and non-streaming).
    /// Returns both the request and the extensions for further processing.
    fn build_generate_request_with_ext(
        &self,
        contents: Vec<Content>,
        config: Option<GenerateContentConfig>,
    ) -> (GenerateContentRequest, Option<RequestExtensions>) {
        let config = config.unwrap_or_default();

        // Build generation config only if there are any generation parameters
        let generation_config = if config.has_generation_params() {
            Some(GenerationConfig {
                temperature: config.temperature,
                top_p: config.top_p,
                top_k: config.top_k,
                max_output_tokens: config.max_output_tokens,
                candidate_count: config.candidate_count,
                stop_sequences: config.stop_sequences.clone(),
                response_logprobs: config.response_logprobs,
                logprobs: config.logprobs,
                response_mime_type: config.response_mime_type.clone(),
                response_schema: config.response_schema.clone(),
                presence_penalty: config.presence_penalty,
                frequency_penalty: config.frequency_penalty,
                seed: config.seed,
                response_modalities: config.response_modalities.clone(),
                thinking_config: config.thinking_config.clone(),
            })
        } else {
            None
        };

        let request = GenerateContentRequest {
            contents,
            system_instruction: config.system_instruction,
            generation_config,
            safety_settings: config.safety_settings,
            tools: config.tools,
            tool_config: config.tool_config,
        };

        (request, config.extensions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_config_default() {
        let config = ClientConfig::default();
        assert!(config.api_key.is_none());
        assert!(config.base_url.is_none());
        assert_eq!(config.timeout_secs, Some(600));
    }

    #[test]
    fn test_client_config_with_api_key() {
        let config = ClientConfig::with_api_key("test-key");
        assert_eq!(config.api_key, Some("test-key".to_string()));
    }

    #[test]
    fn test_model_url() {
        let client = Client {
            http_client: reqwest::Client::new(),
            api_key: "test".to_string(),
            base_url: GEMINI_API_BASE_URL.to_string(),
            api_version: DEFAULT_API_VERSION.to_string(),
            default_extensions: None,
        };

        assert_eq!(
            client.model_url("gemini-2.0-flash", "generateContent"),
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent"
        );

        assert_eq!(
            client.model_url("models/gemini-2.0-flash", "generateContent"),
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent"
        );
    }

    #[test]
    fn test_model_url_streaming() {
        let client = Client {
            http_client: reqwest::Client::new(),
            api_key: "test".to_string(),
            base_url: GEMINI_API_BASE_URL.to_string(),
            api_version: DEFAULT_API_VERSION.to_string(),
            default_extensions: None,
        };

        // Base URL without ?alt=sse (added by generate_content_stream)
        assert_eq!(
            client.model_url("gemini-2.0-flash", "streamGenerateContent"),
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:streamGenerateContent"
        );

        // Full streaming URL (as used in generate_content_stream)
        let streaming_url = format!(
            "{}?alt=sse",
            client.model_url("gemini-2.0-flash", "streamGenerateContent")
        );
        assert_eq!(
            streaming_url,
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:streamGenerateContent?alt=sse"
        );
    }

    #[test]
    fn test_model_url_custom_base() {
        let client = Client {
            http_client: reqwest::Client::new(),
            api_key: "test".to_string(),
            base_url: "https://search.bytedance.net/gpt/openapi/online/multimodal/crawl/google/v1"
                .to_string(),
            api_version: "v1".to_string(),
            default_extensions: None,
        };

        assert_eq!(
            client.model_url("gemini-2.5-flash", "generateContent"),
            "https://search.bytedance.net/gpt/openapi/online/multimodal/crawl/google/v1/models/gemini-2.5-flash:generateContent"
        );

        assert_eq!(
            client.model_url("gemini-2.5-flash", "streamGenerateContent"),
            "https://search.bytedance.net/gpt/openapi/online/multimodal/crawl/google/v1/models/gemini-2.5-flash:streamGenerateContent"
        );
    }
}
