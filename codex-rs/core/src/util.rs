use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use codex_protocol::ThreadId;
use rand::Rng;
use tracing::debug;
use tracing::error;

const INITIAL_DELAY_MS: u64 = 200;
const BACKOFF_FACTOR: f64 = 2.0;

/// Emit structured feedback metadata as key/value pairs.
///
/// This logs a tracing event with `target: "feedback_tags"`. If
/// `codex_feedback::CodexFeedback::metadata_layer()` is installed, these fields are captured and
/// later attached as tags when feedback is uploaded.
///
/// Values are wrapped with [`tracing::field::DebugValue`], so the expression only needs to
/// implement [`std::fmt::Debug`].
///
/// Example:
///
/// ```rust
/// codex_core::feedback_tags!(model = "gpt-5", cached = true);
/// codex_core::feedback_tags!(provider = provider_id, request_id = request_id);
/// ```
#[macro_export]
macro_rules! feedback_tags {
    ($( $key:ident = $value:expr ),+ $(,)?) => {
        ::tracing::info!(
            target: "feedback_tags",
            $( $key = ::tracing::field::debug(&$value) ),+
        );
    };
}

pub(crate) fn backoff(attempt: u64) -> Duration {
    let exp = BACKOFF_FACTOR.powi(attempt.saturating_sub(1) as i32);
    let base = (INITIAL_DELAY_MS as f64 * exp) as u64;
    let jitter = rand::rng().random_range(0.9..1.1);
    Duration::from_millis((base as f64 * jitter) as u64)
}

pub(crate) fn error_or_panic(message: impl std::string::ToString) {
    if cfg!(debug_assertions) {
        panic!("{}", message.to_string());
    } else {
        error!("{}", message.to_string());
    }
}

pub(crate) fn try_parse_error_message(text: &str) -> String {
    debug!("Parsing server error response: {}", text);
    let json = serde_json::from_str::<serde_json::Value>(text).unwrap_or_default();
    if let Some(error) = json.get("error")
        && let Some(message) = error.get("message")
        && let Some(message_str) = message.as_str()
    {
        return message_str.to_string();
    }
    if text.is_empty() {
        return "Unknown error".to_string();
    }
    text.to_string()
}

pub fn resolve_path(base: &Path, path: &PathBuf) -> PathBuf {
    if path.is_absolute() {
        path.clone()
    } else {
        base.join(path)
    }
}

pub fn resume_command(thread_name: Option<&str>, thread_id: Option<ThreadId>) -> Option<String> {
    let resume_target = thread_name
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .or_else(|| thread_id.map(|thread_id| thread_id.to_string()));
    resume_target.map(|target| {
        if needs_shell_escaping(&target) {
            format!("codex resume -- {}", shell_escape(&target))
        } else {
            format!("codex resume {target}")
        }
    })
}

fn needs_shell_escaping(value: &str) -> bool {
    value.starts_with('-') || value.chars().any(char::is_whitespace) || value.contains('\'')
}

fn shell_escape(value: &str) -> String {
    // Single-quote escape for POSIX shells so resume hints are copy/paste safe.
    if value.is_empty() {
        return "''".to_string();
    }

    let mut out = String::with_capacity(value.len() + 2);
    out.push('\'');
    for ch in value.chars() {
        if ch == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_try_parse_error_message() {
        let text = r#"{
  "error": {
    "message": "Your refresh token has already been used to generate a new access token. Please try signing in again.",
    "type": "invalid_request_error",
    "param": null,
    "code": "refresh_token_reused"
  }
}"#;
        let message = try_parse_error_message(text);
        assert_eq!(
            message,
            "Your refresh token has already been used to generate a new access token. Please try signing in again."
        );
    }

    #[test]
    fn test_try_parse_error_message_no_error() {
        let text = r#"{"message": "test"}"#;
        let message = try_parse_error_message(text);
        assert_eq!(message, r#"{"message": "test"}"#);
    }

    #[test]
    fn feedback_tags_macro_compiles() {
        #[derive(Debug)]
        struct OnlyDebug;

        feedback_tags!(model = "gpt-5", cached = true, debug_only = OnlyDebug);
    }

    #[test]
    fn resume_command_prefers_name_over_id() {
        let thread_id = ThreadId::from_string("123e4567-e89b-12d3-a456-426614174000").unwrap();
        let command = resume_command(Some("my-thread"), Some(thread_id));
        assert_eq!(command, Some("codex resume my-thread".to_string()));
    }

    #[test]
    fn resume_command_with_only_id() {
        let thread_id = ThreadId::from_string("123e4567-e89b-12d3-a456-426614174000").unwrap();
        let command = resume_command(None, Some(thread_id));
        assert_eq!(
            command,
            Some("codex resume 123e4567-e89b-12d3-a456-426614174000".to_string())
        );
    }

    #[test]
    fn resume_command_with_no_name_or_id() {
        let command = resume_command(None, None);
        assert_eq!(command, None);
    }

    #[test]
    fn resume_command_quotes_thread_name_when_needed() {
        let command = resume_command(Some("-starts-with-dash"), None);
        assert_eq!(
            command,
            Some("codex resume -- '-starts-with-dash'".to_string())
        );

        let command = resume_command(Some("two words"), None);
        assert_eq!(command, Some("codex resume -- 'two words'".to_string()));

        let command = resume_command(Some("quote'case"), None);
        assert_eq!(
            command,
            Some("codex resume -- 'quote'\\''case'".to_string())
        );
    }
}
