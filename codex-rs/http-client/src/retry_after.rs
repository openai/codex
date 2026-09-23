//! Server retry advice captured once so passing an error between callers does not restart it.

use http::HeaderMap;
use http::header::RETRY_AFTER;
use std::time::Duration;
use std::time::SystemTime;
use tokio::time::Instant;

/// The earliest time a server advised making a follow-up request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetryAfter(Instant);

impl RetryAfter {
    /// Captures an interval measured from receipt of the server's advice.
    pub fn from_delay(delay: Duration) -> Option<Self> {
        Instant::now().checked_add(delay).map(Self)
    }

    /// Reads either nonnegative delay seconds or an HTTP date from `Retry-After`.
    pub fn from_headers(headers: &HeaderMap) -> Option<Self> {
        Self::from_header(headers.get(RETRY_AFTER)?.to_str().ok()?)
    }

    /// Reads a `Retry-After` value, including values carried inside a streamed error.
    pub fn from_header(value: &str) -> Option<Self> {
        // Sample wall time first so a scheduling pause cannot shorten the server's deadline.
        let now = SystemTime::now();
        let received_at = Instant::now();
        let value = value.trim();
        let delay = if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()) {
            Duration::from_secs(value.parse().ok()?)
        } else {
            httpdate::parse_http_date(value)
                .ok()?
                .duration_since(now)
                .unwrap_or_default()
        };
        received_at.checked_add(delay).map(Self)
    }

    /// Returns the original deadline for callers that pass advice on or sleep until it.
    pub fn deadline(self) -> Instant {
        self.0
    }

    /// Returns the remaining delay, or zero after the deadline has passed.
    pub fn remaining_delay(self) -> Duration {
        self.0.saturating_duration_since(Instant::now())
    }
}

#[cfg(test)]
#[path = "retry_after_tests.rs"]
mod tests;
