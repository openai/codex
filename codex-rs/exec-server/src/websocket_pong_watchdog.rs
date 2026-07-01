use std::time::Duration;

use tokio::time::Instant;

#[cfg(test)]
pub(crate) const WEBSOCKET_PONG_TIMEOUT: Duration = Duration::from_millis(100);
#[cfg(not(test))]
pub(crate) const WEBSOCKET_PONG_TIMEOUT: Duration = Duration::from_secs(60);
pub(crate) const WEBSOCKET_PONG_TIMEOUT_REASON: &str = "pong_timeout";

/// Tracks whether a WebSocket peer has acknowledged a keepalive ping.
pub(crate) struct WebSocketPongWatchdog {
    timeout: Duration,
    deadline: Option<Instant>,
}

impl WebSocketPongWatchdog {
    pub(crate) fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            deadline: None,
        }
    }

    pub(crate) fn ping_sent(&mut self, now: Instant) {
        self.deadline.get_or_insert(now + self.timeout);
    }

    pub(crate) fn deadline(&self) -> Option<Instant> {
        self.deadline
    }

    pub(crate) fn write_deadline(&self, now: Instant) -> Instant {
        self.deadline.unwrap_or(now + self.timeout)
    }

    pub(crate) fn received_pong(&mut self) {
        self.deadline = None;
    }
}

#[cfg(test)]
#[path = "websocket_pong_watchdog_tests.rs"]
mod tests;
