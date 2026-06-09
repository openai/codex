use std::future::Future;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_protocol::protocol::GuardianAssessmentEvent;
use tokio::runtime::Handle;
use tokio::task::AbortHandle;
use tokio::task::JoinHandle;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::warn;

const GUARDIAN_REVIEW_DRAIN_TIMEOUT: Duration = Duration::from_secs(6);

#[derive(Debug, PartialEq)]
pub(crate) enum GuardianReviewDrainOutcome {
    Drained,
    Forced {
        events: Vec<GuardianAssessmentEvent>,
        reason: GuardianReviewCleanupReason,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GuardianReviewCleanupReason {
    DrainTimeout,
    MissingTerminal,
}

#[derive(Debug)]
struct TrackedGuardianReview {
    abort_handle: AbortHandle,
    lifecycle: GuardianReviewLifecycle,
}

#[derive(Clone, Debug, Default)]
struct GuardianReviewLifecycle {
    fallback: Arc<Mutex<Option<GuardianAssessmentEvent>>>,
}

impl GuardianReviewLifecycle {
    fn mark_started(&self, fallback: GuardianAssessmentEvent) {
        *self
            .fallback
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(fallback);
    }

    fn mark_terminal(&self) {
        self.fallback
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
    }

    fn take_fallback(&self) -> Option<GuardianAssessmentEvent> {
        self.fallback
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
    }
}

#[derive(Debug, Default)]
struct GuardianReviewTaskOwnerState {
    closed_at: Option<Instant>,
    reviews: Vec<TrackedGuardianReview>,
}

#[derive(Debug, Default)]
pub(crate) struct GuardianReviewTaskOwner {
    cancellation_token: CancellationToken,
    tasks: TaskTracker,
    state: Mutex<GuardianReviewTaskOwnerState>,
}

impl GuardianReviewTaskOwner {
    fn lock_state(&self) -> std::sync::MutexGuard<'_, GuardianReviewTaskOwnerState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub(crate) fn cancellation_token(&self) -> CancellationToken {
        self.cancellation_token.child_token()
    }

    pub(crate) fn begin(self: &Arc<Self>) -> Option<GuardianReviewActivity> {
        if self.lock_state().closed_at.is_some() {
            return None;
        }
        Some(GuardianReviewActivity {
            owner: Arc::clone(self),
            cancellation_token: self.cancellation_token.child_token(),
            committed: Arc::new(AtomicBool::new(false)),
            lifecycle: GuardianReviewLifecycle::default(),
        })
    }

    pub(crate) fn spawn<F>(
        self: &Arc<Self>,
        runtime_handle: &Handle,
        activity: &GuardianReviewActivity,
        future: F,
    ) -> Option<JoinHandle<F::Output>>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let state = self.lock_state();
        if state.closed_at.is_some() {
            return None;
        }
        let task = self.tasks.spawn_on(future, runtime_handle);
        let mut state = state;
        state.reviews.push(TrackedGuardianReview {
            abort_handle: task.abort_handle(),
            lifecycle: activity.lifecycle.clone(),
        });
        drop(state);
        Some(task)
    }

    pub(crate) fn close(self: &Arc<Self>) -> GuardianReviewDrain {
        let closed_at = {
            let mut state = self.lock_state();
            *state.closed_at.get_or_insert_with(|| {
                self.cancellation_token.cancel();
                self.tasks.close();
                Instant::now()
            })
        };
        GuardianReviewDrain {
            owner: Arc::clone(self),
            deadline: closed_at + GUARDIAN_REVIEW_DRAIN_TIMEOUT,
        }
    }
}

#[derive(Clone)]
pub(crate) struct GuardianReviewActivity {
    owner: Arc<GuardianReviewTaskOwner>,
    cancellation_token: CancellationToken,
    committed: Arc<AtomicBool>,
    lifecycle: GuardianReviewLifecycle,
}

impl GuardianReviewActivity {
    pub(crate) fn cancellation_token(&self) -> CancellationToken {
        self.cancellation_token.clone()
    }

    pub(crate) fn mark_started(&self, fallback: GuardianAssessmentEvent) {
        self.lifecycle.mark_started(fallback);
    }

    pub(crate) fn mark_terminal(&self) {
        self.lifecycle.mark_terminal();
    }

    pub(crate) fn cancel(&self) -> bool {
        if self.committed.load(Ordering::Acquire) {
            return false;
        }
        let _state = self.owner.lock_state();
        if self.committed.load(Ordering::Acquire) {
            return false;
        }
        self.cancellation_token.cancel();
        true
    }

    pub(crate) fn is_committed(&self) -> bool {
        self.committed.load(Ordering::Acquire)
    }

    pub(crate) fn try_commit(&self) -> bool {
        if self.committed.load(Ordering::Acquire) {
            return true;
        }
        let state = self.owner.lock_state();
        if self.committed.load(Ordering::Acquire) {
            return true;
        }
        let can_commit = state.closed_at.is_none() && !self.cancellation_token.is_cancelled();
        if can_commit {
            self.committed.store(true, Ordering::Release);
        }
        can_commit
    }
}

#[must_use = "Guardian reviews must be drained after the parent turn is cancelled"]
pub(crate) struct GuardianReviewDrain {
    owner: Arc<GuardianReviewTaskOwner>,
    deadline: Instant,
}

impl GuardianReviewDrain {
    pub(crate) async fn drain(self) -> GuardianReviewDrainOutcome {
        let timed_out = tokio::time::timeout_at(self.deadline, self.owner.tasks.wait())
            .await
            .is_err();
        let reviews = {
            let mut state = self.owner.lock_state();
            std::mem::take(&mut state.reviews)
        };
        if timed_out {
            for review in &reviews {
                review.abort_handle.abort();
            }
            self.owner.tasks.wait().await;
            warn!("timed out waiting for Guardian reviews to stop");
        }
        let forced_aborts: Vec<_> = reviews
            .into_iter()
            .filter_map(|review| review.lifecycle.take_fallback())
            .collect();
        if timed_out {
            GuardianReviewDrainOutcome::Forced {
                events: forced_aborts,
                reason: GuardianReviewCleanupReason::DrainTimeout,
            }
        } else if !forced_aborts.is_empty() {
            GuardianReviewDrainOutcome::Forced {
                events: forced_aborts,
                reason: GuardianReviewCleanupReason::MissingTerminal,
            }
        } else {
            GuardianReviewDrainOutcome::Drained
        }
    }
}

#[cfg(test)]
#[path = "task_owner_tests.rs"]
mod tests;
