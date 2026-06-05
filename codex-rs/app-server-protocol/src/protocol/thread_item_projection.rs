use crate::protocol::thread_history::ThreadHistoryState;
use crate::protocol::v2::CollabAgentToolCallStatus;
use crate::protocol::v2::CommandExecutionStatus;
use crate::protocol::v2::DynamicToolCallStatus;
use crate::protocol::v2::McpToolCallStatus;
use crate::protocol::v2::PatchApplyStatus;
use crate::protocol::v2::ThreadItem;
use crate::protocol::v2::Turn;
use codex_protocol::protocol::RolloutItem;
use codex_thread_store_protocol::ThreadHistoryMutation;
use codex_thread_store_protocol::ThreadHistoryMutationMetadata;
use codex_thread_store_protocol::ThreadHistoryProjectionObserver;
use codex_thread_store_protocol::ThreadItemMutation;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;

#[cfg(test)]
#[path = "thread_item_projection_tests.rs"]
mod tests;

/// Incremental thread-item materialization state that a durable store can persist between appends.
///
/// Persisting it lets the next append reuse stable item ordinals and update non-terminal
/// ThreadItems without replaying the full rollout history first.
///
/// TODO(wiltzius): Before stores persist or restore this checkpoint, include enough reducer state
/// to resume reasoning coalescing, complete open items from any turn, and tombstone rolled-back
/// closed items after restart.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ThreadItemProjectionCheckpoint {
    pub next_thread_item_ordinal: i64,
    /// Next fallback `item-N` ThreadItem id index used when the source event has no durable id.
    pub next_generated_thread_item_id_index: i64,
    pub current_turn: Option<CurrentTurnThreadItemState>,
    pub current_reasoning_item: Option<ReasoningAccumulator>,
    pub open_items: HashMap<String, OpenThreadItemState>,
}

/// Active turn fields needed by incremental thread item projection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CurrentTurnThreadItemState {
    pub turn_id: String,
}

/// Last reasoning item while reasoning deltas are still coalescing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReasoningAccumulator {
    pub turn_id: String,
    pub item_key: String,
}

/// Latest durable identity for an item that may receive a later upsert.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpenThreadItemState {
    pub turn_id: String,
    pub item_ordinal: i64,
}

/// Thread-item mutations emitted after applying one materialization batch.
#[derive(Clone, Debug)]
pub struct ThreadItemProjectionOutputBatch {
    pub updated_checkpoint: ThreadItemProjectionCheckpoint,
    pub thread_history_mutations: Vec<ThreadHistoryMutation>,
}

/// One raw append item reduced into before/after thread-history snapshots.
#[derive(Clone, Debug)]
struct ThreadItemProjectionObservation {
    before_turns: Vec<Turn>,
    after_turns: Vec<Turn>,
}

/// Incrementally applies the event-to-turn reduction needed by ThreadItem materialization.
struct ThreadItemProjectionReducer {
    state: ThreadHistoryState,
}

impl ThreadItemProjectionReducer {
    fn new() -> Self {
        Self {
            state: ThreadHistoryState::new(),
        }
    }

    fn materialized_turns(&self) -> Vec<Turn> {
        self.state.turns_snapshot()
    }

    fn reduce_append(
        &mut self,
        persisted_rollout_items: &[RolloutItem],
        projection_source_events: &[RolloutItem],
    ) -> Vec<ThreadItemProjectionObservation> {
        source_rollout_items(persisted_rollout_items, projection_source_events)
            .iter()
            .map(|item| {
                let before_turns = self.state.turns_snapshot();
                self.state.handle_rollout_item(item);
                ThreadItemProjectionObservation {
                    before_turns,
                    after_turns: self.state.turns_snapshot(),
                }
            })
            .collect()
    }
}

/// Observes append batches and emits only ThreadItem upserts and tombstones.
pub struct ThreadItemProjectionObserver {
    reducer: ThreadItemProjectionReducer,
    checkpoint: ThreadItemProjectionCheckpoint,
    item_ordinals: HashMap<String, i64>,
}

impl Default for ThreadItemProjectionObserver {
    fn default() -> Self {
        Self::new()
    }
}

impl ThreadItemProjectionObserver {
    pub fn new() -> Self {
        Self {
            reducer: ThreadItemProjectionReducer::new(),
            checkpoint: ThreadItemProjectionCheckpoint {
                next_thread_item_ordinal: 1,
                next_generated_thread_item_id_index: 1,
                ..Default::default()
            },
            item_ordinals: HashMap::new(),
        }
    }

    pub fn from_checkpoint(checkpoint: ThreadItemProjectionCheckpoint) -> Self {
        let checkpoint = ThreadItemProjectionCheckpoint {
            next_thread_item_ordinal: checkpoint.next_thread_item_ordinal.max(1),
            next_generated_thread_item_id_index: checkpoint
                .next_generated_thread_item_id_index
                .max(1),
            ..checkpoint
        };
        let item_ordinals = checkpoint
            .open_items
            .iter()
            .map(|(item_key, item)| (item_key.clone(), item.item_ordinal))
            .collect();
        let mut reducer = ThreadItemProjectionReducer::new();
        reducer
            .state
            .set_next_item_index(checkpoint.next_generated_thread_item_id_index);
        if let Some(current_turn) = checkpoint.current_turn.as_ref() {
            reducer
                .state
                .restore_current_turn(current_turn.turn_id.clone());
        }
        Self {
            reducer,
            checkpoint,
            item_ordinals,
        }
    }

    pub fn checkpoint(&self) -> &ThreadItemProjectionCheckpoint {
        &self.checkpoint
    }

    pub fn materialized_turns(&self) -> Vec<Turn> {
        self.reducer.materialized_turns()
    }

    pub fn observe_append(
        &mut self,
        persisted_rollout_items: &[RolloutItem],
        projection_source_events: &[RolloutItem],
    ) -> ThreadItemProjectionOutputBatch {
        let mut thread_history_mutations = Vec::new();
        for observation in self
            .reducer
            .reduce_append(persisted_rollout_items, projection_source_events)
        {
            self.emit_upserted_thread_items(&observation, &mut thread_history_mutations);
            self.emit_removed_thread_items(&observation, &mut thread_history_mutations);
        }
        self.refresh_checkpoint();
        ThreadItemProjectionOutputBatch {
            updated_checkpoint: self.checkpoint.clone(),
            thread_history_mutations,
        }
    }

    fn emit_upserted_thread_items(
        &mut self,
        observation: &ThreadItemProjectionObservation,
        mutations: &mut Vec<ThreadHistoryMutation>,
    ) {
        for turn in &observation.after_turns {
            let previous_turn = observation
                .before_turns
                .iter()
                .find(|previous| previous.id == turn.id);
            for item in &turn.items {
                let previous_item = previous_turn
                    .and_then(|previous| previous.items.iter().find(|old| old.id() == item.id()));
                if previous_item == Some(item) {
                    continue;
                }
                let item_key = item.id().to_string();
                let item_ordinal =
                    *self
                        .item_ordinals
                        .entry(item_key.clone())
                        .or_insert_with(|| {
                            let item_ordinal = self.checkpoint.next_thread_item_ordinal;
                            self.checkpoint.next_thread_item_ordinal += 1;
                            item_ordinal
                        });
                mutations.push(thread_item_mutation(json!({
                    "turnId": turn.id,
                    "itemKey": item_key,
                    "itemOrdinal": item_ordinal,
                    "mutationKind": "upsert",
                    "isOpen": is_open_item(item),
                    "materializedThreadItem": item,
                })));
            }
        }
    }

    fn emit_removed_thread_items(
        &self,
        observation: &ThreadItemProjectionObservation,
        mutations: &mut Vec<ThreadHistoryMutation>,
    ) {
        for turn in &observation.before_turns {
            let current_turn = observation
                .after_turns
                .iter()
                .find(|current| current.id == turn.id);
            for item in &turn.items {
                if current_turn
                    .and_then(|current| current.items.iter().find(|new| new.id() == item.id()))
                    .is_some()
                {
                    continue;
                }
                let item_key = item.id().to_string();
                let Some(item_ordinal) = self.item_ordinals.get(&item_key).copied() else {
                    continue;
                };
                mutations.push(thread_item_mutation(json!({
                    "turnId": turn.id,
                    "itemKey": item_key,
                    "itemOrdinal": item_ordinal,
                    "mutationKind": "tombstone",
                    "isOpen": false,
                    "materializedThreadItem": item,
                })));
            }
        }
    }

    fn refresh_checkpoint(&mut self) {
        let turns = self.reducer.materialized_turns();
        self.checkpoint.next_generated_thread_item_id_index = self.reducer.state.next_item_index();
        self.checkpoint.current_turn = self
            .reducer
            .state
            .current_turn_id()
            .map(|turn_id| CurrentTurnThreadItemState { turn_id });
        self.checkpoint.current_reasoning_item = turns.last().and_then(|turn| {
            let item = turn.items.last()?;
            matches!(item, ThreadItem::Reasoning { .. }).then(|| ReasoningAccumulator {
                turn_id: turn.id.clone(),
                item_key: item.id().to_string(),
            })
        });
        self.checkpoint.open_items = turns
            .iter()
            .flat_map(|turn| {
                turn.items.iter().filter_map(|item| {
                    if !is_open_item(item) {
                        return None;
                    }
                    let item_key = item.id().to_string();
                    let item_ordinal = self.item_ordinals.get(&item_key).copied()?;
                    Some((
                        item_key,
                        OpenThreadItemState {
                            turn_id: turn.id.clone(),
                            item_ordinal,
                        },
                    ))
                })
            })
            .collect();
    }
}

impl ThreadHistoryProjectionObserver for ThreadItemProjectionObserver {
    fn observe_append(
        &mut self,
        persisted_rollout_items: &[RolloutItem],
        projection_source_events: &[RolloutItem],
    ) -> Vec<ThreadHistoryMutation> {
        ThreadItemProjectionObserver::observe_append(
            self,
            persisted_rollout_items,
            projection_source_events,
        )
        .thread_history_mutations
    }
}

fn thread_item_mutation(payload: serde_json::Value) -> ThreadHistoryMutation {
    ThreadHistoryMutation::ThreadItem(ThreadItemMutation {
        metadata: mutation_metadata(),
        payload,
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

fn is_open_item(item: &ThreadItem) -> bool {
    match item {
        ThreadItem::CommandExecution { status, .. } => {
            matches!(status, CommandExecutionStatus::InProgress)
        }
        ThreadItem::McpToolCall { status, .. } => matches!(status, McpToolCallStatus::InProgress),
        ThreadItem::DynamicToolCall { status, .. } => {
            matches!(status, DynamicToolCallStatus::InProgress)
        }
        ThreadItem::CollabAgentToolCall { status, .. } => {
            matches!(status, CollabAgentToolCallStatus::InProgress)
        }
        ThreadItem::WebSearch { action, .. } => action.is_none(),
        ThreadItem::ImageGeneration { status, .. } => status.is_empty(),
        ThreadItem::FileChange { status, .. } => matches!(status, PatchApplyStatus::InProgress),
        ThreadItem::UserMessage { .. }
        | ThreadItem::HookPrompt { .. }
        | ThreadItem::AgentMessage { .. }
        | ThreadItem::Plan { .. }
        | ThreadItem::Reasoning { .. }
        | ThreadItem::ImageView { .. }
        | ThreadItem::EnteredReviewMode { .. }
        | ThreadItem::ExitedReviewMode { .. }
        | ThreadItem::ContextCompaction { .. } => false,
    }
}
