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
        thread_id: String,
        turn_id: String,

        /// Messages that the user sent to the agent to initiate the turn.
        input_messages: Vec<String>,

        /// The last message sent by the assistant in the turn.
        last_assistant_message: Option<String>,
    },
    #[serde(rename_all = "kebab-case")]
    DetachedRunFinished {
        agent_id: String,
        run_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        conversation_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        summary: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    fn test_user_notification() -> Result<()> {
        let notification = UserNotification::AgentTurnComplete {
            thread_id: "b5f6c1c2-1111-2222-3333-444455556666".to_string(),
            turn_id: "12345".to_string(),
            input_messages: vec!["Rename `foo` to `bar` and update the callsites.".to_string()],
            last_assistant_message: Some(
                "Rename complete and verified `cargo build` succeeds.".to_string(),
            ),
        };
        let serialized = serde_json::to_string(&notification)?;
        assert_eq!(
            serialized,
            r#"{"type":"agent-turn-complete","thread-id":"b5f6c1c2-1111-2222-3333-444455556666","turn-id":"12345","input-messages":["Rename `foo` to `bar` and update the callsites."],"last-assistant-message":"Rename complete and verified `cargo build` succeeds."}"#
        );
        Ok(())
    }

    #[test]
    fn test_detached_run_notification() -> Result<()> {
        let notification = UserNotification::DetachedRunFinished {
            agent_id: "ideas_provider".to_string(),
            run_id: "run-1".to_string(),
            conversation_id: Some("conv-123".to_string()),
            summary: Some("Drafted outline".to_string()),
            duration_ms: Some(42_000),
            error: None,
        };
        let serialized = serde_json::to_string(&notification)?;
        assert_eq!(
            serialized,
            r#"{"type":"detached-run-finished","agent-id":"ideas_provider","run-id":"run-1","conversation-id":"conv-123","summary":"Drafted outline","duration-ms":42000}"#
        );
        Ok(())
    }
}
