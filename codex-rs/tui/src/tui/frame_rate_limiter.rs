//! Limits how frequently frame draw notifications may be emitted.
//!
//! Widgets sometimes call `FrameRequester::schedule_frame()` more frequently than a user can
//! perceive. This limiter clamps draw notifications to a maximum of 120 FPS to avoid wasted work.
//!
//! This is intentionally a small, pure helper so it can be unit-tested in isolation and used by
//! the async frame scheduler without adding complexity to the app/event loop.

use std::time::Duration;
use std::time::Instant;

/// A 120 FPS minimum frame interval (≈8.33ms).
pub(super) const MIN_FRAME_INTERVAL: Duration = Duration::from_nanos(8_333_334);

/// Remembers the most recent emitted draw, allowing deadlines to be clamped forward.
#[derive(Debug)]
pub(super) struct FrameRateLimiter {
    last_emitted_at: Option<Instant>,
    min_interval: Duration,
}

impl FrameRateLimiter {
    pub(super) fn new(min_interval: Duration) -> Self {
        Self {
            last_emitted_at: None,
            min_interval,
        }
    }

    /// Returns `requested`, clamped forward if it would exceed the maximum frame rate.
    pub(super) fn clamp_deadline(&self, requested: Instant) -> Instant {
        let Some(last_emitted_at) = self.last_emitted_at else {
            return requested;
        };
        let min_allowed = last_emitted_at
            .checked_add(self.min_interval)
            .unwrap_or(last_emitted_at);
        requested.max(min_allowed)
    }

    /// Records that a draw notification was emitted at `emitted_at`.
    pub(super) fn mark_emitted(&mut self, emitted_at: Instant) {
        self.last_emitted_at = Some(emitted_at);
    }
}

impl Default for FrameRateLimiter {
    fn default() -> Self {
        Self::new(MIN_FRAME_INTERVAL)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn default_does_not_clamp() {
        let t0 = Instant::now();
        let limiter = FrameRateLimiter::default();
        assert_eq!(limiter.clamp_deadline(t0), t0);
    }

    #[test]
    fn clamps_to_min_interval_since_last_emit() {
        let t0 = Instant::now();
        let mut limiter = FrameRateLimiter::default();

        assert_eq!(limiter.clamp_deadline(t0), t0);
        limiter.mark_emitted(t0);

        let too_soon = t0 + Duration::from_millis(1);
        assert_eq!(limiter.clamp_deadline(too_soon), t0 + MIN_FRAME_INTERVAL);
    }

    #[test]
    fn clamps_to_custom_interval_since_last_emit() {
        let t0 = Instant::now();
        let mut limiter = FrameRateLimiter::new(Duration::from_millis(100));

        assert_eq!(limiter.clamp_deadline(t0), t0);
        limiter.mark_emitted(t0);

        let too_soon = t0 + Duration::from_millis(10);
        assert_eq!(
            limiter.clamp_deadline(too_soon),
            t0 + Duration::from_millis(100)
        );
    }
}
