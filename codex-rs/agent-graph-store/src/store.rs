use async_trait::async_trait;
use codex_protocol::ThreadId;

use crate::AgentGraphStoreResult;
use crate::ThreadSpawnEdgeStatus;

/// Storage-neutral boundary for persisted thread-spawn parent/child topology.
///
/// Implementations are expected to return stable ordering for list methods so callers can merge
/// persisted graph state with live in-memory state without introducing nondeterministic output.
#[async_trait]
pub trait AgentGraphStore: Send + Sync {
    /// Insert or replace the directional parent/child edge for a spawned thread.
    ///
    /// `child_thread_id` has at most one persisted parent. Re-inserting the same child should
    /// update both the parent and status to match the supplied values.
    async fn upsert_thread_spawn_edge(
        &self,
        parent_thread_id: ThreadId,
        child_thread_id: ThreadId,
        status: ThreadSpawnEdgeStatus,
    ) -> AgentGraphStoreResult<()>;

    /// Update the persisted lifecycle status of a spawned thread's incoming edge.
    ///
    /// Implementations should treat missing children as a successful no-op.
    async fn set_thread_spawn_edge_status(
        &self,
        child_thread_id: ThreadId,
        status: ThreadSpawnEdgeStatus,
    ) -> AgentGraphStoreResult<()>;

    /// List direct spawned children of a parent thread.
    ///
    /// When `status_filter` is `Some`, only child edges with that exact status are returned. When
    /// it is `None`, all direct child edges are returned regardless of status, including statuses
    /// that may be added by a future store implementation.
    async fn list_thread_spawn_children(
        &self,
        parent_thread_id: ThreadId,
        status_filter: Option<ThreadSpawnEdgeStatus>,
    ) -> AgentGraphStoreResult<Vec<ThreadId>>;

    /// List spawned descendants breadth-first by depth, then by thread id.
    ///
    /// `status_filter` is applied to every traversed edge, not just to the returned descendants.
    /// For example, `Some(Open)` walks only open edges, so descendants under a closed edge are not
    /// included even if their own incoming edge is open. `None` walks and returns every persisted
    /// edge regardless of status.
    async fn list_thread_spawn_descendants(
        &self,
        root_thread_id: ThreadId,
        status_filter: Option<ThreadSpawnEdgeStatus>,
    ) -> AgentGraphStoreResult<Vec<ThreadId>>;
}
