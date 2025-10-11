use std::sync::Arc;

use anyhow::Result;
use codex_rmcp_client::SamplingHandler;
use futures::StreamExt;
use tokio::sync::RwLock;
use tracing::debug;
use tracing::info;
use tracing::warn;

use crate::client::ModelClient;
use crate::client_common::Prompt;
use crate::client_common::ResponseEvent;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseInputItem;

/// Implementation of SamplingHandler that uses Codex's ModelClient to provide
/// real LLM responses to MCP sampling requests.
///
/// The ModelClient is stored in an RwLock because it needs to be set
/// after the handler is created (due to initialization order constraints).
pub struct CodexSamplingHandler {
    model_client: RwLock<Option<Arc<ModelClient>>>,
}

impl Default for CodexSamplingHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl CodexSamplingHandler {
    pub fn new() -> Self {
        Self {
            model_client: RwLock::new(None),
        }
    }

    /// Set the ModelClient to use for sampling requests.
    /// This must be called before any sampling requests are made.
    pub async fn set_model_client(&self, client: Arc<ModelClient>) {
        *self.model_client.write().await = Some(client);
    }

    /// Get a clone of the model client if it has been initialized.
    async fn get_model_client(&self) -> Result<Arc<ModelClient>, rmcp::ErrorData> {
        self.model_client.read().await.clone().ok_or_else(|| {
            warn!("Sampling requested but ModelClient not yet initialized");
            rmcp::ErrorData::internal_error("ModelClient not initialized for sampling", None)
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

        let model_client = self.get_model_client().await?;

        let items = convert_sampling_messages_to_items(&params)?;

        // Create prompt from items
        let prompt = Prompt {
            input: items.into_iter().map(Into::into).collect(),
            ..Default::default()
        };

        debug!("Calling ModelClient with {} items", prompt.input.len());
        let response_stream = model_client.stream(&prompt).await.map_err(|err| {
            warn!("ModelClient stream failed: {err}");
            rmcp::ErrorData::internal_error(format!("LLM call failed: {err}"), None)
        })?;

        let (response_text, stop_reason) = collect_response_from_stream(response_stream).await?;
        let model_name = model_client.get_model();

        info!("Generated response with {} characters", response_text.len());

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

/// Convert MCP sampling messages to Codex response input items.
fn convert_sampling_messages_to_items(
    params: &rmcp::model::CreateMessageRequestParam,
) -> Result<Vec<ResponseInputItem>, rmcp::ErrorData> {
    use rmcp::model::Role;

    let mut items = Vec::new();

    // Add system prompt if provided
    if let Some(system_prompt) = &params.system_prompt {
        items.push(ResponseInputItem::Message {
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: system_prompt.clone(),
            }],
        });
    }

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
