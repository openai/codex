use crate::protocol::v2::TurnStatus;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_thread_store_protocol::ThreadHistoryMutation;
use codex_thread_store_protocol::ThreadHistoryMutationMetadata;
use codex_thread_store_protocol::ThreadHistoryProjectionObserver;
use codex_thread_store_protocol::TurnSummaryMutation;
use serde_json::json;
use std::collections::HashMap;

#[cfg(test)]
#[path = "turn_summary_projection_tests.rs"]
mod tests;

/// Observes append batches and emits only turn-summary mutations.
pub struct TurnSummaryProjectionObserver {
    current_turn_id: Option<String>,
    turn_order: Vec<String>,
    summaries: HashMap<String, TurnSummaryState>,
}

impl Default for TurnSummaryProjectionObserver {
    fn default() -> Self {
        Self::new()
    }
}

impl TurnSummaryProjectionObserver {
    pub fn new() -> Self {
        Self {
            current_turn_id: None,
            turn_order: Vec::new(),
            summaries: HashMap::new(),
        }
    }

    pub fn observe_append(
        &mut self,
        persisted_rollout_items: &[RolloutItem],
        projection_source_events: &[RolloutItem],
    ) -> Vec<ThreadHistoryMutation> {
        source_rollout_items(persisted_rollout_items, projection_source_events)
            .iter()
            .filter_map(|rollout_item| self.observe_item(rollout_item))
            .collect()
    }

    fn observe_item(&mut self, rollout_item: &RolloutItem) -> Option<ThreadHistoryMutation> {
        let RolloutItem::EventMsg(event) = rollout_item else {
            return None;
        };
        let (turn_id, summary) = match event {
            EventMsg::TurnStarted(payload) => {
                let summary = TurnSummaryState {
                    status: TurnStatus::InProgress,
                    started_at: payload.started_at,
                    completed_at: None,
                    duration_ms: None,
                };
                self.upsert_summary(payload.turn_id.clone(), summary.clone());
                self.current_turn_id = Some(payload.turn_id.clone());
                (payload.turn_id.clone(), summary)
            }
            EventMsg::TurnComplete(payload) => {
                let turn_id = self
                    .summaries
                    .contains_key(&payload.turn_id)
                    .then(|| payload.turn_id.clone())
                    .or_else(|| self.current_turn_id.clone())?;
                let summary = self.update_summary(&turn_id, |summary| {
                    if matches!(
                        summary.status,
                        TurnStatus::Completed | TurnStatus::InProgress
                    ) {
                        summary.status = TurnStatus::Completed;
                    }
                    summary.completed_at = payload.completed_at;
                    summary.duration_ms = payload.duration_ms;
                });
                if self.current_turn_id.as_deref() == Some(turn_id.as_str()) {
                    self.current_turn_id = None;
                }
                (turn_id, summary)
            }
            EventMsg::TurnAborted(payload) => {
                let turn_id = payload
                    .turn_id
                    .clone()
                    .filter(|turn_id| self.summaries.contains_key(turn_id))
                    .or_else(|| self.current_turn_id.clone())?;
                let summary = self.update_summary(&turn_id, |summary| {
                    summary.status = TurnStatus::Interrupted;
                    summary.completed_at = payload.completed_at;
                    summary.duration_ms = payload.duration_ms;
                });
                (turn_id, summary)
            }
            EventMsg::Error(payload) if payload.affects_turn_status() => {
                let turn_id = self.current_turn_id.clone()?;
                let summary = self.update_summary(&turn_id, |summary| {
                    summary.status = TurnStatus::Failed;
                });
                (turn_id, summary)
            }
            EventMsg::ThreadRolledBack(payload) => {
                self.roll_back(payload.num_turns);
                return None;
            }
            _ => return None,
        };
        Some(turn_summary_mutation(&turn_id, &summary))
    }

    fn upsert_summary(&mut self, turn_id: String, summary: TurnSummaryState) {
        if !self.summaries.contains_key(&turn_id) {
            self.turn_order.push(turn_id.clone());
        }
        self.summaries.insert(turn_id, summary);
    }

    fn update_summary(
        &mut self,
        turn_id: &str,
        update: impl FnOnce(&mut TurnSummaryState),
    ) -> TurnSummaryState {
        let summary = self.summaries.entry(turn_id.to_string()).or_default();
        update(summary);
        summary.clone()
    }

    fn roll_back(&mut self, num_turns: u32) {
        let num_turns = usize::try_from(num_turns).unwrap_or(usize::MAX);
        for _ in 0..num_turns.min(self.turn_order.len()) {
            let Some(turn_id) = self.turn_order.pop() else {
                return;
            };
            self.summaries.remove(&turn_id);
            if self.current_turn_id.as_deref() == Some(turn_id.as_str()) {
                self.current_turn_id = None;
            }
        }
    }
}

impl ThreadHistoryProjectionObserver for TurnSummaryProjectionObserver {
    fn observe_append(
        &mut self,
        persisted_rollout_items: &[RolloutItem],
        projection_source_events: &[RolloutItem],
    ) -> Vec<ThreadHistoryMutation> {
        TurnSummaryProjectionObserver::observe_append(
            self,
            persisted_rollout_items,
            projection_source_events,
        )
    }
}

#[derive(Clone)]
struct TurnSummaryState {
    status: TurnStatus,
    started_at: Option<i64>,
    completed_at: Option<i64>,
    duration_ms: Option<i64>,
}

impl Default for TurnSummaryState {
    fn default() -> Self {
        Self {
            status: TurnStatus::Completed,
            started_at: None,
            completed_at: None,
            duration_ms: None,
        }
    }
}

fn turn_summary_mutation(turn_id: &str, summary: &TurnSummaryState) -> ThreadHistoryMutation {
    ThreadHistoryMutation::TurnSummary(TurnSummaryMutation {
        metadata: mutation_metadata(),
        payload: json!({
            "turnId": turn_id,
            "mutation": {
                "status": summary.status,
                "startedAt": summary.started_at,
                "completedAt": summary.completed_at,
                "durationMs": summary.duration_ms,
            },
        }),
    })
}

fn mutation_metadata() -> ThreadHistoryMutationMetadata {
    ThreadHistoryMutationMetadata { schema_version: 1 }
}

fn source_rollout_items<'a>(
    persisted_rollout_items: &'a [RolloutItem],
    projection_source_events: &'a [RolloutItem],
) -> &'a [RolloutItem] {
    if projection_source_events.is_empty() {
        persisted_rollout_items
    } else {
        projection_source_events
    }
}
