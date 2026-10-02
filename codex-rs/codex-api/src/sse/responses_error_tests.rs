//! Regression coverage for retry advice carried by failed Responses events.

use super::process_responses_event;
use super::spawn_response_stream;
use crate::api_bridge::map_api_error;
use crate::common::ResponseEvent;
use bytes::Bytes;
use codex_client::StreamResponse;
use codex_http_client::RetryAfter;
use codex_protocol::protocol::CodexErrorInfo;
use futures::StreamExt;
use futures::stream;
use http::HeaderMap;
use http::StatusCode;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::time::Duration;

/// Flex failures remain terminal even with retry headers and a later completed event.
#[tokio::test(start_paused = true)]
async fn flex_failure_with_retry_header_ends_stream_immediately() {
    let error = json!({
        "code": "flex_unavailable",
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
                event => panic!("expected a Flex error, got {event:?}"),
            }
        };
        assert_eq!(
            (
                error.to_codex_protocol_error(),
                error.retry_delay(/*retry_count*/ 1)
            ),
            (CodexErrorInfo::FlexUnavailable, None)
        );
        assert!(stream.next().await.is_none());
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
