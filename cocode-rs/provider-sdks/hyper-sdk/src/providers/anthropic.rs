//! Anthropic provider implementation.

use crate::error::HyperError;
use crate::messages::ContentBlock;
use crate::messages::ImageSource;
use crate::messages::Role;
use crate::model::Model;
use crate::options::AnthropicOptions;
use crate::options::downcast_options;
use crate::provider::Provider;
use crate::provider::ProviderConfig;
use crate::request::GenerateRequest;
use crate::response::FinishReason;
use crate::response::GenerateResponse;
use crate::response::TokenUsage;
use crate::stream::EventStream;
use crate::stream::StreamEvent;
use crate::stream::StreamResponse;
use crate::tools::ToolDefinition;
use anthropic_sdk as ant;
use async_trait::async_trait;
use std::env;
use std::sync::Arc;
use tracing::debug;
use tracing::info;
use tracing::instrument;

/// Anthropic provider configuration.
#[derive(Debug, Clone)]
pub struct AnthropicConfig {
    /// API key.
    pub api_key: String,
    /// Base URL (default: https://api.anthropic.com).
    pub base_url: String,
    /// Request timeout in seconds.
    pub timeout_secs: i64,
}

impl Default for AnthropicConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: "https://api.anthropic.com".to_string(),
            timeout_secs: 600,
        }
    }
}

/// Anthropic provider.
#[derive(Debug)]
pub struct AnthropicProvider {
    config: AnthropicConfig,
    sdk_client: ant::Client,
}

impl AnthropicProvider {
    /// Create a new Anthropic provider with the given configuration.
    pub fn new(config: AnthropicConfig) -> Result<Self, HyperError> {
        if config.api_key.is_empty() {
            return Err(HyperError::ConfigError(
                "Anthropic API key is required".to_string(),
            ));
        }

        let sdk_config = ant::ClientConfig::new(&config.api_key)
            .base_url(&config.base_url)
            .timeout(std::time::Duration::from_secs(config.timeout_secs as u64));

        let sdk_client = ant::Client::new(sdk_config).map_err(|e| {
            HyperError::ConfigError(format!("Failed to create Anthropic client: {e}"))
        })?;

        Ok(Self { config, sdk_client })
    }

    /// Create a provider from environment variables.
    ///
    /// Uses ANTHROPIC_API_KEY and ANTHROPIC_BASE_URL (optional).
    pub fn from_env() -> Result<Self, HyperError> {
        let api_key = env::var("ANTHROPIC_API_KEY").map_err(|_| {
            HyperError::ConfigError(
                "Anthropic: ANTHROPIC_API_KEY environment variable not set".to_string(),
            )
        })?;

        let base_url = env::var("ANTHROPIC_BASE_URL")
            .unwrap_or_else(|_| "https://api.anthropic.com".to_string());

        Self::new(AnthropicConfig {
            api_key,
            base_url,
            ..Default::default()
        })
    }

    /// Create a builder for configuring the provider.
    pub fn builder() -> AnthropicProviderBuilder {
        AnthropicProviderBuilder::new()
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
impl Provider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    fn model(&self, model_id: &str) -> Result<Arc<dyn Model>, HyperError> {
        Ok(Arc::new(AnthropicModel {
            model_id: model_id.to_string(),
            client: self.sdk_client.clone(),
        }))
    }
}

/// Builder for Anthropic provider.
#[derive(Debug, Default)]
pub struct AnthropicProviderBuilder {
    config: AnthropicConfig,
}

impl AnthropicProviderBuilder {
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
    pub fn build(self) -> Result<AnthropicProvider, HyperError> {
        AnthropicProvider::new(self.config)
    }
}

impl From<ProviderConfig> for AnthropicProviderBuilder {
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

/// Anthropic model implementation.
#[derive(Debug, Clone)]
struct AnthropicModel {
    model_id: String,
    client: ant::Client,
}

#[async_trait]
impl Model for AnthropicModel {
    fn model_name(&self) -> &str {
        &self.model_id
    }

    fn provider(&self) -> &str {
        "anthropic"
    }

    #[instrument(skip(self, request), fields(provider = "anthropic", model = %self.model_id))]
    async fn generate(&self, mut request: GenerateRequest) -> Result<GenerateResponse, HyperError> {
        debug!(messages = request.messages.len(), "Starting generation");
        // Built-in cross-provider sanitization: strip thinking signatures from other providers
        request.sanitize_for_target(self.provider(), self.model_name());

        // Convert messages
        let mut messages = Vec::new();
        let mut system_prompt = None;

        for msg in &request.messages {
            match msg.role {
                Role::System => {
                    system_prompt = Some(msg.text());
                }
                Role::User => {
                    let content_blocks = convert_content_to_ant(&msg.content);
                    messages.push(ant::MessageParam::user_with_content(content_blocks));
                }
                Role::Assistant => {
                    let content_blocks = convert_content_to_ant(&msg.content);
                    messages.push(ant::MessageParam::assistant_with_content(content_blocks));
                }
                Role::Tool => {
                    // Tool results - add to the most recent user message or create new one
                    let mut tool_results = Vec::new();
                    for block in &msg.content {
                        if let ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            is_error,
                            ..
                        } = block
                        {
                            tool_results.push(ant::ContentBlockParam::ToolResult {
                                tool_use_id: tool_use_id.clone(),
                                content: Some(ant::ToolResultContent::Text(content.to_text())),
                                is_error: Some(*is_error),
                                cache_control: None,
                            });
                        }
                    }
                    if !tool_results.is_empty() {
                        messages.push(ant::MessageParam::user_with_content(tool_results));
                    }
                }
            }
        }

        // Build request params
        let max_tokens = request.max_tokens.unwrap_or(8192);
        let mut params = ant::MessageCreateParams::new(&self.model_id, max_tokens, messages);

        if let Some(system) = system_prompt {
            params = params.system(system);
        }
        if let Some(temp) = request.temperature {
            params = params.temperature(temp);
        }
        if let Some(top_p) = request.top_p {
            params = params.top_p(top_p);
        }

        // Convert tools
        if let Some(tools) = &request.tools {
            let ant_tools: Vec<_> = tools
                .iter()
                .filter(|t| t.custom_format.is_none())
                .map(convert_tool_to_ant)
                .collect();
            params = params.tools(ant_tools);
        }

        // Convert tool choice
        if let Some(choice) = &request.tool_choice {
            params = params.tool_choice(convert_tool_choice_to_ant(choice));
        }

        // Handle provider-specific options
        if let Some(ref options) = request.provider_options {
            if let Some(ant_opts) = downcast_options::<AnthropicOptions>(options) {
                if let Some(budget) = ant_opts.thinking_budget_tokens {
                    params = params.thinking(ant::ThinkingConfig::enabled(budget));
                }
                if let Some(ref metadata) = ant_opts.metadata {
                    if let Some(ref user_id) = metadata.user_id {
                        params = params.metadata(ant::Metadata {
                            user_id: Some(user_id.clone()),
                        });
                    }
                }
                // Apply catchall extra params
                if !ant_opts.extra.is_empty() {
                    params
                        .extra
                        .extend(ant_opts.extra.iter().map(|(k, v)| (k.clone(), v.clone())));
                }
            }
        }

        // Make API call
        debug!("Sending request to Anthropic API");
        let response = self
            .client
            .messages()
            .create(params)
            .await
            .map_err(|e| map_anthropic_error(&e))?;

        info!(response_id = %response.id, "Generation complete");
        // Convert response
        convert_ant_response(response)
    }

    #[instrument(skip(self, request), fields(provider = "anthropic", model = %self.model_id))]
    async fn stream(&self, mut request: GenerateRequest) -> Result<StreamResponse, HyperError> {
        debug!(
            messages = request.messages.len(),
            "Starting streaming generation"
        );
        // Built-in cross-provider sanitization: strip thinking signatures from other providers
        request.sanitize_for_target(self.provider(), self.model_name());

        // Convert messages (same as generate)
        let mut messages = Vec::new();
        let mut system_prompt = None;

        for msg in &request.messages {
            match msg.role {
                Role::System => {
                    system_prompt = Some(msg.text());
                }
                Role::User => {
                    let content_blocks = convert_content_to_ant(&msg.content);
                    messages.push(ant::MessageParam::user_with_content(content_blocks));
                }
                Role::Assistant => {
                    let content_blocks = convert_content_to_ant(&msg.content);
                    messages.push(ant::MessageParam::assistant_with_content(content_blocks));
                }
                Role::Tool => {
                    let mut tool_results = Vec::new();
                    for block in &msg.content {
                        if let ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            is_error,
                            ..
                        } = block
                        {
                            tool_results.push(ant::ContentBlockParam::ToolResult {
                                tool_use_id: tool_use_id.clone(),
                                content: Some(ant::ToolResultContent::Text(content.to_text())),
                                is_error: Some(*is_error),
                                cache_control: None,
                            });
                        }
                    }
                    if !tool_results.is_empty() {
                        messages.push(ant::MessageParam::user_with_content(tool_results));
                    }
                }
            }
        }

        let max_tokens = request.max_tokens.unwrap_or(8192);
        let mut params = ant::MessageCreateParams::new(&self.model_id, max_tokens, messages);

        if let Some(system) = system_prompt {
            params = params.system(system);
        }
        if let Some(temp) = request.temperature {
            params = params.temperature(temp);
        }
        if let Some(top_p) = request.top_p {
            params = params.top_p(top_p);
        }

        if let Some(tools) = &request.tools {
            let ant_tools: Vec<_> = tools
                .iter()
                .filter(|t| t.custom_format.is_none())
                .map(convert_tool_to_ant)
                .collect();
            params = params.tools(ant_tools);
        }

        if let Some(choice) = &request.tool_choice {
            params = params.tool_choice(convert_tool_choice_to_ant(choice));
        }

        if let Some(ref options) = request.provider_options {
            if let Some(ant_opts) = downcast_options::<AnthropicOptions>(options) {
                if let Some(budget) = ant_opts.thinking_budget_tokens {
                    params = params.thinking(ant::ThinkingConfig::enabled(budget));
                }
                // Apply catchall extra params
                if !ant_opts.extra.is_empty() {
                    params
                        .extra
                        .extend(ant_opts.extra.iter().map(|(k, v)| (k.clone(), v.clone())));
                }
            }
        }

        // Create streaming request
        debug!("Starting stream from Anthropic API");
        let sdk_stream = self
            .client
            .messages()
            .create_stream(params)
            .await
            .map_err(|e| map_anthropic_error(&e))?;

        info!("Stream initiated successfully");
        // Create hyper-sdk event stream with state tracking
        // State: (stream, message_id, tool_calls: HashMap<index, (id, name)>)
        let initial_state = AnthropicStreamState::new(sdk_stream);
        let event_stream: EventStream = Box::pin(futures::stream::unfold(
            initial_state,
            |mut state| async move {
                match state.stream.next_event().await {
                    Some(Ok(event)) => {
                        let hyper_event = convert_stream_event_stateful(event, &mut state);
                        Some((hyper_event, state))
                    }
                    Some(Err(e)) => Some((Err(map_anthropic_error(&e)), state)),
                    None => None,
                }
            },
        ));

        Ok(StreamResponse::new(event_stream))
    }
}

/// State for Anthropic streaming to track IDs across events.
struct AnthropicStreamState {
    stream: ant::MessageStream,
    /// Message ID from MessageStart event.
    message_id: String,
    /// Tool call info (id, name) by content block index.
    tool_calls: std::collections::HashMap<i64, (String, String)>,
}

impl AnthropicStreamState {
    fn new(stream: ant::MessageStream) -> Self {
        Self {
            stream,
            message_id: String::new(),
            tool_calls: std::collections::HashMap::new(),
        }
    }
}

// ============================================================================
// Conversion helpers
// ============================================================================

fn convert_content_to_ant(content: &[ContentBlock]) -> Vec<ant::ContentBlockParam> {
    content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(ant::ContentBlockParam::text(text)),
            ContentBlock::Image { source, .. } => match source {
                ImageSource::Base64 { data, media_type } => {
                    let media = match media_type.as_str() {
                        "image/jpeg" => ant::ImageMediaType::Jpeg,
                        "image/png" => ant::ImageMediaType::Png,
                        "image/gif" => ant::ImageMediaType::Gif,
                        "image/webp" => ant::ImageMediaType::Webp,
                        _ => ant::ImageMediaType::Jpeg,
                    };
                    Some(ant::ContentBlockParam::image_base64(data, media))
                }
                ImageSource::Url { url } => Some(ant::ContentBlockParam::image_url(url)),
            },
            ContentBlock::ToolUse { id, name, input } => Some(ant::ContentBlockParam::ToolUse {
                id: id.clone(),
                name: name.clone(),
                input: input.clone(),
            }),
            _ => None,
        })
        .collect()
}

fn convert_tool_to_ant(tool: &ToolDefinition) -> ant::Tool {
    // Try to create with validation, fall back to direct construction
    ant::Tool::new(
        &tool.name,
        tool.description.clone(),
        tool.parameters.clone(),
    )
    .unwrap_or_else(|_| ant::Tool {
        name: tool.name.clone(),
        description: tool.description.clone(),
        input_schema: tool.parameters.clone(),
        cache_control: None,
    })
}

fn convert_tool_choice_to_ant(choice: &crate::tools::ToolChoice) -> ant::ToolChoice {
    match choice {
        crate::tools::ToolChoice::Auto => ant::ToolChoice::Auto {
            disable_parallel_tool_use: None,
        },
        crate::tools::ToolChoice::Required => ant::ToolChoice::Any {
            disable_parallel_tool_use: None,
        },
        crate::tools::ToolChoice::None => ant::ToolChoice::None,
        crate::tools::ToolChoice::Tool { name } => ant::ToolChoice::Tool {
            name: name.clone(),
            disable_parallel_tool_use: None,
        },
    }
}

fn convert_ant_response(response: ant::Message) -> Result<GenerateResponse, HyperError> {
    let mut content = Vec::new();

    for block in &response.content {
        match block {
            ant::ContentBlock::Text { text, .. } => {
                content.push(ContentBlock::text(text));
            }
            ant::ContentBlock::ToolUse { id, name, input } => {
                content.push(ContentBlock::tool_use(id, name, input.clone()));
            }
            ant::ContentBlock::Thinking {
                thinking,
                signature,
            } => {
                content.push(ContentBlock::Thinking {
                    content: thinking.clone(),
                    signature: Some(signature.clone()),
                });
            }
            _ => {}
        }
    }

    let finish_reason = match response.stop_reason {
        Some(ant::StopReason::EndTurn) => FinishReason::Stop,
        Some(ant::StopReason::MaxTokens) => FinishReason::MaxTokens,
        Some(ant::StopReason::ToolUse) => FinishReason::ToolCalls,
        Some(ant::StopReason::StopSequence) => FinishReason::Stop,
        Some(ant::StopReason::Refusal) => FinishReason::ContentFilter,
        Some(ant::StopReason::PauseTurn) => FinishReason::InProgress,
        None => FinishReason::Stop,
    };

    let cache_read = response.usage.cache_read_input_tokens.unwrap_or(0);
    let cache_creation = response.usage.cache_creation_input_tokens.unwrap_or(0);

    let usage = TokenUsage {
        prompt_tokens: response.usage.input_tokens as i64,
        completion_tokens: response.usage.output_tokens as i64,
        total_tokens: (response.usage.input_tokens + response.usage.output_tokens) as i64,
        cache_read_tokens: if cache_read > 0 {
            Some(cache_read as i64)
        } else {
            None
        },
        cache_creation_tokens: if cache_creation > 0 {
            Some(cache_creation as i64)
        } else {
            None
        },
        reasoning_tokens: None,
    };

    Ok(GenerateResponse {
        id: response.id,
        content,
        finish_reason,
        usage: Some(usage),
        model: response.model,
    })
}

/// Stateful stream event conversion that tracks message ID and tool call info.
fn convert_stream_event_stateful(
    event: ant::RawMessageStreamEvent,
    state: &mut AnthropicStreamState,
) -> Result<StreamEvent, HyperError> {
    match event {
        ant::RawMessageStreamEvent::MessageStart { message } => {
            // Track message ID for ResponseDone
            state.message_id = message.id.clone();
            Ok(StreamEvent::response_created(&message.id))
        }
        ant::RawMessageStreamEvent::ContentBlockStart {
            index,
            content_block,
        } => match content_block {
            ant::ContentBlockStartData::ToolUse { id, name, .. } => {
                // Track tool call info for ToolCallDelta events
                state
                    .tool_calls
                    .insert(index as i64, (id.clone(), name.clone()));
                Ok(StreamEvent::ToolCallStart {
                    index: index as i64,
                    id,
                    name,
                })
            }
            _ => Ok(StreamEvent::Ignored),
        },
        ant::RawMessageStreamEvent::ContentBlockDelta { index, delta } => match delta {
            ant::ContentBlockDelta::TextDelta { text } => {
                Ok(StreamEvent::text_delta(index as i64, &text))
            }
            ant::ContentBlockDelta::ThinkingDelta { thinking } => {
                Ok(StreamEvent::thinking_delta(index as i64, &thinking))
            }
            ant::ContentBlockDelta::InputJsonDelta { partial_json } => {
                // Get tracked tool call ID from state
                let id = state
                    .tool_calls
                    .get(&(index as i64))
                    .map(|(id, _)| id.clone())
                    .unwrap_or_default();
                Ok(StreamEvent::ToolCallDelta {
                    index: index as i64,
                    id,
                    arguments_delta: partial_json,
                })
            }
            _ => Ok(StreamEvent::Ignored),
        },
        ant::RawMessageStreamEvent::MessageDelta { delta, usage, .. } => {
            let finish_reason = match delta.stop_reason {
                Some(ant::StopReason::EndTurn) => FinishReason::Stop,
                Some(ant::StopReason::MaxTokens) => FinishReason::MaxTokens,
                Some(ant::StopReason::ToolUse) => FinishReason::ToolCalls,
                Some(ant::StopReason::StopSequence) => FinishReason::Stop,
                Some(ant::StopReason::Refusal) => FinishReason::ContentFilter,
                Some(ant::StopReason::PauseTurn) => FinishReason::InProgress,
                None => FinishReason::Stop,
            };

            let token_usage = TokenUsage {
                prompt_tokens: 0,
                completion_tokens: usage.output_tokens as i64,
                total_tokens: usage.output_tokens as i64,
                cache_read_tokens: None,
                cache_creation_tokens: None,
                reasoning_tokens: None,
            };

            Ok(StreamEvent::response_done_full(
                state.message_id.clone(),
                String::new(), // Anthropic doesn't provide model in MessageDelta
                Some(token_usage),
                finish_reason,
            ))
        }
        ant::RawMessageStreamEvent::Error { error } => Err(HyperError::ProviderError {
            code: error.error_type,
            message: error.message,
        }),
        _ => Ok(StreamEvent::Ignored),
    }
}

fn map_anthropic_error(e: &ant::AnthropicError) -> HyperError {
    match e {
        ant::AnthropicError::RateLimited { retry_after } => {
            // Use the retry_after value from the SDK if available
            HyperError::Retryable {
                message: "Rate limited".to_string(),
                delay: *retry_after,
            }
        }
        ant::AnthropicError::InternalServerError => {
            // 5xx server errors are retryable
            HyperError::Retryable {
                message: "Internal server error".to_string(),
                delay: None,
            }
        }
        ant::AnthropicError::Authentication(msg) => HyperError::AuthenticationFailed(msg.clone()),
        ant::AnthropicError::Api {
            status, message, ..
        } => {
            // Check for 5xx errors first - these are retryable
            if *status >= 500 {
                return HyperError::Retryable {
                    message: format!("Server error ({status}): {message}"),
                    delay: None,
                };
            }
            // Check for quota exceeded patterns in API errors
            let lower_msg = message.to_lowercase();
            if lower_msg.contains("quota") || lower_msg.contains("insufficient_quota") {
                HyperError::QuotaExceeded(message.clone())
            } else if lower_msg.contains("context") && lower_msg.contains("length") {
                HyperError::ContextWindowExceeded(message.clone())
            } else {
                HyperError::ProviderError {
                    code: "api_error".to_string(),
                    message: message.clone(),
                }
            }
        }
        _ => HyperError::ProviderError {
            code: "anthropic_error".to_string(),
            message: e.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder() {
        let result = AnthropicProvider::builder()
            .api_key("sk-ant-test-key")
            .base_url("https://custom.anthropic.com")
            .timeout_secs(120)
            .build();

        assert!(result.is_ok());
        let provider = result.unwrap();
        assert_eq!(provider.name(), "anthropic");
        assert_eq!(provider.api_key(), "sk-ant-test-key");
    }

    #[test]
    fn test_builder_missing_key() {
        let result = AnthropicProvider::builder().build();
        assert!(result.is_err());
    }
}
