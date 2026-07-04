//! Reconciles a loaded running session with its authoritative persisted rollout.

mod cursor;
mod install;

pub(super) use self::install::AutoCompactWindowInstallMode;
pub(super) use self::install::RolloutHistoryInstallMode;
pub(super) use self::install::RolloutReconstructionInstallOptions;
pub(super) use self::install::history_contains_token_budget_reminder;

use self::cursor::PersistedCursorComparison;
use self::cursor::advance_persisted_history_cursor;
pub(super) use self::cursor::empty_persisted_history_cursor;
use self::cursor::is_persisted_history_rewrite_item;
pub(super) use self::cursor::persisted_history_cursor;
use self::cursor::persisted_suffix_after_cursor;
use super::session::Session;
use crate::codex_thread::ThreadHistoryReconciliationOutcome;
use crate::codex_thread::ThreadHistoryReconciliationSnapshot;
use crate::context_manager::ContextManager;
use crate::image_preparation::prepare_response_items;
use crate::state::PersistedHistoryCursorState;
use crate::state::PersistedHistoryCursorUncertainty;
use codex_app_server_protocol::ThreadHistoryBuilder;
use codex_app_server_protocol::TurnStatus;
use codex_protocol::config_types::AutoCompactTokenLimitScope;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::TruncationPolicy;
use std::sync::Arc;

pub(super) struct HistoryReconciliationConfig {
    pub(super) truncation_policy: TruncationPolicy,
    pub(super) auto_compact_token_limit_scope: AutoCompactTokenLimitScope,
}

impl Session {
    pub(crate) async fn acquire_history_persistence_lock(
        &self,
    ) -> tokio::sync::OwnedMutexGuard<()> {
        Arc::clone(&self.history_persistence_lock)
            .lock_owned()
            .await
    }

    pub(crate) async fn acquire_rollout_persistence_lock(
        &self,
    ) -> tokio::sync::OwnedMutexGuard<()> {
        Arc::clone(&self.rollout_persistence_lock)
            .lock_owned()
            .await
    }

    pub(super) async fn acquire_event_delivery_lock(&self) -> tokio::sync::OwnedMutexGuard<()> {
        Arc::clone(&self.event_delivery_lock).lock_owned().await
    }

    pub(crate) async fn acquire_history_reconciliation_event_cut(
        &self,
    ) -> (
        tokio::sync::OwnedMutexGuard<()>,
        tokio::sync::OwnedMutexGuard<()>,
        tokio::sync::OwnedMutexGuard<()>,
    ) {
        // History mutations can emit events while holding the history lock, so preserve the
        // global history -> event delivery -> rollout lock order.
        let history_guard = self.acquire_history_persistence_lock().await;
        let event_delivery_guard = self.acquire_event_delivery_lock().await;
        let rollout_guard = self.acquire_rollout_persistence_lock().await;
        (history_guard, event_delivery_guard, rollout_guard)
    }

    pub(crate) async fn note_persisted_non_metadata_items(
        &self,
        _rollout_guard: &tokio::sync::OwnedMutexGuard<()>,
        items: &[RolloutItem],
    ) {
        let mut state = self.state.lock().await;
        state.persisted_history_cursor = match state.persisted_history_cursor {
            PersistedHistoryCursorState::Known(cursor) => {
                advance_persisted_history_cursor(cursor, items).map_or(
                    PersistedHistoryCursorState::Unknown,
                    PersistedHistoryCursorState::Known,
                )
            }
            PersistedHistoryCursorState::Uncertain {
                uncertainty,
                expected,
            } => PersistedHistoryCursorState::Uncertain {
                uncertainty,
                expected: expected
                    .and_then(|cursor| advance_persisted_history_cursor(cursor, items)),
            },
            PersistedHistoryCursorState::Unknown => PersistedHistoryCursorState::Unknown,
        };
    }

    pub(crate) async fn invalidate_persisted_item_cursor(
        &self,
        _rollout_guard: &tokio::sync::OwnedMutexGuard<()>,
        items: &[RolloutItem],
    ) {
        let mut state = self.state.lock().await;
        let expected = match state.persisted_history_cursor {
            PersistedHistoryCursorState::Known(cursor) => Some(cursor),
            PersistedHistoryCursorState::Uncertain { expected, .. } => expected,
            PersistedHistoryCursorState::Unknown => None,
        }
        .and_then(|cursor| advance_persisted_history_cursor(cursor, items));
        let uncertainty = if items.iter().any(is_persisted_history_rewrite_item)
            || matches!(
                state.persisted_history_cursor,
                PersistedHistoryCursorState::Uncertain {
                    uncertainty: PersistedHistoryCursorUncertainty::HistoryRewrite,
                    ..
                }
            ) {
            PersistedHistoryCursorUncertainty::HistoryRewrite
        } else {
            PersistedHistoryCursorUncertainty::AppendOnly
        };
        state.persisted_history_cursor = PersistedHistoryCursorState::Uncertain {
            uncertainty,
            expected,
        };
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "history snapshots must stay atomic with the active-turn check"
    )]
    pub(crate) async fn history_reconciliation_snapshot(
        &self,
    ) -> Option<ThreadHistoryReconciliationSnapshot> {
        let active_turn = self.active_turn.lock().await;
        if active_turn.is_some() {
            return None;
        }
        let state = self.state.lock().await;
        Some(ThreadHistoryReconciliationSnapshot {
            history: state.history.raw_items().to_vec(),
            history_version: state.history.history_version(),
            known_persisted_incomplete_tail: state.known_persisted_incomplete_tail(),
            persisted_history_cursor: state.persisted_history_cursor,
        })
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "history replacement must stay atomic with the final active-turn check"
    )]
    pub(crate) async fn reconcile_persisted_history(
        &self,
        snapshot: ThreadHistoryReconciliationSnapshot,
        rollout_items: &[RolloutItem],
    ) -> ThreadHistoryReconciliationOutcome {
        let persisted_incomplete_tail = persisted_rollout_incomplete_turn_id(rollout_items);
        {
            let active_turn = self.active_turn.lock().await;
            if active_turn.is_some() {
                return ThreadHistoryReconciliationOutcome::Busy;
            }
        }
        // Incremental replay is safe only when the loaded rollout still has the exact known
        // non-metadata prefix. A mismatch means another writer interleaved before a local append,
        // so fall back to canonical reconstruction instead of duplicating already-applied items.
        let (reconciliation_cursor, requires_uncertain_cursor_proof) =
            match snapshot.persisted_history_cursor {
                PersistedHistoryCursorState::Known(cursor) => (Some(cursor), false),
                PersistedHistoryCursorState::Uncertain {
                    expected: Some(expected),
                    ..
                } => (Some(expected), true),
                PersistedHistoryCursorState::Uncertain {
                    uncertainty: PersistedHistoryCursorUncertainty::HistoryRewrite,
                    expected: None,
                } => {
                    // Without a pre-failure cursor, model-history equality cannot prove that an
                    // event-only rollback or compaction marker became durable. Restart is the
                    // recovery boundary for this deliberately fail-closed state.
                    return ThreadHistoryReconciliationOutcome::Conflict;
                }
                PersistedHistoryCursorState::Uncertain {
                    uncertainty: PersistedHistoryCursorUncertainty::AppendOnly,
                    expected: None,
                }
                | PersistedHistoryCursorState::Unknown => (None, false),
            };
        let persisted_suffix = match reconciliation_cursor {
            Some(cursor) => match persisted_suffix_after_cursor(rollout_items, cursor) {
                PersistedCursorComparison::Matched(suffix) => Some(suffix),
                PersistedCursorComparison::Mismatched => {
                    if requires_uncertain_cursor_proof {
                        return ThreadHistoryReconciliationOutcome::Conflict;
                    }
                    None
                }
                PersistedCursorComparison::Shorter => {
                    return ThreadHistoryReconciliationOutcome::Conflict;
                }
            },
            None => None,
        };
        let uncertainty_proven_by_cursor =
            requires_uncertain_cursor_proof && persisted_suffix.is_some();
        let imports_compaction = persisted_suffix.is_some_and(|suffix| {
            suffix
                .iter()
                .any(|item| matches!(item, RolloutItem::Compacted(_)))
        });
        let imports_rollback = persisted_suffix.is_some_and(|suffix| {
            suffix
                .iter()
                .any(|item| matches!(item, RolloutItem::EventMsg(EventMsg::ThreadRolledBack(_))))
        });
        let imports_history_rewrite = imports_compaction || imports_rollback;

        let reconciliation_config = self.history_reconciliation_config().await;
        let mut reconstruction = self
            .reconstruct_history_from_rollout_with_policy(
                reconciliation_config.truncation_policy,
                rollout_items,
            )
            .await;
        prepare_response_items(&mut reconstruction.history);
        if let Some(persisted_suffix) = persisted_suffix
            && let Some(history) = reconcile_model_history_from_persisted_suffix(
                &snapshot.history,
                persisted_suffix,
                reconciliation_config.truncation_policy,
            )
        {
            reconstruction.history = history;
        }
        let final_history_extends_snapshot = reconstruction.history.starts_with(&snapshot.history);
        if matches!(
            snapshot.persisted_history_cursor,
            PersistedHistoryCursorState::Uncertain {
                uncertainty: PersistedHistoryCursorUncertainty::AppendOnly,
                ..
            }
        ) && !uncertainty_proven_by_cursor
            && !final_history_extends_snapshot
        {
            // An append error does not reveal whether storage accepted the item. Never let an
            // older or divergent read replace authoritative in-memory history in that state.
            // Exact cursor agreement proves that the ambiguous append landed; without it,
            // only exact model-history matches and strict extensions provide that proof.
            return ThreadHistoryReconciliationOutcome::Conflict;
        }
        let imports_effective_rollback = persisted_suffix.map_or_else(
            || {
                rollout_items.iter().any(|item| {
                    matches!(item, RolloutItem::EventMsg(EventMsg::ThreadRolledBack(_)))
                }) && !final_history_extends_snapshot
            },
            |_| imports_rollback,
        );
        let token_info = Self::last_token_info_from_rollout(rollout_items);
        let token_budget_reminder_delivered =
            history_contains_token_budget_reminder(&reconstruction.history);

        let active_turn = self.active_turn.lock().await;
        if active_turn.is_some() {
            return ThreadHistoryReconciliationOutcome::Busy;
        }
        let mut state = self.state.lock().await;
        if state.history.history_version() != snapshot.history_version
            || state.history.raw_items() != snapshot.history
            || state.known_persisted_incomplete_tail().as_ref()
                != snapshot.known_persisted_incomplete_tail.as_ref()
            || state.persisted_history_cursor != snapshot.persisted_history_cursor
        {
            return ThreadHistoryReconciliationOutcome::Conflict;
        }

        if persisted_incomplete_tail.is_some()
            && (persisted_incomplete_tail.as_ref()
                != snapshot.known_persisted_incomplete_tail.as_ref()
                || state.history.raw_items() != reconstruction.history)
        {
            return ThreadHistoryReconciliationOutcome::Incomplete;
        }

        let outcome = if state.history.raw_items() == reconstruction.history {
            ThreadHistoryReconciliationOutcome::Unchanged
        } else {
            ThreadHistoryReconciliationOutcome::Refreshed
        };
        let fallback_window_ids = state.auto_compact_window_ids();
        let reconstructed_window_ids = Self::rollout_reconstruction_window_ids(
            reconstruction.first_window_id,
            reconstruction.previous_window_id,
            reconstruction.window_id,
            fallback_window_ids,
        );
        let preserves_auto_compact_window = reconstruction.window_number
            == state.auto_compact_window_number()
            && reconstructed_window_ids == fallback_window_ids;
        let imports_unconsumed_compaction = persisted_suffix.map_or_else(
            || {
                // A cursor mismatch forces canonical reconstruction, so there is no trustworthy
                // suffix. A changed compaction-window identity still proves that the newest
                // surviving compaction was imported. The newly installed full cursor makes this
                // fallback one-shot on the next reconciliation.
                !preserves_auto_compact_window && rollout_has_unconsumed_compaction(rollout_items)
            },
            rollout_has_unconsumed_compaction,
        );
        let preserves_append_only_prefill = final_history_extends_snapshot
            && !imports_history_rewrite
            && (persisted_suffix.is_some()
                || matches!(
                    snapshot.persisted_history_cursor,
                    PersistedHistoryCursorState::Uncertain { .. }
                ));
        let preserves_rollback_prefill =
            imports_effective_rollback && !imports_compaction && preserves_auto_compact_window;
        let preserve_auto_compact_prefill =
            matches!(outcome, ThreadHistoryReconciliationOutcome::Refreshed)
                && (preserves_append_only_prefill || preserves_rollback_prefill);
        let history_install = if matches!(outcome, ThreadHistoryReconciliationOutcome::Refreshed) {
            if preserve_auto_compact_prefill {
                RolloutHistoryInstallMode::ReplacePreservingAutoCompactPrefill
            } else {
                RolloutHistoryInstallMode::Replace
            }
        } else {
            RolloutHistoryInstallMode::KeepExisting
        };
        Self::install_rollout_reconstruction(
            &mut state,
            reconciliation_config.auto_compact_token_limit_scope,
            reconstruction,
            RolloutReconstructionInstallOptions {
                token_info,
                history: history_install,
                auto_compact_window: AutoCompactWindowInstallMode::Reconcile,
                token_budget_reminder_delivered,
            },
        );
        if imports_unconsumed_compaction {
            // Compaction queues this lifecycle source locally after replacing history. Mirror that
            // transition only when no later turn has already consumed it in the other process.
            state.queue_pending_session_start_source(codex_hooks::SessionStartSource::Compact);
        }
        state.set_known_persisted_incomplete_tail(persisted_incomplete_tail);
        if imports_history_rewrite
            || matches!(outcome, ThreadHistoryReconciliationOutcome::Refreshed)
        {
            state.reset_additional_context();
        }
        if !matches!(
            snapshot.persisted_history_cursor,
            PersistedHistoryCursorState::Unknown
        ) || self.live_thread().is_some()
        {
            state.set_known_persisted_history_cursor(persisted_history_cursor(rollout_items));
        }
        drop(state);
        if let Some(live_thread) = self.live_thread() {
            live_thread.seed_metadata_from_history(rollout_items).await;
        }
        if imports_effective_rollback {
            self.services
                .agent_control
                .rollout_budget()
                .rearm_reminder(self.thread_id());
        }
        drop(active_turn);
        outcome
    }
}

fn rollout_has_unconsumed_compaction(items: &[RolloutItem]) -> bool {
    items
        .iter()
        .rposition(|item| matches!(item, RolloutItem::Compacted(_)))
        .is_some_and(|compaction_index| {
            !items[compaction_index + 1..]
                .iter()
                .any(|item| matches!(item, RolloutItem::EventMsg(EventMsg::TurnStarted(_))))
        })
}

pub(super) fn persisted_rollout_incomplete_turn_id(
    rollout_items: &[RolloutItem],
) -> Option<String> {
    let mut history = ThreadHistoryBuilder::new();
    for item in rollout_items {
        history.handle_rollout_item(item);
    }
    if !history.has_active_turn() {
        return None;
    }
    let turn = history.active_turn_snapshot()?;
    matches!(turn.status, TurnStatus::InProgress | TurnStatus::Failed).then_some(turn.id)
}

fn reconcile_model_history_from_persisted_suffix(
    snapshot_history: &[ResponseItem],
    persisted_suffix: &[RolloutItem],
    truncation_policy: TruncationPolicy,
) -> Option<Vec<ResponseItem>> {
    if persisted_suffix
        .iter()
        .any(|item| matches!(item, RolloutItem::Compacted(_)))
    {
        return None;
    }

    let mut history = ContextManager::new();
    history.replace(snapshot_history.to_vec());
    for item in persisted_suffix {
        match item {
            RolloutItem::ResponseItem(response_item) => {
                let mut response_items = vec![response_item.clone()];
                prepare_response_items(&mut response_items);
                history.record_items(response_items.iter(), truncation_policy);
            }
            RolloutItem::InterAgentCommunication(communication) => {
                let mut response_items = vec![communication.to_model_input_item()];
                prepare_response_items(&mut response_items);
                history.record_items(response_items.iter(), truncation_policy);
            }
            RolloutItem::EventMsg(EventMsg::ThreadRolledBack(rollback)) => {
                history.drop_last_n_user_turns(rollback.num_turns);
            }
            RolloutItem::InterAgentCommunicationMetadata { .. }
            | RolloutItem::EventMsg(_)
            | RolloutItem::TurnContext(_)
            | RolloutItem::WorldState(_)
            | RolloutItem::SessionMeta(_) => {}
            RolloutItem::Compacted(_) => unreachable!("compaction was rejected above"),
        }
    }
    Some(history.raw_items().to_vec())
}
