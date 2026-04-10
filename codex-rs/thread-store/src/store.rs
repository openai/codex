use std::path::Path;

use async_trait::async_trait;
use codex_protocol::ThreadId;
use codex_protocol::dynamic_tools::DynamicToolSpec;

use crate::AppendThreadItemsParams;
use crate::ArchiveThreadParams;
use crate::CreateThreadParams;
use crate::DynamicToolsParams;
use crate::FindThreadByNameParams;
use crate::FindThreadSpawnByPathParams;
use crate::ListThreadSpawnEdgesParams;
use crate::ListThreadsParams;
use crate::LoadThreadHistoryParams;
use crate::ReadThreadParams;
use crate::ResolveLegacyPathParams;
use crate::ResumeThreadRecorderParams;
use crate::SetThreadMemoryModeParams;
use crate::SetThreadNameParams;
use crate::StoredThread;
use crate::StoredThreadHistory;
use crate::ThreadMemoryModeParams;
use crate::ThreadPage;
use crate::ThreadRecorder;
use crate::ThreadSpawnEdge;
use crate::ThreadStoreResult;
use crate::UpdateThreadMetadataParams;

/// Storage-neutral thread persistence boundary.
#[async_trait]
pub trait ThreadStore: Send + Sync {
    /// Creates a new thread and returns a live recorder for future appends.
    async fn create_thread(
        &self,
        params: CreateThreadParams,
    ) -> ThreadStoreResult<Box<dyn ThreadRecorder>>;

    /// Reopens a live recorder for an existing thread.
    async fn resume_thread_recorder(
        &self,
        params: ResumeThreadRecorderParams,
    ) -> ThreadStoreResult<Box<dyn ThreadRecorder>>;

    /// Appends items to a stored thread outside the live-recorder path.
    async fn append_items(&self, params: AppendThreadItemsParams) -> ThreadStoreResult<()>;

    /// Loads persisted history for resume, fork, rollback, and memory jobs.
    async fn load_history(
        &self,
        params: LoadThreadHistoryParams,
    ) -> ThreadStoreResult<StoredThreadHistory>;

    /// Reads a thread summary and optionally its persisted history.
    async fn read_thread(&self, params: ReadThreadParams) -> ThreadStoreResult<StoredThread>;

    /// Lists stored threads matching the supplied filters.
    async fn list_threads(&self, params: ListThreadsParams) -> ThreadStoreResult<ThreadPage>;

    /// Finds the newest thread whose user-facing name exactly matches the supplied name.
    async fn find_thread_by_name(
        &self,
        params: FindThreadByNameParams,
    ) -> ThreadStoreResult<Option<StoredThread>>;

    /// Sets a user-facing thread name.
    async fn set_thread_name(&self, params: SetThreadNameParams) -> ThreadStoreResult<()>;

    /// Applies a mutable metadata patch and returns the updated thread.
    async fn update_thread_metadata(
        &self,
        params: UpdateThreadMetadataParams,
    ) -> ThreadStoreResult<StoredThread>;

    /// Archives a thread.
    async fn archive_thread(&self, params: ArchiveThreadParams) -> ThreadStoreResult<()>;

    /// Unarchives a thread and returns its updated metadata.
    async fn unarchive_thread(
        &self,
        params: ArchiveThreadParams,
    ) -> ThreadStoreResult<StoredThread>;

    /// Resolves a legacy rollout path to a thread id, if this store supports local path lookup.
    async fn resolve_legacy_path(
        &self,
        params: ResolveLegacyPathParams,
    ) -> ThreadStoreResult<Option<ThreadId>>;

    /// Reads dynamic tools persisted for a thread.
    async fn dynamic_tools(
        &self,
        params: DynamicToolsParams,
    ) -> ThreadStoreResult<Option<Vec<DynamicToolSpec>>>;

    /// Reads a thread's memory mode.
    async fn memory_mode(
        &self,
        params: ThreadMemoryModeParams,
    ) -> ThreadStoreResult<Option<String>>;

    /// Updates a thread's memory mode.
    async fn set_memory_mode(&self, params: SetThreadMemoryModeParams) -> ThreadStoreResult<()>;

    /// Marks a thread's memory context as polluted.
    async fn mark_memory_mode_polluted(
        &self,
        params: ThreadMemoryModeParams,
    ) -> ThreadStoreResult<()>;

    /// Persists or replaces a thread-spawn parent-child edge.
    async fn upsert_thread_spawn_edge(&self, edge: ThreadSpawnEdge) -> ThreadStoreResult<()>;

    /// Lists thread-spawn children or descendants.
    async fn list_thread_spawn_edges(
        &self,
        params: ListThreadSpawnEdgesParams,
    ) -> ThreadStoreResult<Vec<ThreadSpawnEdge>>;

    /// Finds a thread-spawn child or descendant by canonical agent path.
    async fn find_thread_spawn_by_path(
        &self,
        params: FindThreadSpawnByPathParams,
    ) -> ThreadStoreResult<Option<ThreadId>>;

    /// Returns true if this store can resolve the supplied legacy path.
    fn supports_legacy_path(&self, path: &Path) -> bool;
}
