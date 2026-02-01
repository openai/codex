//! Unified stream abstraction over streaming and non-streaming responses.
//!
//! This module provides [`UnifiedStream`] which provides a consistent interface
//! for both streaming and non-streaming API responses. The agent loop can use
//! the same code path regardless of whether streaming is enabled.

use crate::error::ApiError;
use crate::error::Result;
use cocode_protocol::TokenUsage as ProtocolUsage;
use hyper_sdk::ContentBlock;
use hyper_sdk::FinishReason;
use hyper_sdk::GenerateResponse;
use hyper_sdk::Message;
use hyper_sdk::ProviderMetadata;
use hyper_sdk::Role;
use hyper_sdk::StreamProcessor;
use hyper_sdk::StreamSnapshot;
use hyper_sdk::StreamUpdate;
use hyper_sdk::TokenUsage;
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::mpsc;

/// Type of result from the unified stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryResultType {
    /// Assistant message with completed content.
    Assistant,
    /// UI update event (delta).
    Event,
    /// Retry attempt indicator.
    Retry,
    /// Error occurred.
    Error,
    /// Stream is complete.
    Done,
}

/// Result from a unified stream iteration.
#[derive(Debug, Clone)]
pub struct StreamingQueryResult {
    /// Type of this result.
    pub result_type: QueryResultType,
    /// Completed content blocks (for Assistant type).
    pub content: Vec<ContentBlock>,
    /// Stream event for UI updates.
    pub event: Option<StreamUpdate>,
    /// Error if result_type is Error.
    pub error: Option<String>,
    /// Token usage (available on Done).
    pub usage: Option<ProtocolUsage>,
    /// Finish reason (available on Done).
    pub finish_reason: Option<FinishReason>,
}

impl StreamingQueryResult {
    /// Create an assistant result with completed content.
    pub fn assistant(content: Vec<ContentBlock>) -> Self {
        Self {
            result_type: QueryResultType::Assistant,
            content,
            event: None,
            error: None,
            usage: None,
            finish_reason: None,
        }
    }

    /// Create an event result for UI updates.
    pub fn event(update: StreamUpdate) -> Self {
        Self {
            result_type: QueryResultType::Event,
            content: Vec::new(),
            event: Some(update),
            error: None,
            usage: None,
            finish_reason: None,
        }
    }

    /// Create a retry result.
    pub fn retry() -> Self {
        Self {
            result_type: QueryResultType::Retry,
            content: Vec::new(),
            event: None,
            error: None,
            usage: None,
            finish_reason: None,
        }
    }

    /// Create an error result.
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            result_type: QueryResultType::Error,
            content: Vec::new(),
            event: None,
            error: Some(message.into()),
            usage: None,
            finish_reason: None,
        }
    }

    /// Create a done result.
    pub fn done(usage: Option<ProtocolUsage>, finish_reason: FinishReason) -> Self {
        Self {
            result_type: QueryResultType::Done,
            content: Vec::new(),
            event: None,
            error: None,
            usage,
            finish_reason: Some(finish_reason),
        }
    }

    /// Check if this is an assistant result with content.
    pub fn has_content(&self) -> bool {
        self.result_type == QueryResultType::Assistant && !self.content.is_empty()
    }

    /// Check if this result has tool calls.
    pub fn has_tool_calls(&self) -> bool {
        self.content
            .iter()
            .any(|b| matches!(b, ContentBlock::ToolUse { .. }))
    }

    /// Get tool calls from this result.
    pub fn tool_calls(&self) -> Vec<&ContentBlock> {
        self.content
            .iter()
            .filter(|b| matches!(b, ContentBlock::ToolUse { .. }))
            .collect()
    }
}

/// Inner implementation of unified stream.
enum UnifiedStreamInner {
    /// Streaming mode using StreamProcessor.
    Streaming(StreamProcessor),
    /// Non-streaming mode with a single response.
    NonStreaming(Option<GenerateResponse>),
}

/// Unified abstraction for streaming and non-streaming API responses.
///
/// This provides a consistent interface for the agent loop to consume API
/// responses regardless of whether streaming is enabled. In streaming mode,
/// it yields results as content blocks complete. In non-streaming mode,
/// it yields a single result with all content.
pub struct UnifiedStream {
    inner: UnifiedStreamInner,
    event_tx: Option<mpsc::Sender<StreamUpdate>>,
}

impl UnifiedStream {
    /// Create a unified stream from a streaming processor.
    pub fn from_stream(processor: StreamProcessor) -> Self {
        Self {
            inner: UnifiedStreamInner::Streaming(processor),
            event_tx: None,
        }
    }

    /// Create a unified stream from a non-streaming response.
    pub fn from_response(response: GenerateResponse) -> Self {
        Self {
            inner: UnifiedStreamInner::NonStreaming(Some(response)),
            event_tx: None,
        }
    }

    /// Set an event sender for UI updates.
    pub fn with_event_sender(mut self, tx: mpsc::Sender<StreamUpdate>) -> Self {
        self.event_tx = Some(tx);
        self
    }

    /// Get the next result from the stream.
    ///
    /// Returns `None` when the stream is complete.
    pub async fn next(&mut self) -> Option<Result<StreamingQueryResult>> {
        match &mut self.inner {
            UnifiedStreamInner::Streaming(processor) => {
                Self::process_streaming_event(processor, &self.event_tx).await
            }
            UnifiedStreamInner::NonStreaming(opt) => Self::process_non_streaming(opt.take()),
        }
    }

    /// Process streaming events from the processor.
    async fn process_streaming_event(
        processor: &mut StreamProcessor,
        event_tx: &Option<mpsc::Sender<StreamUpdate>>,
    ) -> Option<Result<StreamingQueryResult>> {
        loop {
            match processor.next().await {
                Some(Ok((update, snapshot))) => {
                    // Send update to UI if configured
                    if let Some(tx) = event_tx {
                        if let Err(e) = tx.send(update.clone()).await {
                            tracing::debug!("Failed to send stream event to UI: {e}");
                        }
                    }

                    // Check for completed content
                    let completed = Self::check_for_completed_content(&update);

                    if let Some(result) = completed {
                        return Some(Ok(result));
                    }

                    // Check if stream is done
                    if snapshot.is_complete {
                        let usage = Self::convert_usage(&snapshot);
                        let finish_reason = snapshot.finish_reason.unwrap_or(FinishReason::Stop);
                        return Some(Ok(StreamingQueryResult::done(usage, finish_reason)));
                    }

                    // Continue to next event for deltas
                    if update.is_delta() {
                        continue;
                    }
                }
                Some(Err(e)) => {
                    return Some(Err(ApiError::from(e)));
                }
                None => {
                    return None;
                }
            }
        }
    }

    /// Convert snapshot usage to protocol usage.
    fn convert_usage(snapshot: &StreamSnapshot) -> Option<ProtocolUsage> {
        snapshot.usage.as_ref().map(|u| ProtocolUsage {
            input_tokens: u.prompt_tokens,
            output_tokens: u.completion_tokens,
            cache_read_tokens: u.cache_read_tokens,
            cache_creation_tokens: u.cache_creation_tokens,
            reasoning_tokens: u.reasoning_tokens,
        })
    }

    /// Check if the update indicates completed content.
    fn check_for_completed_content(update: &StreamUpdate) -> Option<StreamingQueryResult> {
        match update {
            StreamUpdate::TextDone { text, .. } => {
                if !text.is_empty() {
                    Some(StreamingQueryResult::assistant(vec![ContentBlock::text(
                        text,
                    )]))
                } else {
                    None
                }
            }
            StreamUpdate::ThinkingDone {
                content, signature, ..
            } => Some(StreamingQueryResult::assistant(vec![
                ContentBlock::Thinking {
                    content: content.clone(),
                    signature: signature.clone(),
                },
            ])),
            StreamUpdate::ToolCallCompleted { tool_call, .. } => {
                Some(StreamingQueryResult::assistant(vec![
                    ContentBlock::tool_use(
                        &tool_call.id,
                        &tool_call.name,
                        tool_call.arguments.clone(),
                    ),
                ]))
            }
            _ => None,
        }
    }

    /// Handle non-streaming response.
    fn process_non_streaming(
        response: Option<GenerateResponse>,
    ) -> Option<Result<StreamingQueryResult>> {
        let response = response?;

        // Build usage
        let usage = response.usage.as_ref().map(|u| ProtocolUsage {
            input_tokens: u.prompt_tokens,
            output_tokens: u.completion_tokens,
            cache_read_tokens: u.cache_read_tokens,
            cache_creation_tokens: u.cache_creation_tokens,
            reasoning_tokens: u.reasoning_tokens,
        });

        // Return all content at once
        Some(Ok(StreamingQueryResult {
            result_type: QueryResultType::Assistant,
            content: response.content,
            event: None,
            error: None,
            usage,
            finish_reason: Some(response.finish_reason),
        }))
    }

    /// Collect all results into a single response.
    pub async fn collect(mut self) -> Result<CollectedResponse> {
        let mut content = Vec::new();
        let mut usage = None;
        let mut finish_reason = FinishReason::Stop;

        while let Some(result) = self.next().await {
            let result = result?;

            match result.result_type {
                QueryResultType::Assistant => {
                    content.extend(result.content);
                    // Capture usage from non-streaming responses
                    if result.usage.is_some() {
                        usage = result.usage;
                    }
                    if result.finish_reason.is_some() {
                        finish_reason = result.finish_reason.unwrap();
                    }
                }
                QueryResultType::Done => {
                    usage = result.usage;
                    finish_reason = result.finish_reason.unwrap_or(FinishReason::Stop);
                    break;
                }
                QueryResultType::Error => {
                    return Err(crate::error::api_error::StreamSnafu {
                        message: result.error.unwrap_or_default(),
                    }
                    .build());
                }
                _ => {}
            }
        }

        Ok(CollectedResponse {
            content,
            usage,
            finish_reason,
        })
    }
}

impl std::fmt::Debug for UnifiedStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mode = match &self.inner {
            UnifiedStreamInner::Streaming(_) => "Streaming",
            UnifiedStreamInner::NonStreaming(_) => "NonStreaming",
        };
        f.debug_struct("UnifiedStream")
            .field("mode", &mode)
            .finish_non_exhaustive()
    }
}

/// Collected response from a unified stream.
#[derive(Debug, Clone)]
pub struct CollectedResponse {
    /// All content blocks.
    pub content: Vec<ContentBlock>,
    /// Token usage.
    pub usage: Option<ProtocolUsage>,
    /// Finish reason.
    pub finish_reason: FinishReason,
}

impl CollectedResponse {
    /// Get the text content.
    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    /// Get thinking content if present.
    pub fn thinking(&self) -> Option<&str> {
        self.content.iter().find_map(|b| match b {
            ContentBlock::Thinking { content, .. } => Some(content.as_str()),
            _ => None,
        })
    }

    /// Get tool calls.
    pub fn tool_calls(&self) -> Vec<&ContentBlock> {
        self.content
            .iter()
            .filter(|b| matches!(b, ContentBlock::ToolUse { .. }))
            .collect()
    }

    /// Check if the response has tool calls.
    pub fn has_tool_calls(&self) -> bool {
        self.content
            .iter()
            .any(|b| matches!(b, ContentBlock::ToolUse { .. }))
    }

    /// Convert to a Message for history management.
    ///
    /// Creates an assistant message with proper source metadata.
    /// Use this when the agent loop manages history directly.
    ///
    /// # Arguments
    ///
    /// * `provider` - Provider name (e.g., "openai", "anthropic")
    /// * `model` - Model name (e.g., "gpt-4o", "claude-sonnet-4")
    ///
    /// # Example
    ///
    /// ```ignore
    /// let collected = stream.collect().await?;
    /// let msg = collected.into_message("anthropic", "claude-sonnet-4");
    /// history.push(msg);
    /// ```
    pub fn into_message(self, provider: &str, model: &str) -> Message {
        let mut msg = Message::new(Role::Assistant, self.content);
        msg.metadata = ProviderMetadata::with_source(provider, model);
        msg
    }

    /// Convert to GenerateResponse.
    ///
    /// Useful when you need the full response structure with ID.
    ///
    /// # Arguments
    ///
    /// * `id` - Response ID
    /// * `model` - Model name that generated the response
    pub fn into_response(
        self,
        id: impl Into<String>,
        model: impl Into<String>,
    ) -> GenerateResponse {
        GenerateResponse {
            id: id.into(),
            content: self.content,
            finish_reason: self.finish_reason,
            usage: self.usage.map(|u| TokenUsage {
                prompt_tokens: u.input_tokens,
                completion_tokens: u.output_tokens,
                total_tokens: u.input_tokens + u.output_tokens,
                cache_read_tokens: u.cache_read_tokens,
                cache_creation_tokens: u.cache_creation_tokens,
                reasoning_tokens: u.reasoning_tokens,
            }),
            model: model.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyper_sdk::TokenUsage;

    fn make_response(text: &str) -> GenerateResponse {
        GenerateResponse {
            id: "resp_1".to_string(),
            content: vec![ContentBlock::text(text)],
            finish_reason: FinishReason::Stop,
            usage: Some(TokenUsage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
                cache_read_tokens: None,
                cache_creation_tokens: None,
                reasoning_tokens: None,
            }),
            model: "test-model".to_string(),
        }
    }

    #[tokio::test]
    async fn test_non_streaming_response() {
        let response = make_response("Hello, world!");
        let mut stream = UnifiedStream::from_response(response);

        let result = stream.next().await;
        assert!(result.is_some());

        let result = result.unwrap().unwrap();
        assert_eq!(result.result_type, QueryResultType::Assistant);
        assert!(!result.content.is_empty());

        // Should be consumed
        let result = stream.next().await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_collect_non_streaming() {
        let response = make_response("Hello!");
        let stream = UnifiedStream::from_response(response);

        let collected = stream.collect().await.unwrap();
        assert_eq!(collected.text(), "Hello!");
        assert_eq!(collected.finish_reason, FinishReason::Stop);
        assert!(collected.usage.is_some());
    }

    #[test]
    fn test_streaming_query_result_constructors() {
        let assistant = StreamingQueryResult::assistant(vec![ContentBlock::text("test")]);
        assert!(assistant.has_content());

        let event = StreamingQueryResult::event(StreamUpdate::TextDelta {
            index: 0,
            delta: "hi".to_string(),
        });
        assert_eq!(event.result_type, QueryResultType::Event);

        let retry = StreamingQueryResult::retry();
        assert_eq!(retry.result_type, QueryResultType::Retry);

        let error = StreamingQueryResult::error("test error");
        assert_eq!(error.result_type, QueryResultType::Error);
        assert_eq!(error.error, Some("test error".to_string()));

        let done = StreamingQueryResult::done(None, FinishReason::Stop);
        assert_eq!(done.result_type, QueryResultType::Done);
    }

    #[test]
    fn test_tool_call_detection() {
        let result = StreamingQueryResult::assistant(vec![
            ContentBlock::text("Let me help"),
            ContentBlock::tool_use("call_1", "get_weather", serde_json::json!({"city": "NYC"})),
        ]);

        assert!(result.has_tool_calls());
        assert_eq!(result.tool_calls().len(), 1);
    }

    #[test]
    fn test_collected_response_into_message() {
        let collected = CollectedResponse {
            content: vec![ContentBlock::text("Hello, world!")],
            usage: Some(ProtocolUsage {
                input_tokens: 10,
                output_tokens: 5,
                cache_read_tokens: None,
                cache_creation_tokens: None,
                reasoning_tokens: None,
            }),
            finish_reason: FinishReason::Stop,
        };

        let msg = collected.into_message("anthropic", "claude-sonnet-4");

        assert_eq!(msg.role, Role::Assistant);
        assert_eq!(msg.text(), "Hello, world!");
        assert_eq!(msg.source_provider(), Some("anthropic"));
        assert_eq!(msg.source_model(), Some("claude-sonnet-4"));
    }

    #[test]
    fn test_collected_response_into_response() {
        let collected = CollectedResponse {
            content: vec![ContentBlock::text("Response text")],
            usage: Some(ProtocolUsage {
                input_tokens: 100,
                output_tokens: 50,
                cache_read_tokens: Some(20),
                cache_creation_tokens: None,
                reasoning_tokens: None,
            }),
            finish_reason: FinishReason::Stop,
        };

        let response = collected.into_response("resp_123", "gpt-4o");

        assert_eq!(response.id, "resp_123");
        assert_eq!(response.model, "gpt-4o");
        assert_eq!(response.finish_reason, FinishReason::Stop);
        assert_eq!(response.text(), "Response text");

        let usage = response.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.completion_tokens, 50);
        assert_eq!(usage.total_tokens, 150);
        assert_eq!(usage.cache_read_tokens, Some(20));
    }
}
