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

/// Requested amount of item detail for stored turns.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum StoredTurnItemsView {
    /// Return turn metadata only.
    NotLoaded,
    /// Return display summary items for each turn.
    #[default]
    Summary,
    /// Return every projected thread item available for each turn.
    Full,
}

/// Store-owned status for a persisted turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StoredTurnStatus {
    /// The turn completed normally.
    Completed,
    /// The turn was interrupted before normal completion.
    Interrupted,
    /// The turn failed.
    Failed,
    /// The turn is still in progress.
    InProgress,
}

/// Store-owned error details for a failed persisted turn.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredTurnError {
    /// User-visible error message.
    pub message: String,
    /// Optional additional detail for clients that expose expanded error context.
    pub additional_details: Option<String>,
}

/// Store-owned projected ThreadItem row.
///
/// The store keeps `item` storage-neutral because app-server owns the concrete `ThreadItem` type.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoredThreadItem {
    /// Turn that owns this projected item.
    pub turn_id: String,
    /// Stable projection key for upserting/tombstoning this item.
    pub item_key: String,
    /// Stable order of this item within the projected thread history.
    pub item_ordinal: i64,
    /// Whether the item may receive future updates.
    pub is_open: bool,
    /// Materialized app-server `ThreadItem` JSON.
    pub item: Value,
}

/// Store-owned turn representation used by turn pagination APIs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoredTurn {
    /// Turn id.
    pub turn_id: String,
    /// Projected thread items associated with this turn, according to `items_view`.
    pub items: Vec<StoredThreadItem>,
    /// Amount of item detail included in `items`.
    pub items_view: StoredTurnItemsView,
    /// Store-owned status for API layer projection.
    pub status: StoredTurnStatus,
    /// Error message when the turn failed.
    pub error: Option<StoredTurnError>,
    /// Unix timestamp (seconds) when the turn started.
    pub started_at: Option<i64>,
    /// Unix timestamp (seconds) when the turn completed.
    pub completed_at: Option<i64>,
    /// Duration between turn start and completion in milliseconds, if known.
    pub duration_ms: Option<i64>,
}

/// Store-owned current materialized thread-item projection state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoredThreadItemProjectionState {
    /// Current projected turns in turn order, without requiring app-server `ThreadItem` decoding.
    pub turns: Vec<StoredTurn>,
    /// Current non-tombstoned materialized ThreadItem rows in item ordinal order.
    pub items: Vec<StoredThreadItem>,
    /// Current active turn, if one exists.
    pub current_turn_id: Option<String>,
    pub next_thread_item_ordinal: i64,
    pub next_generated_thread_item_id_index: i64,
}

impl Default for StoredThreadItemProjectionState {
    fn default() -> Self {
        Self {
            turns: Vec::new(),
            items: Vec::new(),
            current_turn_id: None,
            next_thread_item_ordinal: 1,
            next_generated_thread_item_id_index: 1,
        }
    }
}

/// Store-owned current materialized turn-summary projection state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoredTurnSummaryProjectionState {
    pub turns: Vec<StoredTurn>,
    /// Current active turn, if one exists.
    pub current_turn_id: Option<String>,
    pub next_generated_thread_item_id_index: i64,
}

impl Default for StoredTurnSummaryProjectionState {
    fn default() -> Self {
        Self {
            turns: Vec::new(),
            current_turn_id: None,
            next_generated_thread_item_id_index: 1,
        }
    }
}

/// Store-owned current lifecycle projection state.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredLifecycleProjectionState {
    pub current_turn_id: Option<String>,
}

/// Store-owned current state for each thread-history projection observer.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StoredThreadHistoryProjectionState {
    pub thread_items: StoredThreadItemProjectionState,
    pub turn_summaries: StoredTurnSummaryProjectionState,
    pub lifecycle: StoredLifecycleProjectionState,
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
