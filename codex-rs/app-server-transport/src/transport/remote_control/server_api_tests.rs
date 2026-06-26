use super::*;
use crate::transport::remote_control::protocol::normalize_remote_control_url;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::time::SystemTime;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

const TEST_REQUEST_TIMEOUT: Duration = Duration::from_millis(100);

fn auth() -> RemoteControlConnectionAuth {
    RemoteControlConnectionAuth {
        auth_provider: codex_model_provider::unauthenticated_auth_provider(),
        account_id: "account-a".to_string(),
    }
}

fn assert_transient_timeout(err: &io::Error, expected_status: Option<StatusCode>) {
    let request_error = remote_control_server_request_error(err)
        .expect("request error should preserve refresh metadata");
    assert_eq!(
        (
            err.kind(),
            request_error.status,
            request_error.is_transient(err.kind()),
        ),
        (ErrorKind::TimedOut, expected_status, true)
    );
}

async fn timed_out_request(partial_response: Option<&'static [u8]>) -> io::Error {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let url = format!(
        "http://{}/backend-api/wham/remote/control/server/refresh",
        listener
            .local_addr()
            .expect("listener should have a local address")
    );
    let (request_done_tx, request_done_rx) = oneshot::channel();
    let server_task = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("request should connect");
        if let Some(partial_response) = partial_response {
            stream
                .write_all(partial_response)
                .await
                .expect("partial response should write");
        }
        request_done_rx
            .await
            .expect("test should report request completion");
    });

    let err = send_remote_control_server_request::<_, serde_json::Value>(
        &url,
        &auth(),
        "installation-id",
        &json!({"server_id": "server-id"}),
        "refresh",
        "server refresh",
        TEST_REQUEST_TIMEOUT,
    )
    .await
    .expect_err("incomplete response should time out");
    request_done_tx
        .send(())
        .expect("server should wait for request completion");
    server_task.await.expect("server task should finish");
    err
}

fn enrollment(now: OffsetDateTime) -> RemoteControlEnrollment {
    RemoteControlEnrollment {
        remote_control_target: normalize_remote_control_url("http://localhost/backend-api/")
            .expect("target should normalize"),
        account_id: "account-a".to_string(),
        environment_id: "env_first".to_string(),
        server_id: "srv_e_first".to_string(),
        server_name: "first-server".to_string(),
        remote_control_token: Some("token".to_string()),
        expires_at: Some(now + time::Duration::seconds(300)),
        next_refresh_at: None,
    }
}

#[test]
fn remote_control_enrollment_classifies_server_token_refresh_requirement() {
    let now =
        OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("test timestamp should parse");
    let enrollment = enrollment(now);
    let cases = [
        (
            enrollment.clone(),
            RemoteControlServerTokenRefreshRequirement::Proactive,
        ),
        (
            RemoteControlEnrollment {
                expires_at: Some(now + time::Duration::seconds(301)),
                ..enrollment.clone()
            },
            RemoteControlServerTokenRefreshRequirement::NotNeeded,
        ),
        (
            RemoteControlEnrollment {
                next_refresh_at: Some(now + time::Duration::seconds(30)),
                ..enrollment.clone()
            },
            RemoteControlServerTokenRefreshRequirement::NotNeeded,
        ),
        (
            RemoteControlEnrollment {
                next_refresh_at: Some(now),
                ..enrollment.clone()
            },
            RemoteControlServerTokenRefreshRequirement::Proactive,
        ),
        (
            RemoteControlEnrollment {
                remote_control_token: None,
                ..enrollment.clone()
            },
            RemoteControlServerTokenRefreshRequirement::Required,
        ),
        (
            RemoteControlEnrollment {
                expires_at: None,
                ..enrollment.clone()
            },
            RemoteControlServerTokenRefreshRequirement::Required,
        ),
        (
            RemoteControlEnrollment {
                expires_at: Some(now),
                next_refresh_at: Some(now + time::Duration::hours(1)),
                ..enrollment
            },
            RemoteControlServerTokenRefreshRequirement::Required,
        ),
    ];

    for (enrollment, expected) in cases {
        assert_eq!(
            enrollment.server_token_refresh_requirement_at(now),
            expected
        );
    }
}

#[test]
fn remote_control_server_request_error_classifies_status_before_timeout() {
    let cases = [
        (None, true, ErrorKind::TimedOut, true),
        (Some(StatusCode::OK), true, ErrorKind::TimedOut, true),
        (
            Some(StatusCode::TOO_MANY_REQUESTS),
            false,
            ErrorKind::Other,
            true,
        ),
        (Some(StatusCode::BAD_GATEWAY), false, ErrorKind::Other, true),
        (
            Some(StatusCode::UNAUTHORIZED),
            true,
            ErrorKind::PermissionDenied,
            false,
        ),
        (
            Some(StatusCode::FORBIDDEN),
            true,
            ErrorKind::PermissionDenied,
            false,
        ),
        (
            Some(StatusCode::NOT_FOUND),
            true,
            ErrorKind::NotFound,
            false,
        ),
        (Some(StatusCode::BAD_REQUEST), true, ErrorKind::Other, false),
        (None, false, ErrorKind::Other, true),
    ];

    for (status, timed_out, expected_kind, expected_transient) in cases {
        let err = RemoteControlServerRequestError::io_error(
            String::new(),
            status,
            /*retry_at*/ None,
            timed_out,
        );
        let request_error = remote_control_server_request_error(&err)
            .expect("request error should preserve refresh metadata");
        assert_eq!(
            (err.kind(), request_error.is_transient(err.kind())),
            (expected_kind, expected_transient)
        );
    }
}

#[tokio::test]
async fn request_timeout_before_response_headers_is_transient() {
    let err = timed_out_request(/*partial_response*/ None).await;
    assert_transient_timeout(&err, /*expected_status*/ None);
}

#[tokio::test]
async fn response_body_timeout_is_transient() {
    let err = timed_out_request(Some(b"HTTP/1.1 200 OK\r\nContent-Length: 20\r\n\r\n{")).await;
    assert_transient_timeout(&err, Some(StatusCode::OK));
}

#[test]
fn retry_after_supports_delta_seconds_and_http_dates() {
    let now =
        OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("test timestamp should parse");
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::RETRY_AFTER,
        axum::http::HeaderValue::from_static("120"),
    );
    assert_eq!(
        parse_retry_after(&headers, now),
        Some(now + time::Duration::seconds(120))
    );

    let retry_at = now + time::Duration::seconds(90);
    let retry_at_system = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_090);
    headers.insert(
        axum::http::header::RETRY_AFTER,
        httpdate::fmt_http_date(retry_at_system)
            .parse()
            .expect("HTTP date should be a valid header value"),
    );
    assert_eq!(parse_retry_after(&headers, now), Some(retry_at));
}

#[test]
fn invalid_or_expired_retry_after_uses_bounded_fallback() {
    let now =
        OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("test timestamp should parse");
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::RETRY_AFTER,
        axum::http::HeaderValue::from_static("invalid"),
    );
    assert_eq!(parse_retry_after(&headers, now), None);

    headers.insert(
        axum::http::header::RETRY_AFTER,
        httpdate::fmt_http_date(SystemTime::UNIX_EPOCH + Duration::from_secs(1_699_999_999))
            .parse()
            .expect("HTTP date should be a valid header value"),
    );
    assert_eq!(parse_retry_after(&headers, now), None);

    let expired_while_reading_body = Some(now + time::Duration::seconds(1));
    for retry_at in [None, expired_while_reading_body] {
        let deferred_at = now + time::Duration::seconds(2);
        let (delay, next_refresh_at) = refresh_deferral(retry_at, deferred_at);
        assert!(
            (Duration::from_secs(REMOTE_CONTROL_SERVER_TOKEN_REFRESH_BACKOFF_MIN_SECS)
                ..=Duration::from_secs(REMOTE_CONTROL_SERVER_TOKEN_REFRESH_BACKOFF_MAX_SECS,))
                .contains(&delay)
        );
        assert_eq!(
            next_refresh_at,
            deferred_at + time::Duration::seconds(delay.as_secs() as i64)
        );
    }
}

#[test]
fn http_date_retry_after_preserves_absolute_deadline() {
    let received_at =
        OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("test timestamp should parse");
    let retry_at = received_at + time::Duration::seconds(120);
    let body_read_at = received_at + time::Duration::seconds(30);

    assert_eq!(
        refresh_deferral(Some(retry_at), body_read_at),
        (Duration::from_secs(90), retry_at)
    );
}
