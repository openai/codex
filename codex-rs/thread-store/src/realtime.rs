//! Shared query contract for the independent, immutable realtime history stream.

use codex_app_server_protocol::ThreadRealtimeOrder;
use codex_protocol::ThreadId;
use serde::Deserialize;
use serde::Serialize;

pub use codex_app_server_protocol::ThreadRealtimeListResponse as RealtimePage;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListRealtimeParams {
    pub thread_id: ThreadId,
    pub session_id: Option<String>,
    pub from_observed_at_ms: Option<i64>,
    pub to_observed_at_ms: Option<i64>,
    pub order: ThreadRealtimeOrder,
    pub after_position: Option<u64>,
    pub event_id: Option<String>,
    pub cursor: Option<String>,
    pub page_size: usize,
    pub snapshot_end: Option<u64>,
    pub required_through_position: Option<u64>,
}
