use crate::error::TransportError;
use http::StatusCode;
use std::time::Duration;

/// API specific telemetry.
pub trait RequestTelemetry: Send + Sync {
    fn on_request(
// codex-client/src/telemetry.rs
        &self,
        attempt: u64,
        status: Option<StatusCode>,
        error: Option<&TransportError>,
        duration: Duration,
    );
}
