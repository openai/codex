use std::future::Future;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

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
    Forced,
}

#[derive(Debug)]
struct TrackedGuardianReview {
    abort_handle: AbortHandle,
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
            cancellation_token: self.cancellation_token.child_token(),
        })
    }

    pub(crate) fn spawn<F>(
        self: &Arc<Self>,
        runtime_handle: &Handle,
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
    cancellation_token: CancellationToken,
}

impl GuardianReviewActivity {
    pub(crate) fn cancellation_token(&self) -> CancellationToken {
        self.cancellation_token.clone()
    }

    pub(crate) fn cancel(&self) {
        self.cancellation_token.cancel();
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
            GuardianReviewDrainOutcome::Forced
        } else {
            GuardianReviewDrainOutcome::Drained
        }
    }
}

#[cfg(test)]
#[path = "task_owner_tests.rs"]
mod tests;
