//! Telemetry traits for monitoring SDK operations.
//!
//! This module provides traits for instrumenting hyper-sdk operations.
//! Implementations can collect metrics, log events, or integrate with
//! observability platforms.
//!
//! # Example
//!
//! ```ignore
//! use hyper_sdk::telemetry::{RequestTelemetry, StreamTelemetry};
//! use std::time::Duration;
//! use http::StatusCode;
//!
//! struct MyTelemetry;
//!
//! impl RequestTelemetry for MyTelemetry {
//!     fn on_request(
//!         &self,
//!         attempt: i32,
//!         status: Option<StatusCode>,
//!         error: Option<&HyperError>,
//!         duration: Duration,
//!     ) {
//!         metrics::counter!("api_requests", 1, "attempt" => attempt.to_string());
//!         metrics::histogram!("api_latency", duration.as_secs_f64());
//!     }
//! }
//! ```

use crate::error::HyperError;
use crate::stream::StreamEvent;
use http::StatusCode;
use std::fmt::Debug;
use std::time::Duration;

/// Telemetry trait for HTTP request monitoring.
///
/// Implement this trait to receive callbacks for API request lifecycle events.
/// This is useful for:
/// - Collecting latency metrics
/// - Tracking retry attempts
/// - Logging request/response details
/// - Alerting on error patterns
pub trait RequestTelemetry: Send + Sync + Debug {
    /// Called after each request attempt.
    ///
    /// # Arguments
    ///
    /// * `attempt` - The attempt number (1-indexed, increments on retries)
    /// * `status` - HTTP status code if the request completed, None if it failed before getting a response
    /// * `error` - The error if the request failed, None if successful
    /// * `duration` - How long the request took
    fn on_request(
        &self,
        attempt: i32,
        status: Option<StatusCode>,
        error: Option<&HyperError>,
        duration: Duration,
    );

    /// Called when a request is about to be retried.
    ///
    /// Default implementation does nothing.
    fn on_retry(&self, _attempt: i32, _delay: Duration) {}

    /// Called when all retries are exhausted.
    ///
    /// Default implementation does nothing.
    fn on_exhausted(&self, _total_attempts: i32, _final_error: &HyperError) {}
}

/// Telemetry trait for stream monitoring.
///
/// Implement this trait to receive callbacks for streaming events.
/// This is useful for:
/// - Tracking stream progress
/// - Measuring token throughput
/// - Detecting slow or stalled streams
/// - Collecting per-event metrics
pub trait StreamTelemetry: Send + Sync + Debug {
    /// Called after each stream poll that returns an event.
    ///
    /// # Arguments
    ///
    /// * `event` - The event that was received, if any
    /// * `duration` - How long the poll took
    fn on_stream_poll(&self, event: Option<&StreamEvent>, duration: Duration);

    /// Called when the stream completes (successfully or with error).
    ///
    /// # Arguments
    ///
    /// * `total_events` - Total number of events received
    /// * `total_duration` - Total time from stream start to completion
    fn on_stream_complete(&self, total_events: i64, total_duration: Duration);

    /// Called when a stream error occurs.
    ///
    /// Default implementation does nothing.
    fn on_stream_error(&self, _error: &HyperError) {}

    /// Called when stream idle timeout is triggered.
    ///
    /// Default implementation does nothing.
    fn on_idle_timeout(&self, _timeout: Duration) {}
}

/// A no-op telemetry implementation for when telemetry is disabled.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoopTelemetry;

impl RequestTelemetry for NoopTelemetry {
    fn on_request(
        &self,
        _attempt: i32,
        _status: Option<StatusCode>,
        _error: Option<&HyperError>,
        _duration: Duration,
    ) {
    }
}

impl StreamTelemetry for NoopTelemetry {
    fn on_stream_poll(&self, _event: Option<&StreamEvent>, _duration: Duration) {}
    fn on_stream_complete(&self, _total_events: i64, _total_duration: Duration) {}
}

/// A simple logging telemetry implementation.
#[derive(Debug, Clone, Copy, Default)]
pub struct LoggingTelemetry;

impl RequestTelemetry for LoggingTelemetry {
    fn on_request(
        &self,
        attempt: i32,
        status: Option<StatusCode>,
        error: Option<&HyperError>,
        duration: Duration,
    ) {
        match (status, error) {
            (Some(status), None) => {
                tracing::debug!(
                    attempt,
                    status = %status,
                    duration_ms = duration.as_millis() as i64,
                    "request completed"
                );
            }
            (status, Some(err)) => {
                tracing::warn!(
                    attempt,
                    status = status.map(|s| s.as_u16()),
                    duration_ms = duration.as_millis() as i64,
                    error = %err,
                    "request failed"
                );
            }
            (None, None) => {
                tracing::debug!(
                    attempt,
                    duration_ms = duration.as_millis() as i64,
                    "request completed (no status)"
                );
            }
        }
    }

    fn on_retry(&self, attempt: i32, delay: Duration) {
        tracing::info!(
            attempt,
            delay_ms = delay.as_millis() as i64,
            "retrying request"
        );
    }

    fn on_exhausted(&self, total_attempts: i32, final_error: &HyperError) {
        tracing::error!(
            total_attempts,
            error = %final_error,
            "all retries exhausted"
        );
    }
}

impl StreamTelemetry for LoggingTelemetry {
    fn on_stream_poll(&self, event: Option<&StreamEvent>, duration: Duration) {
        if let Some(event) = event {
            tracing::trace!(
                event_type = ?std::mem::discriminant(event),
                duration_us = duration.as_micros() as i64,
                "stream event"
            );
        }
    }

    fn on_stream_complete(&self, total_events: i64, total_duration: Duration) {
        tracing::debug!(
            total_events,
            total_duration_ms = total_duration.as_millis() as i64,
            "stream completed"
        );
    }

    fn on_stream_error(&self, error: &HyperError) {
        tracing::warn!(error = %error, "stream error");
    }

    fn on_idle_timeout(&self, timeout: Duration) {
        tracing::warn!(
            timeout_ms = timeout.as_millis() as i64,
            "stream idle timeout"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noop_telemetry() {
        let telemetry = NoopTelemetry;
        telemetry.on_request(1, Some(StatusCode::OK), None, Duration::from_millis(100));
        telemetry.on_stream_poll(None, Duration::from_micros(10));
        telemetry.on_stream_complete(10, Duration::from_secs(1));
    }

    #[test]
    fn test_logging_telemetry() {
        let telemetry = LoggingTelemetry;

        // Test successful request
        telemetry.on_request(1, Some(StatusCode::OK), None, Duration::from_millis(100));

        // Test failed request
        let error = HyperError::NetworkError("connection refused".to_string());
        telemetry.on_request(2, None, Some(&error), Duration::from_millis(50));

        // Test retry
        telemetry.on_retry(2, Duration::from_secs(1));

        // Test exhausted
        telemetry.on_exhausted(3, &error);

        // Test stream events
        telemetry.on_stream_poll(None, Duration::from_micros(10));
        telemetry.on_stream_complete(100, Duration::from_secs(5));
        telemetry.on_stream_error(&error);
        telemetry.on_idle_timeout(Duration::from_secs(60));
    }

    #[test]
    fn test_telemetry_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<NoopTelemetry>();
        assert_send_sync::<LoggingTelemetry>();
    }
}
