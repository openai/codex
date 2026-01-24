//! Stream response wrapper for consuming streaming results.

use super::events::StreamEvent;
use crate::error::HyperError;
use crate::messages::ContentBlock;
use crate::response::FinishReason;
use crate::response::GenerateResponse;
use crate::response::TokenUsage;
use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::time::Duration;
use tokio::time::timeout;

/// Type alias for the underlying event stream.
pub type EventStream = Pin<Box<dyn Stream<Item = Result<StreamEvent, HyperError>> + Send>>;

/// Default idle timeout for streams (60 seconds).
pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(60);

/// Configuration for stream behavior.
#[derive(Debug, Clone)]
pub struct StreamConfig {
    /// Maximum time to wait between events before timing out.
    /// If no event is received within this duration, the stream will return an error.
    pub idle_timeout: Duration,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
        }
    }
}

/// State accumulated during streaming.
#[derive(Debug, Default)]
struct StreamState {
    /// Response ID.
    response_id: Option<String>,
    /// Accumulated text by content block index.
    text_blocks: HashMap<i64, String>,
    /// Accumulated thinking by content block index.
    thinking_blocks: HashMap<i64, ThinkingState>,
    /// Tool calls by content block index.
    tool_calls: HashMap<i64, ToolCallState>,
    /// Final token usage.
    usage: Option<TokenUsage>,
    /// Finish reason.
    finish_reason: Option<FinishReason>,
    /// Model name.
    model: Option<String>,
}

#[derive(Debug, Default)]
struct ThinkingState {
    content: String,
    signature: Option<String>,
}

#[derive(Debug, Default)]
struct ToolCallState {
    id: String,
    name: String,
    arguments: String,
}

/// Response wrapper for streaming generation.
///
/// Provides methods to consume the stream as events, as a text stream,
/// or to wait for the complete response.
///
/// For advanced use cases with accumulated state, convert to [`StreamProcessor`]
/// using [`into_processor()`](Self::into_processor).
///
/// # Idle Timeout
///
/// The stream includes an idle timeout (default 60 seconds) to prevent hanging
/// on unresponsive streams. Use [`with_config`](Self::with_config) or
/// [`idle_timeout`](Self::idle_timeout) to customize this behavior.
pub struct StreamResponse {
    /// The underlying event stream.
    pub(crate) inner: EventStream,
    state: StreamState,
    config: StreamConfig,
}

impl StreamResponse {
    /// Create a new stream response from an event stream.
    ///
    /// Uses the default idle timeout of 60 seconds.
    pub fn new(inner: EventStream) -> Self {
        Self {
            inner,
            state: StreamState::default(),
            config: StreamConfig::default(),
        }
    }

    /// Create a new stream response with custom configuration.
    pub fn with_config(inner: EventStream, config: StreamConfig) -> Self {
        Self {
            inner,
            state: StreamState::default(),
            config,
        }
    }

    /// Set the idle timeout for the stream.
    ///
    /// This is a builder method that allows chaining.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let stream = StreamResponse::new(event_stream)
    ///     .idle_timeout(Duration::from_secs(120));
    /// ```
    pub fn idle_timeout(mut self, timeout: Duration) -> Self {
        self.config.idle_timeout = timeout;
        self
    }

    /// Get the current stream configuration.
    pub fn config(&self) -> &StreamConfig {
        &self.config
    }

    /// Get the next event from the stream.
    ///
    /// Returns `None` when the stream is exhausted.
    /// Returns `Err(HyperError::StreamIdleTimeout)` if no event is received
    /// within the configured idle timeout.
    pub async fn next_event(&mut self) -> Option<Result<StreamEvent, HyperError>> {
        use futures::StreamExt;

        let result = timeout(self.config.idle_timeout, self.inner.next()).await;

        match result {
            Ok(Some(event)) => {
                // Update internal state based on event
                if let Ok(ref ev) = event {
                    self.update_state(ev);
                }
                Some(event)
            }
            Ok(None) => None, // Stream exhausted
            Err(_) => Some(Err(HyperError::StreamIdleTimeout(self.config.idle_timeout))),
        }
    }

    fn update_state(&mut self, event: &StreamEvent) {
        match event {
            StreamEvent::ResponseCreated { id } => {
                self.state.response_id = Some(id.clone());
            }
            StreamEvent::TextDelta { index, delta } => {
                self.state
                    .text_blocks
                    .entry(*index)
                    .or_default()
                    .push_str(delta);
            }
            StreamEvent::TextDone { index, text } => {
                self.state.text_blocks.insert(*index, text.clone());
            }
            StreamEvent::ThinkingDelta { index, delta } => {
                self.state
                    .thinking_blocks
                    .entry(*index)
                    .or_default()
                    .content
                    .push_str(delta);
            }
            StreamEvent::ThinkingDone {
                index,
                content,
                signature,
            } => {
                self.state.thinking_blocks.insert(
                    *index,
                    ThinkingState {
                        content: content.clone(),
                        signature: signature.clone(),
                    },
                );
            }
            StreamEvent::ToolCallStart { index, id, name } => {
                self.state.tool_calls.insert(
                    *index,
                    ToolCallState {
                        id: id.clone(),
                        name: name.clone(),
                        arguments: String::new(),
                    },
                );
            }
            StreamEvent::ToolCallDelta {
                index,
                arguments_delta,
                ..
            } => {
                if let Some(tc) = self.state.tool_calls.get_mut(index) {
                    tc.arguments.push_str(arguments_delta);
                }
            }
            StreamEvent::ToolCallDone { index, tool_call } => {
                self.state.tool_calls.insert(
                    *index,
                    ToolCallState {
                        id: tool_call.id.clone(),
                        name: tool_call.name.clone(),
                        arguments: tool_call.arguments.to_string(),
                    },
                );
            }
            StreamEvent::ResponseDone {
                usage,
                finish_reason,
                ..
            } => {
                self.state.usage = usage.clone();
                self.state.finish_reason = Some(*finish_reason);
            }
            StreamEvent::Error(_) => {}
            StreamEvent::Ignored => {
                // Explicitly ignored - no state change
            }
        }
    }

    /// Get the current accumulated text.
    pub fn current_text(&self) -> String {
        let mut indices: Vec<_> = self.state.text_blocks.keys().collect();
        indices.sort();
        indices
            .into_iter()
            .filter_map(|i| self.state.text_blocks.get(i))
            .cloned()
            .collect::<Vec<_>>()
            .join("")
    }

    /// Convert to a text-only stream.
    ///
    /// Returns a stream that yields only text deltas.
    pub fn text_stream(self) -> impl Stream<Item = Result<String, HyperError>> + Send {
        use futures::StreamExt;

        self.inner.filter_map(|result| async move {
            match result {
                Ok(StreamEvent::TextDelta { delta, .. }) => Some(Ok(delta)),
                Ok(_) => None,
                Err(e) => Some(Err(e)),
            }
        })
    }

    /// Consume the stream and return the final concatenated text.
    pub async fn get_final_text(mut self) -> Result<String, HyperError> {
        while let Some(result) = self.next_event().await {
            result?;
        }
        Ok(self.current_text())
    }

    /// Consume the stream and return the final response.
    pub async fn get_final_response(mut self) -> Result<GenerateResponse, HyperError> {
        while let Some(result) = self.next_event().await {
            result?;
        }
        self.build_response()
    }

    /// Convert to a StreamProcessor for Crush-like accumulated state processing.
    ///
    /// This consumes the StreamResponse and returns a StreamProcessor which
    /// provides the closure-based API with accumulated state.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let response = model.stream(request).await?.into_processor()
    ///     .on_update(|snapshot| async move {
    ///         db.update_message(msg_id, &snapshot.text).await?;
    ///         Ok(())
    ///     })
    ///     .await?;
    /// ```
    pub fn into_processor(self) -> super::processor::StreamProcessor {
        super::processor::StreamProcessor::new(self.inner)
    }

    /// Build the final response from accumulated state.
    pub fn build_response(self) -> Result<GenerateResponse, HyperError> {
        let mut content = Vec::new();

        // Collect all indices and sort
        let mut all_indices: Vec<i64> = self
            .state
            .text_blocks
            .keys()
            .chain(self.state.thinking_blocks.keys())
            .chain(self.state.tool_calls.keys())
            .copied()
            .collect();
        all_indices.sort();
        all_indices.dedup();

        for index in all_indices {
            // Add thinking first (if present)
            if let Some(thinking) = self.state.thinking_blocks.get(&index) {
                content.push(ContentBlock::Thinking {
                    content: thinking.content.clone(),
                    signature: thinking.signature.clone(),
                });
            }

            // Add text (if present)
            if let Some(text) = self.state.text_blocks.get(&index) {
                content.push(ContentBlock::text(text));
            }

            // Add tool call (if present)
            if let Some(tc) = self.state.tool_calls.get(&index) {
                let args: serde_json::Value = match serde_json::from_str(&tc.arguments) {
                    Ok(value) => value,
                    Err(err) => {
                        tracing::debug!(
                            tool_call_id = %tc.id,
                            tool_name = %tc.name,
                            arguments = %tc.arguments,
                            error = %err,
                            "Failed to parse tool call arguments, using null"
                        );
                        serde_json::Value::Null
                    }
                };
                content.push(ContentBlock::tool_use(&tc.id, &tc.name, args));
            }
        }

        let response = GenerateResponse {
            id: self
                .state
                .response_id
                .unwrap_or_else(|| "unknown".to_string()),
            content,
            finish_reason: self.state.finish_reason.unwrap_or(FinishReason::Stop),
            usage: self.state.usage,
            model: self.state.model.unwrap_or_else(|| "unknown".to_string()),
        };

        Ok(response)
    }
}

impl std::fmt::Debug for StreamResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamResponse")
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;

    fn make_stream(
        events: Vec<StreamEvent>,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamEvent, HyperError>> + Send>> {
        Box::pin(stream::iter(events.into_iter().map(Ok)))
    }

    #[tokio::test]
    async fn test_stream_response_text() {
        let events = vec![
            StreamEvent::response_created("resp_1"),
            StreamEvent::text_delta(0, "Hello "),
            StreamEvent::text_delta(0, "world!"),
            StreamEvent::text_done(0, "Hello world!"),
            StreamEvent::response_done("resp_1", FinishReason::Stop),
        ];

        let stream = StreamResponse::new(make_stream(events));
        let text = stream.get_final_text().await.unwrap();
        assert_eq!(text, "Hello world!");
    }

    #[tokio::test]
    async fn test_stream_response_with_thinking() {
        let events = vec![
            StreamEvent::response_created("resp_1"),
            StreamEvent::thinking_delta(0, "Let me think..."),
            StreamEvent::thinking_done(0, "Let me think..."),
            StreamEvent::text_delta(1, "The answer is 42."),
            StreamEvent::response_done("resp_1", FinishReason::Stop),
        ];

        let stream = StreamResponse::new(make_stream(events));
        let response = stream.get_final_response().await.unwrap();

        assert!(response.has_thinking());
        assert_eq!(response.thinking(), Some("Let me think..."));
        assert_eq!(response.text(), "The answer is 42.");
    }

    #[tokio::test]
    async fn test_text_stream() {
        use futures::StreamExt;

        let events = vec![
            StreamEvent::response_created("resp_1"),
            StreamEvent::text_delta(0, "Hello "),
            StreamEvent::text_delta(0, "world"),
            StreamEvent::response_done("resp_1", FinishReason::Stop),
        ];

        let stream = StreamResponse::new(make_stream(events));
        let text_stream = stream.text_stream();
        let texts: Vec<String> = text_stream.map(|r| r.unwrap()).collect().await;

        assert_eq!(texts, vec!["Hello ", "world"]);
    }

    #[tokio::test]
    async fn test_stream_config_default() {
        let config = StreamConfig::default();
        assert_eq!(config.idle_timeout, DEFAULT_IDLE_TIMEOUT);
    }

    #[tokio::test]
    async fn test_stream_with_custom_timeout() {
        let events = vec![
            StreamEvent::response_created("resp_1"),
            StreamEvent::text_delta(0, "Hello"),
            StreamEvent::response_done("resp_1", FinishReason::Stop),
        ];

        let config = StreamConfig {
            idle_timeout: Duration::from_secs(120),
        };
        let stream = StreamResponse::with_config(make_stream(events), config);
        assert_eq!(stream.config().idle_timeout, Duration::from_secs(120));
    }

    #[tokio::test]
    async fn test_stream_idle_timeout_builder() {
        let events = vec![StreamEvent::response_created("resp_1")];

        let stream = StreamResponse::new(make_stream(events)).idle_timeout(Duration::from_secs(30));
        assert_eq!(stream.config().idle_timeout, Duration::from_secs(30));
    }

    #[tokio::test]
    async fn test_stream_idle_timeout_triggers() {
        use futures::StreamExt as _;

        // Create a stream that never yields events after the first
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        let stream: EventStream = Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx).map(Ok));

        // Send one event, then let the stream hang
        tx.send(StreamEvent::response_created("resp_1"))
            .await
            .unwrap();
        drop(tx); // Don't send more events, but also don't close cleanly

        // Use a very short timeout for testing
        let mut stream_response =
            StreamResponse::new(stream).idle_timeout(Duration::from_millis(1));

        // First event should succeed
        let first = stream_response.next_event().await;
        assert!(first.is_some());
        assert!(first.unwrap().is_ok());

        // Wait a bit to ensure the timeout triggers
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Stream is now exhausted (channel closed), should return None
        let second = stream_response.next_event().await;
        assert!(second.is_none());
    }
}
