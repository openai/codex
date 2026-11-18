//! Regression coverage for retry advice carried by failed Responses events.

use super::process_responses_event;
use super::spawn_response_stream;
use crate::api_bridge::map_api_error;
use crate::common::ResponseEvent;
use bytes::Bytes;
use codex_client::StreamResponse;
use codex_http_client::RetryAfter;
use codex_protocol::error::SpendLimitScope;
use codex_protocol::protocol::CodexErrorInfo;
use futures::StreamExt;
use futures::stream;
use http::HeaderMap;
use http::StatusCode;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::time::Duration;

#[test]
fn audio_validation_codes_map_to_non_retryable_audio_input_errors_on_all_transports() {
    for code in [
        "audio_too_large",
        "audio_too_short",
        "invalid_audio",
        "invalid_audio_data",
        "invalid_audio_format",
        "missing_audio_sample_rate",
    ] {
        let detail = json!({"code": code, "message": "Invalid recording."});
        let http_error = map_api_error(crate::error::ApiError::Transport(
            crate::TransportError::Http {
                status: StatusCode::BAD_REQUEST,
                url: None,
                headers: None,
                body: Some(json!({"error": detail}).to_string()),
                retry_after: None,
            },
        ));
        let mut errors = vec![http_error];
        for failure in [
            json!({"type": "error", "error": detail}),
            json!({"type": "response.failed", "response": {"error": detail}}),
        ] {
            let event = serde_json::from_value(failure).unwrap();
            errors.push(map_api_error(
                process_responses_event(event).unwrap_err().into_api_error(),
            ));
        }
        for error in errors {
            assert_eq!(error.retry_delay(/*retry_count*/ 1), None);
            assert!(matches!(
                error.details(),
                codex_protocol::error::CodexErrorDetails::InvalidAudioInput { .. }
            ));
            assert_eq!(error.to_string(), "Invalid recording.");
        }
    }
}

/// Only the explicit policy capacity code changes overload attribution.
#[tokio::test(start_paused = true)]
async fn policy_capacity_classification_preserves_overload_retry_behavior() {
    for (code, error_type, expected) in [
        (
            "policy_capacity_unavailable",
            None,
            CodexErrorInfo::PolicyCapacityUnavailable,
        ),
        (
            "server_is_overloaded",
            Some("policy_error"),
            CodexErrorInfo::ServerOverloaded,
        ),
        (
            "server_is_overloaded",
            None,
            CodexErrorInfo::ServerOverloaded,
        ),
    ] {
        for retry_after in [Some("3"), None] {
            let event = serde_json::from_value(json!({
                "type": "response.failed",
                "response": {"error": {
                    "type": error_type,
                    "code": code,
                    "headers": {"Retry-After": retry_after},
                }},
            }))
            .unwrap();
            let error = map_api_error(process_responses_event(event).unwrap_err().into_api_error());
            assert_eq!(
                (
                    error.to_codex_protocol_error(),
                    error.retry_delay(/*retry_count*/ 1),
                    error.to_string()
                ),
                (
                    expected.clone(),
                    retry_after.map(|_| Duration::from_secs(3)),
                    if expected == CodexErrorInfo::PolicyCapacityUnavailable {
                        "Policy-restricted capacity is unavailable. Please try again later."
                    } else {
                        "Selected model is at capacity. Please try a different model."
                    }
                    .to_string()
                )
            );
        }
    }
}

/// Terminal provider failures ignore retry headers and stop before a later completed event.
#[tokio::test(start_paused = true)]
async fn terminal_failure_with_retry_header_ends_stream_immediately() {
    for (code, expected) in [
        ("flex_unavailable", CodexErrorInfo::FlexUnavailable),
        ("budget_reached", CodexErrorInfo::SessionBudgetExceeded),
        (
            "api_key_spend_limit_exceeded",
            CodexErrorInfo::ApiKeySpendLimitExceeded,
        ),
        (
            "project_spend_limit_exceeded",
            CodexErrorInfo::ProjectSpendLimitExceeded,
        ),
        (
            "organization_spend_limit_exceeded",
            CodexErrorInfo::OrganizationSpendLimitExceeded,
        ),
        ("inference_hook_denied", CodexErrorInfo::InferenceHookDenied),
        (
            "context_length_exceeded",
            CodexErrorInfo::ContextWindowExceeded,
        ),
        ("invalid_access_program", CodexErrorInfo::Other),
        ("unsupported_access_program", CodexErrorInfo::Other),
        ("content_policy_violation", CodexErrorInfo::InvalidPrompt),
        (
            "access_program_not_enabled",
            CodexErrorInfo::HttpConnectionFailed {
                http_status_code: None,
            },
        ),
    ] {
        let error = json!({
            "code": code,
            "plan_type": 42,
            "headers": {"retry-after": "300"}
        });
        for failure in [
            json!({"type": "error", "error": error}),
            json!({"type": "response.failed", "response": {"error": error}}),
        ] {
            let completed = json!({"type": "response.completed", "response": {"id": "later"}});
            let bytes = stream::iter([Ok(Bytes::from(format!(
                "data: {failure}\n\ndata: {completed}\n\n"
            )))])
            .chain(stream::pending());
            let mut stream = spawn_response_stream(
                StreamResponse {
                    status: StatusCode::OK,
                    headers: HeaderMap::new(),
                    bytes: Box::pin(bytes),
                },
                Duration::from_secs(60),
                /*telemetry*/ None,
                /*turn_state*/ None,
            );
            let error = loop {
                match stream.next().await {
                    Some(Ok(ResponseEvent::RateLimits(_))) => {}
                    Some(Err(error)) => break map_api_error(error),
                    event => panic!("expected a terminal error for {code}, got {event:?}"),
                }
            };
            assert_eq!(
                (
                    error.to_codex_protocol_error(),
                    error.retry_delay(/*retry_count*/ 1)
                ),
                (expected.clone(), None)
            );
            assert!(stream.next().await.is_none());
        }
    }
}

#[test]
fn statusless_model_errors_have_no_http_status() {
    let error = json!({"code": "model_not_found"});
    for event in [
        json!({"type": "error", "error": error}),
        json!({"type": "response.failed", "response": {"error": error}}),
    ] {
        let event = serde_json::from_value(event).unwrap();
        let error = map_api_error(process_responses_event(event).unwrap_err().into_api_error());
        assert_eq!(
            (
                error.to_codex_protocol_error(),
                error.http_status_code_value()
            ),
            (CodexErrorInfo::ModelNotFound, None)
        );
    }
}

/// Error headers beat rate-limit message advice; invalid ones fall back.
#[tokio::test(start_paused = true)]
async fn rate_limit_plaintext_loses_to_valid_error_header() {
    for code in ["rate_limit_exceeded", "slow_down"] {
        for (error_header, expected_seconds) in [
            (Some("2"), 2),
            (Some("0"), 0),
            (Some("Wed, 21 Oct 2015 07:28:00 GMT"), 0),
            (Some("bad"), 35),
            (None, 35),
        ] {
            let event = serde_json::from_value(json!({
                "type": "response.failed",
                "response": {
                    "error": {
                        "code": code,
                        "message": "Try again in 35 seconds.",
                        "headers": {"Retry-After": error_header}
                    }
                }
            }))
            .unwrap();
            let error = map_api_error(process_responses_event(event).unwrap_err().into_api_error());
            assert_eq!(
                error.retry_after(),
                RetryAfter::from_delay(Duration::from_secs(expected_seconds))
            );
        }
    }
}

/// Retry advice uses the same HTTP value validation as other event headers.
#[tokio::test(start_paused = true)]
async fn streamed_retry_after_uses_http_header_validation() {
    for (error_headers, expected_seconds) in [
        (json!({"retry-after": "\n5\n"}), 12),
        (json!({"retry-after": "\t5\t"}), 5),
        (json!({"Retry-After": "5", "retry-after": "30"}), 30),
    ] {
        let event = serde_json::from_value(json!({
            "type": "response.failed",
            "response": {"error": {"code": "rate_limit_exceeded", "message": "Try again in 12 seconds.", "headers": error_headers}}
        }))
        .unwrap();
        let error = map_api_error(process_responses_event(event).unwrap_err().into_api_error());
        assert_eq!(
            error.retry_after(),
            RetryAfter::from_delay(Duration::from_secs(expected_seconds))
        );
    }
}

/// Event headers retain their deadlines for overload and generic failures.
#[tokio::test(start_paused = true)]
async fn retry_header_survives_other_failed_responses() {
    for (code, expected_code) in [
        ("server_is_overloaded", CodexErrorInfo::ServerOverloaded),
        ("safety_violation", CodexErrorInfo::Other),
        ("unknown_failure", CodexErrorInfo::Other),
    ] {
        let event = serde_json::from_value(json!({
            "type": "response.failed",
            "response": {"error": {"code": code, "headers": {"retry-after": "7"}}},
        }))
        .unwrap();
        let error = map_api_error(process_responses_event(event).unwrap_err().into_api_error());
        assert_eq!(
            (
                error.to_codex_protocol_error(),
                error.retry_after(),
                error.retry_delay(/*retry_count*/ 1),
            ),
            (
                expected_code,
                RetryAfter::from_delay(Duration::from_secs(7)),
                Some(Duration::from_secs(7)),
            )
        );
    }
}

/// An event header cannot make a quota or policy failure retryable.
#[tokio::test(start_paused = true)]
async fn retry_header_does_not_override_terminal_errors() {
    for code in ["insufficient_quota", "cyber_policy", "bio_policy"] {
        let event = serde_json::from_value(json!({
            "type": "response.failed",
            "response": {"error": {"code": code, "headers": {"retry-after": "1"}}}
        }))
        .unwrap();
        let error = map_api_error(process_responses_event(event).unwrap_err().into_api_error());
        assert_eq!(error.retry_delay(/*retry_count*/ 1), None, "{code}");
    }
}

/// Exercise the SSE delivery path while the provider leaves its connection open.
async fn keep_open_error(event: serde_json::Value) -> crate::ApiError {
    let bytes =
        stream::iter([Ok(Bytes::from(format!("data: {event}\n\n")))]).chain(stream::pending());
    let mut response = spawn_response_stream(
        StreamResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            bytes: Box::pin(bytes),
        },
        Duration::from_secs(60),
        /*telemetry*/ None,
        /*turn_state*/ None,
    );
    let started = tokio::time::Instant::now();
    let error = loop {
        match response.next().await {
            Some(Ok(ResponseEvent::RateLimits(_))) => {}
            Some(Err(error)) => break error,
            other => panic!("expected an error, got {other:?}"),
        }
    };
    assert_eq!(started.elapsed(), Duration::ZERO, "{event}");
    assert!(response.next().await.is_none());
    error
}

fn error_forms(detail: serde_json::Value) -> [serde_json::Value; 2] {
    [
        json!({"type": "error", "error": detail}),
        json!({"type": "response.failed", "response": {"error": detail}}),
    ]
}

#[tokio::test(start_paused = true)]
async fn terminal_codes_exit_keep_open_sse_in_both_event_forms() {
    use crate::ApiError;
    for code in [
        "audio_too_large",
        "audio_too_short",
        "invalid_audio",
        "invalid_audio_data",
        "invalid_audio_format",
        "missing_audio_sample_rate",
        "model_not_found",
        "insufficient_quota",
        "credit_balance_exhausted",
        "api_key_spend_limit_exceeded",
        "organization_spend_limit_exceeded",
        "project_spend_limit_exceeded",
        "usage_not_included",
        "invalid_prompt",
        "misalignment_policy_violation",
    ] {
        for event in error_forms(
            json!({"code": code, "message": "Rejected.", "headers": {"Retry-After": "7"}}),
        ) {
            let error = keep_open_error(event).await;
            match (code, &error) {
                (
                    "audio_too_large",
                    ApiError::InvalidAudioInput {
                        per_recording: false,
                        ..
                    },
                )
                | (
                    "audio_too_short"
                    | "invalid_audio"
                    | "invalid_audio_data"
                    | "invalid_audio_format"
                    | "missing_audio_sample_rate",
                    ApiError::InvalidAudioInput {
                        per_recording: true,
                        ..
                    },
                )
                | ("model_not_found", ApiError::ModelNotFound { status: None, .. })
                | ("insufficient_quota" | "credit_balance_exhausted", ApiError::QuotaExceeded)
                | (
                    "api_key_spend_limit_exceeded",
                    ApiError::SpendLimitExceeded(SpendLimitScope::ApiKey),
                )
                | (
                    "project_spend_limit_exceeded",
                    ApiError::SpendLimitExceeded(SpendLimitScope::Project),
                )
                | (
                    "organization_spend_limit_exceeded",
                    ApiError::SpendLimitExceeded(SpendLimitScope::Organization),
                )
                | ("usage_not_included", ApiError::UsageNotIncluded)
                | ("invalid_prompt", ApiError::InvalidPrompt { .. })
                | ("misalignment_policy_violation", ApiError::MisalignmentPolicyViolation { .. }) =>
                    {}
                _ => panic!("wrong error for {code}: {error:?}"),
            }
            let error = map_api_error(error);
            assert_eq!(error.retry_delay(/*retry_count*/ 1), None, "{code}");
            assert!(!error.requires_http_fallback(), "{code}");
        }
    }
}

#[tokio::test(start_paused = true)]
async fn overload_advice_survives_keep_open_sse_in_both_event_forms() {
    for (code, expected) in [
        ("server_is_overloaded", CodexErrorInfo::ServerOverloaded),
        (
            "policy_capacity_unavailable",
            CodexErrorInfo::PolicyCapacityUnavailable,
        ),
    ] {
        for (header, delay) in [
            (Some("7"), Some(Duration::from_secs(7))),
            (Some("bad"), None),
            (None, None),
        ] {
            let mut detail = json!({"code": code});
            if let Some(value) = header {
                detail["headers"] = json!({"Retry-After": value});
            }
            for event in error_forms(detail) {
                let error = map_api_error(keep_open_error(event).await);
                assert_eq!(error.to_codex_protocol_error(), expected);
                assert_eq!(error.retry_delay(/*retry_count*/ 1), delay, "{code}");
            }
        }
        let event = json!({"type": "error", "headers": {"Retry-After": "9"}, "error": {"code": code, "headers": {"Retry-After": "3"}}});
        assert_eq!(
            map_api_error(keep_open_error(event).await).retry_delay(/*retry_count*/ 1),
            Some(Duration::from_secs(3))
        );
        let event =
            json!({"type": "error", "headers": {"Retry-After": "9"}, "error": {"code": code}});
        assert_eq!(
            map_api_error(keep_open_error(event).await).retry_delay(/*retry_count*/ 1),
            Some(Duration::from_secs(9))
        );
    }
}

#[tokio::test(start_paused = true)]
async fn retryable_failed_response_preserves_advice_before_sse_eof() {
    let event = json!({"type": "response.failed", "response": {"error": {"code": "rate_limit_exceeded", "message": "Try again in 35 seconds.", "headers": {"Retry-After": "2"}}}});
    let error = map_api_error(keep_open_error(event).await);
    assert_eq!(
        error.retry_delay(/*retry_count*/ 1),
        Some(Duration::from_secs(2))
    );
}
