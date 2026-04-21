use std::any::Any;
use std::path::PathBuf;

use async_trait::async_trait;
use codex_protocol::ThreadId;
use codex_rollout::StateDbHandle;

use crate::AppendThreadItemsParams;
use crate::ArchiveThreadParams;
use crate::CreateThreadParams;
use crate::ListThreadsParams;
use crate::LoadThreadHistoryParams;
use crate::ReadThreadParams;
use crate::ResumeThreadParams;
use crate::StoredThread;
use crate::StoredThreadHistory;
use crate::ThreadPage;
use crate::ThreadStoreResult;
use crate::UpdateThreadMetadataParams;

/// Storage-neutral thread persistence boundary.
#[async_trait]
pub trait ThreadStore: Any + Send + Sync {
    /// Return this store as [`Any`] so callers at API boundaries can reject requests that only
    /// make sense for a concrete store implementation.
    fn as_any(&self) -> &dyn Any;

    /// Creates a new live thread.
    async fn create_thread(&self, params: CreateThreadParams) -> ThreadStoreResult<()>;

    /// Reopens an existing thread for live appends.
    async fn resume_thread(&self, params: ResumeThreadParams) -> ThreadStoreResult<()>;

    /// Appends items to a live thread.
    async fn append_items(&self, params: AppendThreadItemsParams) -> ThreadStoreResult<()>;

    /// Materializes the thread if persistence is lazy, then persists all queued items.
    async fn persist_thread(&self, thread_id: ThreadId) -> ThreadStoreResult<()>;

    /// Flushes all queued items and returns once they are durable/readable.
    async fn flush_thread(&self, thread_id: ThreadId) -> ThreadStoreResult<()>;

    /// Flushes pending items and closes the live thread writer.
    async fn shutdown_thread(&self, thread_id: ThreadId) -> ThreadStoreResult<()>;

    /// Returns the local rollout path when this thread is backed by a filesystem rollout.
    async fn rollout_path(&self, thread_id: ThreadId) -> ThreadStoreResult<Option<PathBuf>>;

    /// Returns the local state database handle when one is available.
    async fn state_db(&self, thread_id: ThreadId) -> ThreadStoreResult<Option<StateDbHandle>>;

    /// Loads persisted history for resume, fork, rollback, and memory jobs.
    async fn load_history(
        &self,
        params: LoadThreadHistoryParams,
    ) -> ThreadStoreResult<StoredThreadHistory>;

    /// Reads a thread summary and optionally its persisted history.
    async fn read_thread(&self, params: ReadThreadParams) -> ThreadStoreResult<StoredThread>;

    /// Lists stored threads matching the supplied filters.
    async fn list_threads(&self, params: ListThreadsParams) -> ThreadStoreResult<ThreadPage>;

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
}
