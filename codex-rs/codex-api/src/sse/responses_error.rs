//! Classifies failed Responses events and their rate-limit retry delays.
//! Only rate-limit codes derive retry delays from plaintext messages.

use crate::error::ApiError;
use crate::error::parse_flex_unavailable;
use codex_http_client::RetryAfter;
use codex_protocol::protocol::MisalignmentErrorDetails;
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;

// Keep decoding unused fields: malformed values currently produce a stream error.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Error {
    r#type: Option<String>,
    code: Option<String>,
    message: Option<String>,
    plan_type: Option<String>,
    resets_at: Option<i64>,
    #[serde(default)]
    misalignment: Option<Value>,
}

pub(super) fn parse_failed_response(response: Option<Value>) -> ApiError {
    if let Some(error) = response
        .as_ref()
        .and_then(|response| response.get("error"))
        .and_then(parse_flex_unavailable)
    {
        return error;
    }
    let Some(error) = response
        .as_ref()
        .and_then(|response| response.get("error"))
        .and_then(|error| serde_json::from_value::<Error>(error.clone()).ok())
    else {
        return ApiError::Stream("response.failed event received".into());
    };

    match error.code.as_deref() {
        Some("context_length_exceeded") => ApiError::ContextWindowExceeded,
        Some(
            "insufficient_quota"
            | "credit_balance_exhausted"
            | "organization_spend_limit_exceeded"
            | "project_spend_limit_exceeded",
        ) => ApiError::QuotaExceeded,
        Some("usage_not_included") => ApiError::UsageNotIncluded,
        Some("cyber_policy") => ApiError::CyberPolicy {
            message: cyber_policy_message(error.message),
        },
        Some("bio_policy") => {
            let message = error
                .message
                .filter(|message| !message.trim().is_empty())
                .unwrap_or_else(|| {
                    "This content was flagged for possible biological risk.".to_string()
                });
            ApiError::BioPolicy { message }
        }
        Some("misalignment_policy_violation") => {
            let message = error
                .message
                .filter(|message| !message.trim().is_empty())
                .unwrap_or_else(|| {
                    "This request was blocked due to a misalignment policy violation.".to_string()
                });
            ApiError::MisalignmentPolicyViolation {
                message,
                misalignment: error.misalignment.and_then(|details| {
                    serde_json::from_value::<MisalignmentErrorDetails>(details).ok()
                }),
            }
        }
        Some("invalid_prompt") => ApiError::InvalidPrompt {
            message: error
                .message
                .unwrap_or_else(|| "Invalid request.".to_string()),
        },
        Some("server_is_overloaded") => ApiError::ServerOverloaded { retry_after: None },
        Some("rate_limit_exceeded" | "slow_down") => {
            let retry_after = try_parse_retry_delay(&error).and_then(RetryAfter::from_delay);
            ApiError::RateLimitExceeded {
                message: error.message.unwrap_or_default(),
                retry_after,
            }
        }
        _ => ApiError::Retryable {
            message: error.message.unwrap_or_default(),
            retry_after: None,
        },
    }
}

fn try_parse_retry_delay(err: &Error) -> Option<Duration> {
    let re = rate_limit_regex();
    if let Some(message) = &err.message
        && let Some(captures) = re.captures(message)
    {
        let seconds = captures.get(1);
        let unit = captures.get(2);

        if let (Some(value), Some(unit)) = (seconds, unit) {
            let value = value.as_str().parse::<f64>().ok()?;
            let unit = unit.as_str().to_ascii_lowercase();

            if unit == "s" || unit.starts_with("second") {
                return Some(Duration::from_secs_f64(value));
            } else if unit == "ms" {
                return Some(Duration::from_millis(value as u64));
            }
        }
    }
    None
}

fn cyber_policy_fallback_message() -> String {
    "This request has been flagged for possible cybersecurity risk.".to_string()
}

fn cyber_policy_message(message: Option<String>) -> String {
    message
        .filter(|message| !message.trim().is_empty())
        .unwrap_or_else(cyber_policy_fallback_message)
}

fn rate_limit_regex() -> &'static regex_lite::Regex {
    static RE: std::sync::OnceLock<regex_lite::Regex> = std::sync::OnceLock::new();
    #[expect(clippy::unwrap_used)]
    RE.get_or_init(|| {
        regex_lite::Regex::new(r"(?i)try again in\s*(\d+(?:\.\d+)?)\s*(s|ms|seconds?)").unwrap()
    })
}

#[cfg(test)]
#[path = "responses_error_parser_tests.rs"]
mod tests;
