//! Tracked messages with metadata for turn tracking.
//!
//! This module provides [`TrackedMessage`] which wraps hyper-sdk's [`Message`]
//! with additional metadata for tracking in the agent loop.

use chrono::DateTime;
use chrono::Utc;
use hyper_sdk::ContentBlock;
use hyper_sdk::Message;
use hyper_sdk::Role;
use hyper_sdk::ToolResultContent;
use serde::Deserialize;
use serde::Serialize;
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
    /// System reminder (dynamic context injection).
    SystemReminder {
        /// The type of reminder (e.g., "changed_files", "plan_mode_enter").
        reminder_type: String,
    },
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

    /// Create a system reminder source with reminder type.
    pub fn system_reminder(reminder_type: impl Into<String>) -> Self {
        MessageSource::SystemReminder {
            reminder_type: reminder_type.into(),
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
    /// Whether this is a meta message (hidden from user, visible to model).
    ///
    /// Meta messages are included in API requests but not shown in user-facing
    /// conversation history. Used for system reminders and other injected context.
    #[serde(default)]
    pub is_meta: bool,
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
            is_meta: false,
        }
    }

    /// Create a new meta message (hidden from user, visible to model).
    pub fn new_meta(inner: Message, turn_id: impl Into<String>, source: MessageSource) -> Self {
        Self {
            inner,
            uuid: Uuid::new_v4().to_string(),
            turn_id: turn_id.into(),
            timestamp: Utc::now(),
            source,
            tombstoned: false,
            is_meta: true,
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

    /// Create a system reminder message (meta message for dynamic context).
    ///
    /// System reminders are injected as user messages with `is_meta: true`,
    /// meaning they are included in API requests but not shown to the user.
    pub fn system_reminder(
        content: impl Into<String>,
        reminder_type: impl Into<String>,
        turn_id: impl Into<String>,
    ) -> Self {
        Self::new_meta(
            Message::user(content),
            turn_id,
            MessageSource::system_reminder(reminder_type),
        )
    }

    /// Check if this message is a meta message.
    pub fn is_meta(&self) -> bool {
        self.is_meta
    }

    /// Set the meta flag on this message.
    pub fn set_meta(&mut self, is_meta: bool) {
        self.is_meta = is_meta;
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

    #[test]
    fn test_system_reminder_message() {
        let msg = TrackedMessage::system_reminder(
            "<system-reminder>File changed</system-reminder>",
            "changed_files",
            "turn-1",
        );
        assert_eq!(msg.role(), Role::User); // System reminders are sent as user messages
        assert!(msg.is_meta()); // But marked as meta
        assert!(matches!(
            msg.source,
            MessageSource::SystemReminder { reminder_type: ref t } if t == "changed_files"
        ));
    }

    #[test]
    fn test_is_meta_default() {
        // Regular messages should not be meta
        let msg = TrackedMessage::user("Hello", "turn-1");
        assert!(!msg.is_meta());

        let msg = TrackedMessage::assistant("Hi", "turn-1", None);
        assert!(!msg.is_meta());
    }

    #[test]
    fn test_set_meta() {
        let mut msg = TrackedMessage::user("Hello", "turn-1");
        assert!(!msg.is_meta());

        msg.set_meta(true);
        assert!(msg.is_meta());

        msg.set_meta(false);
        assert!(!msg.is_meta());
    }

    #[test]
    fn test_new_meta() {
        let msg =
            TrackedMessage::new_meta(Message::user("meta content"), "turn-1", MessageSource::User);
        assert!(msg.is_meta());
    }
}
