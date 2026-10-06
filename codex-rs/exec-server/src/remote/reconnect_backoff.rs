//! Spread executor reconnects and retain backoff until a connection has been stable.

use std::time::Duration;

const INITIAL_BACKOFF: Duration = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(30);
// Longer than the first keepalive interval plus its pong timeout (30s + 60s).
const STABLE_CONNECTION_DURATION: Duration = Duration::from_secs(120);

pub(super) struct ReconnectBackoff {
    delay: Duration,
}

impl ReconnectBackoff {
    pub(super) fn new() -> Self {
        Self {
            delay: INITIAL_BACKOFF,
        }
    }

    pub(super) fn connection_closed(&mut self, connected_for: Duration) {
        if connected_for >= STABLE_CONNECTION_DURATION {
            self.delay = INITIAL_BACKOFF;
        }
    }

    pub(super) fn next_delay(&mut self, random_sample: u64) -> Duration {
        let delay = reconnect_delay(self.delay, random_sample);
        self.delay = (self.delay * 2).min(MAX_BACKOFF);
        delay
    }
}

fn reconnect_delay(backoff: Duration, random_sample: u64) -> Duration {
    let upper_ms = backoff.as_millis() as u64;
    let lower_ms = upper_ms / 2;
    Duration::from_millis(lower_ms + random_sample % (upper_ms - lower_ms + 1))
}

#[cfg(test)]
#[path = "reconnect_backoff_tests.rs"]
mod tests;
