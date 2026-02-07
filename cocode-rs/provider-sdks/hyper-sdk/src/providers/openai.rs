//! OpenAI provider implementation.

use crate::error::HyperError;
use crate::messages::ContentBlock;
use crate::messages::ImageSource;
use crate::messages::Role;
use crate::model::Model;
use crate::options::OpenAIOptions;
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
use async_trait::async_trait;
use openai_sdk as oai;
use std::env;
use std::sync::Arc;
use tracing::debug;
use tracing::info;
use tracing::instrument;

/// OpenAI provider configuration.
#[derive(Debug, Clone)]
pub struct OpenAIConfig {
    /// API key.
    pub api_key: String,
    /// Base URL (default: https://api.openai.com/v1).
    pub base_url: String,
    /// Organization ID.
    pub organization_id: Option<String>,
    /// Request timeout in seconds.
    pub timeout_secs: i64,
}

impl Default for OpenAIConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: "https://api.openai.com/v1".to_string(),
            organization_id: None,
            timeout_secs: 600,
        }
    }
}

/// OpenAI provider.
#[derive(Debug)]
pub struct OpenAIProvider {
    config: OpenAIConfig,
    sdk_client: oai::Client,
}

impl OpenAIProvider {
    /// Create a new OpenAI provider with the given configuration.
    pub fn new(config: OpenAIConfig) -> Result<Self, HyperError> {
        if config.api_key.is_empty() {
            return Err(HyperError::ConfigError(
                "OpenAI API key is required".to_string(),
            ));
        }

        let mut sdk_config = oai::ClientConfig::new(&config.api_key)
            .base_url(&config.base_url)
            .timeout(std::time::Duration::from_secs(config.timeout_secs as u64));

        if let Some(ref org_id) = config.organization_id {
            sdk_config = sdk_config.organization(org_id);
        }

        let sdk_client = oai::Client::new(sdk_config)
            .map_err(|e| HyperError::ConfigError(format!("Failed to create OpenAI client: {e}")))?;

        Ok(Self { config, sdk_client })
    }

    /// Create a provider from environment variables.
    ///
    /// Uses OPENAI_API_KEY, OPENAI_BASE_URL (optional), and OPENAI_ORG_ID (optional).
    pub fn from_env() -> Result<Self, HyperError> {
        let api_key = env::var("OPENAI_API_KEY").map_err(|_| {
            HyperError::ConfigError(
                "OpenAI: OPENAI_API_KEY environment variable not set".to_string(),
            )
        })?;

        let base_url =
            env::var("OPENAI_BASE_URL").unwrap_or_else(|_| "https://api.openai.com/v1".to_string());

        let organization_id = env::var("OPENAI_ORG_ID").ok();

        Self::new(OpenAIConfig {
            api_key,
            base_url,
            organization_id,
            ..Default::default()
        })
    }

    /// Create a builder for configuring the provider.
    pub fn builder() -> OpenAIProviderBuilder {
        OpenAIProviderBuilder::new()
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
impl Provider for OpenAIProvider {
    fn name(&self) -> &str {
        "openai"
    }

    fn model(&self, model_id: &str) -> Result<Arc<dyn Model>, HyperError> {
        Ok(Arc::new(OpenAIModel {
            model_id: model_id.to_string(),
            client: self.sdk_client.clone(),
        }))
    }
}

/// Builder for OpenAI provider.
#[derive(Debug, Default)]
pub struct OpenAIProviderBuilder {
    config: OpenAIConfig,
}

impl OpenAIProviderBuilder {
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

    /// Set the organization ID.
    pub fn organization_id(mut self, org_id: impl Into<String>) -> Self {
        self.config.organization_id = Some(org_id.into());
        self
    }

    /// Set the request timeout.
    pub fn timeout_secs(mut self, secs: i64) -> Self {
        self.config.timeout_secs = secs;
        self
    }

    /// Build the provider.
    pub fn build(self) -> Result<OpenAIProvider, HyperError> {
        OpenAIProvider::new(self.config)
    }
}

impl From<ProviderConfig> for OpenAIProviderBuilder {
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

/// OpenAI model implementation.
#[derive(Debug, Clone)]
struct OpenAIModel {
    model_id: String,
    client: oai::Client,
}

#[async_trait]
impl Model for OpenAIModel {
    fn model_name(&self) -> &str {
        &self.model_id
    }

    fn provider(&self) -> &str {
        "openai"
    }

    #[instrument(skip(self, request), fields(provider = "openai", model = %self.model_id))]
    async fn generate(&self, mut request: GenerateRequest) -> Result<GenerateResponse, HyperError> {
        debug!(messages = request.messages.len(), "Starting generation");
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
                    let content_blocks = convert_content_to_oai(&msg.content);
                    input_messages.push(oai::InputMessage::user(content_blocks));
                }
                Role::Assistant => {
                    let content_blocks = convert_content_to_oai(&msg.content);
                    input_messages.push(oai::InputMessage::assistant(content_blocks));
                }
                Role::Tool => {
                    // Extract tool result — route custom vs function tools
                    for block in &msg.content {
                        if let ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            is_error,
                            is_custom,
                        } = block
                        {
                            let output = content.to_text();
                            let content_block = if *is_custom {
                                oai::InputContentBlock::custom_tool_call_output(
                                    tool_use_id,
                                    &output,
                                )
                            } else {
                                oai::InputContentBlock::function_call_output(
                                    tool_use_id,
                                    &output,
                                    Some(*is_error),
                                )
                            };
                            input_messages.push(oai::InputMessage::user(vec![content_block]));
                        }
                    }
                }
            }
        }

        // Build request params
        let mut params = oai::ResponseCreateParams::new(&self.model_id, input_messages);

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
            let oai_tools: Result<Vec<_>, _> = tools.iter().map(convert_tool_to_oai).collect();
            params = params.tools(oai_tools.map_err(HyperError::InvalidRequest)?);
        }

        // Convert tool choice
        if let Some(choice) = &request.tool_choice {
            params = params.tool_choice(convert_tool_choice_to_oai(choice));
        }

        // Handle provider-specific options
        if let Some(ref options) = request.provider_options {
            if let Some(openai_opts) = downcast_options::<OpenAIOptions>(options) {
                if let Some(prev_id) = &openai_opts.previous_response_id {
                    params = params.previous_response_id(prev_id);
                }
                if let Some(effort) = &openai_opts.reasoning_effort {
                    let mut reasoning_config = convert_reasoning_effort_to_oai(effort);
                    // Apply reasoning summary if set
                    if let Some(summary) = &openai_opts.reasoning_summary {
                        if let Some(summary_str) = convert_reasoning_summary_to_string(summary) {
                            reasoning_config = reasoning_config.with_summary(summary_str);
                        }
                    }
                    params = params.reasoning(reasoning_config);
                }
                // Include encrypted content if requested
                if openai_opts.include_encrypted_content == Some(true) {
                    params =
                        params.include(vec![oai::ResponseIncludable::ReasoningEncryptedContent]);
                }
                // Apply catchall extra params
                if !openai_opts.extra.is_empty() {
                    params.extra.extend(
                        openai_opts
                            .extra
                            .iter()
                            .map(|(k, v)| (k.clone(), v.clone())),
                    );
                }
            }
        }

        // Make API call
        debug!("Sending request to OpenAI API");
        let response = self
            .client
            .responses()
            .create(params)
            .await
            .map_err(|e| map_openai_error(&e))?;

        info!(response_id = %response.id, "Generation complete");
        // Convert response
        convert_oai_response(response)
    }

    #[instrument(skip(self, request), fields(provider = "openai", model = %self.model_id))]
    async fn stream(&self, mut request: GenerateRequest) -> Result<StreamResponse, HyperError> {
        debug!(
            messages = request.messages.len(),
            "Starting streaming generation"
        );
        // Built-in cross-provider sanitization: strip thinking signatures from other providers
        request.sanitize_for_target(self.provider(), self.model_name());

        // Convert messages (same as generate)
        let mut input_messages = Vec::new();
        let mut system_instruction = None;

        for msg in &request.messages {
            match msg.role {
                Role::System => {
                    system_instruction = Some(msg.text());
                }
                Role::User => {
                    let content_blocks = convert_content_to_oai(&msg.content);
                    input_messages.push(oai::InputMessage::user(content_blocks));
                }
                Role::Assistant => {
                    let content_blocks = convert_content_to_oai(&msg.content);
                    input_messages.push(oai::InputMessage::assistant(content_blocks));
                }
                Role::Tool => {
                    for block in &msg.content {
                        if let ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            is_error,
                            is_custom,
                        } = block
                        {
                            let output = content.to_text();
                            let content_block = if *is_custom {
                                oai::InputContentBlock::custom_tool_call_output(
                                    tool_use_id,
                                    &output,
                                )
                            } else {
                                oai::InputContentBlock::function_call_output(
                                    tool_use_id,
                                    &output,
                                    Some(*is_error),
                                )
                            };
                            input_messages.push(oai::InputMessage::user(vec![content_block]));
                        }
                    }
                }
            }
        }

        // Build request params
        let mut params = oai::ResponseCreateParams::new(&self.model_id, input_messages);

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

        if let Some(tools) = &request.tools {
            let oai_tools: Result<Vec<_>, _> = tools.iter().map(convert_tool_to_oai).collect();
            params = params.tools(oai_tools.map_err(HyperError::InvalidRequest)?);
        }

        if let Some(choice) = &request.tool_choice {
            params = params.tool_choice(convert_tool_choice_to_oai(choice));
        }

        if let Some(ref options) = request.provider_options {
            if let Some(openai_opts) = downcast_options::<OpenAIOptions>(options) {
                if let Some(prev_id) = &openai_opts.previous_response_id {
                    params = params.previous_response_id(prev_id);
                }
                if let Some(effort) = &openai_opts.reasoning_effort {
                    let mut reasoning_config = convert_reasoning_effort_to_oai(effort);
                    // Apply reasoning summary if set
                    if let Some(summary) = &openai_opts.reasoning_summary {
                        if let Some(summary_str) = convert_reasoning_summary_to_string(summary) {
                            reasoning_config = reasoning_config.with_summary(summary_str);
                        }
                    }
                    params = params.reasoning(reasoning_config);
                }
                // Include encrypted content if requested
                if openai_opts.include_encrypted_content == Some(true) {
                    params =
                        params.include(vec![oai::ResponseIncludable::ReasoningEncryptedContent]);
                }
                // Apply catchall extra params
                if !openai_opts.extra.is_empty() {
                    params.extra.extend(
                        openai_opts
                            .extra
                            .iter()
                            .map(|(k, v)| (k.clone(), v.clone())),
                    );
                }
            }
        }

        // Create streaming request
        debug!("Starting stream from OpenAI API");
        let sdk_stream = self
            .client
            .responses()
            .stream(params)
            .await
            .map_err(|e| map_openai_error(&e))?;

        info!("Stream initiated successfully");
        // Create hyper-sdk event stream with state tracking
        let initial_state = OpenAIStreamState::new(sdk_stream);
        let event_stream: EventStream = Box::pin(futures::stream::unfold(
            initial_state,
            |mut state| async move {
                match state.stream.next().await {
                    Some(Ok(event)) => {
                        let hyper_event = convert_stream_event_stateful(event, &mut state);
                        Some((hyper_event, state))
                    }
                    Some(Err(e)) => Some((Err(map_openai_error(&e)), state)),
                    None => None,
                }
            },
        ));

        Ok(StreamResponse::new(event_stream))
    }

    async fn embed(
        &self,
        request: crate::embedding::EmbedRequest,
    ) -> Result<crate::embedding::EmbedResponse, HyperError> {
        let _ = request;
        Err(HyperError::UnsupportedCapability("embedding".to_string()))
    }
}

/// State for OpenAI streaming to track tool call names across events.
struct OpenAIStreamState {
    stream: oai::ResponseStream,
    /// Tool call info (id, name) by output_index.
    tool_calls: std::collections::HashMap<i64, (String, String)>,
}

impl OpenAIStreamState {
    fn new(stream: oai::ResponseStream) -> Self {
        Self {
            stream,
            tool_calls: std::collections::HashMap::new(),
        }
    }
}

// ============================================================================
// Conversion helpers
// ============================================================================

fn convert_content_to_oai(content: &[ContentBlock]) -> Vec<oai::InputContentBlock> {
    content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(oai::InputContentBlock::text(text)),
            ContentBlock::Image { source, .. } => match source {
                ImageSource::Base64 { data, media_type } => {
                    let data_url = format!("data:{media_type};base64,{data}");
                    Some(oai::InputContentBlock::image_url(&data_url))
                }
                ImageSource::Url { url } => Some(oai::InputContentBlock::image_url(url)),
            },
            // ToolUse in assistant messages represents what the model output in a previous turn.
            // In OpenAI Responses API, this is tracked via previous_response_id, not re-sent as input.
            // We skip these blocks when converting to input messages.
            _ => None,
        })
        .collect()
}

fn convert_tool_to_oai(tool: &ToolDefinition) -> Result<oai::Tool, String> {
    if let Some(format_value) = &tool.custom_format {
        // Custom tool — OpenAI-specific
        let format: oai::CustomToolInputFormat = serde_json::from_value(format_value.clone())
            .map_err(|e| format!("Invalid custom tool format: {e}"))?;
        Ok(oai::Tool::Custom {
            name: tool.name.clone(),
            description: tool.description.clone(),
            format: Some(format),
        })
    } else {
        oai::Tool::function(
            &tool.name,
            tool.description.clone(),
            tool.parameters.clone(),
        )
        .map_err(|e| e.to_string())
    }
}

fn convert_tool_choice_to_oai(choice: &crate::tools::ToolChoice) -> oai::ToolChoice {
    match choice {
        crate::tools::ToolChoice::Auto => oai::ToolChoice::Auto,
        crate::tools::ToolChoice::Required => oai::ToolChoice::Required,
        crate::tools::ToolChoice::None => oai::ToolChoice::None,
        crate::tools::ToolChoice::Tool { name } => oai::ToolChoice::Function { name: name.clone() },
    }
}

fn convert_reasoning_effort_to_oai(
    effort: &crate::options::openai::ReasoningEffort,
) -> oai::ReasoningConfig {
    let oai_effort = match effort {
        crate::options::openai::ReasoningEffort::Low => oai::ReasoningEffort::Low,
        crate::options::openai::ReasoningEffort::Medium => oai::ReasoningEffort::Medium,
        crate::options::openai::ReasoningEffort::High => oai::ReasoningEffort::High,
    };
    oai::ReasoningConfig::with_effort(oai_effort)
}

fn convert_reasoning_summary_to_string(
    summary: &crate::options::openai::ReasoningSummary,
) -> Option<String> {
    use crate::options::openai::ReasoningSummary;
    match summary {
        ReasoningSummary::None => None, // No summary requested
        ReasoningSummary::Auto => Some("auto".to_string()),
        ReasoningSummary::Concise => Some("concise".to_string()),
        ReasoningSummary::Detailed => Some("detailed".to_string()),
    }
}

fn convert_oai_response(response: oai::Response) -> Result<GenerateResponse, HyperError> {
    let mut content = Vec::new();

    for item in &response.output {
        match item {
            oai::OutputItem::Message {
                content: msg_content,
                ..
            } => {
                for block in msg_content {
                    match block {
                        oai::OutputContentBlock::OutputText { text, .. } => {
                            content.push(ContentBlock::text(text));
                        }
                        oai::OutputContentBlock::Refusal { refusal, .. } => {
                            content.push(ContentBlock::text(format!("[Refusal: {}]", refusal)));
                        }
                    }
                }
            }
            oai::OutputItem::FunctionCall {
                call_id,
                name,
                arguments,
                ..
            } => {
                let args: serde_json::Value =
                    serde_json::from_str(arguments).unwrap_or(serde_json::Value::Null);
                content.push(ContentBlock::tool_use(call_id, name, args));
            }
            oai::OutputItem::CustomToolCall {
                call_id,
                name,
                input,
                ..
            } => {
                content.push(ContentBlock::tool_use(
                    call_id,
                    name,
                    serde_json::Value::String(input.clone()),
                ));
            }
            oai::OutputItem::Reasoning {
                content: reasoning, ..
            } => {
                content.push(ContentBlock::Thinking {
                    content: reasoning.clone(),
                    signature: None,
                });
            }
            _ => {}
        }
    }

    let finish_reason = match response.stop_reason {
        Some(oai::StopReason::EndTurn) => FinishReason::Stop,
        Some(oai::StopReason::MaxTokens) => FinishReason::MaxTokens,
        Some(oai::StopReason::ToolUse) => FinishReason::ToolCalls,
        Some(oai::StopReason::StopSequence) => FinishReason::Stop,
        Some(oai::StopReason::ContentFilter) => FinishReason::ContentFilter,
        None => FinishReason::Stop,
    };

    let cached_tokens = response.usage.cached_tokens();
    let reasoning_tokens = response.usage.reasoning_tokens();

    let usage = TokenUsage {
        prompt_tokens: response.usage.input_tokens as i64,
        completion_tokens: response.usage.output_tokens as i64,
        total_tokens: response.usage.total_tokens as i64,
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

/// Stateful stream event conversion that tracks tool call names.
fn convert_stream_event_stateful(
    event: oai::ResponseStreamEvent,
    state: &mut OpenAIStreamState,
) -> Result<StreamEvent, HyperError> {
    match event {
        oai::ResponseStreamEvent::ResponseCreated { response, .. } => {
            Ok(StreamEvent::response_created(&response.id))
        }
        oai::ResponseStreamEvent::OutputTextDelta {
            delta,
            content_index,
            ..
        } => Ok(StreamEvent::text_delta(content_index as i64, &delta)),
        oai::ResponseStreamEvent::OutputTextDone {
            text,
            content_index,
            ..
        } => Ok(StreamEvent::text_done(content_index as i64, &text)),
        oai::ResponseStreamEvent::ReasoningTextDelta {
            delta,
            content_index,
            ..
        } => Ok(StreamEvent::thinking_delta(content_index as i64, &delta)),
        oai::ResponseStreamEvent::ReasoningTextDone {
            text,
            content_index,
            ..
        } => Ok(StreamEvent::thinking_done(content_index as i64, &text)),
        oai::ResponseStreamEvent::FunctionCallArgumentsDelta {
            delta,
            output_index,
            item_id,
            ..
        } => Ok(StreamEvent::ToolCallDelta {
            index: output_index as i64,
            id: item_id,
            arguments_delta: delta,
        }),
        oai::ResponseStreamEvent::FunctionCallArgumentsDone {
            arguments,
            output_index,
            item_id,
            ..
        } => {
            // Get tracked tool call name from state
            let name = state
                .tool_calls
                .get(&(output_index as i64))
                .map(|(_, name)| name.clone())
                .unwrap_or_default();

            let args: serde_json::Value =
                serde_json::from_str(&arguments).unwrap_or(serde_json::Value::Null);
            Ok(StreamEvent::ToolCallDone {
                index: output_index as i64,
                tool_call: crate::tools::ToolCall {
                    id: item_id,
                    name,
                    arguments: args,
                },
            })
        }
        oai::ResponseStreamEvent::OutputItemAdded {
            item, output_index, ..
        } => match item {
            oai::OutputItem::FunctionCall { call_id, name, .. } => {
                state
                    .tool_calls
                    .insert(output_index as i64, (call_id.clone(), name.clone()));
                Ok(StreamEvent::ToolCallStart {
                    index: output_index as i64,
                    id: call_id,
                    name,
                })
            }
            oai::OutputItem::CustomToolCall { call_id, name, .. } => {
                state
                    .tool_calls
                    .insert(output_index as i64, (call_id.clone(), name.clone()));
                Ok(StreamEvent::ToolCallStart {
                    index: output_index as i64,
                    id: call_id,
                    name,
                })
            }
            _ => Ok(StreamEvent::Ignored),
        },
        oai::ResponseStreamEvent::ResponseCompleted { response, .. } => {
            let finish_reason = match response.stop_reason {
                Some(oai::StopReason::EndTurn) => FinishReason::Stop,
                Some(oai::StopReason::MaxTokens) => FinishReason::MaxTokens,
                Some(oai::StopReason::ToolUse) => FinishReason::ToolCalls,
                Some(oai::StopReason::StopSequence) => FinishReason::Stop,
                Some(oai::StopReason::ContentFilter) => FinishReason::ContentFilter,
                None => FinishReason::Stop,
            };

            let cached_tokens = response.usage.cached_tokens();
            let reasoning_tokens = response.usage.reasoning_tokens();

            let usage = TokenUsage {
                prompt_tokens: response.usage.input_tokens as i64,
                completion_tokens: response.usage.output_tokens as i64,
                total_tokens: response.usage.total_tokens as i64,
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

            Ok(StreamEvent::response_done_full(
                response.id,
                response.model.unwrap_or_default(),
                Some(usage),
                finish_reason,
            ))
        }
        oai::ResponseStreamEvent::CustomToolCallInputDelta {
            item_id,
            output_index,
            delta,
            ..
        } => Ok(StreamEvent::ToolCallDelta {
            index: output_index as i64,
            id: item_id,
            arguments_delta: delta,
        }),
        oai::ResponseStreamEvent::CustomToolCallInputDone {
            item_id,
            output_index,
            input,
            ..
        } => {
            let name = state
                .tool_calls
                .get(&(output_index as i64))
                .map(|(_, n)| n.clone())
                .unwrap_or_default();
            Ok(StreamEvent::ToolCallDone {
                index: output_index as i64,
                tool_call: crate::tools::ToolCall::new(
                    item_id,
                    name,
                    serde_json::Value::String(input),
                ),
            })
        }
        oai::ResponseStreamEvent::Error { code, message, .. } => Err(HyperError::ProviderError {
            code: code.unwrap_or_else(|| "unknown".to_string()),
            message,
        }),
        _ => Ok(StreamEvent::Ignored),
    }
}

fn map_openai_error(e: &oai::OpenAIError) -> HyperError {
    match e {
        oai::OpenAIError::ContextWindowExceeded => {
            HyperError::ContextWindowExceeded("Context window exceeded".to_string())
        }
        oai::OpenAIError::QuotaExceeded => {
            // QuotaExceeded is NOT retryable (requires billing change)
            // This is different from RateLimitExceeded which is retryable
            HyperError::QuotaExceeded("Quota exceeded - check your billing settings".to_string())
        }
        oai::OpenAIError::RateLimited { retry_after } => {
            // Use the retry_after value from the SDK if available
            HyperError::Retryable {
                message: "Rate limited".to_string(),
                delay: *retry_after,
            }
        }
        oai::OpenAIError::InternalServerError => {
            // 5xx server errors are retryable
            HyperError::Retryable {
                message: "Internal server error".to_string(),
                delay: None,
            }
        }
        oai::OpenAIError::Authentication(msg) => HyperError::AuthenticationFailed(msg.clone()),
        oai::OpenAIError::Api {
            status, message, ..
        } => {
            // Check for 5xx errors first - these are retryable
            if *status >= 500 {
                return HyperError::Retryable {
                    message: format!("Server error ({status}): {message}"),
                    delay: None,
                };
            }
            HyperError::ProviderError {
                code: "api_error".to_string(),
                message: message.clone(),
            }
        }
        _ => HyperError::ProviderError {
            code: "openai_error".to_string(),
            message: e.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder() {
        let result = OpenAIProvider::builder()
            .api_key("sk-test-key")
            .base_url("https://custom.openai.com")
            .organization_id("org-123")
            .timeout_secs(120)
            .build();

        assert!(result.is_ok());
        let provider = result.unwrap();
        assert_eq!(provider.name(), "openai");
        assert_eq!(provider.api_key(), "sk-test-key");
        assert_eq!(provider.base_url(), "https://custom.openai.com");
    }

    #[test]
    fn test_builder_missing_key() {
        let result = OpenAIProvider::builder().build();
        assert!(result.is_err());
    }
}
