use serde::Serialize;

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
    PendingUserApproval {
        turn_id: String,
        
        /// The type of approval being requested (e.g., "command_approval", "file_approval")
        approval_type: String,
        
        /// Description of what is being requested
        description: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_notification() {
        let notification = UserNotification::AgentTurnComplete {
            turn_id: "12345".to_string(),
            input_messages: vec!["Rename `foo` to `bar` and update the callsites.".to_string()],
            last_assistant_message: Some(
                "Rename complete and verified `cargo build` succeeds.".to_string(),
            ),
        };
        let serialized = serde_json::to_string(&notification).unwrap();
        assert_eq!(
            serialized,
            r#"{"type":"agent-turn-complete","turn-id":"12345","input-messages":["Rename `foo` to `bar` and update the callsites."],"last-assistant-message":"Rename complete and verified `cargo build` succeeds."}"#
        );
    }

    #[test]
    fn test_pending_user_approval_notification() {
        let notification = UserNotification::PendingUserApproval {
            turn_id: "67890".to_string(),
            approval_type: "command_approval".to_string(),
            description: "Waiting for approval to execute: rm -rf /tmp/test".to_string(),
        };
        let serialized = serde_json::to_string(&notification).unwrap();
        assert_eq!(
            serialized,
            r#"{"type":"pending-user-approval","turn-id":"67890","approval-type":"command_approval","description":"Waiting for approval to execute: rm -rf /tmp/test"}"#
        );
    }
}
