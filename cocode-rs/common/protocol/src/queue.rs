//! Queue types for user input during agent execution.
//!
//! Queued commands (Enter while streaming) are consumed once in the agent
//! loop and injected as steering system-reminders that ask the model to
//! address each message (consume-then-remove pattern).

use serde::Deserialize;
use serde::Serialize;

/// A queued command (Enter during streaming).
///
/// These commands are shown in the UI and consumed once in the agent loop
/// as steering system-reminders (consume-then-remove pattern).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserQueuedCommand {
    /// Unique identifier for this command.
    pub id: String,
    /// The prompt/command text.
    pub prompt: String,
    /// Timestamp when queued (Unix milliseconds).
    pub queued_at: i64,
}

impl UserQueuedCommand {
    /// Create a new queued command.
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            prompt: prompt.into(),
            queued_at: chrono::Utc::now().timestamp_millis(),
        }
    }

    /// Get a preview of the command (first N chars).
    pub fn preview(&self, max_len: usize) -> String {
        if self.prompt.len() <= max_len {
            self.prompt.clone()
        } else {
            format!("{}...", &self.prompt[..max_len])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_queued_command() {
        let cmd = UserQueuedCommand::new("test command");
        assert_eq!(cmd.prompt, "test command");
        assert!(!cmd.id.is_empty());
        assert!(cmd.queued_at > 0);
    }

    #[test]
    fn test_command_preview() {
        let cmd = UserQueuedCommand::new("this is a very long command that should be truncated");
        let preview = cmd.preview(20);
        assert_eq!(preview, "this is a very long ...");

        let short_cmd = UserQueuedCommand::new("short");
        assert_eq!(short_cmd.preview(20), "short");
    }

    #[test]
    fn test_serde_roundtrip() {
        let cmd = UserQueuedCommand::new("test");
        let json = serde_json::to_string(&cmd).unwrap();
        let parsed: UserQueuedCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.prompt, cmd.prompt);
    }
}
