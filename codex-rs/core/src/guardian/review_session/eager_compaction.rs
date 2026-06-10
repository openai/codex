use std::sync::Arc;

use codex_protocol::protocol::CompactedItem;
use tokio::sync::Mutex;
use tokio::sync::OwnedMutexGuard;
use tracing::warn;

use crate::guardian::GUARDIAN_REVIEW_TIMEOUT;
use crate::session::turn;

use super::GuardianReviewSession;
use super::GuardianReviewSessionParams;
use super::InitialHistory;
use super::RolloutItem;
use super::spawn_guardian_review_session;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum GuardianEagerCompactionOutcome {
    #[default]
    Reusable,
    DiscardSession,
}

/// The held guard serializes review ownership with eager maintenance.
pub(super) type GuardianEagerCompaction = Arc<Mutex<GuardianEagerCompactionOutcome>>;

impl GuardianReviewSession {
    pub(super) async fn schedule_eager_compaction(
        self: &Arc<Self>,
        params: GuardianReviewSessionParams,
    ) {
        let turn_context = self.codex.session.new_default_turn().await;
        if !turn::auto_compact_needed(self.codex.session.as_ref(), turn_context.as_ref()).await {
            return;
        }
        let mut outcome_guard = Arc::clone(&self.eager_compaction).lock_owned().await;
        *outcome_guard = GuardianEagerCompactionOutcome::DiscardSession;

        let review_session = Arc::clone(self);
        drop(tokio::spawn(async move {
            if review_session
                .run_eager_compaction(turn_context, params)
                .await
            {
                *outcome_guard = GuardianEagerCompactionOutcome::Reusable;
            }
        }));
    }

    pub(super) async fn wait_for_eager_compaction(
        &self,
    ) -> OwnedMutexGuard<GuardianEagerCompactionOutcome> {
        Arc::clone(&self.eager_compaction).lock_owned().await
    }

    async fn run_eager_compaction(
        self: &Arc<Self>,
        turn_context: Arc<crate::session::turn_context::TurnContext>,
        params: GuardianReviewSessionParams,
    ) -> bool {
        let Ok(_review_guard) = self.review_lock.acquire().await else {
            return false;
        };
        let Some(fork_snapshot) = self.fork_snapshot().await else {
            return false;
        };
        let InitialHistory::Forked(items) = &fork_snapshot.initial_history else {
            return false;
        };
        let rollout_item_count = items.len();
        let shadow_cancel = self.cancel_token.child_token();
        let mut shadow_config = params.spawn_config.clone();
        shadow_config.ephemeral = true;
        let shadow = match spawn_guardian_review_session(
            &params,
            shadow_config,
            self.reuse_key.clone(),
            shadow_cancel.clone(),
            Some(fork_snapshot),
        )
        .await
        {
            Ok(shadow) => Arc::new(shadow),
            Err(err) => {
                warn!(
                    guardian_thread_id = %self.codex.session.thread_id,
                    "failed to spawn eager guardian compaction session: {err}"
                );
                return false;
            }
        };
        let shadow_for_events = Arc::clone(&shadow);
        let event_drain =
            tokio::spawn(
                async move { while shadow_for_events.codex.next_event().await.is_ok() {} },
            );

        let shadow_turn = shadow.codex.session.new_default_turn().await;
        let mut client_session = shadow.codex.session.services.model_client.new_session();
        let compact_result = tokio::select! {
            _ = self.cancel_token.cancelled() => None,
            result = tokio::time::timeout(
                GUARDIAN_REVIEW_TIMEOUT,
                turn::run_pre_turn_auto_compact(
                    &shadow.codex.session,
                    &shadow_turn,
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
        let reusable = match compact_result {
            Some(Ok(())) => {
                let history = shadow
                    .codex
                    .session
                    .clone_history()
                    .await
                    .raw_items()
                    .to_vec();
                let compacted_item = CompactedItem {
                    message: String::new(),
                    replacement_history: Some(history.clone()),
                };
                self.codex
                    .session
                    .replace_compacted_history(
                        history,
                        shadow.codex.session.reference_context_item().await,
                        compacted_item,
                    )
                    .await;
                self.codex
                    .session
                    .recompute_token_usage(&turn_context)
                    .await;
                let durable = self
                    .refresh_last_committed_fork_snapshot()
                    .await
                    .is_some_and(|snapshot| {
                        let InitialHistory::Forked(items) = snapshot.initial_history else {
                            return false;
                        };
                        items.get(rollout_item_count..).is_some_and(|items| {
                            items.iter().any(|item| {
                                matches!(
                                    item,
                                    RolloutItem::Compacted(compacted)
                                        if compacted.replacement_history.is_some()
                                )
                            })
                        })
                    });
                if !durable {
                    warn!(
                        guardian_thread_id = %self.codex.session.thread_id,
                        "eager guardian compaction was not durably published"
                    );
                }
                durable
            }
            Some(Err(err)) => {
                warn!(
                    guardian_thread_id = %self.codex.session.thread_id,
                    "eager guardian compaction failed: {err}"
                );
                false
            }
            None => false,
        };
        shadow_cancel.cancel();
        shadow.shutdown().await;
        let _ = event_drain.await;
        reusable
    }
}
