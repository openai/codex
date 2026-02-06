//! Z.AI / ZhipuAI provider implementation.

use crate::error::HyperError;
use crate::messages::ContentBlock;
use crate::messages::ImageSource;
use crate::messages::Role;
use crate::model::Model;
use crate::options::ZaiOptions;
use crate::options::downcast_options;
use crate::provider::Provider;
use crate::provider::ProviderConfig;
use crate::request::GenerateRequest;
use crate::response::FinishReason;
use crate::response::GenerateResponse;
use crate::response::TokenUsage;
use crate::stream::StreamResponse;
use crate::tools::ToolDefinition;
use async_trait::async_trait;
use std::env;
use std::sync::Arc;
use z_ai_sdk as zai;

/// Z.AI provider configuration.
#[derive(Debug, Clone)]
pub struct ZaiConfig {
    /// API key.
    pub api_key: String,
    /// Base URL (default: https://api.z.ai/api/paas/v4 or https://open.bigmodel.cn/api/paas/v4).
    pub base_url: String,
    /// Request timeout in seconds.
    pub timeout_secs: i64,
    /// Use ZhipuAI endpoint instead of Z.AI.
    pub use_zhipuai: bool,
}

impl Default for ZaiConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: "https://api.z.ai/api/paas/v4".to_string(),
            timeout_secs: 600,
            use_zhipuai: false,
        }
    }
}

/// Z.AI provider.
#[derive(Debug)]
pub struct ZaiProvider {
    config: ZaiConfig,
    client: ZaiClientWrapper,
}

/// Wrapper to handle both ZaiClient and ZhipuAiClient.
#[derive(Debug, Clone)]
enum ZaiClientWrapper {
    Zai(zai::ZaiClient),
    ZhipuAi(zai::ZhipuAiClient),
}

impl ZaiProvider {
    /// Create a new Z.AI provider with the given configuration.
    pub fn new(config: ZaiConfig) -> Result<Self, HyperError> {
        if config.api_key.is_empty() {
            return Err(HyperError::ConfigError(
                "Z.AI API key is required".to_string(),
            ));
        }

        let client = if config.use_zhipuai {
            let client_config = zai::ClientConfig::zhipuai(&config.api_key)
                .base_url(&config.base_url)
                .timeout(std::time::Duration::from_secs(config.timeout_secs as u64));

            let client = zai::ZhipuAiClient::new(client_config).map_err(|e| {
                HyperError::ConfigError(format!("Failed to create ZhipuAI client: {e}"))
            })?;
            ZaiClientWrapper::ZhipuAi(client)
        } else {
            let client_config = zai::ClientConfig::zai(&config.api_key)
                .base_url(&config.base_url)
                .timeout(std::time::Duration::from_secs(config.timeout_secs as u64));

            let client = zai::ZaiClient::new(client_config).map_err(|e| {
                HyperError::ConfigError(format!("Failed to create Z.AI client: {e}"))
            })?;
            ZaiClientWrapper::Zai(client)
        };

        Ok(Self { config, client })
    }

    /// Create a provider from environment variables.
    ///
    /// Uses ZAI_API_KEY or ZHIPUAI_API_KEY.
    pub fn from_env() -> Result<Self, HyperError> {
        // Try ZAI_API_KEY first, then ZHIPUAI_API_KEY
        let (api_key, use_zhipuai) = if let Ok(key) = env::var("ZAI_API_KEY") {
            (key, false)
        } else if let Ok(key) = env::var("ZHIPUAI_API_KEY") {
            (key, true)
        } else {
            return Err(HyperError::ConfigError(
                "Z.AI: ZAI_API_KEY or ZHIPUAI_API_KEY environment variable not set".to_string(),
            ));
        };

        let base_url = if use_zhipuai {
            env::var("ZHIPUAI_BASE_URL")
                .unwrap_or_else(|_| "https://open.bigmodel.cn/api/paas/v4".to_string())
        } else {
            env::var("ZAI_BASE_URL").unwrap_or_else(|_| "https://api.z.ai/api/paas/v4".to_string())
        };

        Self::new(ZaiConfig {
            api_key,
            base_url,
            use_zhipuai,
            ..Default::default()
        })
    }

    /// Create a builder for configuring the provider.
    pub fn builder() -> ZaiProviderBuilder {
        ZaiProviderBuilder::new()
    }

    /// Get the API key.
    pub fn api_key(&self) -> &str {
        &self.config.api_key
    }

    /// Get the base URL.
    pub fn base_url(&self) -> &str {
        &self.config.base_url
    }

    /// Check if using ZhipuAI endpoint.
    pub fn is_zhipuai(&self) -> bool {
        self.config.use_zhipuai
    }
}

#[async_trait]
impl Provider for ZaiProvider {
    fn name(&self) -> &str {
        "zhipuai"
    }

    fn model(&self, model_id: &str) -> Result<Arc<dyn Model>, HyperError> {
        Ok(Arc::new(ZaiModel {
            model_id: model_id.to_string(),
            client: self.client.clone(),
        }))
    }
}

/// Builder for Z.AI provider.
#[derive(Debug, Default)]
pub struct ZaiProviderBuilder {
    config: ZaiConfig,
}

impl ZaiProviderBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self::default()
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

    /// Set the request timeout.
    pub fn timeout_secs(mut self, secs: i64) -> Self {
        self.config.timeout_secs = secs;
        self
    }

    /// Use ZhipuAI endpoint.
    pub fn use_zhipuai(mut self, use_zhipuai: bool) -> Self {
        self.config.use_zhipuai = use_zhipuai;
        if use_zhipuai && self.config.base_url == "https://api.z.ai/api/paas/v4" {
            self.config.base_url = "https://open.bigmodel.cn/api/paas/v4".to_string();
        }
        self
    }

    /// Build the provider.
    pub fn build(self) -> Result<ZaiProvider, HyperError> {
        ZaiProvider::new(self.config)
    }
}

impl From<ProviderConfig> for ZaiProviderBuilder {
    fn from(config: ProviderConfig) -> Self {
        let mut builder = Self::new();
        if let Some(key) = config.api_key {
            builder = builder.api_key(key);
        }
        if let Some(url) = config.base_url {
            builder = builder.base_url(url);
        }
        if let Some(timeout) = config.timeout_secs {
            builder = builder.timeout_secs(timeout);
        }
        builder
    }
}

/// Z.AI model implementation.
#[derive(Debug, Clone)]
struct ZaiModel {
    model_id: String,
    client: ZaiClientWrapper,
}

#[async_trait]
impl Model for ZaiModel {
    fn model_name(&self) -> &str {
        &self.model_id
    }

    fn provider(&self) -> &str {
        "zhipuai"
    }

    async fn generate(&self, mut request: GenerateRequest) -> Result<GenerateResponse, HyperError> {
        // Built-in cross-provider sanitization: strip thinking signatures from other providers
        request.sanitize_for_target(self.provider(), self.model_name());

        // Convert messages
        let mut messages = Vec::new();

        for msg in &request.messages {
            match msg.role {
                Role::System => {
                    messages.push(zai::MessageParam::system(msg.text()));
                }
                Role::User => {
                    let content_blocks = convert_content_to_zai(&msg.content);
                    if content_blocks.len() == 1 {
                        if let zai::ContentBlock::Text { text } = &content_blocks[0] {
                            messages.push(zai::MessageParam::user(text.clone()));
                        } else {
                            messages.push(zai::MessageParam::user_with_content(content_blocks));
                        }
                    } else {
                        messages.push(zai::MessageParam::user_with_content(content_blocks));
                    }
                }
                Role::Assistant => {
                    messages.push(zai::MessageParam::assistant(msg.text()));
                }
                Role::Tool => {
                    // Extract tool result
                    for block in &msg.content {
                        if let ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            ..
                        } = block
                        {
                            messages.push(zai::MessageParam::tool_result(
                                tool_use_id,
                                content.to_text(),
                            ));
                        }
                    }
                }
            }
        }

        // Build request params
        let mut params = zai::ChatCompletionsCreateParams::new(&self.model_id, messages);

        if let Some(temp) = request.temperature {
            params = params.temperature(temp);
        }
        if let Some(max_tokens) = request.max_tokens {
            params = params.max_tokens(max_tokens);
        }
        if let Some(top_p) = request.top_p {
            params = params.top_p(top_p);
        }

        // Convert tools
        if let Some(tools) = &request.tools {
            let zai_tools: Vec<_> = tools.iter().map(convert_tool_to_zai).collect();
            params = params.tools(zai_tools);
        }

        // Convert tool choice
        if let Some(choice) = &request.tool_choice {
            params = params.tool_choice(convert_tool_choice_to_zai(choice));
        }

        // Handle provider-specific options
        if let Some(ref options) = request.provider_options {
            if let Some(zai_opts) = downcast_options::<ZaiOptions>(options) {
                if let Some(budget) = zai_opts.thinking_budget_tokens {
                    params = params.thinking(zai::ThinkingConfig::enabled_with_budget(budget));
                }
                if let Some(do_sample) = zai_opts.do_sample {
                    params = params.do_sample(do_sample);
                }
                if let Some(ref request_id) = zai_opts.request_id {
                    params = params.request_id(request_id);
                }
                if let Some(ref user_id) = zai_opts.user_id {
                    params = params.user_id(user_id);
                }
                // Apply catchall extra params
                if !zai_opts.extra.is_empty() {
                    params
                        .extra
                        .extend(zai_opts.extra.iter().map(|(k, v)| (k.clone(), v.clone())));
                }
            }
        }

        // Make API call
        let completion = match &self.client {
            ZaiClientWrapper::Zai(client) => client
                .chat()
                .completions()
                .create(params)
                .await
                .map_err(map_zai_error)?,
            ZaiClientWrapper::ZhipuAi(client) => client
                .chat()
                .completions()
                .create(params)
                .await
                .map_err(map_zai_error)?,
        };

        // Convert response
        convert_zai_response(completion)
    }

    async fn stream(&self, _request: GenerateRequest) -> Result<StreamResponse, HyperError> {
        // Z.AI SDK does not support streaming yet
        Err(HyperError::UnsupportedCapability("streaming".to_string()))
    }
}

// ============================================================================
// Conversion helpers
// ============================================================================

fn convert_content_to_zai(content: &[ContentBlock]) -> Vec<zai::ContentBlock> {
    content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(zai::ContentBlock::text(text)),
            ContentBlock::Image { source, .. } => match source {
                ImageSource::Base64 { data, media_type } => {
                    Some(zai::ContentBlock::image_base64(data, media_type))
                }
                ImageSource::Url { url } => Some(zai::ContentBlock::image_url(url)),
            },
            _ => None,
        })
        .collect()
}

fn convert_tool_to_zai(tool: &ToolDefinition) -> zai::Tool {
    zai::Tool::function(
        &tool.name,
        tool.description.clone(),
        tool.parameters.clone(),
    )
}

fn convert_tool_choice_to_zai(choice: &crate::tools::ToolChoice) -> zai::ToolChoice {
    match choice {
        crate::tools::ToolChoice::Auto => zai::ToolChoice::auto(),
        crate::tools::ToolChoice::Required => zai::ToolChoice::required(),
        crate::tools::ToolChoice::None => zai::ToolChoice::none(),
        crate::tools::ToolChoice::Tool { name } => zai::ToolChoice::function(name),
    }
}

fn convert_zai_response(completion: zai::Completion) -> Result<GenerateResponse, HyperError> {
    let mut content = Vec::new();

    // Get content from first choice
    if let Some(choice) = completion.choices.first() {
        // Add text content
        if let Some(text) = &choice.message.content {
            if !text.is_empty() {
                content.push(ContentBlock::text(text));
            }
        }

        // Add reasoning content as thinking
        if let Some(reasoning) = &choice.message.reasoning_content {
            content.push(ContentBlock::Thinking {
                content: reasoning.clone(),
                signature: None,
            });
        }

        // Add tool calls
        if let Some(tool_calls) = &choice.message.tool_calls {
            for tc in tool_calls {
                let args: serde_json::Value =
                    serde_json::from_str(&tc.function.arguments).unwrap_or(serde_json::Value::Null);
                content.push(ContentBlock::tool_use(&tc.id, &tc.function.name, args));
            }
        }
    }

    let finish_reason = completion
        .choices
        .first()
        .map(|c| match c.finish_reason.as_str() {
            "stop" => FinishReason::Stop,
            "length" => FinishReason::MaxTokens,
            "tool_calls" => FinishReason::ToolCalls,
            "content_filter" => FinishReason::ContentFilter,
            _ => FinishReason::Other,
        })
        .unwrap_or(FinishReason::Stop);

    let cache_read_tokens = completion
        .usage
        .prompt_tokens_details
        .as_ref()
        .map(|d| d.cached_tokens)
        .filter(|&t| t > 0)
        .map(|t| t as i64);
    let reasoning_tokens = completion
        .usage
        .completion_tokens_details
        .as_ref()
        .map(|d| d.reasoning_tokens)
        .filter(|&t| t > 0)
        .map(|t| t as i64);

    let usage = TokenUsage {
        prompt_tokens: completion.usage.prompt_tokens as i64,
        completion_tokens: completion.usage.completion_tokens as i64,
        total_tokens: completion.usage.total_tokens as i64,
        cache_read_tokens,
        cache_creation_tokens: None,
        reasoning_tokens,
    };

    Ok(GenerateResponse {
        id: completion.id.unwrap_or_default(),
        content,
        finish_reason,
        usage: Some(usage),
        model: completion.model.unwrap_or_default(),
    })
}

/// Map Z.AI SDK errors to HyperError using enum variants directly.
///
/// This function leverages the structured error types from the Z.AI SDK
/// instead of string matching, providing more reliable error classification.
fn map_zai_error(e: zai::ZaiError) -> HyperError {
    match e {
        zai::ZaiError::RateLimited { retry_after } => HyperError::Retryable {
            message: "rate limit exceeded".to_string(),
            delay: retry_after,
        },
        zai::ZaiError::Authentication(msg) => HyperError::AuthenticationFailed(msg),
        zai::ZaiError::BadRequest(msg) => HyperError::InvalidRequest(msg),
        zai::ZaiError::Validation(msg) => HyperError::InvalidRequest(msg),
        zai::ZaiError::Configuration(msg) => HyperError::ConfigError(msg),
        zai::ZaiError::Api {
            status,
            message,
            request_id: _,
        } => {
            // Further classify based on HTTP status code
            if status == 429 {
                HyperError::RateLimitExceeded(message)
            } else if status == 401 || status == 403 {
                HyperError::AuthenticationFailed(message)
            } else if status >= 500 {
                HyperError::Retryable {
                    message,
                    delay: None,
                }
            } else {
                HyperError::ProviderError {
                    code: "zai_error".to_string(),
                    message,
                }
            }
        }
        zai::ZaiError::InternalServerError => HyperError::Retryable {
            message: "internal server error".to_string(),
            delay: None,
        },
        zai::ZaiError::ServerFlowExceeded => HyperError::Retryable {
            message: "server flow exceeded".to_string(),
            delay: None,
        },
        zai::ZaiError::Network(e) => HyperError::NetworkError(e.to_string()),
        zai::ZaiError::Serialization(e) => HyperError::ParseError(e.to_string()),
        zai::ZaiError::JwtError(msg) => HyperError::AuthenticationFailed(msg),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder() {
        let result = ZaiProvider::builder()
            .api_key("zai-test-key")
            .base_url("https://custom.zai.com")
            .timeout_secs(120)
            .build();

        assert!(result.is_ok());
        let provider = result.unwrap();
        assert_eq!(provider.name(), "zhipuai");
        assert_eq!(provider.api_key(), "zai-test-key");
    }

    #[test]
    fn test_builder_zhipuai() {
        let result = ZaiProvider::builder()
            .api_key("zhipuai-test-key")
            .use_zhipuai(true)
            .build();

        assert!(result.is_ok());
        let provider = result.unwrap();
        assert_eq!(provider.name(), "zhipuai");
        assert!(provider.is_zhipuai());
    }

    #[test]
    fn test_builder_missing_key() {
        let result = ZaiProvider::builder().build();
        assert!(result.is_err());
    }

    #[test]
    fn test_zai_options_boxing() {
        let opts = ZaiOptions::new()
            .with_thinking_budget(8192)
            .with_request_id("req_123")
            .boxed();

        let downcasted = downcast_options::<ZaiOptions>(&opts);
        assert!(downcasted.is_some());
        assert_eq!(downcasted.unwrap().thinking_budget_tokens, Some(8192));
    }
}
