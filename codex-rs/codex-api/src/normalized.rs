//! Provider-agnostic message representation for cross-model switching.
//!
//! When switching models mid-conversation:
//! 1. Parse `EncryptedContent` from stored `encrypted_content`
//! 2. Check if `(base_url, model)` matches current context
//! 3. If same model: use provider-specific fast path (parse raw response)
//! 4. If different model: `EncryptedContent::to_normalized()` → `normalized_to_*()` → target format

use serde::Deserialize;
use serde::Serialize;

/// Provider-agnostic representation of an assistant response turn.
///
/// Used for cross-model switching: extract from source provider, convert to target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedAssistantMessage {
    /// Text content parts from the assistant (non-thinking content).
    pub text_content: Vec<String>,

    /// Tool/function calls made by the assistant.
    pub tool_calls: Vec<NormalizedToolCall>,

    /// Thinking/reasoning content (if available).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_content: Option<Vec<String>>,
}

impl NormalizedAssistantMessage {
    /// Create a new empty message.
    pub fn new() -> Self {
        Self {
            text_content: Vec::new(),
            tool_calls: Vec::new(),
            thinking_content: None,
        }
    }

    /// Check if the message has any content.
    pub fn is_empty(&self) -> bool {
        self.text_content.is_empty()
            && self.tool_calls.is_empty()
            && self
                .thinking_content
                .as_ref()
                .map_or(true, |t| t.is_empty())
    }

    /// Serialize to JSON Value.
    pub fn to_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }

    /// Deserialize from JSON Value.
    pub fn from_value(value: &serde_json::Value) -> Option<Self> {
        serde_json::from_value(value.clone()).ok()
    }
}

impl Default for NormalizedAssistantMessage {
    fn default() -> Self {
        Self::new()
    }
}

/// A tool/function call in provider-agnostic format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedToolCall {
    /// Call ID for matching with tool results.
    pub call_id: String,

    /// Function/tool name.
    pub name: String,

    /// Arguments as JSON string.
    pub arguments: String,
}

impl NormalizedToolCall {
    /// Create a new tool call.
    pub fn new(
        call_id: impl Into<String>,
        name: impl Into<String>,
        arguments: impl Into<String>,
    ) -> Self {
        Self {
            call_id: call_id.into(),
            name: name.into(),
            arguments: arguments.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_normalized_message_new() {
        let msg = NormalizedAssistantMessage::new();
        assert!(msg.is_empty());
        assert!(msg.text_content.is_empty());
        assert!(msg.tool_calls.is_empty());
        assert!(msg.thinking_content.is_none());
    }

    #[test]
    fn test_normalized_message_with_content() {
        let msg = NormalizedAssistantMessage {
            text_content: vec!["Hello, world!".to_string()],
            tool_calls: vec![NormalizedToolCall::new(
                "call_1",
                "shell",
                r#"{"command":"ls"}"#,
            )],
            thinking_content: Some(vec!["Let me think...".to_string()]),
        };

        assert!(!msg.is_empty());
        assert_eq!(msg.text_content.len(), 1);
        assert_eq!(msg.tool_calls.len(), 1);
        assert_eq!(msg.thinking_content.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_normalized_message_serialization() {
        let msg = NormalizedAssistantMessage {
            text_content: vec!["Hello".to_string()],
            tool_calls: vec![NormalizedToolCall::new("call_1", "test", "{}")],
            thinking_content: None,
        };

        let value = msg.to_value();
        let parsed = NormalizedAssistantMessage::from_value(&value).unwrap();

        assert_eq!(parsed.text_content, msg.text_content);
        assert_eq!(parsed.tool_calls.len(), msg.tool_calls.len());
        assert_eq!(parsed.tool_calls[0].call_id, "call_1");
        assert_eq!(parsed.tool_calls[0].name, "test");
    }

    #[test]
    fn test_normalized_tool_call() {
        let tc = NormalizedToolCall::new("call_123", "read_file", r#"{"path":"/tmp/test.txt"}"#);

        assert_eq!(tc.call_id, "call_123");
        assert_eq!(tc.name, "read_file");
        assert!(tc.arguments.contains("path"));
    }

    #[test]
    fn test_normalized_message_thinking_skipped_when_none() {
        let msg = NormalizedAssistantMessage {
            text_content: vec!["Hello".to_string()],
            tool_calls: vec![],
            thinking_content: None,
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(!json.contains("thinking_content"));
    }

    #[test]
    fn test_normalized_message_thinking_included_when_some() {
        let msg = NormalizedAssistantMessage {
            text_content: vec![],
            tool_calls: vec![],
            thinking_content: Some(vec!["Thinking...".to_string()]),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("thinking_content"));
        assert!(json.contains("Thinking..."));
    }
}
