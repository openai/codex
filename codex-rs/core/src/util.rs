use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use codex_protocol::ThreadId;
use tracing::error;

use crate::auth_env_telemetry::AuthEnvTelemetry;
use crate::parse_command::shlex_join;

const INITIAL_DELAY_MS: u64 = 200;
const BACKOFF_FACTOR: f64 = 2.0;

#[cfg(not(target_arch = "wasm32"))]
fn random_f64_in_range(start: f64, end: f64) -> f64 {
    use rand::Rng;

    rand::rng().random_range(start..end)
}

#[cfg(target_arch = "wasm32")]
fn random_f64_in_range(start: f64, end: f64) -> f64 {
    start + (end - start) * js_sys::Math::random()
}

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

struct FeedbackRequestSnapshot<'a> {
    endpoint: &'a str,
    auth_header_attached: bool,
    auth_header_name: &'a str,
    auth_mode: &'a str,
    auth_retry_after_unauthorized: String,
    auth_recovery_mode: &'a str,
    auth_recovery_phase: &'a str,
    auth_connection_reused: String,
    auth_request_id: &'a str,
    auth_cf_ray: &'a str,
    auth_error: &'a str,
    auth_error_code: &'a str,
    auth_recovery_followup_success: String,
    auth_recovery_followup_status: String,
}

struct Auth401FeedbackSnapshot<'a> {
    request_id: &'a str,
    cf_ray: &'a str,
    error: &'a str,
    error_code: &'a str,
}

impl<'a> Auth401FeedbackSnapshot<'a> {
    fn from_optional_fields(
        request_id: Option<&'a str>,
        cf_ray: Option<&'a str>,
        error: Option<&'a str>,
        error_code: Option<&'a str>,
    ) -> Self {
        Self {
            request_id: request_id.unwrap_or(""),
            cf_ray: cf_ray.unwrap_or(""),
            error: error.unwrap_or(""),
            error_code: error_code.unwrap_or(""),
        }
    }
}

impl<'a> FeedbackRequestSnapshot<'a> {
    fn from_tags(tags: &'a FeedbackRequestTags<'a>) -> Self {
        Self {
            endpoint: tags.endpoint,
            auth_header_attached: tags.auth_header_attached,
            auth_header_name: tags.auth_header_name.unwrap_or(""),
            auth_mode: tags.auth_mode.unwrap_or(""),
            auth_retry_after_unauthorized: tags
                .auth_retry_after_unauthorized
                .map_or_else(String::new, |value| value.to_string()),
            auth_recovery_mode: tags.auth_recovery_mode.unwrap_or(""),
            auth_recovery_phase: tags.auth_recovery_phase.unwrap_or(""),
            auth_connection_reused: tags
                .auth_connection_reused
                .map_or_else(String::new, |value| value.to_string()),
            auth_request_id: tags.auth_request_id.unwrap_or(""),
            auth_cf_ray: tags.auth_cf_ray.unwrap_or(""),
            auth_error: tags.auth_error.unwrap_or(""),
            auth_error_code: tags.auth_error_code.unwrap_or(""),
            auth_recovery_followup_success: tags
                .auth_recovery_followup_success
                .map_or_else(String::new, |value| value.to_string()),
            auth_recovery_followup_status: tags
                .auth_recovery_followup_status
                .map_or_else(String::new, |value| value.to_string()),
        }
    }
}

#[cfg(test)]
pub(crate) fn emit_feedback_request_tags(tags: &FeedbackRequestTags<'_>) {
    let snapshot = FeedbackRequestSnapshot::from_tags(tags);
    feedback_tags!(
        endpoint = snapshot.endpoint,
        auth_header_attached = snapshot.auth_header_attached,
        auth_header_name = snapshot.auth_header_name,
        auth_mode = snapshot.auth_mode,
        auth_retry_after_unauthorized = snapshot.auth_retry_after_unauthorized,
        auth_recovery_mode = snapshot.auth_recovery_mode,
        auth_recovery_phase = snapshot.auth_recovery_phase,
        auth_connection_reused = snapshot.auth_connection_reused,
        auth_request_id = snapshot.auth_request_id,
        auth_cf_ray = snapshot.auth_cf_ray,
        auth_error = snapshot.auth_error,
        auth_error_code = snapshot.auth_error_code,
        auth_recovery_followup_success = snapshot.auth_recovery_followup_success,
        auth_recovery_followup_status = snapshot.auth_recovery_followup_status
    );
}

pub(crate) fn emit_feedback_request_tags_with_auth_env(
    tags: &FeedbackRequestTags<'_>,
    auth_env: &AuthEnvTelemetry,
) {
    let snapshot = FeedbackRequestSnapshot::from_tags(tags);
    feedback_tags!(
        endpoint = snapshot.endpoint,
        auth_header_attached = snapshot.auth_header_attached,
        auth_header_name = snapshot.auth_header_name,
        auth_mode = snapshot.auth_mode,
        auth_retry_after_unauthorized = snapshot.auth_retry_after_unauthorized,
        auth_recovery_mode = snapshot.auth_recovery_mode,
        auth_recovery_phase = snapshot.auth_recovery_phase,
        auth_connection_reused = snapshot.auth_connection_reused,
        auth_request_id = snapshot.auth_request_id,
        auth_cf_ray = snapshot.auth_cf_ray,
        auth_error = snapshot.auth_error,
        auth_error_code = snapshot.auth_error_code,
        auth_recovery_followup_success = snapshot.auth_recovery_followup_success,
        auth_recovery_followup_status = snapshot.auth_recovery_followup_status,
        auth_env_openai_api_key_present = auth_env.openai_api_key_env_present,
        auth_env_codex_api_key_present = auth_env.codex_api_key_env_present,
        auth_env_codex_api_key_enabled = auth_env.codex_api_key_env_enabled,
        auth_env_provider_key_name = auth_env.provider_env_key_name.as_deref().unwrap_or(""),
        auth_env_provider_key_present = auth_env
            .provider_env_key_present
            .map_or_else(String::new, |value| value.to_string()),
        auth_env_refresh_token_url_override_present = auth_env.refresh_token_url_override_present
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
    let auth_401 = Auth401FeedbackSnapshot::from_optional_fields(
        auth_request_id,
        auth_cf_ray,
        auth_error,
        auth_error_code,
    );
    feedback_tags!(
        auth_recovery_mode = auth_recovery_mode,
        auth_recovery_phase = auth_recovery_phase,
        auth_recovery_outcome = auth_recovery_outcome,
        auth_401_request_id = auth_401.request_id,
        auth_401_cf_ray = auth_401.cf_ray,
        auth_401_error = auth_401.error,
        auth_401_error_code = auth_401.error_code
    );
}

pub fn backoff(attempt: u64) -> Duration {
    let exp = BACKOFF_FACTOR.powi(attempt.saturating_sub(1) as i32);
    let base = (INITIAL_DELAY_MS as f64 * exp) as u64;
    let jitter = random_f64_in_range(0.9, 1.1);
    Duration::from_millis((base as f64 * jitter) as u64)
}

pub(crate) fn error_or_panic(message: impl std::string::ToString) {
    if cfg!(debug_assertions) {
        panic!("{}", message.to_string());
    } else {
        error!("{}", message.to_string());
    }
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
