//! Persisted realtime history reads, independent of turn pagination or a live runtime.

use super::ThreadRealtimeItem;
use crate::JsonSchema;
use crate::TS;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeListParams {
    pub thread_id: String,
    #[ts(optional = nullable)]
    pub session_id: Option<String>,
    /// Inclusive canonical observation time lower bound.
    #[ts(optional = nullable, type = "number | null")]
    pub from_observed_at_ms: Option<i64>,
    /// Exclusive canonical observation time upper bound.
    #[ts(optional = nullable, type = "number | null")]
    pub to_observed_at_ms: Option<i64>,
    /// Defaults to canonical observation time.
    #[ts(optional = nullable)]
    pub order: Option<ThreadRealtimeOrder>,
    #[ts(optional = nullable, type = "number | null")]
    pub after_position: Option<u64>,
    /// A required exact event ID, such as a completed item's acknowledgement.
    /// Returns pending until projected. Explicit filters can exclude a known event.
    #[ts(optional = nullable)]
    pub event_id: Option<String>,
    #[ts(optional = nullable)]
    pub cursor: Option<String>,
    #[ts(optional = nullable)]
    pub limit: Option<u32>,
    /// Exclusive committed position delimiting an immutable snapshot.
    #[ts(optional = nullable, type = "number | null")]
    pub snapshot_end: Option<u64>,
    /// Return pending until the projection includes this committed position.
    #[ts(optional = nullable, type = "number | null")]
    pub required_through_position: Option<u64>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase", export_to = "v2/")]
pub enum ThreadRealtimeOrder {
    /// Sort by canonical observation time, then committed position.
    #[default]
    ObservedAt,
    /// Sort by committed position, including events sealed after older snapshots.
    Position,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase", export_to = "v2/")]
pub enum ThreadRealtimeCoverage {
    Unsupported,
    Pending,
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeEntry {
    #[ts(type = "number")]
    pub position: u64,
    pub item: ThreadRealtimeItem,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ThreadRealtimeListResponse {
    pub data: Vec<ThreadRealtimeEntry>,
    pub next_cursor: Option<String>,
    #[ts(type = "number")]
    pub snapshot_end: u64,
    /// Exclusive position through which the projection has processed canonical history.
    #[ts(type = "number")]
    pub projected_through_item_index: u64,
    pub coverage: ThreadRealtimeCoverage,
}
