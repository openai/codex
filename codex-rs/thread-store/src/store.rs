use async_trait::async_trait;

use crate::AppendThreadItemsParams;
use crate::ArchiveThreadParams;
use crate::CreateThreadParams;
use crate::ListThreadsParams;
use crate::LoadThreadHistoryParams;
use crate::ReadThreadParams;
use crate::ResumeThreadRecorderParams;
use crate::StoredThread;
use crate::StoredThreadHistory;
use crate::ThreadPage;
use crate::ThreadRecorder;
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
