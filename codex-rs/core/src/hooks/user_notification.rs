use std::sync::Arc;

use serde::Serialize;
use std::path::Path;
use std::process::Stdio;

use super::registry::command_from_argv;
use super::types::Hook;
use super::types::HookEvent;
use super::types::HookOutcome;
use super::types::HookPayload;

/// Legacy notify payload appended as the final argv argument for backward compatibility.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum UserNotification {
    #[serde(rename_all = "kebab-case")]
    AgentTurnComplete {
        thread_id: String,
        turn_id: String,
        cwd: String,

        /// Messages that the user sent to the agent to initiate the turn.
        input_messages: Vec<String>,

        /// The last message sent by the assistant in the turn.
        last_assistant_message: Option<String>,
    },
}

pub(super) fn legacy_notify_json(
    hook_event: &HookEvent,
    cwd: &Path,
) -> Result<String, serde_json::Error> {
    let notification = match hook_event {
        HookEvent::AfterAgent { event } => UserNotification::AgentTurnComplete {
            thread_id: event.thread_id.to_string(),
            turn_id: event.turn_id.clone(),
            cwd: cwd.display().to_string(),
            input_messages: event.input_messages.clone(),
            last_assistant_message: event.last_assistant_message.clone(),
        },
        // Legacy notification format only supports AfterAgent events.
        // Other events use the new stdin/stdout JSON protocol.
        _ => return serde_json::to_string(hook_event),
    };
    serde_json::to_string(&notification)
}

pub(super) fn notify_hook(argv: Vec<String>) -> Hook {
    let argv = Arc::new(argv);
    Hook {
        func: Arc::new(move |payload: &HookPayload| {
            let argv = Arc::clone(&argv);
            Box::pin(async move {
                let mut command = match command_from_argv(&argv) {
                    Some(command) => command,
                    None => return HookOutcome::Proceed,
                };
                if let Ok(notify_payload) = legacy_notify_json(&payload.hook_event, &payload.cwd) {
                    command.arg(notify_payload);
                }

                // Backwards-compat: match legacy notify behavior (argv + JSON arg, fire-and-forget).
                command
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null());

                let _ = command.spawn();
                HookOutcome::Proceed
            })
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use anyhow::Result;
    use codex_protocol::ThreadId;
    use pretty_assertions::assert_eq;
    use serde_json::Value;
    use serde_json::json;

    fn expected_notification_json() -> Value {
        json!({
            "type": "agent-turn-complete",
            "thread-id": "b5f6c1c2-1111-2222-3333-444455556666",
            "turn-id": "12345",
            "cwd": "/Users/example/project",
            "input-messages": ["Rename `foo` to `bar` and update the callsites."],
            "last-assistant-message": "Rename complete and verified `cargo build` succeeds.",
        })
    }

    #[test]
    fn test_user_notification() -> Result<()> {
        let notification = UserNotification::AgentTurnComplete {
            thread_id: "b5f6c1c2-1111-2222-3333-444455556666".to_string(),
            turn_id: "12345".to_string(),
            cwd: "/Users/example/project".to_string(),
            input_messages: vec!["Rename `foo` to `bar` and update the callsites.".to_string()],
            last_assistant_message: Some(
                "Rename complete and verified `cargo build` succeeds.".to_string(),
            ),
        };
        let serialized = serde_json::to_string(&notification)?;
        let actual: Value = serde_json::from_str(&serialized)?;
        assert_eq!(actual, expected_notification_json());
        Ok(())
    }

    #[test]
    fn legacy_notify_json_matches_historical_wire_shape() -> Result<()> {
        let hook_event = HookEvent::AfterAgent {
            event: super::super::types::HookEventAfterAgent {
                thread_id: ThreadId::from_string("b5f6c1c2-1111-2222-3333-444455556666")
                    .expect("valid thread id"),
                turn_id: "12345".to_string(),
                input_messages: vec!["Rename `foo` to `bar` and update the callsites.".to_string()],
                last_assistant_message: Some(
                    "Rename complete and verified `cargo build` succeeds.".to_string(),
                ),
            },
        };

        let serialized = legacy_notify_json(&hook_event, Path::new("/Users/example/project"))?;
        let actual: Value = serde_json::from_str(&serialized)?;
        assert_eq!(actual, expected_notification_json());

        Ok(())
    }

    #[test]
    fn legacy_notify_json_for_pre_tool_use_returns_event_serialized_directly() -> Result<()> {
        use super::super::types::HookEventPreToolUse;

        let hook_event = HookEvent::PreToolUse {
            event: HookEventPreToolUse {
                tool_name: "bash".to_string(),
                tool_input: r#"{"command": "ls"}"#.to_string(),
            },
        };

        let serialized = legacy_notify_json(&hook_event, Path::new("/tmp"))?;
        let actual: Value = serde_json::from_str(&serialized)?;

        // PreToolUse events use new protocol, not legacy format
        let expected = json!({
            "event_type": "pre_tool_use",
            "tool_name": "bash",
            "tool_input": r#"{"command": "ls"}"#,
        });

        assert_eq!(actual, expected);
        Ok(())
    }

    #[test]
    fn legacy_notify_json_for_post_tool_use_returns_event_serialized_directly() -> Result<()> {
        use super::super::types::HookEventPostToolUse;

        let hook_event = HookEvent::PostToolUse {
            event: HookEventPostToolUse {
                tool_name: "bash".to_string(),
                tool_output: "file1.txt\nfile2.txt".to_string(),
            },
        };

        let serialized = legacy_notify_json(&hook_event, Path::new("/tmp"))?;
        let actual: Value = serde_json::from_str(&serialized)?;

        let expected = json!({
            "event_type": "post_tool_use",
            "tool_name": "bash",
            "tool_output": "file1.txt\nfile2.txt",
        });

        assert_eq!(actual, expected);
        Ok(())
    }

    #[test]
    fn legacy_notify_json_for_stop_returns_event_serialized_directly() -> Result<()> {
        use super::super::types::HookEventStop;

        let hook_event = HookEvent::Stop {
            event: HookEventStop {
                reason: "max_tokens".to_string(),
            },
        };

        let serialized = legacy_notify_json(&hook_event, Path::new("/tmp"))?;
        let actual: Value = serde_json::from_str(&serialized)?;

        let expected = json!({
            "event_type": "stop",
            "reason": "max_tokens",
        });

        assert_eq!(actual, expected);
        Ok(())
    }
}
