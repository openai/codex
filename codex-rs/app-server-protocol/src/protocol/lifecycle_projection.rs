use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_thread_store_protocol::LifecycleMutation;
use codex_thread_store_protocol::StoredLifecycleProjectionState;
use codex_thread_store_protocol::ThreadHistoryMutation;
use codex_thread_store_protocol::ThreadHistoryMutationMetadata;
use codex_thread_store_protocol::ThreadHistoryProjectionObserver;
use serde_json::json;

#[cfg(test)]
#[path = "lifecycle_projection_tests.rs"]
mod tests;

/// Observes append batches and emits only thread lifecycle mutations.
pub struct LifecycleProjectionObserver {
    current_turn_id: Option<String>,
}

impl Default for LifecycleProjectionObserver {
    fn default() -> Self {
        Self::new()
    }
}

impl LifecycleProjectionObserver {
    pub fn new() -> Self {
        Self {
            current_turn_id: None,
        }
    }

    pub fn from_stored_state(state: &StoredLifecycleProjectionState) -> Self {
        Self {
            current_turn_id: state.current_turn_id.clone(),
        }
    }

    pub fn observe_append(
        &mut self,
        persisted_rollout_items: &[RolloutItem],
        projection_source_events: &[RolloutItem],
    ) -> Vec<ThreadHistoryMutation> {
        source_rollout_items(persisted_rollout_items, projection_source_events)
            .iter()
            .filter_map(|rollout_item| self.lifecycle_mutation(rollout_item))
            .collect()
    }

    fn lifecycle_mutation(&mut self, rollout_item: &RolloutItem) -> Option<ThreadHistoryMutation> {
        let RolloutItem::EventMsg(event) = rollout_item else {
            return None;
        };
        let (event_type, turn_id, payload) = match event {
            EventMsg::TurnStarted(payload) => {
                self.current_turn_id = Some(payload.turn_id.clone());
                (
                    "turn.started",
                    Some(payload.turn_id.clone()),
                    json!({
                        "startedAt": payload.started_at,
                    }),
                )
            }
            EventMsg::TurnComplete(payload) => {
                self.clear_current_turn_if_matches(&payload.turn_id);
                (
                    "turn.completed",
                    Some(payload.turn_id.clone()),
                    json!({
                        "completedAt": payload.completed_at,
                        "durationMs": payload.duration_ms,
                    }),
                )
            }
            EventMsg::TurnAborted(payload) => {
                let turn_id = payload
                    .turn_id
                    .clone()
                    .or_else(|| self.current_turn_id.clone());
                if let Some(turn_id) = &turn_id {
                    self.clear_current_turn_if_matches(turn_id);
                }
                (
                    "turn.cancelled",
                    turn_id,
                    json!({
                        "reason": payload.reason,
                        "completedAt": payload.completed_at,
                        "durationMs": payload.duration_ms,
                    }),
                )
            }
            EventMsg::Error(payload) if payload.affects_turn_status() => (
                "turn.failed",
                Some(self.current_turn_id.clone()?),
                json!({
                    "message": payload.message,
                    "codexErrorInfo": payload.codex_error_info,
                }),
            ),
            EventMsg::ThreadRolledBack(payload) => (
                "thread.rolled_back",
                None,
                json!({
                    "numTurns": payload.num_turns,
                }),
            ),
            _ => return None,
        };
        Some(ThreadHistoryMutation::Lifecycle(LifecycleMutation {
            metadata: mutation_metadata(),
            payload: json!({
                "eventType": event_type,
                "turnId": turn_id,
                "payload": payload,
            }),
        }))
    }

    fn clear_current_turn_if_matches(&mut self, turn_id: &str) {
        if self.current_turn_id.as_deref() == Some(turn_id) {
            self.current_turn_id = None;
        }
    }
}

impl ThreadHistoryProjectionObserver for LifecycleProjectionObserver {
    fn observe_append(
        &mut self,
        persisted_rollout_items: &[RolloutItem],
        projection_source_events: &[RolloutItem],
    ) -> Vec<ThreadHistoryMutation> {
        LifecycleProjectionObserver::observe_append(
            self,
            persisted_rollout_items,
            projection_source_events,
        )
    }
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
