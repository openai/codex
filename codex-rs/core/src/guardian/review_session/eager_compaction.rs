use std::sync::Arc;

use tokio::sync::Mutex;
use tokio::sync::OwnedMutexGuard;
use tracing::warn;

use crate::guardian::GUARDIAN_REVIEW_TIMEOUT;
use crate::session::turn;

use super::GuardianReviewForkSnapshot;
use super::GuardianReviewSession;
use super::InitialHistory;
use super::RolloutItem;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum GuardianMaintenanceOutcome {
    #[default]
    Reusable,
    DiscardSession,
}

/// The held guard serializes review ownership with eager maintenance.
pub(super) type GuardianMaintenanceLatch = Arc<Mutex<GuardianMaintenanceOutcome>>;

impl GuardianReviewSession {
    pub(super) async fn schedule_eager_compaction(self: &Arc<Self>) {
        let turn_context = self.codex.session.new_default_turn().await;
        if !turn::auto_compact_needed(self.codex.session.as_ref(), turn_context.as_ref()).await {
            return;
        }
        let mut disposition_guard = Arc::clone(&self.maintenance_latch).lock_owned().await;
        *disposition_guard = GuardianMaintenanceOutcome::DiscardSession;

        let review_session = Arc::clone(self);
        drop(self.background_runtime.spawn(async move {
            *disposition_guard = review_session.run_eager_compaction(turn_context).await;
        }));
    }

    pub(super) async fn acquire_maintenance_latch(
        &self,
    ) -> OwnedMutexGuard<GuardianMaintenanceOutcome> {
        Arc::clone(&self.maintenance_latch).lock_owned().await
    }

    async fn run_eager_compaction(
        self: &Arc<Self>,
        turn_context: Arc<crate::session::turn_context::TurnContext>,
    ) -> GuardianMaintenanceOutcome {
        let Ok(_review_guard) = self.review_lock.acquire().await else {
            return GuardianMaintenanceOutcome::DiscardSession;
        };
        let Some(fork_snapshot) = self.fork_snapshot().await else {
            return GuardianMaintenanceOutcome::DiscardSession;
        };
        let InitialHistory::Forked(items) = &fork_snapshot.initial_history else {
            return GuardianMaintenanceOutcome::DiscardSession;
        };
        let rollout_item_count = items.len();
        let mut client_session = self.codex.session.services.model_client.new_session();
        let compact_result = tokio::select! {
            _ = self.cancel_token.cancelled() => None,
            result = tokio::time::timeout(
                GUARDIAN_REVIEW_TIMEOUT,
                turn::run_pre_turn_auto_compact(
                    &self.codex.session,
                    &turn_context,
                    &mut client_session,
                ),
            ) => match result {
                Ok(result) => Some(result),
                Err(_) => {
                    warn!(
                        guardian_thread_id = %self.codex.session.thread_id,
                        "eager guardian maintenance timed out after {GUARDIAN_REVIEW_TIMEOUT:?}"
                    );
                    None
                }
            },
        };
        match compact_result {
            Some(Ok(())) => {
                let durable = self
                    .refresh_last_committed_fork_snapshot()
                    .await
                    .is_some_and(|snapshot| {
                        snapshot_has_durable_compaction(&snapshot, rollout_item_count)
                    });
                if durable {
                    GuardianMaintenanceOutcome::Reusable
                } else {
                    warn!(
                        guardian_thread_id = %self.codex.session.thread_id,
                        "eager guardian compaction was not durably published"
                    );
                    GuardianMaintenanceOutcome::DiscardSession
                }
            }
            Some(Err(err)) => {
                warn!(
                    guardian_thread_id = %self.codex.session.thread_id,
                    "eager guardian compaction failed: {err}"
                );
                GuardianMaintenanceOutcome::DiscardSession
            }
            None => GuardianMaintenanceOutcome::DiscardSession,
        }
    }
}

fn snapshot_has_durable_compaction(
    snapshot: &GuardianReviewForkSnapshot,
    prior_rollout_item_count: usize,
) -> bool {
    let InitialHistory::Forked(items) = &snapshot.initial_history else {
        return false;
    };
    items.get(prior_rollout_item_count..).is_some_and(|items| {
        items.iter().any(|item| {
            matches!(
                item,
                RolloutItem::Compacted(compacted) if compacted.replacement_history.is_some()
            )
        })
    })
}
