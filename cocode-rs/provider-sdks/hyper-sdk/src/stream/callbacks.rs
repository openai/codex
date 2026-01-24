//! Callback-based stream processing.

use super::events::StreamEvent;
use super::response::StreamResponse;
use crate::error::HyperError;
use crate::response::FinishReason;
use crate::response::GenerateResponse;
use crate::tools::ToolCall;
use async_trait::async_trait;

/// Callbacks for stream events.
///
/// Implement this trait to handle streaming events as they arrive.
/// All methods have default no-op implementations, so you only need
/// to implement the ones you care about.
#[async_trait]
pub trait StreamCallbacks: Send {
    /// Called when a text delta is received.
    async fn on_text_delta(&mut self, _index: i64, _delta: &str) -> Result<(), HyperError> {
        Ok(())
    }

    /// Called when a text block is complete.
    async fn on_text_done(&mut self, _index: i64, _text: &str) -> Result<(), HyperError> {
        Ok(())
    }

    /// Called when a thinking delta is received.
    async fn on_thinking_delta(&mut self, _index: i64, _delta: &str) -> Result<(), HyperError> {
        Ok(())
    }

    /// Called when a thinking block is complete.
    async fn on_thinking_done(&mut self, _index: i64, _content: &str) -> Result<(), HyperError> {
        Ok(())
    }

    /// Called when a tool call starts.
    async fn on_tool_call_start(
        &mut self,
        _index: i64,
        _id: &str,
        _name: &str,
    ) -> Result<(), HyperError> {
        Ok(())
    }

    /// Called when a tool call is complete.
    async fn on_tool_call_done(
        &mut self,
        _index: i64,
        _tool_call: &ToolCall,
    ) -> Result<(), HyperError> {
        Ok(())
    }

    /// Called when the response is complete.
    async fn on_finish(&mut self, _reason: FinishReason) -> Result<(), HyperError> {
        Ok(())
    }

    /// Called when an error occurs.
    async fn on_error(&mut self, _error: &HyperError) -> Result<(), HyperError> {
        Ok(())
    }
}

impl StreamResponse {
    /// Process the stream with callbacks.
    ///
    /// Consumes the stream and calls the appropriate callback for each event.
    /// Returns the final response when complete.
    pub async fn process_with_callbacks<C: StreamCallbacks>(
        mut self,
        mut callbacks: C,
    ) -> Result<GenerateResponse, HyperError> {
        while let Some(result) = self.next_event().await {
            match result {
                Ok(event) => {
                    Self::dispatch_event(&mut callbacks, &event).await?;
                }
                Err(e) => {
                    callbacks.on_error(&e).await?;
                    return Err(e);
                }
            }
        }
        self.build_response()
    }

    async fn dispatch_event<C: StreamCallbacks>(
        callbacks: &mut C,
        event: &StreamEvent,
    ) -> Result<(), HyperError> {
        match event {
            StreamEvent::TextDelta { index, delta } => {
                callbacks.on_text_delta(*index, delta).await?;
            }
            StreamEvent::TextDone { index, text } => {
                callbacks.on_text_done(*index, text).await?;
            }
            StreamEvent::ThinkingDelta { index, delta } => {
                callbacks.on_thinking_delta(*index, delta).await?;
            }
            StreamEvent::ThinkingDone { index, content, .. } => {
                callbacks.on_thinking_done(*index, content).await?;
            }
            StreamEvent::ToolCallStart { index, id, name } => {
                callbacks.on_tool_call_start(*index, id, name).await?;
            }
            StreamEvent::ToolCallDone { index, tool_call } => {
                callbacks.on_tool_call_done(*index, tool_call).await?;
            }
            StreamEvent::ResponseDone { finish_reason, .. } => {
                callbacks.on_finish(*finish_reason).await?;
            }
            StreamEvent::Error(e) => {
                let err = HyperError::StreamError(e.message.clone());
                callbacks.on_error(&err).await?;
            }
            // Other events don't trigger callbacks
            _ => {}
        }
        Ok(())
    }
}

/// A simple callback that prints text deltas to stdout.
pub struct PrintCallbacks;

#[async_trait]
impl StreamCallbacks for PrintCallbacks {
    async fn on_text_delta(&mut self, _index: i64, delta: &str) -> Result<(), HyperError> {
        print!("{delta}");
        Ok(())
    }

    async fn on_finish(&mut self, _reason: FinishReason) -> Result<(), HyperError> {
        println!();
        Ok(())
    }
}

/// A callback that collects all text into a String.
pub struct CollectTextCallbacks {
    /// The collected text.
    pub text: String,
}

impl CollectTextCallbacks {
    /// Create a new collector.
    pub fn new() -> Self {
        Self {
            text: String::new(),
        }
    }
}

impl Default for CollectTextCallbacks {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StreamCallbacks for CollectTextCallbacks {
    async fn on_text_delta(&mut self, _index: i64, delta: &str) -> Result<(), HyperError> {
        self.text.push_str(delta);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;
    use std::pin::Pin;
    use std::sync::Arc;
    use std::sync::atomic::AtomicI32;
    use std::sync::atomic::Ordering;

    fn make_stream(
        events: Vec<StreamEvent>,
    ) -> Pin<Box<dyn futures::Stream<Item = Result<StreamEvent, HyperError>> + Send>> {
        Box::pin(stream::iter(events.into_iter().map(Ok)))
    }

    struct CountingCallbacks {
        text_deltas: Arc<AtomicI32>,
        finished: Arc<AtomicI32>,
    }

    #[async_trait]
    impl StreamCallbacks for CountingCallbacks {
        async fn on_text_delta(&mut self, _index: i64, _delta: &str) -> Result<(), HyperError> {
            self.text_deltas.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn on_finish(&mut self, _reason: FinishReason) -> Result<(), HyperError> {
            self.finished.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_callbacks() {
        let events = vec![
            StreamEvent::response_created("resp_1"),
            StreamEvent::text_delta(0, "Hello "),
            StreamEvent::text_delta(0, "world!"),
            StreamEvent::response_done("resp_1", FinishReason::Stop),
        ];

        let text_deltas = Arc::new(AtomicI32::new(0));
        let finished = Arc::new(AtomicI32::new(0));

        let callbacks = CountingCallbacks {
            text_deltas: text_deltas.clone(),
            finished: finished.clone(),
        };

        let stream = StreamResponse::new(make_stream(events));
        let _ = stream.process_with_callbacks(callbacks).await.unwrap();

        assert_eq!(text_deltas.load(Ordering::SeqCst), 2);
        assert_eq!(finished.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_collect_text_callbacks() {
        let events = vec![
            StreamEvent::text_delta(0, "Hello "),
            StreamEvent::text_delta(0, "world!"),
            StreamEvent::response_done("resp_1", FinishReason::Stop),
        ];

        let callbacks = CollectTextCallbacks::new();

        let stream = StreamResponse::new(make_stream(events));
        let _ = stream.process_with_callbacks(callbacks).await;
    }
}
