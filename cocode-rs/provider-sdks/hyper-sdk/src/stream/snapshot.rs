//! Stream snapshot types for accumulated state during streaming.
//!
//! This module provides types for tracking the accumulated state during streaming,
//! enabling Crush-like message accumulation patterns where the same message is
//! continuously updated rather than creating multiple events.

use crate::response::FinishReason;
use crate::response::TokenUsage;
use crate::tools::ToolCall;
use serde::Deserialize;
use serde::Serialize;

/// Accumulated state snapshot during streaming.
///
/// This is the core type for Crush-like streaming patterns. Each event updates
/// the snapshot, and consumers can access the current accumulated state at any time.
///
/// # Example
///
/// ```ignore
/// let mut processor = model.stream(request).await?;
/// processor.on_update(|snapshot| async move {
///     // Update the same message in DB with accumulated state
///     db.update_message(msg_id, &snapshot.text).await?;
///     Ok(())
/// }).await?;
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StreamSnapshot {
    /// Response ID from the provider.
    pub id: Option<String>,

    /// Model name (may differ from requested model due to aliases/defaults).
    #[serde(default)]
    pub model: String,

    /// Accumulated text content.
    ///
    /// Note: This field grows unbounded during streaming. This is intentional because:
    /// 1. Streaming responses are bounded by `max_tokens` in the request
    /// 2. The entire response is needed to construct the final `GenerateResponse`
    /// 3. Memory pressure should be managed at the application level (e.g., via request limits)
    pub text: String,

    /// Accumulated thinking content (for extended thinking models).
    pub thinking: Option<ThinkingSnapshot>,

    /// All tool calls (partial or complete).
    pub tool_calls: Vec<ToolCallSnapshot>,

    /// Finish reason (set when stream completes).
    pub finish_reason: Option<FinishReason>,

    /// Token usage statistics (set when stream completes).
    pub usage: Option<TokenUsage>,

    /// Whether the stream has completed.
    pub is_complete: bool,
}

impl StreamSnapshot {
    /// Create a new empty snapshot.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if the response has any text content.
    pub fn has_text(&self) -> bool {
        !self.text.is_empty()
    }

    /// Check if the response has thinking content.
    pub fn has_thinking(&self) -> bool {
        self.thinking
            .as_ref()
            .is_some_and(|t| !t.content.is_empty())
    }

    /// Check if the response has tool calls.
    pub fn has_tool_calls(&self) -> bool {
        !self.tool_calls.is_empty()
    }

    /// Get completed tool calls only.
    pub fn completed_tool_calls(&self) -> Vec<&ToolCallSnapshot> {
        self.tool_calls.iter().filter(|tc| tc.is_complete).collect()
    }

    /// Get pending (incomplete) tool calls.
    pub fn pending_tool_calls(&self) -> Vec<&ToolCallSnapshot> {
        self.tool_calls
            .iter()
            .filter(|tc| !tc.is_complete)
            .collect()
    }

    /// Convert to ToolCall objects (only complete ones).
    pub fn to_tool_calls(&self) -> Vec<ToolCall> {
        self.tool_calls
            .iter()
            .filter(|tc| tc.is_complete)
            .map(|tc| tc.to_tool_call())
            .collect()
    }
}

/// Accumulated thinking content snapshot.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ThinkingSnapshot {
    /// Accumulated thinking content.
    pub content: String,

    /// Cryptographic signature (provider-specific, set on completion).
    pub signature: Option<String>,

    /// Whether the thinking block is complete.
    pub is_complete: bool,
}

impl ThinkingSnapshot {
    /// Create a new empty thinking snapshot.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append delta content.
    pub fn append(&mut self, delta: &str) {
        self.content.push_str(delta);
    }

    /// Mark as complete with optional signature.
    pub fn complete(&mut self, signature: Option<String>) {
        self.is_complete = true;
        self.signature = signature;
    }
}

/// Accumulated tool call snapshot.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolCallSnapshot {
    /// Tool call ID.
    pub id: String,

    /// Tool name.
    pub name: String,

    /// Partial or complete JSON arguments.
    pub arguments: String,

    /// Whether the tool call arguments are complete.
    pub is_complete: bool,
}

impl ToolCallSnapshot {
    /// Create a new tool call snapshot.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            arguments: String::new(),
            is_complete: false,
        }
    }

    /// Append delta arguments.
    pub fn append_arguments(&mut self, delta: &str) {
        self.arguments.push_str(delta);
    }

    /// Mark as complete with final arguments.
    pub fn complete(&mut self, arguments: String) {
        self.arguments = arguments;
        self.is_complete = true;
    }

    /// Parse arguments as JSON.
    pub fn parsed_arguments(&self) -> Option<serde_json::Value> {
        serde_json::from_str(&self.arguments).ok()
    }

    /// Convert to a ToolCall (returns None if not complete or invalid JSON).
    pub fn to_tool_call(&self) -> ToolCall {
        let args = match self.parsed_arguments() {
            Some(value) => value,
            None => {
                tracing::debug!(
                    tool_call_id = %self.id,
                    tool_name = %self.name,
                    arguments = %self.arguments,
                    "Failed to parse tool call arguments, using null"
                );
                serde_json::Value::Null
            }
        };
        ToolCall::new(&self.id, &self.name, args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_default() {
        let snapshot = StreamSnapshot::new();
        assert!(!snapshot.has_text());
        assert!(!snapshot.has_thinking());
        assert!(!snapshot.has_tool_calls());
        assert!(!snapshot.is_complete);
    }

    #[test]
    fn test_thinking_snapshot() {
        let mut thinking = ThinkingSnapshot::new();
        thinking.append("Hello ");
        thinking.append("world");
        assert_eq!(thinking.content, "Hello world");
        assert!(!thinking.is_complete);

        thinking.complete(Some("sig123".to_string()));
        assert!(thinking.is_complete);
        assert_eq!(thinking.signature, Some("sig123".to_string()));
    }

    #[test]
    fn test_tool_call_snapshot() {
        let mut tc = ToolCallSnapshot::new("call_1", "get_weather");
        tc.append_arguments("{\"city\":");
        tc.append_arguments("\"NYC\"}");

        assert!(!tc.is_complete);
        assert_eq!(tc.arguments, "{\"city\":\"NYC\"}");

        let args = tc.parsed_arguments().unwrap();
        assert_eq!(args["city"], "NYC");

        tc.complete("{\"city\":\"NYC\"}".to_string());
        assert!(tc.is_complete);
    }

    #[test]
    fn test_snapshot_tool_calls_filtering() {
        let mut snapshot = StreamSnapshot::new();
        snapshot.tool_calls.push(ToolCallSnapshot {
            id: "call_1".to_string(),
            name: "tool_a".to_string(),
            arguments: "{\"a\":1}".to_string(),
            is_complete: true,
        });
        snapshot.tool_calls.push(ToolCallSnapshot {
            id: "call_2".to_string(),
            name: "tool_b".to_string(),
            arguments: "{\"b\":".to_string(),
            is_complete: false,
        });

        assert_eq!(snapshot.completed_tool_calls().len(), 1);
        assert_eq!(snapshot.pending_tool_calls().len(), 1);
        assert_eq!(snapshot.to_tool_calls().len(), 1);
    }
}
