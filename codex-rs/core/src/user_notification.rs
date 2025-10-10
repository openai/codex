use serde::Serialize;
use tracing::error;
use tracing::warn;

use crate::config::ALARM_NOTIFY_SENTINEL_ARG;

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

        let (request, response) = extract_request_response(notification);

        if let Some(mut command) = build_command(notify_command, &request, &response) {
            apply_alarm_environment(&request, &response, &mut command);
            command.arg(json);

            // Fire-and-forget – we do not wait for completion.
            if let Err(e) = command.spawn() {
                warn!("failed to spawn notifier '{}': {e}", notify_command[0]);
            }
        }
    }

    pub(crate) fn new(notify: Option<Vec<String>>) -> Self {
        Self {
            notify_command: notify,
        }
    }
}

fn build_command(
    notify_command: &[String],
    request: &str,
    response: &str,
) -> Option<std::process::Command> {
    let first = notify_command.first()?.clone();
    let mut command = std::process::Command::new(replace_placeholders(&first, request, response));
    if notify_command.len() > 1 {
        let mut args: Vec<String> = notify_command[1..].to_vec();
        if args
            .last()
            .map(|arg| arg.as_str() == ALARM_NOTIFY_SENTINEL_ARG)
            .unwrap_or(false)
        {
            args.pop();
        }
        if !args.is_empty() {
            let replaced: Vec<String> = args
                .into_iter()
                .map(|arg| replace_placeholders(&arg, request, response))
                .collect();
            command.args(replaced);
        }
    }
    Some(command)
}

const ALARM_PLACEHOLDER_MAX_LEN: usize = 512;

fn truncated(text: &str) -> String {
    if text.len() > ALARM_PLACEHOLDER_MAX_LEN {
        let mut truncated = text[..ALARM_PLACEHOLDER_MAX_LEN].to_string();
        truncated.push('…');
        truncated
    } else {
        text.to_string()
    }
}

fn extract_request_response(notification: &UserNotification) -> (String, String) {
    match notification {
        UserNotification::AgentTurnComplete {
            input_messages,
            last_assistant_message,
            ..
        } => {
            let request = sanitize_text(
                input_messages
                    .last()
                    .map(String::as_str)
                    .unwrap_or_default(),
            );
            let response = sanitize_text(last_assistant_message.as_deref().unwrap_or_default());
            (request, response)
        }
    }
}

fn apply_alarm_environment(request: &str, response: &str, command: &mut std::process::Command) {
    command.env("request", request);
    command.env("CODEX_ALARM_LAST_REQUEST", request);
    command.env("response", response);
    command.env("CODEX_ALARM_LAST_RESPONSE", response);
}

fn replace_placeholders(input: &str, request: &str, response: &str) -> String {
    input
        .replace("$request", "${CODEX_ALARM_LAST_REQUEST}")
        .replace("$response", "${CODEX_ALARM_LAST_RESPONSE}")
}

fn sanitize_text(text: &str) -> String {
    let mut cleaned = String::with_capacity(text.len());
    let mut last_was_space = false;

    for ch in text.chars() {
        if matches!(ch, '\r' | '\n' | '\t') {
            if !last_was_space {
                cleaned.push(' ');
                last_was_space = true;
            }
        } else if ch.is_whitespace() && !last_was_space {
            cleaned.push(' ');
            last_was_space = true;
        } else if ch.is_whitespace() {
            continue;
        } else {
            cleaned.push(ch);
            last_was_space = false;
        }
    }

    let trimmed = cleaned.trim();
    truncated(trimmed)
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
}
