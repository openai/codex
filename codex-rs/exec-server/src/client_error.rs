//! Classifies typed executor failures without treating error text as a retry signal.

use std::io::ErrorKind;

use http::StatusCode;
use tokio_tungstenite::tungstenite::Error as WebSocketError;
use tokio_tungstenite::tungstenite::error::ProtocolError;

use crate::ExecServerError;
use crate::client::is_retryable_registry_error;
use crate::rpc::SESSION_ALREADY_ATTACHED_ERROR_CODE;

impl ExecServerError {
    /// Identifies transient failures that read-only preparation can retry.
    ///
    /// Includes transient handshake responses and connection EOFs, including wrapped
    /// failures. Authentication, TLS, and malformed protocol errors are excluded.
    /// This is public because callers in `codex-core` also classify capability discovery.
    pub fn is_retryable_preparation_error(&self) -> bool {
        let mut error = self;
        while let Self::ConnectionAttempt(source) = error {
            error = source;
        }
        if is_retryable_registry_error(error) {
            return true;
        }
        match error {
            Self::Closed
            | Self::Disconnected(_)
            | Self::WebSocketConnectTimeout { .. }
            | Self::InitializeTimedOut { .. } => true,
            Self::WebSocketConnect { source, .. } => match source {
                WebSocketError::ConnectionClosed
                | WebSocketError::AlreadyClosed
                | WebSocketError::Protocol(
                    ProtocolError::HandshakeIncomplete
                    | ProtocolError::ResetWithoutClosingHandshake,
                ) => true,
                WebSocketError::Http(response) => {
                    response.status().is_server_error()
                        || matches!(
                            response.status(),
                            StatusCode::REQUEST_TIMEOUT
                                | StatusCode::CONFLICT
                                | StatusCode::TOO_MANY_REQUESTS
                        )
                }
                WebSocketError::Io(error) => matches!(
                    error.kind(),
                    ErrorKind::ConnectionRefused
                        | ErrorKind::ConnectionReset
                        | ErrorKind::ConnectionAborted
                        | ErrorKind::NetworkDown
                        | ErrorKind::NetworkUnreachable
                        | ErrorKind::HostUnreachable
                        | ErrorKind::BrokenPipe
                        | ErrorKind::NotConnected
                        | ErrorKind::UnexpectedEof
                        | ErrorKind::TimedOut
                ),
                _ => false,
            },
            Self::Server { code, .. } if *code == SESSION_ALREADY_ATTACHED_ERROR_CODE => true,
            _ => false,
        }
    }
}
