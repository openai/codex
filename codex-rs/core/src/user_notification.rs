use serde::Serialize;
use std::path::PathBuf;

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
    PendingCommandApproval {
        turn_id: String,
        command: Vec<String>,
    },

    #[serde(rename_all = "kebab-case")]
    PendingFileApproval {
        turn_id: String,
        changes: Vec<FileChangeInfo>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct FileChangeInfo {
    pub operation: String, // "A", "D", "M"
    pub path: PathBuf,
    pub new_path: Option<PathBuf>, // For rename operations
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
    fn test_pending_command_approval_notification() {
        let notification = UserNotification::PendingCommandApproval {
            turn_id: "67890".to_string(),
            command: vec!["rm".to_string(), "-rf".to_string(), "/tmp/test".to_string()],
        };
        let serialized = serde_json::to_string(&notification).unwrap();
        assert_eq!(
            serialized,
            r#"{"type":"pending-command-approval","turn-id":"67890","command":["rm","-rf","/tmp/test"]}"#
        );
    }

    #[test]
    fn test_pending_file_approval_notification() {
        let notification = UserNotification::PendingFileApproval {
            turn_id: "12345".to_string(),
            changes: vec![
                FileChangeInfo {
                    operation: "A".to_string(),
                    path: "/path/to/new_file.rs".into(),
                    new_path: None,
                },
                FileChangeInfo {
                    operation: "D".to_string(),
                    path: "/path/to/deleted_file.rs".into(),
                    new_path: None,
                },
                FileChangeInfo {
                    operation: "M".to_string(),
                    path: "/path/to/existing.rs".into(),
                    new_path: None,
                },
                FileChangeInfo {
                    operation: "M".to_string(),
                    path: "/path/to/old_file.rs".into(),
                    new_path: Some("/path/to/renamed_file.rs".into()),
                },
            ],
        };
        let serialized = serde_json::to_string(&notification).unwrap();
        assert_eq!(
            serialized,
            r#"{"type":"pending-file-approval","turn-id":"12345","changes":[{"operation":"A","path":"/path/to/new_file.rs","new-path":null},{"operation":"D","path":"/path/to/deleted_file.rs","new-path":null},{"operation":"M","path":"/path/to/existing.rs","new-path":null},{"operation":"M","path":"/path/to/old_file.rs","new-path":"/path/to/renamed_file.rs"}]}"#
        );
    }
}
