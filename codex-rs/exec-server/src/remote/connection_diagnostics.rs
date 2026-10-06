//! Credential-free diagnostics for failed rendezvous connections.
//! Wire headers are untrusted: accept only known rejection codes and never inspect response bodies.

use std::io::ErrorKind;

use tokio_tungstenite::tungstenite::Error;

const REJECTION_REASON_HEADER: &str = "x-codex-rendezvous-rejection-reason";

#[derive(Debug, PartialEq, Eq)]
pub(super) struct ConnectionFailure {
    pub error_kind: &'static str,
    pub io_error_kind: Option<&'static str>,
    pub http_status: Option<u16>,
    pub rejection_reason: Option<&'static str>,
}

impl From<&Error> for ConnectionFailure {
    fn from(error: &Error) -> Self {
        let (error_kind, io_error_kind) = match error {
            Error::Http(response) => {
                return Self {
                    error_kind: "http",
                    io_error_kind: None,
                    http_status: Some(response.status().as_u16()),
                    rejection_reason: response
                        .headers()
                        .get(REJECTION_REASON_HEADER)
                        .and_then(|value| rejection_reason(value.as_bytes())),
                };
            }
            Error::Io(error) => (
                "io",
                Some(match error.kind() {
                    ErrorKind::TimedOut => "timed_out",
                    ErrorKind::ConnectionRefused => "connection_refused",
                    ErrorKind::ConnectionReset => "connection_reset",
                    ErrorKind::ConnectionAborted => "connection_aborted",
                    ErrorKind::NotConnected => "not_connected",
                    ErrorKind::NetworkDown => "network_down",
                    ErrorKind::NetworkUnreachable => "network_unreachable",
                    ErrorKind::HostUnreachable => "host_unreachable",
                    ErrorKind::AddrNotAvailable => "addr_not_available",
                    ErrorKind::PermissionDenied => "permission_denied",
                    ErrorKind::BrokenPipe => "broken_pipe",
                    ErrorKind::UnexpectedEof => "unexpected_eof",
                    ErrorKind::Interrupted => "interrupted",
                    ErrorKind::WouldBlock => "would_block",
                    _ => "other",
                }),
            ),
            Error::Tls(_) => ("tls", None),
            Error::Protocol(_) => ("protocol", None),
            Error::ConnectionClosed | Error::AlreadyClosed => ("connection_closed", None),
            Error::Capacity(_)
            | Error::WriteBufferFull(_)
            | Error::Utf8(_)
            | Error::AttackAttempt
            | Error::Url(_)
            | Error::HttpFormat(_) => ("other", None),
        };
        Self {
            error_kind,
            io_error_kind,
            http_status: None,
            rejection_reason: None,
        }
    }
}

fn rejection_reason(header: &[u8]) -> Option<&'static str> {
    match header {
        b"expired" => Some("expired"),
        b"missing_role" => Some("missing_role"),
        b"missing_expiry" => Some("missing_expiry"),
        b"missing_signature" => Some("missing_signature"),
        b"missing_version" => Some("missing_version"),
        b"missing_executor_registration_id" => Some("missing_executor_registration_id"),
        b"unexpected_executor_registration_id" => Some("unexpected_executor_registration_id"),
        b"malformed_query" => Some("malformed_query"),
        b"invalid_role" => Some("invalid_role"),
        b"invalid_expiry" => Some("invalid_expiry"),
        b"invalid_signature" => Some("invalid_signature"),
        b"invalid_secret" => Some("invalid_secret"),
        b"extra_query_param" => Some("extra_query_param"),
        b"pod_draining" => Some("pod_draining"),
        b"unknown_route" => Some("unknown_route"),
        b"wrong_rendezvous_cluster" => Some("wrong_rendezvous_cluster"),
        b"route_unavailable" => Some("route_unavailable"),
        _ => None,
    }
}

#[cfg(test)]
#[path = "connection_diagnostics_tests.rs"]
mod tests;
