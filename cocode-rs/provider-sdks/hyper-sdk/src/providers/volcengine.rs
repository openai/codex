//! Volcengine Ark provider implementation.

use crate::error::HyperError;
use crate::messages::ContentBlock;
use crate::messages::ImageSource;
use crate::messages::Role;
use crate::model::Model;
use crate::options::VolcengineOptions;
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
use volcengine_ark_sdk as ark;

/// Volcengine Ark provider configuration.
#[derive(Debug, Clone)]
pub struct VolcengineConfig {
    /// API key.
    pub api_key: String,
    /// Base URL (default: https://ark.cn-beijing.volces.com/api/v3).
    pub base_url: String,
    /// Request timeout in seconds.
    pub timeout_secs: i64,
}

impl Default for VolcengineConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: "https://ark.cn-beijing.volces.com/api/v3".to_string(),
            timeout_secs: 600,
        }
    }
}

/// Volcengine Ark provider.
#[derive(Debug)]
pub struct VolcengineProvider {
    config: VolcengineConfig,
    client: ark::Client,
}

impl VolcengineProvider {
    /// Create a new Volcengine provider with the given configuration.
    pub fn new(config: VolcengineConfig) -> Result<Self, HyperError> {
        if config.api_key.is_empty() {
            return Err(HyperError::ConfigError(
                "Volcengine API key is required".to_string(),
            ));
        }

        let client_config = ark::ClientConfig::new(&config.api_key)
            .base_url(&config.base_url)
            .timeout(std::time::Duration::from_secs(config.timeout_secs as u64));

        let client = ark::Client::new(client_config)
            .map_err(|e| HyperError::ConfigError(format!("Failed to create Ark client: {e}")))?;

        Ok(Self { config, client })
    }

    /// Create a provider from environment variables.
    ///
    /// Uses ARK_API_KEY and ARK_BASE_URL (optional).
    pub fn from_env() -> Result<Self, HyperError> {
        let api_key = env::var("ARK_API_KEY").map_err(|_| {
            HyperError::ConfigError(
                "Volcengine: ARK_API_KEY environment variable not set".to_string(),
            )
        })?;

        let base_url = env::var("ARK_BASE_URL")
            .unwrap_or_else(|_| "https://ark.cn-beijing.volces.com/api/v3".to_string());

        Self::new(VolcengineConfig {
            api_key,
            base_url,
            ..Default::default()
        })
    }

    /// Create a builder for configuring the provider.
    pub fn builder() -> VolcengineProviderBuilder {
        VolcengineProviderBuilder::new()
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
impl Provider for VolcengineProvider {
    fn name(&self) -> &str {
        "volcengine"
    }

    fn model(&self, model_id: &str) -> Result<Arc<dyn Model>, HyperError> {
        Ok(Arc::new(VolcengineModel {
            model_id: model_id.to_string(),
            client: self.client.clone(),
        }))
    }
}

/// Builder for Volcengine provider.
#[derive(Debug, Default)]
pub struct VolcengineProviderBuilder {
    config: VolcengineConfig,
}

impl VolcengineProviderBuilder {
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

    /// Build the provider.
    pub fn build(self) -> Result<VolcengineProvider, HyperError> {
        VolcengineProvider::new(self.config)
    }
}

impl From<ProviderConfig> for VolcengineProviderBuilder {
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

/// Volcengine model implementation.
#[derive(Debug, Clone)]
struct VolcengineModel {
    model_id: String,
    client: ark::Client,
}

#[async_trait]
impl Model for VolcengineModel {
    fn model_name(&self) -> &str {
        &self.model_id
    }

    fn provider(&self) -> &str {
        "volcengine"
    }

    async fn generate(&self, mut request: GenerateRequest) -> Result<GenerateResponse, HyperError> {
        // Built-in cross-provider sanitization: strip thinking signatures from other providers
        request.sanitize_for_target(self.provider(), self.model_name());

        // Convert messages
        let mut input_messages = Vec::new();
        let mut system_instruction = None;

        for msg in &request.messages {
            match msg.role {
                Role::System => {
                    system_instruction = Some(msg.text());
                }
                Role::User => {
                    let content_blocks = convert_content_to_ark(&msg.content);
                    input_messages.push(ark::InputMessage::user(content_blocks));
                }
                Role::Assistant => {
                    input_messages.push(ark::InputMessage::assistant_text(msg.text()));
                }
                Role::Tool => {
                    // Extract tool result
                    for block in &msg.content {
                        if let ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            is_error,
                        } = block
                        {
                            input_messages.push(ark::InputMessage::user(vec![
                                ark::InputContentBlock::function_call_output(
                                    tool_use_id,
                                    content.to_text(),
                                    Some(*is_error),
                                ),
                            ]));
                        }
                    }
                }
            }
        }

        // Build request params
        let mut params = ark::ResponseCreateParams::new(&self.model_id, input_messages);

        if let Some(instructions) = system_instruction {
            params = params.instructions(instructions);
        }
        if let Some(temp) = request.temperature {
            params = params.temperature(temp);
        }
        if let Some(max_tokens) = request.max_tokens {
            params = params.max_output_tokens(max_tokens);
        }
        if let Some(top_p) = request.top_p {
            params = params.top_p(top_p);
        }

        // Convert tools
        if let Some(tools) = &request.tools {
            let ark_tools: Result<Vec<_>, _> = tools.iter().map(convert_tool_to_ark).collect();
            params = params.tools(ark_tools.map_err(|e| HyperError::InvalidRequest(e))?);
        }

        // Convert tool choice
        if let Some(choice) = &request.tool_choice {
            params = params.tool_choice(convert_tool_choice_to_ark(choice));
        }

        // Handle provider-specific options
        if let Some(ref options) = request.provider_options {
            if let Some(volcengine_opts) = downcast_options::<VolcengineOptions>(options) {
                if let Some(budget) = volcengine_opts.thinking_budget_tokens {
                    params = params.thinking(ark::ThinkingConfig::enabled(budget));
                }
                if let Some(prev_id) = &volcengine_opts.previous_response_id {
                    params = params.previous_response_id(prev_id);
                }
                if let Some(enabled) = volcengine_opts.caching_enabled {
                    params = params.caching(ark::CachingConfig {
                        enabled: Some(enabled),
                    });
                }
                if let Some(effort) = &volcengine_opts.reasoning_effort {
                    params = params.reasoning_effort(convert_reasoning_effort_to_ark(effort));
                }
                // Apply catchall extra params
                if !volcengine_opts.extra.is_empty() {
                    params.extra.extend(
                        volcengine_opts
                            .extra
                            .iter()
                            .map(|(k, v)| (k.clone(), v.clone())),
                    );
                }
            }
        }

        // Make API call
        let response = self
            .client
            .responses()
            .create(params)
            .await
            .map_err(map_ark_error)?;

        // Convert response
        convert_ark_response(response)
    }

    async fn stream(&self, _request: GenerateRequest) -> Result<StreamResponse, HyperError> {
        // Volcengine Ark SDK does not support streaming yet
        Err(HyperError::UnsupportedCapability("streaming".to_string()))
    }
}

// ============================================================================
// Conversion helpers
// ============================================================================

fn convert_content_to_ark(content: &[ContentBlock]) -> Vec<ark::InputContentBlock> {
    content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(ark::InputContentBlock::text(text)),
            ContentBlock::Image { source, .. } => match source {
                ImageSource::Base64 { data, media_type } => {
                    let ark_media_type = match media_type.as_str() {
                        "image/jpeg" => ark::ImageMediaType::Jpeg,
                        "image/png" => ark::ImageMediaType::Png,
                        "image/gif" => ark::ImageMediaType::Gif,
                        "image/webp" => ark::ImageMediaType::Webp,
                        _ => ark::ImageMediaType::Png, // Default
                    };
                    Some(ark::InputContentBlock::image_base64(data, ark_media_type))
                }
                ImageSource::Url { url } => Some(ark::InputContentBlock::image_url(url)),
            },
            _ => None,
        })
        .collect()
}

fn convert_tool_to_ark(tool: &ToolDefinition) -> Result<ark::Tool, String> {
    ark::Tool::function(
        &tool.name,
        tool.description.clone(),
        tool.parameters.clone(),
    )
    .map_err(|e| e.to_string())
}

fn convert_tool_choice_to_ark(choice: &crate::tools::ToolChoice) -> ark::ToolChoice {
    match choice {
        crate::tools::ToolChoice::Auto => ark::ToolChoice::Auto,
        crate::tools::ToolChoice::Required => ark::ToolChoice::Required,
        crate::tools::ToolChoice::None => ark::ToolChoice::None,
        crate::tools::ToolChoice::Tool { name } => ark::ToolChoice::Function { name: name.clone() },
    }
}

fn convert_reasoning_effort_to_ark(
    effort: &crate::options::volcengine::ReasoningEffort,
) -> ark::ReasoningEffort {
    match effort {
        crate::options::volcengine::ReasoningEffort::Minimal => ark::ReasoningEffort::Minimal,
        crate::options::volcengine::ReasoningEffort::Low => ark::ReasoningEffort::Low,
        crate::options::volcengine::ReasoningEffort::Medium => ark::ReasoningEffort::Medium,
        crate::options::volcengine::ReasoningEffort::High => ark::ReasoningEffort::High,
    }
}

fn convert_ark_response(response: ark::Response) -> Result<GenerateResponse, HyperError> {
    let mut content = Vec::new();

    for item in &response.output {
        match item {
            ark::OutputItem::Message {
                content: msg_content,
                ..
            } => {
                for block in msg_content {
                    match block {
                        ark::OutputContentBlock::Text { text } => {
                            content.push(ContentBlock::text(text));
                        }
                        ark::OutputContentBlock::Thinking {
                            thinking,
                            signature,
                        } => {
                            content.push(ContentBlock::Thinking {
                                content: thinking.clone(),
                                signature: signature.clone(),
                            });
                        }
                        ark::OutputContentBlock::FunctionCall {
                            id,
                            name,
                            arguments,
                        } => {
                            content.push(ContentBlock::tool_use(id, name, arguments.clone()));
                        }
                    }
                }
            }
            ark::OutputItem::FunctionCall {
                call_id,
                name,
                arguments,
                ..
            } => {
                let args: serde_json::Value =
                    serde_json::from_str(arguments).unwrap_or(serde_json::Value::Null);
                content.push(ContentBlock::tool_use(call_id, name, args));
            }
            ark::OutputItem::Reasoning {
                content: reasoning, ..
            } => {
                content.push(ContentBlock::Thinking {
                    content: reasoning.clone(),
                    signature: None,
                });
            }
        }
    }

    let finish_reason = match response.stop_reason {
        Some(ark::StopReason::EndTurn) => FinishReason::Stop,
        Some(ark::StopReason::MaxTokens) => FinishReason::MaxTokens,
        Some(ark::StopReason::ToolUse) => FinishReason::ToolCalls,
        Some(ark::StopReason::StopSequence) => FinishReason::Stop,
        None => FinishReason::Stop,
    };

    let cached_tokens = response.usage.input_tokens_details.cached_tokens;
    let reasoning_tokens = response.usage.output_tokens_details.reasoning_tokens;

    let usage = TokenUsage {
        prompt_tokens: response.usage.input_tokens as i64,
        completion_tokens: response.usage.output_tokens as i64,
        total_tokens: (response.usage.input_tokens + response.usage.output_tokens) as i64,
        cache_read_tokens: if cached_tokens > 0 {
            Some(cached_tokens as i64)
        } else {
            None
        },
        cache_creation_tokens: None,
        reasoning_tokens: if reasoning_tokens > 0 {
            Some(reasoning_tokens as i64)
        } else {
            None
        },
    };

    Ok(GenerateResponse {
        id: response.id,
        content,
        finish_reason,
        usage: Some(usage),
        model: response.model.unwrap_or_default(),
    })
}

/// Map Volcengine Ark SDK errors to HyperError using enum variants directly.
///
/// This function leverages the structured error types from the Ark SDK
/// instead of string matching, providing more reliable error classification.
fn map_ark_error(e: ark::ArkError) -> HyperError {
    match e {
        ark::ArkError::RateLimited { retry_after } => HyperError::Retryable {
            message: "rate limit exceeded".to_string(),
            delay: retry_after,
        },
        ark::ArkError::QuotaExceeded => HyperError::QuotaExceeded("quota exceeded".to_string()),
        ark::ArkError::ContextWindowExceeded => {
            HyperError::ContextWindowExceeded("context window exceeded".to_string())
        }
        ark::ArkError::PreviousResponseNotFound => {
            HyperError::PreviousResponseNotFound("previous response not found".to_string())
        }
        ark::ArkError::Authentication(msg) => HyperError::AuthenticationFailed(msg),
        ark::ArkError::BadRequest(msg) => HyperError::InvalidRequest(msg),
        ark::ArkError::Validation(msg) => HyperError::InvalidRequest(msg),
        ark::ArkError::Configuration(msg) => HyperError::ConfigError(msg),
        ark::ArkError::Api {
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
                    code: "volcengine_error".to_string(),
                    message,
                }
            }
        }
        ark::ArkError::InternalServerError => HyperError::Retryable {
            message: "internal server error".to_string(),
            delay: None,
        },
        ark::ArkError::Network(e) => HyperError::NetworkError(e.to_string()),
        ark::ArkError::Serialization(e) => HyperError::ParseError(e.to_string()),
        ark::ArkError::Parse(msg) => HyperError::ParseError(msg),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder() {
        let result = VolcengineProvider::builder()
            .api_key("ark-test-key")
            .base_url("https://custom.ark.com")
            .timeout_secs(120)
            .build();

        assert!(result.is_ok());
        let provider = result.unwrap();
        assert_eq!(provider.name(), "volcengine");
        assert_eq!(provider.api_key(), "ark-test-key");
    }

    #[test]
    fn test_builder_missing_key() {
        let result = VolcengineProvider::builder().build();
        assert!(result.is_err());
    }

    #[test]
    fn test_volcengine_options_boxing() {
        let opts = VolcengineOptions::new()
            .with_thinking_budget(4096)
            .with_previous_response_id("resp_123")
            .boxed();

        let downcasted = downcast_options::<VolcengineOptions>(&opts);
        assert!(downcasted.is_some());
        assert_eq!(downcasted.unwrap().thinking_budget_tokens, Some(4096));
    }
}
