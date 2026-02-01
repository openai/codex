//! Session state from the agent.
//!
//! This module contains state that comes from or is synchronized with
//! the core agent loop.

use std::path::PathBuf;

use cocode_protocol::ThinkingLevel;
use cocode_protocol::TokenUsage;

/// State synchronized with the agent session.
#[derive(Debug, Clone)]
pub struct SessionState {
    /// Messages in the conversation.
    pub messages: Vec<ChatMessage>,

    /// Current model being used.
    pub current_model: String,

    /// Current thinking level.
    pub thinking_level: ThinkingLevel,

    /// Whether plan mode is active.
    pub plan_mode: bool,

    /// Path to the plan file (when in plan mode).
    pub plan_file: Option<PathBuf>,

    /// Active tool executions.
    pub tool_executions: Vec<ToolExecution>,

    /// Total token usage for the session.
    pub token_usage: TokenUsage,

    /// Session ID (if resuming).
    pub session_id: Option<String>,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            messages: Vec::new(),
            current_model: "claude-sonnet-4-20250514".to_string(),
            thinking_level: ThinkingLevel::default(),
            plan_mode: false,
            plan_file: None,
            tool_executions: Vec::new(),
            token_usage: TokenUsage::default(),
            session_id: None,
        }
    }
}

impl SessionState {
    /// Add a message to the conversation.
    pub fn add_message(&mut self, message: ChatMessage) {
        self.messages.push(message);
    }

    /// Get the last message, if any.
    pub fn last_message(&self) -> Option<&ChatMessage> {
        self.messages.last()
    }

    /// Get the last assistant message, if any.
    pub fn last_assistant_message(&self) -> Option<&ChatMessage> {
        self.messages
            .iter()
            .rev()
            .find(|m| m.role == MessageRole::Assistant)
    }

    /// Update token usage.
    pub fn update_tokens(&mut self, usage: TokenUsage) {
        self.token_usage.input_tokens += usage.input_tokens;
        self.token_usage.output_tokens += usage.output_tokens;
        if let Some(cache) = usage.cache_read_tokens {
            *self.token_usage.cache_read_tokens.get_or_insert(0) += cache;
        }
    }

    /// Start a tool execution.
    pub fn start_tool(&mut self, call_id: String, name: String) {
        self.tool_executions.push(ToolExecution {
            call_id,
            name,
            status: ToolStatus::Running,
            progress: None,
            output: None,
        });
    }

    /// Update tool progress.
    pub fn update_tool_progress(&mut self, call_id: &str, progress: String) {
        if let Some(tool) = self
            .tool_executions
            .iter_mut()
            .find(|t| t.call_id == call_id)
        {
            tool.progress = Some(progress);
        }
    }

    /// Complete a tool execution.
    pub fn complete_tool(&mut self, call_id: &str, output: String, is_error: bool) {
        if let Some(tool) = self
            .tool_executions
            .iter_mut()
            .find(|t| t.call_id == call_id)
        {
            tool.status = if is_error {
                ToolStatus::Failed
            } else {
                ToolStatus::Completed
            };
            tool.output = Some(output);
        }
    }

    /// Remove completed tools older than a certain threshold.
    pub fn cleanup_completed_tools(&mut self, max_completed: usize) {
        let completed_count = self
            .tool_executions
            .iter()
            .filter(|t| matches!(t.status, ToolStatus::Completed | ToolStatus::Failed))
            .count();

        if completed_count > max_completed {
            let to_remove = completed_count - max_completed;
            let mut removed = 0;
            self.tool_executions.retain(|t| {
                if removed >= to_remove {
                    return true;
                }
                if matches!(t.status, ToolStatus::Completed | ToolStatus::Failed) {
                    removed += 1;
                    return false;
                }
                true
            });
        }
    }
}

/// A message in the conversation.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    /// Unique identifier for this message.
    pub id: String,

    /// Role of the message sender.
    pub role: MessageRole,

    /// Content of the message.
    pub content: String,

    /// Whether this message is still being streamed.
    pub streaming: bool,

    /// Thinking content (if applicable).
    pub thinking: Option<String>,
}

impl ChatMessage {
    /// Create a new user message.
    pub fn user(id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            role: MessageRole::User,
            content: content.into(),
            streaming: false,
            thinking: None,
        }
    }

    /// Create a new assistant message.
    pub fn assistant(id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            role: MessageRole::Assistant,
            content: content.into(),
            streaming: false,
            thinking: None,
        }
    }

    /// Create a new streaming assistant message.
    pub fn streaming_assistant(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            role: MessageRole::Assistant,
            content: String::new(),
            streaming: true,
            thinking: None,
        }
    }

    /// Append content to the message.
    pub fn append(&mut self, delta: &str) {
        self.content.push_str(delta);
    }

    /// Append thinking content.
    pub fn append_thinking(&mut self, delta: &str) {
        self.thinking
            .get_or_insert_with(String::new)
            .push_str(delta);
    }

    /// Mark the message as complete (no longer streaming).
    pub fn complete(&mut self) {
        self.streaming = false;
    }
}

/// Role of a message sender.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageRole {
    /// User (human) message.
    User,
    /// Assistant (AI) message.
    Assistant,
    /// System message.
    System,
}

/// Status of a tool execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolStatus {
    /// Tool is currently running.
    Running,
    /// Tool completed successfully.
    Completed,
    /// Tool failed with an error.
    Failed,
}

/// A tool execution in progress or completed.
#[derive(Debug, Clone)]
pub struct ToolExecution {
    /// Call identifier.
    pub call_id: String,
    /// Tool name.
    pub name: String,
    /// Current status.
    pub status: ToolStatus,
    /// Progress message (if available).
    pub progress: Option<String>,
    /// Output (when completed).
    pub output: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_state_default() {
        let state = SessionState::default();
        assert!(state.messages.is_empty());
        assert!(!state.plan_mode);
        assert!(state.tool_executions.is_empty());
    }

    #[test]
    fn test_add_message() {
        let mut state = SessionState::default();
        state.add_message(ChatMessage::user("1", "Hello"));
        assert_eq!(state.messages.len(), 1);
        assert_eq!(
            state.last_message().map(|m| m.content.as_str()),
            Some("Hello")
        );
    }

    #[test]
    fn test_chat_message_streaming() {
        let mut msg = ChatMessage::streaming_assistant("1");
        assert!(msg.streaming);
        assert!(msg.content.is_empty());

        msg.append("Hello ");
        msg.append("World");
        assert_eq!(msg.content, "Hello World");

        msg.complete();
        assert!(!msg.streaming);
    }

    #[test]
    fn test_tool_lifecycle() {
        let mut state = SessionState::default();

        state.start_tool("call-1".to_string(), "bash".to_string());
        assert_eq!(state.tool_executions.len(), 1);
        assert_eq!(state.tool_executions[0].status, ToolStatus::Running);

        state.update_tool_progress("call-1", "Running...".to_string());
        assert_eq!(
            state.tool_executions[0].progress,
            Some("Running...".to_string())
        );

        state.complete_tool("call-1", "Success".to_string(), false);
        assert_eq!(state.tool_executions[0].status, ToolStatus::Completed);
        assert_eq!(state.tool_executions[0].output, Some("Success".to_string()));
    }

    #[test]
    fn test_cleanup_completed_tools() {
        let mut state = SessionState::default();

        // Add 5 completed tools
        for i in 0..5 {
            state.tool_executions.push(ToolExecution {
                call_id: format!("call-{i}"),
                name: "test".to_string(),
                status: ToolStatus::Completed,
                progress: None,
                output: None,
            });
        }

        // Keep only 2
        state.cleanup_completed_tools(2);
        assert_eq!(state.tool_executions.len(), 2);
    }
}
