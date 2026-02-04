//! Google Gemini provider implementation.

use crate::call_id::enhance_server_call_id;
use crate::call_id::generate_client_call_id;
use crate::error::HyperError;
use crate::messages::ContentBlock;
use crate::messages::ImageSource;
use crate::messages::Message;
use crate::messages::Role;
use crate::model::Model;
use crate::options::downcast_options;
use crate::options::gemini::GeminiOptions;
use crate::options::gemini::ThinkingLevel;
use crate::provider::Provider;
use crate::provider::ProviderConfig;
use crate::request::GenerateRequest;
use crate::response::FinishReason;
use crate::response::GenerateResponse;
use crate::response::TokenUsage;
use crate::stream::EventStream;
use crate::stream::StreamEvent;
use crate::stream::StreamResponse;
use crate::tools::ToolCall;
use crate::tools::ToolDefinition;
use crate::tools::ToolResultContent;
use async_trait::async_trait;
use futures::StreamExt;
use google_genai_sdk as gem;
use std::env;
use std::sync::Arc;

/// Gemini provider configuration.
#[derive(Debug, Clone)]
pub struct GeminiConfig {
    /// API key.
    pub api_key: String,
    /// Base URL (default: https://generativelanguage.googleapis.com).
    pub base_url: String,
    /// Request timeout in seconds.
    pub timeout_secs: i64,
}

impl Default for GeminiConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: "https://generativelanguage.googleapis.com".to_string(),
            timeout_secs: 600,
        }
    }
}

/// Google Gemini provider.
#[derive(Debug)]
pub struct GeminiProvider {
    config: GeminiConfig,
    sdk_client: gem::Client,
}

impl GeminiProvider {
    /// Create a new Gemini provider with the given configuration.
    pub fn new(config: GeminiConfig) -> Result<Self, HyperError> {
        if config.api_key.is_empty() {
            return Err(HyperError::ConfigError(
                "Google API key is required".to_string(),
            ));
        }

        let sdk_config = gem::ClientConfig::with_api_key(&config.api_key)
            .base_url(&config.base_url)
            .timeout(config.timeout_secs as u64);

        let sdk_client = gem::Client::new(sdk_config)
            .map_err(|e| HyperError::ConfigError(format!("Failed to create Gemini client: {e}")))?;

        Ok(Self { config, sdk_client })
    }

    /// Create a provider from environment variables.
    ///
    /// Uses GOOGLE_API_KEY or GEMINI_API_KEY, and GOOGLE_BASE_URL (optional).
    pub fn from_env() -> Result<Self, HyperError> {
        let api_key = env::var("GOOGLE_API_KEY")
            .or_else(|_| env::var("GEMINI_API_KEY"))
            .map_err(|_| {
                HyperError::ConfigError(
                    "Gemini: GOOGLE_API_KEY or GEMINI_API_KEY environment variable not set"
                        .to_string(),
                )
            })?;

        let base_url = env::var("GOOGLE_BASE_URL")
            .unwrap_or_else(|_| "https://generativelanguage.googleapis.com".to_string());

        Self::new(GeminiConfig {
            api_key,
            base_url,
            ..Default::default()
        })
    }

    /// Create a builder for configuring the provider.
    pub fn builder() -> GeminiProviderBuilder {
        GeminiProviderBuilder::new()
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
impl Provider for GeminiProvider {
    fn name(&self) -> &str {
        "gemini"
    }

    fn model(&self, model_id: &str) -> Result<Arc<dyn Model>, HyperError> {
        Ok(Arc::new(GeminiModel {
            model_id: model_id.to_string(),
            sdk_client: self.sdk_client.clone(),
        }))
    }
}

/// Builder for Gemini provider.
#[derive(Debug, Default)]
pub struct GeminiProviderBuilder {
    config: GeminiConfig,
}

impl GeminiProviderBuilder {
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
    pub fn build(self) -> Result<GeminiProvider, HyperError> {
        GeminiProvider::new(self.config)
    }
}

impl From<ProviderConfig> for GeminiProviderBuilder {
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

/// Gemini model implementation.
#[derive(Debug)]
struct GeminiModel {
    model_id: String,
    sdk_client: gem::Client,
}

#[async_trait]
impl Model for GeminiModel {
    fn model_name(&self) -> &str {
        &self.model_id
    }

    fn provider(&self) -> &str {
        "gemini"
    }

    async fn generate(&self, mut request: GenerateRequest) -> Result<GenerateResponse, HyperError> {
        // Built-in cross-provider sanitization: strip thinking signatures from other providers
        request.sanitize_for_target(self.provider(), self.model_name());

        // Separate system message from content messages
        let (system_msg, content_messages) = extract_system_message(&request.messages);

        // Convert messages to Gemini format
        let contents = convert_messages_to_contents(&content_messages);

        // Build config
        let mut config = gem::GenerateContentConfig::default();

        // Set generation parameters
        if let Some(temp) = request.temperature {
            config.temperature = Some(temp as f32);
        }
        if let Some(top_p) = request.top_p {
            config.top_p = Some(top_p as f32);
        }
        if let Some(max_tokens) = request.max_tokens {
            config.max_output_tokens = Some(max_tokens);
        }

        // Convert system prompt
        if let Some(system) = system_msg {
            config.system_instruction = Some(gem::Content::user(system));
        }

        // Convert tools
        if let Some(tools) = &request.tools {
            let gem_tools = convert_tools_to_gemini(tools);
            config.tools = Some(gem_tools);
        }

        // Apply thinking config from unified request config and/or provider-specific options
        config.thinking_config = build_gemini_thinking_config(&request);

        // Make request
        let response = self
            .sdk_client
            .generate_content(&self.model_id, contents, Some(config))
            .await
            .map_err(convert_gemini_error)?;

        // Convert response
        convert_gemini_response(response)
    }

    async fn stream(&self, mut request: GenerateRequest) -> Result<StreamResponse, HyperError> {
        // Built-in cross-provider sanitization: strip thinking signatures from other providers
        request.sanitize_for_target(self.provider(), self.model_name());

        // Separate system message from content messages
        let (system_msg, content_messages) = extract_system_message(&request.messages);

        // Convert messages to Gemini format
        let contents = convert_messages_to_contents(&content_messages);

        // Build config
        let mut config = gem::GenerateContentConfig::default();

        // Set generation parameters
        if let Some(temp) = request.temperature {
            config.temperature = Some(temp as f32);
        }
        if let Some(top_p) = request.top_p {
            config.top_p = Some(top_p as f32);
        }
        if let Some(max_tokens) = request.max_tokens {
            config.max_output_tokens = Some(max_tokens);
        }

        // Convert system prompt
        if let Some(system) = system_msg {
            config.system_instruction = Some(gem::Content::user(system));
        }

        // Convert tools
        if let Some(tools) = &request.tools {
            let gem_tools = convert_tools_to_gemini(tools);
            config.tools = Some(gem_tools);
        }

        // Apply thinking config from unified request config and/or provider-specific options
        config.thinking_config = build_gemini_thinking_config(&request);

        // Get streaming response
        let stream = self
            .sdk_client
            .generate_content_stream(&self.model_id, contents, Some(config))
            .await
            .map_err(convert_gemini_error)?;

        // Wrap in EventStream
        // Track both text_index and fc_index (function call index) across chunks
        let initial_state: (_, i32, i64) = (stream, 0, 0);
        let event_stream: EventStream = Box::pin(
            futures::stream::unfold(
                initial_state,
                |(mut stream, text_index, mut fc_index)| async move {
                    match stream.next().await {
                        Some(Ok(chunk)) => {
                            let events =
                                convert_stream_chunk_to_events(&chunk, text_index, &mut fc_index);
                            Some((events, (stream, text_index + 1, fc_index)))
                        }
                        Some(Err(e)) => {
                            let err_event = vec![Err(HyperError::ProviderError {
                                code: "stream_error".to_string(),
                                message: e.to_string(),
                            })];
                            Some((err_event, (stream, text_index, fc_index)))
                        }
                        None => None,
                    }
                },
            )
            .flat_map(futures::stream::iter),
        );

        Ok(StreamResponse::new(event_stream))
    }

    async fn embed(
        &self,
        request: crate::embedding::EmbedRequest,
    ) -> Result<crate::embedding::EmbedResponse, HyperError> {
        // Gemini embeddings not yet implemented via SDK - use dedicated embedding endpoint
        let _ = request;
        Err(HyperError::Internal(
            "Gemini embeddings not yet implemented - use dedicated embedding API".to_string(),
        ))
    }
}

// ============================================================================
// Conversion Functions
// ============================================================================

/// Build Gemini thinking config from request.
///
/// Combines the unified `thinking_config` from GenerateRequest with
/// provider-specific `GeminiOptions.thinking_level` and `include_thoughts`
/// to produce a google_genai ThinkingConfig.
///
/// Priority:
/// 1. Provider-specific GeminiOptions.thinking_level (if set)
/// 2. Unified request.thinking_config (if enabled)
fn build_gemini_thinking_config(request: &GenerateRequest) -> Option<gem::types::ThinkingConfig> {
    // Check provider-specific GeminiOptions first (higher priority)
    if let Some(ref opts) = request.provider_options {
        if let Some(gem_opts) = downcast_options::<GeminiOptions>(opts) {
            if let Some(level) = gem_opts.thinking_level {
                // Convert hyper-sdk ThinkingLevel to google_genai ThinkingLevel
                let gem_level = match level {
                    ThinkingLevel::None => return None, // Explicitly disabled
                    ThinkingLevel::Low => gem::types::ThinkingLevel::Low,
                    ThinkingLevel::Medium => gem::types::ThinkingLevel::Medium,
                    ThinkingLevel::High => gem::types::ThinkingLevel::High,
                };
                let mut config = gem::types::ThinkingConfig::with_level(gem_level);
                // Apply include_thoughts if set (default true when thinking is enabled)
                let include = gem_opts.include_thoughts.unwrap_or(true);
                config.include_thoughts = Some(include);
                return Some(config);
            }
        }
    }

    None
}

/// Extract system message from messages list.
/// Returns (system_text, non_system_messages).
fn extract_system_message(messages: &[Message]) -> (Option<String>, Vec<Message>) {
    let mut system_text = None;
    let mut other_messages = Vec::new();

    for msg in messages {
        if msg.role == Role::System {
            // Extract text from system message
            for block in &msg.content {
                if let ContentBlock::Text { text } = block {
                    system_text = Some(text.clone());
                    break;
                }
            }
        } else {
            other_messages.push(msg.clone());
        }
    }

    (system_text, other_messages)
}

/// Convert hyper-sdk messages to Gemini Content format.
fn convert_messages_to_contents(messages: &[Message]) -> Vec<gem::Content> {
    messages.iter().map(convert_message_to_content).collect()
}

/// Convert a single hyper-sdk Message to Gemini Content.
fn convert_message_to_content(message: &Message) -> gem::Content {
    let role = match message.role {
        Role::User => "user",
        Role::Assistant => "model",
        Role::System => "user", // Gemini doesn't have system role, treat as user
        Role::Tool => "user",   // Tool results come from user side
    };

    let parts: Vec<gem::Part> = message
        .content
        .iter()
        .map(convert_content_block_to_part)
        .collect();

    gem::Content::with_parts(role, parts)
}

/// Convert a hyper-sdk ContentBlock to Gemini Part.
fn convert_content_block_to_part(block: &ContentBlock) -> gem::Part {
    match block {
        ContentBlock::Text { text } => gem::Part::text(text),

        ContentBlock::Image { source, .. } => match source {
            ImageSource::Base64 { media_type, data } => {
                // Decode base64 and create blob
                if let Ok(bytes) =
                    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, data)
                {
                    gem::Part::from_bytes(&bytes, media_type.as_str())
                } else {
                    gem::Part::text("[Invalid base64 image]")
                }
            }
            ImageSource::Url { url } => {
                // Use file_data for URLs
                gem::Part::from_uri(url, "image/*")
            }
        },

        ContentBlock::ToolUse { id, name, input } => {
            // Create a function_call part
            let mut fc = gem::FunctionCall::default();
            fc.id = Some(id.clone());
            fc.name = Some(name.clone());
            fc.args = Some(input.clone());
            gem::Part {
                function_call: Some(fc),
                ..Default::default()
            }
        }

        ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => {
            // Create a function_response part
            // Note: For JSON content, we preserve the structure and add is_error
            let response_content = match content {
                ToolResultContent::Json(json) => {
                    let mut obj = json.clone();
                    if let Some(map) = obj.as_object_mut() {
                        map.insert("is_error".to_string(), serde_json::json!(is_error));
                    }
                    obj
                }
                _ => {
                    // For Text and Blocks, wrap in a result object
                    serde_json::json!({ "result": content.to_text(), "is_error": is_error })
                }
            };
            gem::Part::function_response(tool_use_id.clone(), response_content)
        }

        ContentBlock::Thinking { content, .. } => {
            // Gemini represents thinking as parts with thought=true
            gem::Part {
                text: Some(content.clone()),
                thought: Some(true),
                ..Default::default()
            }
        }
    }
}

/// Convert hyper-sdk ToolDefinitions to Gemini Tools.
fn convert_tools_to_gemini(tools: &[ToolDefinition]) -> Vec<gem::Tool> {
    let declarations: Vec<gem::FunctionDeclaration> = tools
        .iter()
        .map(|tool| {
            let mut fd = gem::FunctionDeclaration::new(&tool.name);
            if let Some(desc) = &tool.description {
                fd = fd.with_description(desc);
            }
            // Use parameters_json_schema for JSON schema format
            fd.parameters_json_schema = Some(tool.parameters.clone());
            fd
        })
        .collect();

    vec![gem::Tool::functions(declarations)]
}

/// Convert Gemini error to HyperError.
fn convert_gemini_error(err: gem::GenAiError) -> HyperError {
    match err {
        gem::GenAiError::Configuration(msg) => HyperError::ConfigError(msg),
        gem::GenAiError::Network(msg) => HyperError::NetworkError(msg),
        gem::GenAiError::Api {
            code,
            message,
            status,
        } => {
            // Check for 5xx errors first - these are retryable
            if code >= 500 {
                return HyperError::Retryable {
                    message: format!("Server error ({code}): {message}"),
                    delay: None,
                };
            }
            // Check for specific error types
            if status.contains("RESOURCE_EXHAUSTED") || code == 429 {
                HyperError::RateLimitExceeded(message)
            } else if message.contains("context length")
                || message.contains("token limit")
                || status.contains("INVALID_ARGUMENT")
            {
                HyperError::ContextWindowExceeded(message)
            } else {
                HyperError::ProviderError {
                    code: code.to_string(),
                    message: format!("{status}: {message}"),
                }
            }
        }
        gem::GenAiError::Parse(msg) => HyperError::Internal(format!("Parse error: {msg}")),
        gem::GenAiError::Validation(msg) => HyperError::InvalidRequest(msg),
        gem::GenAiError::ContextLengthExceeded(msg) => HyperError::ContextWindowExceeded(msg),
        // QuotaExceeded is NOT retryable (requires billing change)
        gem::GenAiError::QuotaExceeded(msg) => HyperError::QuotaExceeded(msg),
        gem::GenAiError::ContentBlocked(msg) => HyperError::ProviderError {
            code: "content_blocked".to_string(),
            message: msg,
        },
    }
}

/// Convert Gemini GenerateContentResponse to hyper-sdk GenerateResponse.
fn convert_gemini_response(
    response: gem::GenerateContentResponse,
) -> Result<GenerateResponse, HyperError> {
    let id = response.response_id.clone().unwrap_or_default();
    let model = response.model_version.clone().unwrap_or_default();

    // Get content blocks from first candidate with fc_index tracking
    let content = if let Some(candidates) = &response.candidates {
        if let Some(first) = candidates.first() {
            if let Some(content) = &first.content {
                if let Some(parts) = &content.parts {
                    convert_parts_to_content_blocks(parts)
                } else {
                    vec![]
                }
            } else {
                vec![]
            }
        } else {
            vec![]
        }
    } else {
        vec![]
    };

    // Get finish reason
    let finish_reason = response
        .finish_reason()
        .map(convert_finish_reason)
        .unwrap_or(FinishReason::Stop);

    // Get usage
    let usage = response.usage_metadata.as_ref().map(|u| TokenUsage {
        prompt_tokens: u.prompt_token_count.unwrap_or(0) as i64,
        completion_tokens: u.candidates_token_count.unwrap_or(0) as i64,
        total_tokens: u.total_token_count.unwrap_or(0) as i64,
        cache_read_tokens: u.cached_content_token_count.map(|v| v as i64),
        cache_creation_tokens: None,
        reasoning_tokens: None,
    });

    Ok(GenerateResponse {
        id,
        model,
        content,
        finish_reason,
        usage,
    })
}

/// Convert Gemini Parts to hyper-sdk ContentBlocks with enhanced call_id tracking.
fn convert_parts_to_content_blocks(parts: &[gem::Part]) -> Vec<ContentBlock> {
    let mut result = Vec::new();
    let mut fc_index: i64 = 0;

    for part in parts {
        // Text part
        if let Some(text) = &part.text {
            if part.thought == Some(true) {
                result.push(ContentBlock::Thinking {
                    content: text.clone(),
                    signature: None,
                });
            } else {
                result.push(ContentBlock::Text { text: text.clone() });
            }
            continue;
        }

        // Function call part with enhanced call_id
        if let Some(fc) = &part.function_call {
            let name = fc.name.clone().unwrap_or_default();
            let call_id = match &fc.id {
                Some(server_id) => enhance_server_call_id(server_id, &name),
                None => {
                    let id = generate_client_call_id(&name, fc_index);
                    fc_index += 1;
                    id
                }
            };
            result.push(ContentBlock::ToolUse {
                id: call_id,
                name,
                input: fc.args.clone().unwrap_or(serde_json::Value::Null),
            });
        }
    }

    result
}

/// Convert Gemini FinishReason to hyper-sdk FinishReason.
fn convert_finish_reason(reason: gem::types::FinishReason) -> FinishReason {
    match reason {
        gem::types::FinishReason::Stop => FinishReason::Stop,
        gem::types::FinishReason::MaxTokens => FinishReason::MaxTokens,
        gem::types::FinishReason::Safety => FinishReason::ContentFilter,
        gem::types::FinishReason::Recitation => FinishReason::ContentFilter,
        gem::types::FinishReason::Language => FinishReason::Stop,
        gem::types::FinishReason::Other => FinishReason::Stop,
        gem::types::FinishReason::Blocklist => FinishReason::ContentFilter,
        gem::types::FinishReason::ProhibitedContent => FinishReason::ContentFilter,
        gem::types::FinishReason::Spii => FinishReason::ContentFilter,
        gem::types::FinishReason::MalformedFunctionCall => FinishReason::Stop,
        gem::types::FinishReason::FinishReasonUnspecified => FinishReason::Stop,
        gem::types::FinishReason::ImageSafety => FinishReason::ContentFilter,
        gem::types::FinishReason::UnexpectedToolCall => FinishReason::Stop,
    }
}

/// Convert a streaming chunk to hyper-sdk StreamEvents.
///
/// The `fc_index` parameter tracks the current function call index across chunks.
/// It returns the updated function call index for the next chunk.
fn convert_stream_chunk_to_events(
    chunk: &gem::GenerateContentResponse,
    _text_index: i32,
    fc_index: &mut i64,
) -> Vec<Result<StreamEvent, HyperError>> {
    let mut events = Vec::new();

    if let Some(candidates) = &chunk.candidates {
        if let Some(first) = candidates.first() {
            if let Some(content) = &first.content {
                if let Some(parts) = &content.parts {
                    for part in parts {
                        // Text delta
                        if let Some(text) = &part.text {
                            if part.thought == Some(true) {
                                events.push(Ok(StreamEvent::ThinkingDelta {
                                    index: 0,
                                    delta: text.clone(),
                                }));
                            } else {
                                events.push(Ok(StreamEvent::TextDelta {
                                    index: 0,
                                    delta: text.clone(),
                                }));
                            }
                        }

                        // Function call
                        if let Some(fc) = &part.function_call {
                            let name = fc.name.clone().unwrap_or_default();
                            let current_index = *fc_index;
                            *fc_index += 1;

                            // Generate enhanced call_id with embedded function name:
                            // - Server-provided ID: srvgen@<name>@<original_id>
                            // - Client-generated ID: cligen@<name>#<index>@<uuid>
                            let call_id = match &fc.id {
                                Some(server_id) => enhance_server_call_id(server_id, &name),
                                None => generate_client_call_id(&name, current_index),
                            };

                            events.push(Ok(StreamEvent::ToolCallStart {
                                index: current_index,
                                id: call_id.clone(),
                                name: name.clone(),
                            }));
                            // Send arguments as a done event if available
                            if let Some(args) = &fc.args {
                                events.push(Ok(StreamEvent::ToolCallDone {
                                    index: current_index,
                                    tool_call: ToolCall {
                                        id: call_id,
                                        name,
                                        arguments: args.clone(),
                                    },
                                }));
                            }
                        }
                    }
                }
            }

            // Check for finish reason
            if let Some(reason) = first.finish_reason {
                let finish = convert_finish_reason(reason);
                let usage = chunk.usage_metadata.as_ref().map(|u| TokenUsage {
                    prompt_tokens: u.prompt_token_count.unwrap_or(0) as i64,
                    completion_tokens: u.candidates_token_count.unwrap_or(0) as i64,
                    total_tokens: u.total_token_count.unwrap_or(0) as i64,
                    cache_read_tokens: u.cached_content_token_count.map(|v| v as i64),
                    cache_creation_tokens: None,
                    reasoning_tokens: None,
                });

                events.push(Ok(StreamEvent::response_done_full(
                    chunk.response_id.clone().unwrap_or_default(),
                    chunk.model_version.clone().unwrap_or_default(),
                    usage,
                    finish,
                )));
            }
        }
    }

    events
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder() {
        let result = GeminiProvider::builder()
            .api_key("test-key")
            .base_url("https://custom.google.com")
            .timeout_secs(120)
            .build();

        assert!(result.is_ok());
        let provider = result.unwrap();
        assert_eq!(provider.name(), "gemini");
        assert_eq!(provider.api_key(), "test-key");
    }

    #[test]
    fn test_builder_missing_key() {
        let result = GeminiProvider::builder().build();
        assert!(result.is_err());
    }
}
