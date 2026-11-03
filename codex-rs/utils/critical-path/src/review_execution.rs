//! Collects exact child execution intervals for one automatic review, then seals them.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

/// Observes child turn execution for one review, including concurrent or retried turns.
/// The owner must seal observations before publishing the completed dependency path.
/// Equality compares review identity, not its collected intervals.
#[derive(Clone, Debug, Default)]
pub struct ReviewExecution(Arc<Mutex<State>>);

/// Resolves one submission exactly once, including cancellation before acknowledgement.
#[must_use = "keep the attempt alive until submission completes"]
pub struct ReviewExecutionAttempt {
    execution: ReviewExecution,
    finished: bool,
}

impl ReviewExecutionAttempt {
    /// Records whether the submitted child's execution boundaries are known.
    pub fn finish(mut self, known: bool) {
        self.execution.attempt_finished(known);
        self.finished = true;
    }
}

impl Drop for ReviewExecutionAttempt {
    fn drop(&mut self) {
        if !self.finished {
            self.execution.attempt_finished(/*known*/ false);
        }
    }
}

#[derive(Debug, Default)]
struct State {
    observed: bool,
    sealed: bool,
    unknown: bool,
    pending: BTreeSet<Instant>,
    pending_attempts: usize,
    intervals: Vec<(Instant, Instant)>,
}

impl ReviewExecution {
    /// Declares that this review's child executions are instrumented, even if none run.
    pub fn observe(&self) {
        let mut state = self.state();
        if !state.sealed {
            state.observed = true;
        }
    }

    /// Registers an unresolved submission before asynchronous work can be detached.
    pub fn start_attempt(&self) -> ReviewExecutionAttempt {
        let mut state = self.state();
        if !state.sealed {
            state.pending_attempts += 1;
        }
        ReviewExecutionAttempt {
            execution: self.clone(),
            finished: false,
        }
    }

    /// Resolves one submission. Unknown execution remains unknown after it finishes.
    fn attempt_finished(&self, known: bool) {
        let mut state = self.state();
        if state.sealed {
            return;
        }
        if let Some(pending) = state.pending_attempts.checked_sub(1) {
            state.pending_attempts = pending;
        } else {
            state.unknown = true;
        }
        state.unknown |= !known;
    }

    /// Records the actual child turn start, before it can be canceled.
    pub fn started(&self, started_at: Instant) {
        let mut state = self.state();
        if !state.sealed && !state.pending.insert(started_at) {
            // Equal clock readings cannot distinguish two concurrent attempts.
            state.unknown = true;
        }
    }

    /// Records exact turn bounds. Repeated completion notifications are harmless.
    pub fn completed(&self, started_at: Instant, completed_at: Instant) {
        let mut state = self.state();
        if state.sealed {
            return;
        }
        state.pending.remove(&started_at);
        if completed_at < started_at {
            state.unknown = true;
        } else {
            state.intervals.push((started_at, completed_at));
        }
    }

    /// Marks unavailable child attribution, rather than reporting it as review overhead.
    pub fn mark_unknown(&self) {
        let mut state = self.state();
        if !state.sealed {
            state.unknown = true;
        }
    }

    /// Freezes this review's evidence. Later callbacks cannot change a completed path.
    pub fn seal(&self) {
        self.state().sealed = true;
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

pub(crate) fn overhead(
    reviews: &[ReviewExecution],
    start: Instant,
    end: Instant,
) -> Option<Duration> {
    if reviews.is_empty() {
        return None;
    }
    let mut intervals = Vec::new();
    for review in reviews {
        let state = review.state();
        if !state.observed
            || state.unknown
            || !state.pending.is_empty()
            || state.pending_attempts > 0
        {
            return None;
        }
        intervals.extend(state.intervals.iter().filter_map(|&(left, right)| {
            let left = left.max(start);
            let right = right.min(end);
            (left < right).then_some((left, right))
        }));
    }
    intervals.sort_unstable();
    let mut execution = Duration::ZERO;
    let mut covered_until = start;
    for (left, right) in intervals {
        if right > covered_until {
            execution += right.duration_since(left.max(covered_until));
            covered_until = right;
        }
    }
    Some(end.duration_since(start).saturating_sub(execution))
}

impl PartialEq for ReviewExecution {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for ReviewExecution {}
