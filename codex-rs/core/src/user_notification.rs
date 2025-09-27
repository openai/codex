use serde::Serialize;
use tracing::error;
use tracing::warn;

#[derive(Debug, Default)]
pub(crate) struct UserNotifier {
    notify_command: Option<Vec<String>>,
}

impl UserNotifier {
    pub(crate) fn notify(&self, notification: &UserNotification) {
        if let Some(notify_command) = &self.notify_command
            && !notify_command.is_empty()
        {
            self.invoke_notify(notify_command, notification)
        }
    }

    fn invoke_notify(&self, notify_command: &[String], notification: &UserNotification) {
        let Ok(json) = serde_json::to_string(&notification) else {
            error!("failed to serialise notification payload");
            return;
        };

        let mut command = std::process::Command::new(&notify_command[0]);
        if notify_command.len() > 1 {
            command.args(&notify_command[1..]);
        }
        command.arg(json);

        // Fire-and-forget – we do not wait for completion.
        if let Err(e) = command.spawn() {
            warn!("failed to spawn notifier '{}': {e}", notify_command[0]);
        }
    }

    pub(crate) fn new(notify: Option<Vec<String>>) -> Self {
        Self {
            notify_command: notify,
        }
    }
}

/// User can configure a program that will receive notifications. Each
/// notification is serialized as JSON and passed as an argument to the
/// program.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub(crate) enum UserNotification {
    #[serde(rename_all = "kebab-case")]
    AgentTurnComplete {
        turn_id: String,

        /// Messages that the user sent to the agent to initiate the turn.
        input_messages: Vec<String>,

        /// The last message sent by the assistant in the turn.
        last_assistant_message: Option<String>,
    },

    #[serde(rename_all = "kebab-case")]
    UserInputRequired {
        turn_id: String,

        /// The reason user input is required (e.g., "approval", "confirmation", "input")
        reason: String,

        /// Additional context about what input is needed
        message: Option<String>,
    },

    #[serde(rename_all = "kebab-case")]
    SessionStarted {
        session_id: String,

        /// Working directory for the session
        cwd: String,
    },

    #[serde(rename_all = "kebab-case")]
    SessionEnded { session_id: String },

    #[serde(rename_all = "kebab-case")]
    ToolExecutionStarted {
        turn_id: String,

        /// Name of the tool being executed
        tool_name: String,

        /// Tool arguments
        tool_args: Option<serde_json::Value>,
    },

    #[serde(rename_all = "kebab-case")]
    ToolExecutionCompleted {
        turn_id: String,

        /// Name of the tool that was executed
        tool_name: String,

        /// Whether the execution was successful
        success: bool,

        /// Error message if execution failed
        error_message: Option<String>,
    },
}

/// Configuration for event hooks
#[derive(Debug, Clone, PartialEq, serde::Deserialize, Default)]
pub struct EventHooks {
    /// Hooks to run when agent turn completes
    pub agent_turn_complete: Option<Vec<String>>,

    /// Hooks to run when user input is required
    pub user_input_required: Option<Vec<String>>,

    /// Hooks to run when session starts
    pub session_started: Option<Vec<String>>,

    /// Hooks to run when session ends
    pub session_ended: Option<Vec<String>>,

    /// Hooks to run when tool execution starts
    pub tool_execution_started: Option<Vec<String>>,

    /// Hooks to run when tool execution completes
    pub tool_execution_completed: Option<Vec<String>>,
}

impl EventHooks {
    /// Get the hooks configured for a specific notification type
    pub(crate) fn get_hooks_for_notification(
        &self,
        notification: &UserNotification,
    ) -> Option<&Vec<String>> {
        match notification {
            UserNotification::AgentTurnComplete { .. } => self.agent_turn_complete.as_ref(),
            UserNotification::UserInputRequired { .. } => self.user_input_required.as_ref(),
            UserNotification::SessionStarted { .. } => self.session_started.as_ref(),
            UserNotification::SessionEnded { .. } => self.session_ended.as_ref(),
            UserNotification::ToolExecutionStarted { .. } => self.tool_execution_started.as_ref(),
            UserNotification::ToolExecutionCompleted { .. } => {
                self.tool_execution_completed.as_ref()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    fn test_user_notification() -> Result<()> {
        let notification = UserNotification::AgentTurnComplete {
            turn_id: "12345".to_string(),
            input_messages: vec!["Rename `foo` to `bar` and update the callsites.".to_string()],
            last_assistant_message: Some(
                "Rename complete and verified `cargo build` succeeds.".to_string(),
            ),
        };
        let serialized = serde_json::to_string(&notification)?;
        assert_eq!(
            serialized,
            r#"{"type":"agent-turn-complete","turn-id":"12345","input-messages":["Rename `foo` to `bar` and update the callsites."],"last-assistant-message":"Rename complete and verified `cargo build` succeeds."}"#
        );
        Ok(())
    }

    #[test]
    fn test_user_input_required_notification() {
        let notification = UserNotification::UserInputRequired {
            turn_id: "67890".to_string(),
            reason: "approval".to_string(),
            message: Some("Approve file deletion?".to_string()),
        };
        let serialized = serde_json::to_string(&notification).unwrap();
        assert_eq!(
            serialized,
            r#"{"type":"user-input-required","turn-id":"67890","reason":"approval","message":"Approve file deletion?"}"#
        );
    }

    #[test]
    fn test_session_started_notification() {
        let notification = UserNotification::SessionStarted {
            session_id: "abc123".to_string(),
            cwd: "/path/to/project".to_string(),
        };
        let serialized = serde_json::to_string(&notification).unwrap();
        assert_eq!(
            serialized,
            r#"{"type":"session-started","session-id":"abc123","cwd":"/path/to/project"}"#
        );
    }

    #[test]
    fn test_tool_execution_started_notification() {
        let notification = UserNotification::ToolExecutionStarted {
            turn_id: "turn123".to_string(),
            tool_name: "bash".to_string(),
            tool_args: Some(serde_json::json!({"command": "ls -la"})),
        };
        let serialized = serde_json::to_string(&notification).unwrap();
        assert_eq!(
            serialized,
            r#"{"type":"tool-execution-started","turn-id":"turn123","tool-name":"bash","tool-args":{"command":"ls -la"}}"#
        );
    }

    #[test]
    fn test_event_hooks_get_hooks_for_notification() {
        let hooks = EventHooks {
            agent_turn_complete: Some(vec!["hook1".to_string(), "hook2".to_string()]),
            user_input_required: Some(vec!["input-hook".to_string()]),
            ..Default::default()
        };

        let notification = UserNotification::AgentTurnComplete {
            turn_id: "test".to_string(),
            input_messages: vec![],
            last_assistant_message: None,
        };

        assert_eq!(
            hooks.get_hooks_for_notification(&notification),
            Some(&vec!["hook1".to_string(), "hook2".to_string()])
        );

        let input_notification = UserNotification::UserInputRequired {
            turn_id: "test".to_string(),
            reason: "approval".to_string(),
            message: None,
        };

        assert_eq!(
            hooks.get_hooks_for_notification(&input_notification),
            Some(&vec!["input-hook".to_string()])
        );
    }
}
