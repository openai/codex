//! Stream update events for typed event handling.
//!
//! This module provides a typed enum for stream updates, used in conjunction
//! with [`StreamSnapshot`] for the Crush-like streaming API.

use crate::error::HyperError;
use crate::response::FinishReason;
use crate::response::TokenUsage;
use crate::tools::ToolCall;
use serde::Deserialize;
use serde::Serialize;

/// Typed stream update event.
///
/// Unlike [`StreamEvent`](super::StreamEvent) which is used for serialization and
/// provider communication, `StreamUpdate` is designed for typed event handling
/// in the streaming API.
///
/// Each update represents a meaningful state change during streaming.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamUpdate {
    /// An event that should be ignored (no meaningful state change).
    ///
    /// This is emitted for provider-specific events that don't map to
    /// the unified stream event model.
    Ignored,

    /// Response was created (stream started).
    ResponseCreated {
        /// Response ID.
        id: String,
    },

    /// Text content delta.
    TextDelta {
        /// Index of the content block.
        index: i64,
        /// Incremental text content.
        delta: String,
    },

    /// Text block completed.
    TextDone {
        /// Index of the content block.
        index: i64,
        /// Complete text content.
        text: String,
    },

    /// Thinking content delta.
    ThinkingDelta {
        /// Index of the content block.
        index: i64,
        /// Incremental thinking content.
        delta: String,
    },

    /// Thinking block completed.
    ThinkingDone {
        /// Index of the content block.
        index: i64,
        /// Complete thinking content.
        content: String,
        /// Optional cryptographic signature.
        signature: Option<String>,
    },

    /// Tool call started.
    ToolCallStarted {
        /// Index of the content block.
        index: i64,
        /// Tool call ID.
        id: String,
        /// Tool name.
        name: String,
    },

    /// Tool call arguments delta.
    ToolCallDelta {
        /// Index of the content block.
        index: i64,
        /// Tool call ID.
        id: String,
        /// Incremental arguments JSON.
        delta: String,
    },

    /// Tool call completed.
    ToolCallCompleted {
        /// Index of the content block.
        index: i64,
        /// Complete tool call.
        tool_call: ToolCall,
    },

    /// Stream completed.
    Done {
        /// Response ID.
        id: String,
        /// Finish reason.
        finish_reason: FinishReason,
        /// Token usage statistics.
        usage: Option<TokenUsage>,
    },

    /// Error occurred.
    Error(StreamError),
}

/// Error within a stream update.
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

impl StreamUpdate {
    /// Check if this is a text delta.
    pub fn is_text_delta(&self) -> bool {
        matches!(self, StreamUpdate::TextDelta { .. })
    }

    /// Check if this is a thinking delta.
    pub fn is_thinking_delta(&self) -> bool {
        matches!(self, StreamUpdate::ThinkingDelta { .. })
    }

    /// Check if this is any kind of delta (text, thinking, or tool arguments).
    pub fn is_delta(&self) -> bool {
        matches!(
            self,
            StreamUpdate::TextDelta { .. }
                | StreamUpdate::ThinkingDelta { .. }
                | StreamUpdate::ToolCallDelta { .. }
        )
    }

    /// Check if this is a completion event.
    pub fn is_done(&self) -> bool {
        matches!(self, StreamUpdate::Done { .. })
    }

    /// Check if this is an error event.
    pub fn is_error(&self) -> bool {
        matches!(self, StreamUpdate::Error(_))
    }

    /// Get text delta content if this is a text delta.
    pub fn as_text_delta(&self) -> Option<&str> {
        match self {
            StreamUpdate::TextDelta { delta, .. } => Some(delta),
            _ => None,
        }
    }

    /// Get thinking delta content if this is a thinking delta.
    pub fn as_thinking_delta(&self) -> Option<&str> {
        match self {
            StreamUpdate::ThinkingDelta { delta, .. } => Some(delta),
            _ => None,
        }
    }

    /// Get the finish reason if this is a done event.
    pub fn finish_reason(&self) -> Option<FinishReason> {
        match self {
            StreamUpdate::Done { finish_reason, .. } => Some(*finish_reason),
            _ => None,
        }
    }
}

/// Convert from the low-level StreamEvent to StreamUpdate.
impl From<super::StreamEvent> for StreamUpdate {
    fn from(event: super::StreamEvent) -> Self {
        match event {
            super::StreamEvent::Ignored => StreamUpdate::Ignored,
            super::StreamEvent::ResponseCreated { id } => StreamUpdate::ResponseCreated { id },
            super::StreamEvent::TextDelta { index, delta } => {
                StreamUpdate::TextDelta { index, delta }
            }
            super::StreamEvent::TextDone { index, text } => StreamUpdate::TextDone { index, text },
            super::StreamEvent::ThinkingDelta { index, delta } => {
                StreamUpdate::ThinkingDelta { index, delta }
            }
            super::StreamEvent::ThinkingDone {
                index,
                content,
                signature,
            } => StreamUpdate::ThinkingDone {
                index,
                content,
                signature,
            },
            super::StreamEvent::ToolCallStart { index, id, name } => {
                StreamUpdate::ToolCallStarted { index, id, name }
            }
            super::StreamEvent::ToolCallDelta {
                index,
                id,
                arguments_delta,
            } => StreamUpdate::ToolCallDelta {
                index,
                id,
                delta: arguments_delta,
            },
            super::StreamEvent::ToolCallDone { index, tool_call } => {
                StreamUpdate::ToolCallCompleted { index, tool_call }
            }
            super::StreamEvent::ResponseDone {
                id,
                usage,
                finish_reason,
                ..
            } => StreamUpdate::Done {
                id,
                finish_reason,
                usage,
            },
            super::StreamEvent::Error(e) => StreamUpdate::Error(StreamError {
                code: e.code,
                message: e.message,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_classification() {
        let text_delta = StreamUpdate::TextDelta {
            index: 0,
            delta: "hello".to_string(),
        };
        assert!(text_delta.is_text_delta());
        assert!(text_delta.is_delta());
        assert!(!text_delta.is_done());

        let done = StreamUpdate::Done {
            id: "resp_1".to_string(),
            finish_reason: FinishReason::Stop,
            usage: None,
        };
        assert!(done.is_done());
        assert!(!done.is_delta());
    }

    #[test]
    fn test_update_accessors() {
        let text_delta = StreamUpdate::TextDelta {
            index: 0,
            delta: "world".to_string(),
        };
        assert_eq!(text_delta.as_text_delta(), Some("world"));

        let thinking_delta = StreamUpdate::ThinkingDelta {
            index: 0,
            delta: "thinking...".to_string(),
        };
        assert_eq!(thinking_delta.as_thinking_delta(), Some("thinking..."));

        let done = StreamUpdate::Done {
            id: "resp_1".to_string(),
            finish_reason: FinishReason::ToolCalls,
            usage: None,
        };
        assert_eq!(done.finish_reason(), Some(FinishReason::ToolCalls));
    }

    #[test]
    fn test_from_stream_event() {
        use super::super::StreamEvent;

        let event = StreamEvent::text_delta(0, "hello");
        let update: StreamUpdate = event.into();
        assert!(matches!(update, StreamUpdate::TextDelta { delta, .. } if delta == "hello"));
    }
}
