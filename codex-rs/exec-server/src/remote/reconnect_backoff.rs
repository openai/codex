//! Spread executor reconnects after a shared outage without exceeding the existing backoff cap.

use std::time::Duration;

pub(super) fn reconnect_delay(backoff: Duration, random_sample: u64) -> Duration {
    let upper_ms = backoff.as_millis() as u64;
    let lower_ms = upper_ms / 2;
    Duration::from_millis(lower_ms + random_sample % (upper_ms - lower_ms + 1))
}

#[cfg(test)]
#[path = "reconnect_backoff_tests.rs"]
mod tests;
