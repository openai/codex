use crate::error::ApiError;
use codex_client::Request;
use codex_client::RequestTelemetry;
use codex_client::Response;
use codex_client::RetryPolicy;
use codex_client::StreamResponse;
use codex_client::TransportError;
use codex_client::run_with_retry;
use http::StatusCode;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant;

/// Generic telemetry.
pub trait SseTelemetry: Send + Sync {
    fn on_sse_poll(
        &self,
        result: &Result<
            Option<
                Result<
                    eventsource_stream::Event,
                    eventsource_stream::EventStreamError<TransportError>,
                >,
            >,
            tokio::time::error::Elapsed,
        >,
        duration: Duration,
    );
}

/// Telemetry for Responses WebSocket transport.
pub trait WebsocketTelemetry: Send + Sync {
    /// Reports how long it took to send the request and whether the send failed.
    fn on_ws_request(&self, duration: Duration, error: Option<&ApiError>, connection_reused: bool);

    /// Reports the poll duration for timeouts, stream errors, end of stream,
    /// and binary, close, or raw frames.
    fn on_ws_receive_failure(&self, duration: Duration);

    /// Reports each text frame and its poll duration. `kind` is `None` if decoding the stream event failed.
    fn on_ws_text_event(&self, text: &str, kind: Option<&str>, duration: Duration);
}

pub(crate) trait WithStatus {
    fn status(&self) -> StatusCode;
}

fn http_status(err: &TransportError) -> Option<StatusCode> {
    match err {
        TransportError::Http { status, .. } => Some(*status),
        _ => None,
    }
}

impl WithStatus for Response {
    fn status(&self) -> StatusCode {
        self.status
    }
}

impl WithStatus for StreamResponse {
    fn status(&self) -> StatusCode {
        self.status
    }
}

pub(crate) async fn run_with_request_telemetry<T, F, Fut>(
    policy: RetryPolicy,
    telemetry: Option<Arc<dyn RequestTelemetry>>,
    make_request: impl FnMut() -> Request,
    send: F,
) -> Result<T, TransportError>
where
    T: WithStatus,
    F: Clone + Fn(Request) -> Fut,
    Fut: Future<Output = Result<T, TransportError>>,
{
    // Wraps `run_with_retry` to attach per-attempt request telemetry for both
    // unary and streaming HTTP calls.
    run_with_retry(policy, make_request, move |req, attempt| {
        let telemetry = telemetry.clone();
        let send = send.clone();
        async move {
            let start = Instant::now();
            let result = send(req).await;
            if let Some(t) = telemetry.as_ref() {
                let (status, err) = match &result {
                    Ok(resp) => (Some(resp.status()), None),
                    Err(err) => (http_status(err), Some(err)),
                };
                t.on_request(attempt, status, err, start.elapsed());
            }
            result
        }
    })
    .await
}
