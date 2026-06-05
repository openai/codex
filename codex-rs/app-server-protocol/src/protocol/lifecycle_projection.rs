use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_thread_store_protocol::LifecycleMutation;
use codex_thread_store_protocol::ThreadHistoryMutation;
use codex_thread_store_protocol::ThreadHistoryMutationMetadata;
use codex_thread_store_protocol::ThreadHistoryProjectionObserver;
use serde_json::json;

#[cfg(test)]
#[path = "lifecycle_projection_tests.rs"]
mod tests;

/// Observes append batches and emits only thread lifecycle mutations.
pub struct LifecycleProjectionObserver;

impl Default for LifecycleProjectionObserver {
    fn default() -> Self {
        Self::new()
    }
}

impl LifecycleProjectionObserver {
    pub fn new() -> Self {
        Self
    }

    pub fn observe_append(
        &mut self,
        persisted_rollout_items: &[RolloutItem],
        projection_source_events: &[RolloutItem],
    ) -> Vec<ThreadHistoryMutation> {
        source_rollout_items(persisted_rollout_items, projection_source_events)
            .iter()
            .filter_map(lifecycle_mutation)
            .collect()
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

fn lifecycle_mutation(rollout_item: &RolloutItem) -> Option<ThreadHistoryMutation> {
    let RolloutItem::EventMsg(event) = rollout_item else {
        return None;
    };
    let (event_type, turn_id, payload) = match event {
        EventMsg::TurnStarted(payload) => (
            "turn.started",
            Some(payload.turn_id.clone()),
            json!({
                "startedAt": payload.started_at,
            }),
        ),
        EventMsg::TurnComplete(payload) => (
            "turn.completed",
            Some(payload.turn_id.clone()),
            json!({
                "completedAt": payload.completed_at,
                "durationMs": payload.duration_ms,
            }),
        ),
        EventMsg::TurnAborted(payload) => (
            "turn.cancelled",
            payload.turn_id.clone(),
            json!({
                "reason": payload.reason,
                "completedAt": payload.completed_at,
                "durationMs": payload.duration_ms,
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
