//! Splits the union of thread-owned approval waits across active turns.
//! Turn completion exports one sample; pending reviews survive idle gaps and turn changes.
//! Saved durations are removed when their turn finishes or its handle drops.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::sync::Weak;
use std::time::Duration;
use std::time::Instant;

/// Tracks pending approvals independently of the turn currently servicing them.
#[derive(Default)]
pub struct ReviewBlocking {
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    interval: Interval,
    turn: Option<String>,
    completed: HashMap<String, Duration>,
}

/// Identifies a turn's sample and removes it when the turn is released.
pub struct ReviewBlockingTurn {
    id: String,
    tracker: Weak<ReviewBlocking>,
    telemetry: Arc<codex_otel::SessionTelemetry>,
}

impl State {
    fn start_turn(&mut self, turn_id: &str, now: Instant) {
        if let Some(previous) = self.turn.replace(turn_id.to_owned()) {
            self.completed
                .insert(previous, self.interval.finish_turn(now));
        }
        self.interval.start_turn(now);
    }

    fn finish_turn(&mut self, turn_id: &str, now: Instant) -> Option<Duration> {
        if self.turn.as_deref() == Some(turn_id) {
            self.turn = None;
            Some(self.interval.finish_turn(now))
        } else {
            self.completed.remove(turn_id)
        }
    }
}

#[derive(Default)]
struct Interval {
    active: usize,
    since: Option<Instant>,
    elapsed: Duration,
}

impl Interval {
    fn start_turn(&mut self, now: Instant) {
        self.elapsed = Duration::ZERO;
        self.since = (self.active > 0).then_some(now);
    }

    fn finish_turn(&mut self, now: Instant) -> Duration {
        if let Some(since) = self.since.take() {
            self.elapsed += now.saturating_duration_since(since);
        }
        std::mem::take(&mut self.elapsed)
    }

    fn enter(&mut self, now: Instant) {
        if self.active == 0 {
            self.since = Some(now);
        }
        self.active += 1;
    }
    fn leave(&mut self, now: Instant) {
        self.active = self.active.saturating_sub(1);
        if self.active == 0
            && let Some(since) = self.since.take()
        {
            self.elapsed += now.saturating_duration_since(since);
        }
    }
}

impl ReviewBlocking {
    pub fn start_turn(
        self: &Arc<Self>,
        turn_id: &str,
        telemetry: Arc<codex_otel::SessionTelemetry>,
    ) -> ReviewBlockingTurn {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .start_turn(turn_id, Instant::now());
        ReviewBlockingTurn {
            id: turn_id.to_owned(),
            tracker: Arc::downgrade(self),
            telemetry,
        }
    }

    pub fn finish_turn(&self, turn: &ReviewBlockingTurn, outcome: &'static str) {
        let elapsed = self
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .finish_turn(&turn.id, Instant::now());
        if let Some(elapsed) = elapsed {
            turn.telemetry.record_duration(
                "codex.guardian.turn.blocking_ms",
                elapsed,
                &[("outcome", outcome)],
            );
        }
    }

    pub(crate) fn enter(self: &Arc<Self>) -> BlockingGuard {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.interval.enter(Instant::now());
        BlockingGuard(Arc::clone(self))
    }
}

impl Drop for ReviewBlockingTurn {
    fn drop(&mut self) {
        if let Some(tracker) = self.tracker.upgrade() {
            tracker
                .state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .finish_turn(&self.id, Instant::now());
        }
    }
}

pub(crate) struct BlockingGuard(Arc<ReviewBlocking>);
impl Drop for BlockingGuard {
    fn drop(&mut self) {
        let mut state = self.0.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.interval.leave(Instant::now());
    }
}

#[cfg(test)]
#[path = "blocking_tests.rs"]
mod tests;
