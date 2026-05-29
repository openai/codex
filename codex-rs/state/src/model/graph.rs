use codex_protocol::ThreadId;
use strum::AsRefStr;
use strum::Display;
use strum::EnumString;

/// Status attached to a directional thread-spawn edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, AsRefStr, Display, EnumString)]
#[strum(serialize_all = "snake_case")]
pub enum DirectionalThreadSpawnEdgeStatus {
    Open,
    Closed,
}

/// Persisted directional parent-child edge for a spawned thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThreadSpawnEdge {
    pub parent_thread_id: ThreadId,
    pub child_thread_id: ThreadId,
    pub status: DirectionalThreadSpawnEdgeStatus,
}

/// Keyset-paginated direct children from the persisted thread spawn graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadSpawnEdgesPage {
    pub items: Vec<ThreadSpawnEdge>,
    pub next_cursor: Option<ThreadId>,
}
