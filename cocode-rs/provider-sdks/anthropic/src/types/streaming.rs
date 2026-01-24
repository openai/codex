//! Streaming event types for the Anthropic Messages API.
//!
//! These types represent the SSE (Server-Sent Events) returned when streaming
//! is enabled via `stream=true`.

use serde::Deserialize;
use serde::Serialize;

use super::Role;
use super::ServerToolUsage;
use super::StopReason;
use super::Usage;

// ============================================================================
// Raw SSE Event Types (wire format)
// ============================================================================

/// Raw message stream event as received from the API.
///
/// These are the low-level events sent via SSE when streaming is enabled.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RawMessageStreamEvent {
    /// Message has started.
    MessageStart { message: MessageStartData },

    /// Content block has started.
    ContentBlockStart {
        index: i32,
        content_block: ContentBlockStartData,
    },

    /// Content block delta (partial content).
    ContentBlockDelta {
        index: i32,
        delta: ContentBlockDelta,
    },

    /// Content block has finished.
    ContentBlockStop { index: i32 },

    /// Message delta (stop_reason, usage updates).
    MessageDelta {
        delta: MessageDeltaData,
        usage: MessageDeltaUsage,
    },

    /// Message has finished.
    MessageStop,

    /// Ping event (keep-alive).
    Ping,

    /// Error event.
    Error { error: StreamError },
}

// ============================================================================
// Message Start Event
// ============================================================================

/// Initial message data from message_start event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageStartData {
    /// Unique message identifier.
    pub id: String,

    /// Object type (always "message").
    #[serde(rename = "type")]
    pub message_type: String,

    /// The role (always "assistant" for responses).
    pub role: Role,

    /// The model used.
    pub model: String,

    /// Initial content blocks (usually empty).
    #[serde(default)]
    pub content: Vec<serde_json::Value>,

    /// Reason the model stopped generating (null initially).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<StopReason>,

    /// The stop sequence that was matched, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequence: Option<String>,

    /// Token usage information.
    pub usage: Usage,
}

// ============================================================================
// Content Block Start Event
// ============================================================================

/// Content block start data.
///
/// Represents the initial state of a content block before deltas are applied.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlockStartData {
    /// Text block start.
    Text {
        #[serde(default)]
        text: String,
    },

    /// Tool use block start.
    ToolUse {
        id: String,
        name: String,
        #[serde(default)]
        input: serde_json::Value,
    },

    /// Server-side tool use block start (e.g., web_search).
    ServerToolUse {
        id: String,
        name: String,
        #[serde(default)]
        input: serde_json::Value,
    },

    /// Web search tool result block start.
    WebSearchToolResult {
        tool_use_id: String,
        #[serde(default)]
        content: serde_json::Value,
    },

    /// Thinking block start (extended thinking mode).
    Thinking {
        #[serde(default)]
        thinking: String,
    },

    /// Redacted thinking block start.
    RedactedThinking {
        #[serde(default)]
        data: String,
    },
}

// ============================================================================
// Content Block Delta Event
// ============================================================================

/// Content block delta types.
///
/// These represent incremental updates to content blocks during streaming.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlockDelta {
    /// Text delta.
    TextDelta { text: String },

    /// Tool input JSON delta (partial JSON string).
    InputJsonDelta { partial_json: String },

    /// Thinking delta.
    ThinkingDelta { thinking: String },

    /// Signature delta (for thinking blocks).
    SignatureDelta { signature: String },

    /// Citation delta.
    CitationsDelta { citation: serde_json::Value },
}

// ============================================================================
// Message Delta Event
// ============================================================================

/// Message delta data from message_delta event.
///
/// Contains updates to message-level fields that occur at the end of streaming.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessageDeltaData {
    /// Updated stop reason.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<StopReason>,

    /// Updated stop sequence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequence: Option<String>,
}

/// Usage data from message_delta event.
///
/// Contains output token count updates during streaming.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessageDeltaUsage {
    /// Number of output tokens generated so far.
    #[serde(default)]
    pub output_tokens: i32,

    /// Number of input tokens (may be updated).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<i32>,

    /// Tokens used to create cache entries.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_tokens: Option<i32>,

    /// Tokens read from cache.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_input_tokens: Option<i32>,

    /// Server tool usage information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_tool_use: Option<ServerToolUsage>,
}

// ============================================================================
// Error Event
// ============================================================================

/// Stream error data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamError {
    /// Error type (e.g., "overloaded_error", "api_error").
    #[serde(rename = "type")]
    pub error_type: String,

    /// Human-readable error message.
    pub message: String,
}

// ============================================================================
// Helper implementations
// ============================================================================

impl ContentBlockDelta {
    /// Get the text content if this is a text delta.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            ContentBlockDelta::TextDelta { text } => Some(text),
            _ => None,
        }
    }

    /// Get the partial JSON if this is an input_json delta.
    pub fn as_partial_json(&self) -> Option<&str> {
        match self {
            ContentBlockDelta::InputJsonDelta { partial_json } => Some(partial_json),
            _ => None,
        }
    }

    /// Get the thinking content if this is a thinking delta.
    pub fn as_thinking(&self) -> Option<&str> {
        match self {
            ContentBlockDelta::ThinkingDelta { thinking } => Some(thinking),
            _ => None,
        }
    }
}
