//! Stream processor for Crush-like message accumulation.
//!
//! This module provides [`StreamProcessor`], a high-level API for processing
//! streaming responses with accumulated state. The design is inspired by
//! Crush's message update pattern where a single message is continuously
//! updated during streaming, enabling real-time UI updates while maintaining
//! a single aggregated message in history.
//!
//! # Key Features
//!
//! - **Accumulated State**: Access the current snapshot at any time during streaming
//! - **Closure-based API**: No need to implement traits, just pass closures
//! - **Async Native**: All handlers support async operations (DB, WebSocket, etc.)
//! - **Progressive Complexity**: Simple use cases are simple, complex ones are possible
//!
//! # Examples
//!
//! ## Simplest: Collect to response
//!
//! ```ignore
//! let response = model.stream(request).await?.into_processor().collect().await?;
//! ```
//!
//! ## Print to stdout
//!
//! ```ignore
//! let response = model.stream(request).await?.into_processor().print().await?;
//! ```
//!
//! ## Crush-like pattern: Update same message
//!
//! ```ignore
//! let msg_id = db.insert_message(conv_id, Role::Assistant).await?;
//!
//! let response = model.stream(request).await?.into_processor()
//!     .on_update(|snapshot| async move {
//!         // UPDATE same message (not INSERT)
//!         db.update_message(msg_id, &snapshot.text).await?;
//!         // Notify UI subscribers
//!         pubsub.publish(format!("message:{}", msg_id), "updated").await;
//!         Ok(())
//!     })
//!     .await?;
//! ```

use super::EventStream;
use super::StreamEvent;
use super::processor_state::ProcessorState;
use super::response::StreamConfig;
use super::snapshot::StreamSnapshot;
use super::update::StreamUpdate;
use crate::error::HyperError;
use crate::messages::ContentBlock;
use crate::response::FinishReason;
use crate::response::GenerateResponse;
use futures::StreamExt;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;
use tokio::time::timeout;

/// Stream processor with Crush-like accumulated state.
///
/// This is the main type for processing streaming responses. It wraps an
/// [`EventStream`] and maintains accumulated state that can be accessed
/// at any time during streaming.
///
/// # Design
///
/// The processor accumulates all events into a [`StreamSnapshot`] which
/// represents the current state of the response. This enables the
/// "update same message" pattern used by Crush and similar systems.
///
/// # Idle Timeout
///
/// The processor includes an idle timeout (default 60 seconds) to prevent
/// hanging on unresponsive streams. Use [`idle_timeout`](Self::idle_timeout)
/// to customize this behavior.
pub struct StreamProcessor {
    inner: EventStream,
    state: ProcessorState,
    config: StreamConfig,
}

impl StreamProcessor {
    /// Create a new processor from an event stream.
    pub fn new(inner: EventStream) -> Self {
        Self {
            inner,
            state: ProcessorState::default(),
            config: StreamConfig::default(),
        }
    }

    /// Create a new processor with custom configuration.
    pub fn with_config(inner: EventStream, config: StreamConfig) -> Self {
        Self {
            inner,
            state: ProcessorState::default(),
            config,
        }
    }

    /// Set the idle timeout for the processor.
    ///
    /// This is a builder method that allows chaining.
    pub fn idle_timeout(mut self, idle_timeout: Duration) -> Self {
        self.config.idle_timeout = idle_timeout;
        self
    }

    /// Get the current configuration.
    pub fn config(&self) -> &StreamConfig {
        &self.config
    }

    // =========================================================================
    // Low-level API: Iterator style
    // =========================================================================

    /// Get the next raw event from the stream.
    ///
    /// This is the lowest-level API. Events are NOT automatically accumulated
    /// into the snapshot when using this method.
    ///
    /// Respects the configured idle timeout.
    pub async fn next_raw_event(&mut self) -> Option<Result<StreamEvent, HyperError>> {
        match timeout(self.config.idle_timeout, self.inner.next()).await {
            Ok(Some(event)) => Some(event),
            Ok(None) => None,
            Err(_) => Some(Err(HyperError::StreamIdleTimeout(self.config.idle_timeout))),
        }
    }

    /// Get the next event, update the snapshot, and return both.
    ///
    /// Returns the update event along with a clone of the current accumulated snapshot.
    /// Respects the configured idle timeout.
    pub async fn next(&mut self) -> Option<Result<(StreamUpdate, StreamSnapshot), HyperError>> {
        let result = timeout(self.config.idle_timeout, self.inner.next()).await;

        match result {
            Ok(Some(Ok(ev))) => {
                let update: StreamUpdate = ev.clone().into();
                self.update_state(&ev);
                Some(Ok((update, self.state.snapshot.clone())))
            }
            Ok(Some(Err(e))) => Some(Err(e)),
            Ok(None) => None,
            Err(_) => Some(Err(HyperError::StreamIdleTimeout(self.config.idle_timeout))),
        }
    }

    /// Get the current accumulated snapshot.
    ///
    /// Can be called at any time to get the current state.
    pub fn snapshot(&self) -> &StreamSnapshot {
        &self.state.snapshot
    }

    /// Clone the current snapshot.
    ///
    /// Useful when you need to capture the state at a specific point.
    pub fn snapshot_clone(&self) -> StreamSnapshot {
        self.state.snapshot.clone()
    }

    // =========================================================================
    // High-level API: Closure style (recommended)
    // =========================================================================

    /// Process the stream, calling the handler after each update with the accumulated snapshot.
    ///
    /// This is the core API for Crush-like patterns. The handler receives a clone of
    /// the current accumulated state after each event, enabling "UPDATE same message"
    /// patterns.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let response = processor
    ///     .on_update(|snapshot| async move {
    ///         db.update_message(msg_id, &snapshot.text).await?;
    ///         Ok(())
    ///     })
    ///     .await?;
    /// ```
    #[must_use = "this returns a Result that must be handled"]
    pub async fn on_update<F, Fut>(mut self, mut handler: F) -> Result<GenerateResponse, HyperError>
    where
        F: FnMut(StreamSnapshot) -> Fut,
        Fut: Future<Output = Result<(), HyperError>>,
    {
        while let Some(result) = self.next().await {
            let (_, snapshot) = result?;
            handler(snapshot).await?;
        }
        self.into_response()
    }

    /// Process the stream, calling the handler with both the update event and snapshot.
    ///
    /// Use this when you need to distinguish between event types while also
    /// having access to the accumulated state.
    ///
    /// # Example
    ///
    /// ```ignore
    /// processor.for_each(|update, snapshot| async move {
    ///     match update {
    ///         StreamUpdate::TextDelta { delta, .. } => {
    ///             ui.append_text(&delta);
    ///         }
    ///         StreamUpdate::Done { .. } => {
    ///             ui.mark_complete();
    ///         }
    ///         _ => {}
    ///     }
    ///     // Also update status with accumulated stats
    ///     ui.set_status(&format!("Chars: {}", snapshot.text.len()));
    ///     Ok(())
    /// }).await?;
    /// ```
    #[must_use = "this returns a Result that must be handled"]
    pub async fn for_each<F, Fut>(mut self, mut handler: F) -> Result<GenerateResponse, HyperError>
    where
        F: FnMut(StreamUpdate, StreamSnapshot) -> Fut,
        Fut: Future<Output = Result<(), HyperError>>,
    {
        while let Some(result) = self.next().await {
            let (update, snapshot) = result?;
            handler(update, snapshot).await?;
        }
        self.into_response()
    }

    /// Process only text deltas.
    ///
    /// Simple API for just handling text output (e.g., printing to console).
    ///
    /// # Example
    ///
    /// ```ignore
    /// processor.on_text(|delta| async move {
    ///     print!("{}", delta);
    ///     std::io::stdout().flush()?;
    ///     Ok(())
    /// }).await?;
    /// ```
    #[must_use = "this returns a Result that must be handled"]
    pub async fn on_text<F, Fut>(mut self, mut handler: F) -> Result<GenerateResponse, HyperError>
    where
        F: FnMut(String) -> Fut,
        Fut: Future<Output = Result<(), HyperError>>,
    {
        while let Some(result) = self.next().await {
            let (update, _) = result?;
            if let Some(delta) = update.as_text_delta() {
                handler(delta.to_string()).await?;
            }
        }
        self.into_response()
    }

    /// Process text deltas with accumulated text.
    ///
    /// Like `on_text`, but also receives the full accumulated text so far.
    #[must_use = "this returns a Result that must be handled"]
    pub async fn on_text_with_full<F, Fut>(
        mut self,
        mut handler: F,
    ) -> Result<GenerateResponse, HyperError>
    where
        F: FnMut(String, String) -> Fut,
        Fut: Future<Output = Result<(), HyperError>>,
    {
        while let Some(result) = self.next().await {
            let (update, snapshot) = result?;
            if let Some(delta) = update.as_text_delta() {
                handler(delta.to_string(), snapshot.text).await?;
            }
        }
        self.into_response()
    }

    // =========================================================================
    // Convenience API: One-liners
    // =========================================================================

    /// Silently consume the stream and return the final response.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let response = processor.collect().await?;
    /// println!("{}", response.text());
    /// ```
    #[must_use = "this returns a Result that must be handled"]
    pub async fn collect(mut self) -> Result<GenerateResponse, HyperError> {
        while let Some(result) = self.next().await {
            result?;
        }
        self.into_response()
    }

    /// Print text deltas to stdout and return the final response.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let response = processor.print().await?;
    /// ```
    #[must_use = "this returns a Result that must be handled"]
    pub async fn print(self) -> Result<GenerateResponse, HyperError> {
        self.on_text(|delta| {
            let fut: Pin<Box<dyn Future<Output = Result<(), HyperError>> + Send>> =
                Box::pin(async move {
                    print!("{delta}");
                    Ok(())
                });
            fut
        })
        .await
    }

    /// Print text deltas to stdout with a final newline.
    #[must_use = "this returns a Result that must be handled"]
    pub async fn println(self) -> Result<GenerateResponse, HyperError> {
        let response = self.print().await?;
        println!();
        Ok(response)
    }

    // =========================================================================
    // Internal: State management (delegated to processor_state module)
    // =========================================================================

    fn update_state(&mut self, event: &StreamEvent) {
        self.state.update(event);
    }

    /// Convert the accumulated state into a GenerateResponse.
    #[must_use = "this returns a Result that must be handled"]
    pub fn into_response(self) -> Result<GenerateResponse, HyperError> {
        let snapshot = self.state.snapshot;

        let mut content = Vec::new();

        // Add thinking if present
        if let Some(thinking) = &snapshot.thinking {
            content.push(ContentBlock::Thinking {
                content: thinking.content.clone(),
                signature: thinking.signature.clone(),
            });
        }

        // Add text if present
        if !snapshot.text.is_empty() {
            content.push(ContentBlock::text(&snapshot.text));
        }

        // Add tool calls
        for tc in &snapshot.tool_calls {
            if tc.is_complete {
                content.push(ContentBlock::tool_use(
                    &tc.id,
                    &tc.name,
                    tc.parsed_arguments().unwrap_or(serde_json::Value::Null),
                ));
            }
        }

        Ok(GenerateResponse {
            id: snapshot.id.unwrap_or_else(|| "unknown".to_string()),
            content,
            finish_reason: snapshot.finish_reason.unwrap_or(FinishReason::Stop),
            usage: snapshot.usage,
            model: if snapshot.model.is_empty() {
                "unknown".to_string()
            } else {
                snapshot.model
            },
        })
    }
}

impl std::fmt::Debug for StreamProcessor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamProcessor")
            .field("snapshot", &self.state.snapshot)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::response::TokenUsage;
    use futures::stream;
    use std::sync::Arc;
    use std::sync::Mutex;

    fn make_stream(
        events: Vec<StreamEvent>,
    ) -> Pin<Box<dyn futures::Stream<Item = Result<StreamEvent, HyperError>> + Send>> {
        Box::pin(stream::iter(events.into_iter().map(Ok)))
    }

    #[tokio::test]
    async fn test_processor_collect() {
        let events = vec![
            StreamEvent::response_created("resp_1"),
            StreamEvent::text_delta(0, "Hello "),
            StreamEvent::text_delta(0, "world!"),
            StreamEvent::response_done("resp_1", FinishReason::Stop),
        ];

        let processor = StreamProcessor::new(make_stream(events));
        let response = processor.collect().await.unwrap();

        assert_eq!(response.text(), "Hello world!");
        assert_eq!(response.finish_reason, FinishReason::Stop);
    }

    #[tokio::test]
    async fn test_processor_on_update_receives_accumulated_state() {
        let events = vec![
            StreamEvent::response_created("resp_1"),
            StreamEvent::text_delta(0, "Hello "),
            StreamEvent::text_delta(0, "world!"),
            StreamEvent::response_done("resp_1", FinishReason::Stop),
        ];

        let snapshots: Arc<Mutex<Vec<StreamSnapshot>>> = Arc::new(Mutex::new(Vec::new()));
        let snapshots_clone = snapshots.clone();

        let processor = StreamProcessor::new(make_stream(events));
        processor
            .on_update(|snapshot| {
                let snapshots = snapshots_clone.clone();
                async move {
                    snapshots.lock().unwrap().push(snapshot);
                    Ok(())
                }
            })
            .await
            .unwrap();

        let snapshots = snapshots.lock().unwrap();
        assert_eq!(snapshots.len(), 4);

        // Verify progressive accumulation
        assert_eq!(snapshots[0].text, ""); // response_created
        assert_eq!(snapshots[1].text, "Hello "); // first delta
        assert_eq!(snapshots[2].text, "Hello world!"); // second delta
        assert_eq!(snapshots[3].text, "Hello world!"); // response_done
        assert!(snapshots[3].is_complete);
    }

    #[tokio::test]
    async fn test_processor_for_each() {
        let events = vec![
            StreamEvent::text_delta(0, "Hi"),
            StreamEvent::ToolCallStart {
                index: 1,
                id: "call_1".to_string(),
                name: "get_weather".to_string(),
            },
            StreamEvent::response_done("resp_1", FinishReason::ToolCalls),
        ];

        let updates: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));
        let updates_clone = updates.clone();

        let processor = StreamProcessor::new(make_stream(events));
        processor
            .for_each(|update, snapshot| {
                let updates = updates_clone.clone();
                let update_type = format!("{:?}", std::mem::discriminant(&update));
                let text = snapshot.text.clone();
                async move {
                    updates.lock().unwrap().push((update_type, text));
                    Ok(())
                }
            })
            .await
            .unwrap();

        let updates = updates.lock().unwrap();
        assert_eq!(updates.len(), 3);
    }

    #[tokio::test]
    async fn test_processor_on_text() {
        let events = vec![
            StreamEvent::text_delta(0, "A"),
            StreamEvent::text_delta(0, "B"),
            StreamEvent::text_delta(0, "C"),
            StreamEvent::response_done("resp_1", FinishReason::Stop),
        ];

        let deltas: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let deltas_clone = deltas.clone();

        let processor = StreamProcessor::new(make_stream(events));
        processor
            .on_text(|delta| {
                let deltas = deltas_clone.clone();
                async move {
                    deltas.lock().unwrap().push(delta);
                    Ok(())
                }
            })
            .await
            .unwrap();

        let deltas = deltas.lock().unwrap();
        assert_eq!(*deltas, vec!["A", "B", "C"]);
    }

    #[tokio::test]
    async fn test_processor_with_thinking() {
        let events = vec![
            StreamEvent::response_created("resp_1"),
            StreamEvent::thinking_delta(0, "Let me "),
            StreamEvent::thinking_delta(0, "think..."),
            StreamEvent::ThinkingDone {
                index: 0,
                content: "Let me think...".to_string(),
                signature: Some("sig123".to_string()),
            },
            StreamEvent::text_delta(1, "The answer is 42."),
            StreamEvent::response_done("resp_1", FinishReason::Stop),
        ];

        let processor = StreamProcessor::new(make_stream(events));
        let response = processor.collect().await.unwrap();

        assert!(response.has_thinking());
        assert_eq!(response.thinking(), Some("Let me think..."));
        assert_eq!(response.text(), "The answer is 42.");
    }

    #[tokio::test]
    async fn test_processor_with_tool_calls() {
        let events = vec![
            StreamEvent::ToolCallStart {
                index: 0,
                id: "call_1".to_string(),
                name: "get_weather".to_string(),
            },
            StreamEvent::ToolCallDelta {
                index: 0,
                id: "call_1".to_string(),
                arguments_delta: "{\"city\":".to_string(),
            },
            StreamEvent::ToolCallDelta {
                index: 0,
                id: "call_1".to_string(),
                arguments_delta: "\"NYC\"}".to_string(),
            },
            StreamEvent::ToolCallDone {
                index: 0,
                tool_call: crate::tools::ToolCall::new(
                    "call_1",
                    "get_weather",
                    serde_json::json!({"city": "NYC"}),
                ),
            },
            StreamEvent::response_done("resp_1", FinishReason::ToolCalls),
        ];

        let processor = StreamProcessor::new(make_stream(events));
        let response = processor.collect().await.unwrap();

        let tool_calls = response.tool_calls();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].name, "get_weather");
        assert_eq!(tool_calls[0].arguments["city"], "NYC");
    }

    #[tokio::test]
    async fn test_processor_snapshot_accessible_during_iteration() {
        let events = vec![
            StreamEvent::text_delta(0, "A"),
            StreamEvent::text_delta(0, "B"),
            StreamEvent::response_done("resp_1", FinishReason::Stop),
        ];

        let mut processor = StreamProcessor::new(make_stream(events));

        // Initial state
        assert_eq!(processor.snapshot().text, "");

        // After first event
        let _ = processor.next().await;
        assert_eq!(processor.snapshot().text, "A");

        // After second event
        let _ = processor.next().await;
        assert_eq!(processor.snapshot().text, "AB");
    }

    #[tokio::test]
    async fn test_processor_with_usage() {
        let events = vec![
            StreamEvent::text_delta(0, "Hi"),
            StreamEvent::response_done_full(
                "resp_1",
                "test-model",
                Some(TokenUsage {
                    prompt_tokens: 10,
                    completion_tokens: 5,
                    total_tokens: 15,
                    cache_read_tokens: None,
                    cache_creation_tokens: None,
                    reasoning_tokens: None,
                }),
                FinishReason::Stop,
            ),
        ];

        let processor = StreamProcessor::new(make_stream(events));
        let response = processor.collect().await.unwrap();

        assert!(response.usage.is_some());
        let usage = response.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 5);
    }

    #[tokio::test]
    async fn test_processor_thinking_preserves_accumulated_deltas() {
        // Test that ThinkingDone does NOT replace accumulated deltas
        // (pure accumulation principle)
        let events = vec![
            StreamEvent::response_created("resp_1"),
            // Deltas accumulate to "Accumulated content"
            StreamEvent::thinking_delta(0, "Accumulated "),
            StreamEvent::thinking_delta(0, "content"),
            // ThinkingDone has DIFFERENT content - should be ignored
            StreamEvent::ThinkingDone {
                index: 0,
                content: "Different final content".to_string(),
                signature: Some("sig_abc".to_string()),
            },
            StreamEvent::text_delta(1, "Response text"),
            StreamEvent::response_done("resp_1", FinishReason::Stop),
        ];

        let processor = StreamProcessor::new(make_stream(events));
        let response = processor.collect().await.unwrap();

        // Verify accumulated deltas are preserved, not replaced
        assert_eq!(response.thinking(), Some("Accumulated content"));

        // Verify signature from ThinkingDone is still applied
        if let Some(ContentBlock::Thinking { signature, .. }) = response.content.first() {
            assert_eq!(*signature, Some("sig_abc".to_string()));
        } else {
            panic!("Expected Thinking block");
        }
    }

    #[tokio::test]
    async fn test_processor_thinking_uses_final_content_when_no_deltas() {
        // Test that ThinkingDone content is used when no deltas were received
        let events = vec![
            StreamEvent::response_created("resp_1"),
            // No thinking deltas - only ThinkingDone
            StreamEvent::ThinkingDone {
                index: 0,
                content: "Final content only".to_string(),
                signature: Some("sig_xyz".to_string()),
            },
            StreamEvent::text_delta(1, "Response"),
            StreamEvent::response_done("resp_1", FinishReason::Stop),
        ];

        let processor = StreamProcessor::new(make_stream(events));
        let response = processor.collect().await.unwrap();

        // Should use ThinkingDone content since no deltas
        assert_eq!(response.thinking(), Some("Final content only"));
    }

    #[tokio::test]
    async fn test_processor_with_custom_timeout() {
        let events = vec![
            StreamEvent::response_created("resp_1"),
            StreamEvent::text_delta(0, "Hello"),
            StreamEvent::response_done("resp_1", FinishReason::Stop),
        ];

        let config = StreamConfig {
            idle_timeout: Duration::from_secs(120),
        };
        let processor = StreamProcessor::with_config(make_stream(events), config);
        assert_eq!(processor.config().idle_timeout, Duration::from_secs(120));
    }

    #[tokio::test]
    async fn test_processor_idle_timeout_builder() {
        let events = vec![StreamEvent::response_created("resp_1")];

        let processor =
            StreamProcessor::new(make_stream(events)).idle_timeout(Duration::from_secs(30));
        assert_eq!(processor.config().idle_timeout, Duration::from_secs(30));
    }
}
