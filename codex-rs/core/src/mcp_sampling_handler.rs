use std::sync::Arc;

use anyhow::Result;
use codex_rmcp_client::SamplingHandler;
use futures::StreamExt;
use tokio::sync::RwLock;
use tracing::debug;
use tracing::info;
use tracing::warn;

use crate::client_common::Prompt;
use crate::client_common::ResponseEvent;
use crate::config::Config;
use codex_protocol::ConversationId;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseInputItem;

/// Implementation of SamplingHandler that creates independent LLM calls
/// for MCP sampling requests, respecting the request's systemPrompt and
/// model preferences rather than reusing the session's ModelClient.
pub struct CodexSamplingHandler {
    config: RwLock<Option<Arc<Config>>>,
}

impl Default for CodexSamplingHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl CodexSamplingHandler {
    pub fn new() -> Self {
        Self {
            config: RwLock::new(None),
        }
    }

    /// Set the Config to use for sampling requests.
    /// This must be called before any sampling requests are made.
    pub async fn set_config(&self, config: Arc<Config>) {
        *self.config.write().await = Some(config);
    }

    /// Get a clone of the config if it has been initialized.
    async fn get_config(&self) -> Result<Arc<Config>, rmcp::ErrorData> {
        self.config.read().await.clone().ok_or_else(|| {
            warn!("Sampling requested but Config not yet initialized");
            rmcp::ErrorData::internal_error("Config not initialized for sampling", None)
        })
    }
}

#[async_trait::async_trait]
impl SamplingHandler for CodexSamplingHandler {
    async fn create_message(
        &self,
        params: rmcp::model::CreateMessageRequestParam,
    ) -> Result<rmcp::model::CreateMessageResult, rmcp::ErrorData> {
        use rmcp::model::Content;
        use rmcp::model::CreateMessageResult;
        use rmcp::model::Role;
        use rmcp::model::SamplingMessage;

        info!(
            "Processing MCP sampling request with {} messages",
            params.messages.len()
        );

        let config = self.get_config().await?;

        // Build prompt for MCP sampling request.
        // Per MCP spec: use the provided systemPrompt, or empty string if not provided
        // (not Codex's default instructions).
        let items = convert_sampling_messages_to_items(&params)?;
        let system_prompt = params.system_prompt.clone().unwrap_or_default();
        let prompt = Prompt {
            input: items.into_iter().map(Into::into).collect(),
            base_instructions_override: Some(system_prompt),
            ..Default::default()
        };

        // Select model based on preferences or use config's default
        let model_name = select_model_from_preferences(&params, &config);
        debug!(
            "Calling LLM for sampling with model={}, {} items, system_prompt={}",
            model_name,
            prompt.input.len(),
            params.system_prompt.is_some()
        );

        // Create a temporary client for this sampling request
        let response_stream = call_llm_for_sampling(&prompt, &model_name, &config).await?;

        let (response_text, stop_reason) = collect_response_from_stream(response_stream).await?;

        info!(
            "Generated sampling response with {} characters",
            response_text.len()
        );

        Ok(CreateMessageResult {
            message: SamplingMessage {
                role: Role::Assistant,
                content: Content::text(&response_text),
            },
            model: model_name,
            stop_reason,
        })
    }
}

/// Select model based on MCP sampling preferences or fall back to config default.
fn select_model_from_preferences(
    params: &rmcp::model::CreateMessageRequestParam,
    config: &Config,
) -> String {
    if let Some(prefs) = &params.model_preferences
        && let Some(hints) = &prefs.hints
    {
        for hint in hints {
            if let Some(name) = &hint.name {
                debug!("Using model from MCP preference: {name}");
                return name.clone();
            }
        }
    }

    // Fall back to config's model
    config.model.clone()
}

/// Call the LLM directly for sampling, bypassing the session's ModelClient.
async fn call_llm_for_sampling(
    prompt: &Prompt,
    model: &str,
    config: &Config,
) -> Result<crate::client_common::ResponseStream, rmcp::ErrorData> {
    use crate::chat_completions::stream_chat_completions;
    use crate::default_client::create_client;
    use crate::model_family::find_family_for_model;
    use codex_otel::otel_event_manager::OtelEventManager;

    let model_family = find_family_for_model(model).ok_or_else(|| {
        warn!("Unknown model family for sampling: {model}");
        rmcp::ErrorData::invalid_params(format!("Unknown model: {model}"), None)
    })?;

    let client = create_client();
    let otel_manager = OtelEventManager::new(
        ConversationId::new(),
        model,
        &model_family.slug,
        None,  // No account_id for sampling
        None,  // No auth_mode for sampling
        false, // Don't log user prompts
        "mcp-sampling".to_string(),
    );

    stream_chat_completions(
        prompt,
        &model_family,
        &client,
        &config.model_provider,
        &otel_manager,
    )
    .await
    .map_err(|err| {
        warn!("LLM call failed for sampling: {err}");
        rmcp::ErrorData::internal_error(format!("LLM call failed: {err}"), None)
    })
}

/// Convert MCP sampling messages to Codex response input items.
fn convert_sampling_messages_to_items(
    params: &rmcp::model::CreateMessageRequestParam,
) -> Result<Vec<ResponseInputItem>, rmcp::ErrorData> {
    use rmcp::model::Role;

    let mut items = Vec::new();

    // Convert each sampling message
    for msg in &params.messages {
        let role = match msg.role {
            Role::User => "user",
            Role::Assistant => "assistant",
        }
        .to_string();

        let content = convert_message_content(&msg.content)?;
        items.push(ResponseInputItem::Message { role, content });
    }

    Ok(items)
}

/// Convert MCP message content to Codex content items.
fn convert_message_content(
    content: &rmcp::model::Content,
) -> Result<Vec<ContentItem>, rmcp::ErrorData> {
    match &content.raw {
        rmcp::model::RawContent::Text(text_content) => Ok(vec![ContentItem::InputText {
            text: text_content.text.clone(),
        }]),
        rmcp::model::RawContent::Image(_) => {
            warn!("Image content in sampling messages is not yet supported");
            Err(rmcp::ErrorData::invalid_params(
                "Image content is not supported",
                None,
            ))
        }
        rmcp::model::RawContent::Resource(_) | rmcp::model::RawContent::ResourceLink(_) => {
            warn!("Resource content in sampling messages is not yet supported");
            Err(rmcp::ErrorData::invalid_params(
                "Resource content is not supported",
                None,
            ))
        }
        rmcp::model::RawContent::Audio(_) => {
            warn!("Audio content in sampling messages is not yet supported");
            Err(rmcp::ErrorData::invalid_params(
                "Audio content is not supported",
                None,
            ))
        }
    }
}

/// Collect the text response from the model's response stream.
async fn collect_response_from_stream(
    response_stream: crate::client_common::ResponseStream,
) -> Result<(String, Option<String>), rmcp::ErrorData> {
    use rmcp::model::CreateMessageResult;

    let mut response_text = String::new();
    let mut stop_reason = None;

    tokio::pin!(response_stream);

    while let Some(event_result) = response_stream.next().await {
        match event_result {
            Ok(ResponseEvent::OutputItemDone(item)) => {
                extract_text_from_item(&item, &mut response_text);
            }
            Ok(ResponseEvent::OutputTextDelta(text)) => {
                response_text.push_str(&text);
            }
            Ok(ResponseEvent::Completed {
                response_id,
                token_usage,
            }) => {
                debug!("Response completed: id={response_id}, tokens={token_usage:?}");
                stop_reason = Some(CreateMessageResult::STOP_REASON_END_TURN.to_string());
            }
            Ok(ResponseEvent::Created) => {
                debug!("Response created");
            }
            Ok(ResponseEvent::RateLimits(_)) => {
                debug!("Rate limits info received during sampling");
            }
            Ok(ResponseEvent::ReasoningSummaryDelta(_))
            | Ok(ResponseEvent::ReasoningContentDelta(_))
            | Ok(ResponseEvent::ReasoningSummaryPartAdded)
            | Ok(ResponseEvent::WebSearchCallBegin { .. }) => {
                // Ignore reasoning and web search events
            }
            Err(err) => {
                warn!("Error in response stream: {err}");
                return Err(rmcp::ErrorData::internal_error(
                    format!("Stream error: {err}"),
                    None,
                ));
            }
        }
    }

    if response_text.is_empty() {
        warn!("Model returned empty response");
        response_text = "No response from model".to_string();
    }

    Ok((response_text, stop_reason))
}

/// Extract text content from a response item.
fn extract_text_from_item(item: &codex_protocol::models::ResponseItem, output: &mut String) {
    if let codex_protocol::models::ResponseItem::Message { content, .. } = item {
        for content_item in content {
            if let ContentItem::OutputText { text } = content_item {
                output.push_str(text);
            }
        }
    }
}
