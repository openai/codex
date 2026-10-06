//! Verify that connection failures remain correlated without exposing wire payloads.

use pretty_assertions::assert_eq;
use tokio_tungstenite::tungstenite::Error;

use super::ConnectionFailure;
use super::REJECTION_REASON_HEADER;

#[test]
fn only_known_http_rejection_headers_are_reported() {
    for (header, expected) in [
        (Some("expired"), Some("expired")),
        (Some("invalid_signature"), Some("invalid_signature")),
        (Some("pod_draining"), Some("pod_draining")),
        (Some("expired secret=credential"), None),
        (Some("private-proxy-hostname"), None),
        (None, None),
    ] {
        let mut response = http::Response::builder().status(401);
        if let Some(header) = header {
            response = response.header(REJECTION_REASON_HEADER, header);
        }
        // A known legacy body must not override a header or stand in for a missing one.
        let error = Error::Http(Box::new(
            response.body(Some(b"pod_draining".to_vec())).unwrap(),
        ));
        assert_eq!(
            ConnectionFailure::from(&error),
            ConnectionFailure {
                error_kind: "http",
                io_error_kind: None,
                http_status: Some(401),
                rejection_reason: expected,
            }
        );
    }
}

#[test]
fn io_failure_reports_typed_kind_without_error_text() {
    use std::io::ErrorKind;

    for (kind, expected) in [
        (ErrorKind::TimedOut, "timed_out"),
        (ErrorKind::ConnectionRefused, "connection_refused"),
        (ErrorKind::ConnectionReset, "connection_reset"),
        (ErrorKind::ConnectionAborted, "connection_aborted"),
        (ErrorKind::NotConnected, "not_connected"),
        (ErrorKind::NetworkDown, "network_down"),
        (ErrorKind::NetworkUnreachable, "network_unreachable"),
        (ErrorKind::HostUnreachable, "host_unreachable"),
        (ErrorKind::AddrNotAvailable, "addr_not_available"),
        (ErrorKind::PermissionDenied, "permission_denied"),
        (ErrorKind::BrokenPipe, "broken_pipe"),
        (ErrorKind::UnexpectedEof, "unexpected_eof"),
        (ErrorKind::Interrupted, "interrupted"),
        (ErrorKind::WouldBlock, "would_block"),
        (ErrorKind::Other, "other"),
        (ErrorKind::InvalidData, "other"),
    ] {
        let error = Error::Io(std::io::Error::new(kind, "private host and credentials"));
        assert_eq!(
            ConnectionFailure::from(&error),
            ConnectionFailure {
                error_kind: "io",
                io_error_kind: Some(expected),
                http_status: None,
                rejection_reason: None,
            }
        );
    }
}

#[tokio::test]
async fn websocket_rejection_preserves_status_and_sends_correlation_id() {
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::header;
    use wiremock::matchers::method;

    let server = MockServer::start().await;
    let request_id = "req_0123456789abcdef0123456789abcdef";
    Mock::given(method("GET"))
        .and(header("x-request-id", request_id))
        .respond_with(
            ResponseTemplate::new(401)
                .insert_header(REJECTION_REASON_HEADER, "expired")
                .set_body_string("private proxy response that must not be logged"),
        )
        .expect(1)
        .mount(&server)
        .await;
    let result = super::super::connect_rendezvous(
        &format!("ws://{}/", server.address()),
        request_id,
        &crate::ExecServerTelemetry::default(),
        &codex_http_client::HttpClientFactory::new(
            codex_http_client::OutboundProxyPolicy::ReqwestDefault,
        ),
    )
    .await;
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("expected the websocket upgrade to be rejected"),
    };
    assert_eq!(
        ConnectionFailure::from(&error),
        ConnectionFailure {
            error_kind: "http",
            io_error_kind: None,
            http_status: Some(401),
            rejection_reason: Some("expired"),
        }
    );
}
