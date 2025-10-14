use std::borrow::Cow;
use std::process::Stdio;

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

        let (request, response, session_id) = extract_request_response(notification);

        if let Some(mut command) = build_command(notify_command, &request, &response, &session_id) {
            apply_alarm_environment(&request, &response, &session_id, &mut command);
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
    session_id: &str,
) -> Option<std::process::Command> {
    let first = notify_command.first()?;
    let mut command = std::process::Command::new(replace_placeholders(
        first,
        request,
        response,
        session_id,
        PlaceholderMode::Direct,
    ));
    // Prevent alarm scripts from writing to the Codex TUI stdout/stderr streams.
    command.stdin(Stdio::null());
    command.stdout(Stdio::null());
    command.stderr(Stdio::null());

    let uses_shell_script = is_shell_script_command(notify_command);

    for (index, arg) in notify_command.iter().enumerate().skip(1) {
        if arg == ALARM_NOTIFY_SENTINEL_ARG {
            continue;
        }

        let mode = if uses_shell_script && index == 2 {
            PlaceholderMode::Env
        } else {
            PlaceholderMode::Direct
        };

        command.arg(replace_placeholders(
            arg, request, response, session_id, mode,
        ));
    }
    Some(command)
}

const ALARM_PLACEHOLDER_MAX_LEN: usize = 512;

fn truncated(text: &str) -> String {
    if text.len() > ALARM_PLACEHOLDER_MAX_LEN {
        let mut boundary = ALARM_PLACEHOLDER_MAX_LEN;
        while !text.is_char_boundary(boundary) {
            boundary -= 1;
        }
        let mut truncated = text[..boundary].to_string();
        truncated.push('…');
        truncated
    } else {
        text.to_string()
    }
}

fn extract_request_response(notification: &UserNotification) -> (String, String, String) {
    match notification {
        UserNotification::AgentTurnComplete {
            input_messages,
            last_assistant_message,
            session_id,
            ..
        } => {
            let request = sanitize_text(
                input_messages
                    .last()
                    .map(String::as_str)
                    .unwrap_or_default(),
            );
            let response = sanitize_text(last_assistant_message.as_deref().unwrap_or_default());
            (request, response, session_id.clone())
        }
    }
}

fn apply_alarm_environment(
    request: &str,
    response: &str,
    session_id: &str,
    command: &mut std::process::Command,
) {
    command.env("request", request);
    command.env("CODEX_ALARM_LAST_REQUEST", request);
    command.env("response", response);
    command.env("CODEX_ALARM_LAST_RESPONSE", response);
    command.env("session_id", session_id);
    command.env("CODEX_ALARM_SESSION_ID", session_id);
}

fn replace_placeholders(
    input: &str,
    request: &str,
    response: &str,
    session_id: &str,
    mode: PlaceholderMode,
) -> String {
    let mut result = String::with_capacity(input.len());
    let mut remainder = input;

    while let Some(dollar_pos) = remainder.find('$') {
        result.push_str(&remainder[..dollar_pos]);
        remainder = &remainder[dollar_pos + 1..];

        if remainder.is_empty() {
            result.push('$');
            break;
        }

        if remainder.starts_with('{') {
            if let Some(close_brace) = remainder[1..].find('}') {
                let name = &remainder[1..close_brace + 1];
                let placeholder = &remainder[..close_brace + 2];
                remainder = &remainder[close_brace + 2..];

                if let Some(value) = placeholder_value(name, request, response, session_id, mode) {
                    result.push_str(value.as_ref());
                } else {
                    result.push('$');
                    result.push_str(placeholder);
                }
                continue;
            }

            result.push('$');
            result.push_str(remainder);
            break;
        }

        let mut name_len = 0;
        for ch in remainder.chars() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                name_len += ch.len_utf8();
            } else {
                break;
            }
        }

        if name_len == 0 {
            result.push('$');
            continue;
        }

        let name = &remainder[..name_len];
        remainder = &remainder[name_len..];
        if let Some(value) = placeholder_value(name, request, response, session_id, mode) {
            result.push_str(value.as_ref());
        } else {
            result.push('$');
            result.push_str(name);
        }
    }

    result.push_str(remainder);
    result
}

fn placeholder_value<'a>(
    name: &str,
    request: &'a str,
    response: &'a str,
    session_id: &'a str,
    mode: PlaceholderMode,
) -> Option<Cow<'a, str>> {
    match name {
        "request" => Some(mode.request_value(request)),
        "response" => Some(mode.response_value(response)),
        "session_id" => Some(mode.session_id_value(session_id)),
        "" => None,
        other => Some(mode.env_value(other)),
    }
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

#[derive(Clone, Copy)]
enum PlaceholderMode {
    Direct,
    Env,
}

impl PlaceholderMode {
    fn request_value<'a>(self, request: &'a str) -> Cow<'a, str> {
        match self {
            Self::Direct => Cow::Borrowed(request),
            Self::Env => Cow::Borrowed("${CODEX_ALARM_LAST_REQUEST}"),
        }
    }

    fn response_value<'a>(self, response: &'a str) -> Cow<'a, str> {
        match self {
            Self::Direct => Cow::Borrowed(response),
            Self::Env => Cow::Borrowed("${CODEX_ALARM_LAST_RESPONSE}"),
        }
    }

    fn session_id_value<'a>(self, session_id: &'a str) -> Cow<'a, str> {
        match self {
            Self::Direct => Cow::Borrowed(session_id),
            Self::Env => Cow::Borrowed("${CODEX_ALARM_SESSION_ID}"),
        }
    }

    fn env_value<'a>(self, name: &str) -> Cow<'a, str> {
        match self {
            Self::Direct => {
                let value = std::env::var_os(name)
                    .map(|raw| raw.to_string_lossy().into_owned())
                    .unwrap_or_default();
                Cow::Owned(value)
            }
            Self::Env => Cow::Owned(format!("${{{name}}}")),
        }
    }
}

fn is_shell_script_command(notify_command: &[String]) -> bool {
    let Some(program) = notify_command.first() else {
        return false;
    };
    let Some(flag) = notify_command.get(1) else {
        return false;
    };

    matches!(program.as_str(), "sh" | "/bin/sh" | "bash" | "/bin/bash")
        && matches!(flag.as_str(), "-c" | "-lc")
        && notify_command.get(2).is_some()
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
        session_id: String,

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
            session_id: "session-1".to_string(),
            input_messages: vec!["Rename `foo` to `bar` and update the callsites.".to_string()],
            last_assistant_message: Some(
                "Rename complete and verified `cargo build` succeeds.".to_string(),
            ),
        };
        let serialized = serde_json::to_string(&notification)?;
        assert_eq!(
            serialized,
            r#"{"type":"agent-turn-complete","turn-id":"12345","session-id":"session-1","input-messages":["Rename `foo` to `bar` and update the callsites."],"last-assistant-message":"Rename complete and verified `cargo build` succeeds."}"#
        );
        Ok(())
    }

    #[test]
    fn test_replace_placeholders_direct_mode_expands_values() {
        let actual = replace_placeholders(
            "notify $request -> $response ($session_id)",
            "req",
            "resp",
            "session",
            PlaceholderMode::Direct,
        );
        assert_eq!(actual, "notify req -> resp (session)");
    }

    #[test]
    fn test_replace_placeholders_env_mode_uses_env_vars() {
        let actual = replace_placeholders(
            "notify $request -> $response ($session_id)",
            "req",
            "resp",
            "session",
            PlaceholderMode::Env,
        );
        assert_eq!(
            actual,
            "notify ${CODEX_ALARM_LAST_REQUEST} -> ${CODEX_ALARM_LAST_RESPONSE} (${CODEX_ALARM_SESSION_ID})"
        );
    }

    #[test]
    fn test_replace_placeholders_direct_mode_expands_environment_variables() {
        const KEY: &str = "CODEX_TEST_ALARM_ENV";
        // SAFETY: tests run single-threaded here and use a unique environment variable key.
        unsafe {
            std::env::set_var(KEY, "value");
        }
        let actual = replace_placeholders(
            "hello $CODEX_TEST_ALARM_ENV and ${CODEX_TEST_ALARM_ENV}",
            "req",
            "resp",
            "session",
            PlaceholderMode::Direct,
        );
        // SAFETY: same rationale as above when cleaning up the environment variable.
        unsafe {
            std::env::remove_var(KEY);
        }
        assert_eq!(actual, "hello value and value");
    }

    #[test]
    fn test_replace_placeholders_env_mode_keeps_environment_references() {
        let actual = replace_placeholders(
            "user is $USER and ${USER}",
            "req",
            "resp",
            "session",
            PlaceholderMode::Env,
        );
        assert_eq!(actual, "user is ${USER} and ${USER}");
    }
}
