//! Pause-aware status time owned independently of the optional status row.
//! A phase can override the displayed clock without changing turn accounting.

use std::time::Duration;
use std::time::Instant;

#[derive(Debug)]
pub(crate) struct StatusTimer {
    /// Optional wall-clock origin for the displayed phase, without resetting turn accounting.
    pub(crate) display_started_at: Option<Instant>,
    pub(super) elapsed_running: Duration,
    pub(super) last_resume_at: Instant,
    pub(super) is_paused: bool,
}

impl Default for StatusTimer {
    fn default() -> Self {
        Self {
            display_started_at: None,
            elapsed_running: Duration::ZERO,
            last_resume_at: Instant::now(),
            is_paused: false,
        }
    }
}

impl StatusTimer {
    pub(super) fn display_elapsed_at(&self, now: Instant) -> Duration {
        self.display_started_at.map_or_else(
            || self.elapsed_at(now),
            |started_at| now.saturating_duration_since(started_at),
        )
    }

    /// Time until the displayed second changes, independent of decorative motion.
    pub(super) fn next_tick_delay(&self, now: Instant) -> Option<Duration> {
        if self.is_paused && self.display_started_at.is_none() {
            return None;
        }
        let elapsed = self.display_elapsed_at(now);
        let fraction = Duration::from_nanos(u64::from(elapsed.subsec_nanos()));
        Some(Duration::from_secs(1) - fraction)
    }

    pub(crate) fn reset(&mut self, elapsed: Duration) {
        self.elapsed_running = elapsed;
        self.last_resume_at = Instant::now();
        // A turn can start while an MCP elicitation or approval is still open.
        // Reset elapsed time without changing that modal's pause state.
    }

    pub(crate) fn pause_at(&mut self, now: Instant) {
        if !self.is_paused {
            self.elapsed_running += now.saturating_duration_since(self.last_resume_at);
            self.is_paused = true;
        }
    }

    pub(crate) fn resume_at(&mut self, now: Instant) {
        if self.is_paused {
            self.last_resume_at = now;
            self.is_paused = false;
        }
    }

    pub(crate) fn elapsed_at(&self, now: Instant) -> Duration {
        self.elapsed_running
            + if self.is_paused {
                Duration::ZERO
            } else {
                now.saturating_duration_since(self.last_resume_at)
            }
    }
}

#[cfg(test)]
#[path = "timer_tests.rs"]
mod tests;
