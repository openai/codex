//! Handshake probe evidence. Failure stages require typed evidence, and Debug output hides wire
//! values that may contain credentials; diagnostic callers must redact displayed values themselves.

use crate::error::ApiError;
use http::HeaderMap;
use http::StatusCode;

/// Close frame information captured by a handshake probe.
#[derive(Clone, PartialEq, Eq)]
pub struct ResponsesWebsocketClose {
    /// WebSocket close code returned by the server.
    pub code: String,
    /// Human-readable close reason returned by the server.
    pub reason: String,
}

impl std::fmt::Debug for ResponsesWebsocketClose {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResponsesWebsocketClose")
            .field("code", &self.code)
            .finish_non_exhaustive()
    }
}

/// Result of a handshake-only Responses WebSocket probe.
#[derive(Clone, PartialEq, Eq)]
pub struct ResponsesWebsocketProbe {
    /// Redacted by callers before displaying or serializing support reports.
    pub url: String,
    /// HTTP status returned by the successful WebSocket upgrade.
    pub status: StatusCode,
    /// Allowlisted tracking headers from the successful upgrade.
    pub headers: HeaderMap,
    /// Whether the server reported reasoning support in the upgrade response.
    pub reasoning_included: bool,
    /// Whether the server returned a server-selected model in the upgrade response.
    pub server_model_present: bool,
    /// Close frame received immediately after upgrade, when one arrives quickly.
    pub immediate_close: Option<ResponsesWebsocketClose>,
}

impl std::fmt::Debug for ResponsesWebsocketProbe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResponsesWebsocketProbe")
            .field("status", &self.status)
            .field("reasoning_included", &self.reasoning_included)
            .field("server_model_present", &self.server_model_present)
            .field("immediate_close", &self.immediate_close)
            .finish_non_exhaustive()
    }
}

/// A failure stage established by typed evidence at the handshake boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponsesWebsocketProbeFailureStage {
    /// TLS configuration or a typed TLS handshake failure.
    Tls,
    /// The transport did not expose enough evidence to identify a failure stage.
    Unknown,
}

/// Probe failure with any HTTP upgrade evidence received before the failure.
///
/// Rejected upgrades retain status and headers in the original [`ApiError`]. An error after a
/// successful upgrade also retains that upgrade's status and allowlisted tracking headers here.
#[derive(thiserror::Error)]
#[error("Responses WebSocket probe failed")]
pub struct ResponsesWebsocketProbeError {
    /// Original API error, preserving the production connection's error behavior.
    #[source]
    pub error: Box<ApiError>,
    /// Failure stage, when typed evidence establishes one.
    pub stage: ResponsesWebsocketProbeFailureStage,
    /// Status received before a subsequent read failure.
    pub status: Option<StatusCode>,
    /// Allowlisted tracking headers received before a subsequent read failure.
    pub headers: Option<HeaderMap>,
}

impl ResponsesWebsocketProbeError {
    pub(crate) fn before_upgrade(
        error: ApiError,
        stage: ResponsesWebsocketProbeFailureStage,
    ) -> Self {
        Self {
            error: Box::new(error),
            stage,
            status: None,
            headers: None,
        }
    }
}

impl std::fmt::Debug for ResponsesWebsocketProbeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResponsesWebsocketProbeError")
            .field("stage", &self.stage)
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}
