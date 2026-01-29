//! Tracked messages with metadata for turn tracking.
//!
//! This module provides [`TrackedMessage`] which wraps hyper-sdk's [`Message`]
//! with additional metadata for tracking in the agent loop.

use chrono::{DateTime, Utc};
use hyper_sdk::{ContentBlock, Message, Role, ToolResultContent};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Source of a message in the conversation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageSource {
    /// User input.
    User,
    /// Assistant response.
    Assistant {
        /// Request ID from the API.
        #[serde(skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
    },
    /// System instruction.
    System,
    /// Tool result.
    Tool {
        /// Tool call ID this result is for.
        call_id: String,
    },
    /// Subagent response.
    Subagent {
        /// ID of the subagent.
        agent_id: String,
    },
    /// Compaction summary.
    CompactionSummary,
}

impl MessageSource {
    /// Create an assistant source with optional request ID.
    pub fn assistant(request_id: Option<String>) -> Self {
        MessageSource::Assistant { request_id }
    }

    /// Create a tool source with call ID.
    pub fn tool(call_id: impl Into<String>) -> Self {
        MessageSource::Tool {
            call_id: call_id.into(),
        }
    }

    /// Create a subagent source with agent ID.
    pub fn subagent(agent_id: impl Into<String>) -> Self {
        MessageSource::Subagent {
            agent_id: agent_id.into(),
        }
    }
}

/// A message with tracking metadata.
///
/// This wraps hyper-sdk's [`Message`] with additional information needed
/// for the agent loop, including unique IDs, turn tracking, and timestamps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedMessage {
    /// The underlying message.
    pub inner: Message,
    /// Unique identifier for this message.
    pub uuid: String,
    /// Turn this message belongs to.
    pub turn_id: String,
    /// Timestamp when the message was created.
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub timestamp: DateTime<Utc>,
    /// Source of the message.
    pub source: MessageSource,
    /// Whether this message has been tombstoned (marked for removal).
    #[serde(default)]
    pub tombstoned: bool,
}

impl TrackedMessage {
    /// Create a new tracked message.
    pub fn new(inner: Message, turn_id: impl Into<String>, source: MessageSource) -> Self {
        Self {
            inner,
            uuid: Uuid::new_v4().to_string(),
            turn_id: turn_id.into(),
            timestamp: Utc::now(),
            source,
            tombstoned: false,
        }
    }

    /// Create a user message.
    pub fn user(content: impl Into<String>, turn_id: impl Into<String>) -> Self {
        Self::new(Message::user(content), turn_id, MessageSource::User)
    }

    /// Create an assistant message.
    pub fn assistant(
        content: impl Into<String>,
        turn_id: impl Into<String>,
        request_id: Option<String>,
    ) -> Self {
        Self::new(
            Message::assistant(content),
            turn_id,
            MessageSource::assistant(request_id),
        )
    }

    /// Create an assistant message with content blocks.
    pub fn assistant_with_content(
        content: Vec<ContentBlock>,
        turn_id: impl Into<String>,
        request_id: Option<String>,
    ) -> Self {
        Self::new(
            Message::new(Role::Assistant, content),
            turn_id,
            MessageSource::assistant(request_id),
        )
    }

    /// Create a system message.
    pub fn system(content: impl Into<String>, turn_id: impl Into<String>) -> Self {
        Self::new(Message::system(content), turn_id, MessageSource::System)
    }

    /// Create a tool result message.
    pub fn tool_result(
        tool_use_id: impl Into<String>,
        content: impl Into<String>,
        turn_id: impl Into<String>,
    ) -> Self {
        let call_id = tool_use_id.into();
        Self::new(
            Message::tool_result(&call_id, ToolResultContent::Text(content.into())),
            turn_id,
            MessageSource::tool(&call_id),
        )
    }

    /// Create a tool error message.
    pub fn tool_error(
        tool_use_id: impl Into<String>,
        error: impl Into<String>,
        turn_id: impl Into<String>,
    ) -> Self {
        let call_id = tool_use_id.into();
        Self::new(
            Message::tool_error(&call_id, error),
            turn_id,
            MessageSource::tool(&call_id),
        )
    }

    /// Get the message role.
    pub fn role(&self) -> Role {
        self.inner.role
    }

    /// Get the message content blocks.
    pub fn content(&self) -> &[ContentBlock] {
        &self.inner.content
    }

    /// Get text content from the message.
    pub fn text(&self) -> String {
        crate::type_guards::get_text_content(&self.inner)
    }

    /// Check if this message has tool calls.
    pub fn has_tool_calls(&self) -> bool {
        crate::type_guards::has_tool_use(&self.inner)
    }

    /// Get tool calls from this message.
    pub fn tool_calls(&self) -> Vec<hyper_sdk::ToolCall> {
        crate::type_guards::get_tool_calls(&self.inner)
    }

    /// Mark this message as tombstoned.
    pub fn tombstone(&mut self) {
        self.tombstoned = true;
    }

    /// Check if this message is tombstoned.
    pub fn is_tombstoned(&self) -> bool {
        self.tombstoned
    }

    /// Convert to the underlying message for API requests.
    pub fn into_message(self) -> Message {
        self.inner
    }

    /// Get a reference to the underlying message.
    pub fn as_message(&self) -> &Message {
        &self.inner
    }
}

impl AsRef<Message> for TrackedMessage {
    fn as_ref(&self) -> &Message {
        &self.inner
    }
}

impl From<TrackedMessage> for Message {
    fn from(tracked: TrackedMessage) -> Self {
        tracked.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_message() {
        let msg = TrackedMessage::user("Hello", "turn-1");
        assert_eq!(msg.role(), Role::User);
        assert_eq!(msg.turn_id, "turn-1");
        assert_eq!(msg.text(), "Hello");
        assert!(!msg.is_tombstoned());
        assert!(matches!(msg.source, MessageSource::User));
    }

    #[test]
    fn test_assistant_message() {
        let msg = TrackedMessage::assistant("Hi there", "turn-1", Some("req-123".to_string()));
        assert_eq!(msg.role(), Role::Assistant);
        assert!(matches!(
            msg.source,
            MessageSource::Assistant { request_id: Some(ref id) } if id == "req-123"
        ));
    }

    #[test]
    fn test_tool_result_message() {
        let msg = TrackedMessage::tool_result("call-1", "result data", "turn-1");
        assert!(matches!(
            msg.source,
            MessageSource::Tool { call_id: ref id } if id == "call-1"
        ));
    }

    #[test]
    fn test_tombstoning() {
        let mut msg = TrackedMessage::user("Hello", "turn-1");
        assert!(!msg.is_tombstoned());

        msg.tombstone();
        assert!(msg.is_tombstoned());
    }

    #[test]
    fn test_uuid_uniqueness() {
        let msg1 = TrackedMessage::user("Hello", "turn-1");
        let msg2 = TrackedMessage::user("Hello", "turn-1");
        assert_ne!(msg1.uuid, msg2.uuid);
    }

    #[test]
    fn test_into_message() {
        let tracked = TrackedMessage::user("Hello", "turn-1");
        let message: Message = tracked.into();
        assert_eq!(message.role, Role::User);
    }

    #[test]
    fn test_assistant_with_content() {
        let content = vec![
            ContentBlock::text("Let me help"),
            ContentBlock::tool_use("call_1", "get_weather", serde_json::json!({"city": "NYC"})),
        ];
        let msg = TrackedMessage::assistant_with_content(content, "turn-1", None);
        assert!(msg.has_tool_calls());
        assert_eq!(msg.tool_calls().len(), 1);
    }
}
