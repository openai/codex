use super::MAX_SAFE_MILLIS_DURATION;
use crate::time::{Clock, Duration, Instant};

/// A structure which handles conversion from Instants to `u64` timestamps.
#[derive(Debug)]
pub(crate) struct TimeSource {
    start_time: Instant,
}

impl TimeSource {
    pub(crate) fn new(clock: &Clock) -> Self {
        Self {
            start_time: clock.now(),
        }
    }

    pub(crate) fn deadline_to_tick(&self, t: Instant) -> u64 {
        // Round up to the end of a ms
        self.instant_to_tick(t + Duration::from_nanos(999_999))
    }

    pub(crate) fn instant_to_tick(&self, t: Instant) -> u64 {
        // round up
        let dur: Duration = t.saturating_duration_since(self.start_time);
        let ms = dur
            .as_millis()
            .try_into()
            .unwrap_or(MAX_SAFE_MILLIS_DURATION);
        ms.min(MAX_SAFE_MILLIS_DURATION)
    }

    pub(crate) fn tick_to_duration(&self, t: u64) -> Duration {
        Duration::from_millis(t)
    }

    pub(crate) fn now(&self, clock: &Clock) -> u64 {
        self.instant_to_tick(clock.now())
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(super) fn start_time(&self) -> Instant {
        self.start_time
    }
}
