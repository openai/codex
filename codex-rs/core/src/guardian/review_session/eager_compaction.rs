use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::guardian::GUARDIAN_REVIEW_TIMEOUT;
use crate::session::turn;

use super::GuardianReviewSession;

#[cfg(test)]
#[path = "eager_compaction_tests.rs"]
mod tests;

#[derive(Default)]
pub(super) struct GuardianEagerCompaction {
    completion: Mutex<Option<watch::Receiver<bool>>>,
}

#[derive(Debug, Eq, PartialEq)]
enum EagerCompactionRunOutcome {
    Completed,
    Cancelled,
    TimedOut,
}

struct EagerCompactionRun {
    completion: watch::Sender<bool>,
}

impl EagerCompactionRun {
    async fn run_bounded<F>(
        self,
        cancel_token: &CancellationToken,
        timeout: Duration,
        maintenance: F,
    ) -> EagerCompactionRunOutcome
    where
        F: Future<Output = ()>,
    {
        let outcome = tokio::select! {
            _ = cancel_token.cancelled() => EagerCompactionRunOutcome::Cancelled,
            result = tokio::time::timeout(timeout, maintenance) => {
                if result.is_ok() {
                    EagerCompactionRunOutcome::Completed
                } else {
                    EagerCompactionRunOutcome::TimedOut
                }
            }
        };
        self.completion.send_replace(true);
        outcome
    }
}

impl GuardianEagerCompaction {
    async fn begin(&self) -> Option<EagerCompactionRun> {
        let mut completion = self.completion.lock().await;
        if let Some(receiver) = completion.as_ref()
            && !*receiver.borrow()
            && receiver.has_changed().is_ok()
        {
            return None;
        }

        let (sender, receiver) = watch::channel(false);
        *completion = Some(receiver);
        Some(EagerCompactionRun { completion: sender })
    }

    async fn wait(&self) {
        let Some(mut completion) = self.completion.lock().await.clone() else {
            return;
        };
        if *completion.borrow() {
            return;
        }
        while completion.changed().await.is_ok() {
            if *completion.borrow() {
                return;
            }
        }
    }
}

impl GuardianReviewSession {
    pub(super) async fn schedule_eager_compaction(self: &Arc<Self>) {
        let turn_context = self.codex.session.new_default_turn().await;
        if !turn::auto_compact_needed(self.codex.session.as_ref(), turn_context.as_ref()).await {
            return;
        }
        let Some(run) = self.eager_compaction.begin().await else {
            return;
        };

        let review_session = Arc::clone(self);
        drop(tokio::spawn(async move {
            let cancel_token = review_session.cancel_token.clone();
            let outcome = run
                .run_bounded(
                    &cancel_token,
                    GUARDIAN_REVIEW_TIMEOUT,
                    review_session.run_eager_compaction(turn_context),
                )
                .await;
            if outcome == EagerCompactionRunOutcome::TimedOut {
                warn!(
                    guardian_thread_id = %review_session.codex.session.thread_id,
                    "eager guardian maintenance timed out after {GUARDIAN_REVIEW_TIMEOUT:?}"
                );
            }
        }));
    }

    pub(super) async fn wait_for_eager_compaction(&self) {
        self.eager_compaction.wait().await;
    }

    async fn run_eager_compaction(
        self: &Arc<Self>,
        turn_context: Arc<crate::session::turn_context::TurnContext>,
    ) {
        let Ok(review_guard) = self.review_lock.acquire().await else {
            return;
        };

        let mut client_session = self.codex.session.services.model_client.new_session();
        let compact_result = turn::run_pre_turn_auto_compact_if_needed(
            &self.codex.session,
            &turn_context,
            &mut client_session,
        )
        .await;

        match compact_result {
            Ok(true) => {
                self.refresh_last_committed_fork_snapshot().await;
            }
            Ok(false) => {}
            Err(err) => {
                warn!(
                    guardian_thread_id = %self.codex.session.thread_id,
                    "eager guardian compaction failed: {err}"
                );
            }
        }

        drop(review_guard);
    }
}
