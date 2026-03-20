use crate::auth_env_telemetry::AuthEnvTelemetry;

pub(crate) struct FeedbackRequestTags<'a> {
    pub(crate) endpoint: &'a str,
    pub(crate) auth_header_attached: bool,
    pub(crate) auth_header_name: Option<&'a str>,
    pub(crate) auth_mode: Option<&'a str>,
    pub(crate) auth_retry_after_unauthorized: Option<bool>,
    pub(crate) auth_recovery_mode: Option<&'a str>,
    pub(crate) auth_recovery_phase: Option<&'a str>,
    pub(crate) auth_connection_reused: Option<bool>,
    pub(crate) auth_request_id: Option<&'a str>,
    pub(crate) auth_cf_ray: Option<&'a str>,
    pub(crate) auth_error: Option<&'a str>,
    pub(crate) auth_error_code: Option<&'a str>,
    pub(crate) auth_recovery_followup_success: Option<bool>,
    pub(crate) auth_recovery_followup_status: Option<u16>,
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

pub(crate) fn emit_feedback_request_tags_with_auth_env(
    tags: &FeedbackRequestTags<'_>,
    auth_env: &AuthEnvTelemetry,
) {
    let snapshot = FeedbackRequestSnapshot::from_tags(tags);
    tracing::info!(
        target: "feedback_tags",
        endpoint = tracing::field::debug(snapshot.endpoint),
        auth_header_attached = tracing::field::debug(snapshot.auth_header_attached),
        auth_header_name = tracing::field::debug(snapshot.auth_header_name),
        auth_mode = tracing::field::debug(snapshot.auth_mode),
        auth_retry_after_unauthorized =
            tracing::field::debug(snapshot.auth_retry_after_unauthorized),
        auth_recovery_mode = tracing::field::debug(snapshot.auth_recovery_mode),
        auth_recovery_phase = tracing::field::debug(snapshot.auth_recovery_phase),
        auth_connection_reused = tracing::field::debug(snapshot.auth_connection_reused),
        auth_request_id = tracing::field::debug(snapshot.auth_request_id),
        auth_cf_ray = tracing::field::debug(snapshot.auth_cf_ray),
        auth_error = tracing::field::debug(snapshot.auth_error),
        auth_error_code = tracing::field::debug(snapshot.auth_error_code),
        auth_recovery_followup_success =
            tracing::field::debug(snapshot.auth_recovery_followup_success),
        auth_recovery_followup_status =
            tracing::field::debug(snapshot.auth_recovery_followup_status),
        auth_env_openai_api_key_present =
            tracing::field::debug(auth_env.openai_api_key_env_present),
        auth_env_codex_api_key_present =
            tracing::field::debug(auth_env.codex_api_key_env_present),
        auth_env_codex_api_key_enabled =
            tracing::field::debug(auth_env.codex_api_key_env_enabled),
        auth_env_provider_key_name =
            tracing::field::debug(auth_env.provider_env_key_name.as_deref().unwrap_or("")),
        auth_env_provider_key_present = tracing::field::debug(
            &auth_env
                .provider_env_key_present
                .map_or_else(String::new, |value| value.to_string())
        ),
        auth_env_refresh_token_url_override_present =
            tracing::field::debug(auth_env.refresh_token_url_override_present),
    );
}
