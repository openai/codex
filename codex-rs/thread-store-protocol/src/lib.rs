use codex_protocol::protocol::RolloutItem;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

/// Metadata shared by one derived thread-history mutation.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ThreadHistoryMutationMetadata {
    pub schema_version: u32,
}

/// One typed thread-item mutation emitted alongside a durable rollout append.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ThreadItemMutation {
    #[serde(flatten)]
    pub metadata: ThreadHistoryMutationMetadata,
    pub payload: Value,
}

/// One typed turn-summary mutation emitted alongside a durable rollout append.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TurnSummaryMutation {
    #[serde(flatten)]
    pub metadata: ThreadHistoryMutationMetadata,
    pub payload: Value,
}

/// One typed lifecycle mutation emitted alongside a durable rollout append.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LifecycleMutation {
    #[serde(flatten)]
    pub metadata: ThreadHistoryMutationMetadata,
    pub payload: Value,
}

/// Derived thread-history mutation emitted alongside a durable rollout append.
///
/// Store implementations match this finite union to route each mutation to its durable indexed
/// view. Payloads stay storage-neutral here because this crate cannot depend on app-server
/// `ThreadItem`; examples include thread-item materializations, turn summaries, and lifecycle
/// outbox rows derived from the same accepted rollout append.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ThreadHistoryMutation {
    ThreadItem(ThreadItemMutation),
    TurnSummary(TurnSummaryMutation),
    Lifecycle(LifecycleMutation),
}

/// Observes a live append batch and derives typed thread-history mutations for the store.
///
/// Implementations may materialize higher-level ThreadItem views from rollout items, but the
/// store boundary only receives storage-neutral mutation payloads. Examples include app-server
/// thread-item rows, turn summary rows, or lifecycle/search updates that should be committed
/// alongside the accepted canonical rollout append.
///
/// `persisted_rollout_items` are the new canonical items that the store will append.
/// `projection_source_events` are the same append's new unfiltered raw rollout items so observers
/// can see filtered events such as exec begin; open/non-terminal projected items belong in the
/// observer's checkpoint, not in this append argument.
pub trait ThreadHistoryProjectionObserver: Send {
    fn observe_append(
        &mut self,
        persisted_rollout_items: &[RolloutItem],
        projection_source_events: &[RolloutItem],
    ) -> Vec<ThreadHistoryMutation>;
}
