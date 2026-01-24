//! Stream events for real-time generation updates.

use crate::error::HyperError;
use crate::response::FinishReason;
use crate::response::TokenUsage;
use crate::tools::ToolCall;
use serde::Deserialize;
use serde::Serialize;

/// Events emitted during streaming generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    /// Event that should be ignored by consumers.
    ///
    /// This is emitted for provider-specific events that don't map to
    /// the unified stream event model. Using this instead of empty TextDelta
    /// prevents UI flicker, incorrect event counting, and hook pollution.
    Ignored,

    // Text events
    /// Delta (incremental) text content.
    TextDelta {
        /// Content block index.
        index: i64,
        /// The delta text.
        delta: String,
    },
    /// Text block completed.
    TextDone {
        /// Content block index.
        index: i64,
        /// The complete text.
        text: String,
    },

    // Thinking events
    /// Delta thinking content.
    ThinkingDelta {
        /// Content block index.
        index: i64,
        /// The delta thinking text.
        delta: String,
    },
    /// Thinking block completed.
    ThinkingDone {
        /// Content block index.
        index: i64,
        /// The complete thinking content.
        content: String,
        /// Optional signature for verification.
        #[serde(skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    },

    // Tool call events
    /// Tool call started.
    ToolCallStart {
        /// Content block index.
        index: i64,
        /// Tool call ID.
        id: String,
        /// Tool name.
        name: String,
    },
    /// Delta tool call arguments.
    ToolCallDelta {
        /// Content block index.
        index: i64,
        /// Tool call ID.
        id: String,
        /// Incremental arguments JSON.
        arguments_delta: String,
    },
    /// Tool call completed.
    ToolCallDone {
        /// Content block index.
        index: i64,
        /// The complete tool call.
        tool_call: ToolCall,
    },

    // Lifecycle events
    /// Response created.
    ResponseCreated {
        /// Response ID.
        id: String,
    },
    /// Response completed.
    ResponseDone {
        /// Response ID.
        id: String,
        /// Model name (may differ from requested model due to aliases/defaults).
        #[serde(default)]
        model: String,
        /// Token usage.
        #[serde(skip_serializing_if = "Option::is_none")]
        usage: Option<TokenUsage>,
        /// Finish reason.
        finish_reason: FinishReason,
    },

    /// Error occurred.
    Error(StreamError),
}

/// Error within a stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamError {
    /// Error code.
    pub code: String,
    /// Error message.
    pub message: String,
}

impl From<HyperError> for StreamError {
    fn from(err: HyperError) -> Self {
        Self {
            code: "error".to_string(),
            message: err.to_string(),
        }
    }
}

impl StreamEvent {
    /// Create a text delta event.
    pub fn text_delta(index: i64, delta: impl Into<String>) -> Self {
        StreamEvent::TextDelta {
            index,
            delta: delta.into(),
        }
    }

    /// Create a text done event.
    pub fn text_done(index: i64, text: impl Into<String>) -> Self {
        StreamEvent::TextDone {
            index,
            text: text.into(),
        }
    }

    /// Create a thinking delta event.
    pub fn thinking_delta(index: i64, delta: impl Into<String>) -> Self {
        StreamEvent::ThinkingDelta {
            index,
            delta: delta.into(),
        }
    }

    /// Create a thinking done event.
    pub fn thinking_done(index: i64, content: impl Into<String>) -> Self {
        StreamEvent::ThinkingDone {
            index,
            content: content.into(),
            signature: None,
        }
    }

    /// Create a tool call start event.
    pub fn tool_call_start(index: i64, id: impl Into<String>, name: impl Into<String>) -> Self {
        StreamEvent::ToolCallStart {
            index,
            id: id.into(),
            name: name.into(),
        }
    }

    /// Create a tool call done event.
    pub fn tool_call_done(index: i64, tool_call: ToolCall) -> Self {
        StreamEvent::ToolCallDone { index, tool_call }
    }

    /// Create a response created event.
    pub fn response_created(id: impl Into<String>) -> Self {
        StreamEvent::ResponseCreated { id: id.into() }
    }

    /// Create a response done event (without model - for backward compatibility).
    pub fn response_done(id: impl Into<String>, finish_reason: FinishReason) -> Self {
        StreamEvent::ResponseDone {
            id: id.into(),
            model: String::new(),
            usage: None,
            finish_reason,
        }
    }

    /// Create a full response done event.
    pub fn response_done_full(
        id: impl Into<String>,
        model: impl Into<String>,
        usage: Option<TokenUsage>,
        finish_reason: FinishReason,
    ) -> Self {
        StreamEvent::ResponseDone {
            id: id.into(),
            model: model.into(),
            usage,
            finish_reason,
        }
    }

    /// Check if this is a delta event (text or thinking).
    pub fn is_delta(&self) -> bool {
        matches!(
            self,
            StreamEvent::TextDelta { .. }
                | StreamEvent::ThinkingDelta { .. }
                | StreamEvent::ToolCallDelta { .. }
        )
    }

    /// Check if this is a completion event.
    pub fn is_done(&self) -> bool {
        matches!(self, StreamEvent::ResponseDone { .. })
    }

    /// Get text delta if this is a text delta event.
    pub fn as_text_delta(&self) -> Option<&str> {
        match self {
            StreamEvent::TextDelta { delta, .. } => Some(delta),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_event_constructors() {
        let delta = StreamEvent::text_delta(0, "Hello");
        assert!(delta.is_delta());
        assert_eq!(delta.as_text_delta(), Some("Hello"));

        let done = StreamEvent::response_done("resp_1", FinishReason::Stop);
        assert!(done.is_done());
    }

    #[test]
    fn test_stream_event_serde() {
        let event = StreamEvent::text_delta(0, "world");
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"text_delta\""));

        let parsed: StreamEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.as_text_delta(), Some("world"));
    }

    #[test]
    fn test_tool_call_events() {
        let start = StreamEvent::tool_call_start(0, "call_1", "get_weather");
        assert!(!start.is_delta());

        let done = StreamEvent::tool_call_done(
            0,
            ToolCall::new("call_1", "get_weather", serde_json::json!({})),
        );
        assert!(!done.is_done());
    }

    #[test]
    fn test_ignored_event() {
        let ignored = StreamEvent::Ignored;
        assert!(!ignored.is_delta());
        assert!(!ignored.is_done());
        assert!(ignored.as_text_delta().is_none());
    }
}
