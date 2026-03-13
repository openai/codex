use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use codex_protocol::ThreadId;
use rand::Rng;
use tracing::debug;
use tracing::error;

use crate::parse_command::shlex_join;

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

pub(crate) struct FeedbackRequestTags<'a> {
    pub endpoint: &'a str,
    pub auth_header_attached: bool,
    pub auth_header_name: Option<&'a str>,
    pub auth_mode: Option<&'a str>,
    pub auth_retry_after_unauthorized: Option<bool>,
    pub auth_recovery_mode: Option<&'a str>,
    pub auth_recovery_phase: Option<&'a str>,
    pub auth_connection_reused: Option<bool>,
    pub auth_request_id: Option<&'a str>,
    pub auth_cf_ray: Option<&'a str>,
    pub auth_error: Option<&'a str>,
    pub auth_error_code: Option<&'a str>,
    pub auth_recovery_followup_success: Option<bool>,
    pub auth_recovery_followup_status: Option<u16>,
}

pub(crate) fn emit_feedback_request_tags(tags: &FeedbackRequestTags<'_>) {
    let preserve_401_context_only = tags.auth_retry_after_unauthorized == Some(true);
    let auth_header_name = tags.auth_header_name.unwrap_or("");
    let auth_mode = tags.auth_mode.unwrap_or("");
    let auth_retry_after_unauthorized = tags
        .auth_retry_after_unauthorized
        .map_or_else(String::new, |value| value.to_string());
    let auth_recovery_mode = tags.auth_recovery_mode.unwrap_or("");
    let auth_recovery_phase = tags.auth_recovery_phase.unwrap_or("");
    let auth_connection_reused = tags
        .auth_connection_reused
        .map_or_else(String::new, |value| value.to_string());
    let auth_request_id = (!preserve_401_context_only)
        .then_some(tags.auth_request_id.unwrap_or(""))
        .unwrap_or("");
    let auth_cf_ray = (!preserve_401_context_only)
        .then_some(tags.auth_cf_ray.unwrap_or(""))
        .unwrap_or("");
    let auth_error = (!preserve_401_context_only)
        .then_some(tags.auth_error.unwrap_or(""))
        .unwrap_or("");
    let auth_error_code = (!preserve_401_context_only)
        .then_some(tags.auth_error_code.unwrap_or(""))
        .unwrap_or("");
    let auth_recovery_followup_success = tags
        .auth_recovery_followup_success
        .map_or_else(String::new, |value| value.to_string());
    let auth_recovery_followup_status = tags
        .auth_recovery_followup_status
        .map_or_else(String::new, |value| value.to_string());
    feedback_tags!(
        endpoint = tags.endpoint,
        auth_header_attached = tags.auth_header_attached,
        auth_header_name = auth_header_name,
        auth_mode = auth_mode,
        auth_retry_after_unauthorized = auth_retry_after_unauthorized,
        auth_recovery_mode = auth_recovery_mode,
        auth_recovery_phase = auth_recovery_phase,
        auth_connection_reused = auth_connection_reused,
        auth_request_id = auth_request_id,
        auth_cf_ray = auth_cf_ray,
        auth_error = auth_error,
        auth_error_code = auth_error_code,
        auth_recovery_followup_success = auth_recovery_followup_success,
        auth_recovery_followup_status = auth_recovery_followup_status
    );
}

pub(crate) fn emit_feedback_auth_recovery_tags(
    auth_recovery_mode: &str,
    auth_recovery_phase: &str,
    auth_recovery_outcome: &str,
    auth_request_id: Option<&str>,
    auth_cf_ray: Option<&str>,
    auth_error: Option<&str>,
    auth_error_code: Option<&str>,
) {
    feedback_tags!(
        auth_recovery_mode = auth_recovery_mode,
        auth_recovery_phase = auth_recovery_phase,
        auth_recovery_outcome = auth_recovery_outcome
    );
    if let Some(auth_request_id) = auth_request_id {
        feedback_tags!(auth_401_request_id = auth_request_id);
    }
    if let Some(auth_cf_ray) = auth_cf_ray {
        feedback_tags!(auth_401_cf_ray = auth_cf_ray);
    }
    if let Some(auth_error) = auth_error {
        feedback_tags!(auth_401_error = auth_error);
    }
    if let Some(auth_error_code) = auth_error_code {
        feedback_tags!(auth_401_error_code = auth_error_code);
    }
}

pub fn backoff(attempt: u64) -> Duration {
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

/// Trim a thread name and return `None` if it is empty after trimming.
pub fn normalize_thread_name(name: &str) -> Option<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn resume_command(thread_name: Option<&str>, thread_id: Option<ThreadId>) -> Option<String> {
    let resume_target = thread_name
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .or_else(|| thread_id.map(|thread_id| thread_id.to_string()));
    resume_target.map(|target| {
        let needs_double_dash = target.starts_with('-');
        let escaped = shlex_join(&[target]);
        if needs_double_dash {
            format!("codex resume -- {escaped}")
        } else {
            format!("codex resume {escaped}")
        }
    })
}

#[cfg(test)]
#[path = "util_tests.rs"]
mod tests;
