use std::sync::Arc;

use tokio::sync::Mutex;
use tokio::sync::OwnedMutexGuard;
use tracing::warn;

use crate::guardian::GUARDIAN_REVIEW_TIMEOUT;
use crate::session::turn;

use super::GuardianReviewSession;

/// A held guard means eager maintenance is in flight.
#[derive(Default)]
pub(super) struct GuardianEagerCompaction {
    in_flight: Arc<Mutex<()>>,
}

impl GuardianEagerCompaction {
    fn begin(&self) -> Option<OwnedMutexGuard<()>> {
        Arc::clone(&self.in_flight).try_lock_owned().ok()
    }

    async fn wait(&self) {
        drop(Arc::clone(&self.in_flight).lock_owned().await);
    }
}

impl GuardianReviewSession {
    pub(super) async fn schedule_eager_compaction(self: &Arc<Self>) {
        let turn_context = self.codex.session.new_default_turn().await;
        if !turn::auto_compact_needed(self.codex.session.as_ref(), turn_context.as_ref()).await {
            return;
        }
        let Some(in_flight_guard) = self.eager_compaction.begin() else {
            return;
        };

        let review_session = Arc::clone(self);
        drop(tokio::spawn(async move {
            // Keep the latch closed through compaction and snapshot refresh. Any exit path drops it.
            let _in_flight_guard = in_flight_guard;
            let cancel_token = review_session.cancel_token.clone();
            let timed_out = tokio::select! {
                _ = cancel_token.cancelled() => false,
                result = tokio::time::timeout(
                    GUARDIAN_REVIEW_TIMEOUT,
                    review_session.run_eager_compaction(turn_context),
                ) => result.is_err(),
            };
            if timed_out {
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
