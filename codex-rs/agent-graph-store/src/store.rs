use async_trait::async_trait;
use codex_protocol::ThreadId;

use crate::AgentGraphStoreResult;
use crate::ThreadSpawnEdgeStatus;

/// Storage-neutral boundary for persisted thread-spawn parent/child topology.
#[async_trait]
pub trait AgentGraphStore: Send + Sync {
    /// Insert or replace the directional parent/child edge for a spawned thread.
    async fn upsert_thread_spawn_edge(
        &self,
        parent_thread_id: ThreadId,
        child_thread_id: ThreadId,
        status: ThreadSpawnEdgeStatus,
    ) -> AgentGraphStoreResult<()>;

    /// Update the persisted lifecycle status of a spawned thread's incoming edge.
    async fn set_thread_spawn_edge_status(
        &self,
        child_thread_id: ThreadId,
        status: ThreadSpawnEdgeStatus,
    ) -> AgentGraphStoreResult<()>;

    /// List direct spawned children of a parent thread.
    async fn list_thread_spawn_children(
        &self,
        parent_thread_id: ThreadId,
        status_filter: Option<ThreadSpawnEdgeStatus>,
    ) -> AgentGraphStoreResult<Vec<ThreadId>>;

    /// List spawned descendants breadth-first by depth, then by thread id.
    async fn list_thread_spawn_descendants(
        &self,
        root_thread_id: ThreadId,
        status_filter: Option<ThreadSpawnEdgeStatus>,
    ) -> AgentGraphStoreResult<Vec<ThreadId>>;
}
