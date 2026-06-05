use crate::protocol::thread_history::ThreadHistoryState;
use crate::protocol::v2::ThreadItem;
use crate::protocol::v2::Turn;
use crate::protocol::v2::TurnStatus;
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
    reducer: ThreadHistoryState,
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
            reducer: ThreadHistoryState::new(),
            summaries: HashMap::new(),
        }
    }

    pub fn observe_append(
        &mut self,
        persisted_rollout_items: &[RolloutItem],
        projection_source_events: &[RolloutItem],
    ) -> Vec<ThreadHistoryMutation> {
        let mut mutations = Vec::new();
        for rollout_item in source_rollout_items(persisted_rollout_items, projection_source_events)
        {
            self.reducer.handle_rollout_item(rollout_item);
            let turns = self.reducer.turns_snapshot();
            self.summaries
                .retain(|turn_id, _summary| turns.iter().any(|turn| turn.id == *turn_id));
            for turn in turns {
                let summary = TurnSummaryState::from_turn(&turn);
                if self.summaries.get(&turn.id) == Some(&summary) {
                    continue;
                }
                self.summaries.insert(turn.id.clone(), summary.clone());
                mutations.push(turn_summary_mutation(&turn.id, &summary));
            }
        }
        mutations
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

#[derive(Clone, PartialEq)]
struct TurnSummaryState {
    status: TurnStatus,
    started_at: Option<i64>,
    completed_at: Option<i64>,
    duration_ms: Option<i64>,
    summary_items: Vec<ThreadItem>,
}

impl TurnSummaryState {
    fn from_turn(turn: &Turn) -> Self {
        let first_user_message = turn
            .items
            .iter()
            .find(|item| matches!(item, ThreadItem::UserMessage { .. }))
            .cloned();
        let final_agent_message = turn
            .items
            .iter()
            .rev()
            .find(|item| matches!(item, ThreadItem::AgentMessage { .. }))
            .cloned();
        let summary_items = match (first_user_message, final_agent_message) {
            (Some(user_message), Some(agent_message))
                if user_message.id() != agent_message.id() =>
            {
                vec![user_message, agent_message]
            }
            (Some(user_message), Some(_agent_message)) => vec![user_message],
            (Some(user_message), None) => vec![user_message],
            (None, Some(agent_message)) => vec![agent_message],
            (None, None) => Vec::new(),
        };
        Self {
            status: turn.status.clone(),
            started_at: turn.started_at,
            completed_at: turn.completed_at,
            duration_ms: turn.duration_ms,
            summary_items,
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
                "summaryItems": summary.summary_items,
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
