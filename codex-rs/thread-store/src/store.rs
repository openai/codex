use codex_protocol::ThreadId;
use codex_protocol::protocol::ThreadHistoryMode;
use std::any::Any;
use std::future::Future;
use std::pin::Pin;

use crate::AppendThreadItemsParams;
use crate::ArchiveThreadParams;
use crate::CreateThreadParams;
use crate::DeleteThreadParams;
use crate::ItemPage;
use crate::ListItemsParams;
use crate::ListThreadsParams;
use crate::ListTurnsParams;
use crate::LoadThreadHistoryParams;
use crate::ReadThreadByRolloutPathParams;
use crate::ReadThreadParams;
use crate::ResumeThreadParams;
use crate::SearchThreadsParams;
use crate::StoredModelContext;
use crate::StoredThread;
use crate::StoredThreadHistory;
use crate::ThreadPage;
use crate::ThreadSearchPage;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;
use crate::TurnPage;
use crate::UpdateThreadMetadataParams;

/// Future returned by [`ThreadStore`] operations.
pub type ThreadStoreFuture<'a, T> = Pin<Box<dyn Future<Output = ThreadStoreResult<T>> + Send + 'a>>;

/// Storage-neutral thread persistence boundary.
pub trait ThreadStore: Any + Send + Sync {
    /// Return this store as [`Any`] for implementation-owned escape hatches.
    fn as_any(&self) -> &dyn Any;

    /// Returns the history mode to use when history does not carry a persisted mode.
    ///
    /// The default is legacy so existing stores stay compatible. Stores whose durable contract is
    /// already paginated should override this instead of relying on core to infer storage behavior.
    fn default_history_mode(&self) -> ThreadHistoryMode {
        ThreadHistoryMode::Legacy
    }

    /// Creates a new live thread.
    fn create_thread(&self, params: CreateThreadParams) -> ThreadStoreFuture<'_, ()>;

    /// Reopens an existing thread for live appends.
    fn resume_thread(&self, params: ResumeThreadParams) -> ThreadStoreFuture<'_, ()>;

    /// Appends raw rollout items to a live thread.
    ///
    /// Implementations should apply the shared rollout persistence policy before writing durable
    /// replay history and before updating any implementation-owned projections.
    fn append_items(&self, params: AppendThreadItemsParams) -> ThreadStoreFuture<'_, ()>;

    /// Materializes the thread if persistence is lazy, then persists all queued items.
    fn persist_thread(&self, thread_id: ThreadId) -> ThreadStoreFuture<'_, ()>;

    /// Flushes all queued items and returns once they are durable/readable.
    fn flush_thread(&self, thread_id: ThreadId) -> ThreadStoreFuture<'_, ()>;

    /// Flushes pending items and closes the live thread writer.
    fn shutdown_thread(&self, thread_id: ThreadId) -> ThreadStoreFuture<'_, ()>;

    /// Discards the live thread writer without forcing pending in-memory items to become durable.
    ///
    /// Core calls this when session initialization fails after a live writer has been created.
    /// Implementations should release any live writer resources for the thread while preserving
    /// already-durable thread data.
    fn discard_thread(&self, thread_id: ThreadId) -> ThreadStoreFuture<'_, ()>;

    /// Loads persisted history for resume, fork, rollback, and memory jobs.
    fn load_history(
        &self,
        params: LoadThreadHistoryParams,
    ) -> ThreadStoreFuture<'_, StoredThreadHistory>;

    /// Loads the persisted rollout items needed to reconstruct the latest model-visible context.
    ///
    /// Implementations that cannot perform a targeted read may return the full persisted history.
    fn load_latest_model_context(
        &self,
        _params: LoadThreadHistoryParams,
    ) -> ThreadStoreFuture<'_, StoredModelContext> {
        Box::pin(async {
            Err(ThreadStoreError::Unsupported {
                operation: "load_latest_model_context",
            })
        })
    }

    /// Reads a thread summary and optionally its persisted history.
    fn read_thread(&self, params: ReadThreadParams) -> ThreadStoreFuture<'_, StoredThread>;

    /// Reads a rollout-backed thread by path when the store supports path-addressed lookups.
    ///
    /// Deprecated: new callers should use [`ThreadStore::read_thread`] instead.
    fn read_thread_by_rollout_path(
        &self,
        params: ReadThreadByRolloutPathParams,
    ) -> ThreadStoreFuture<'_, StoredThread>;

    /// Lists stored threads matching the supplied filters.
    fn list_threads(&self, params: ListThreadsParams) -> ThreadStoreFuture<'_, ThreadPage>;

    /// Searches stored threads and returns search-only preview metadata.
    fn search_threads(
        &self,
        _params: SearchThreadsParams,
    ) -> ThreadStoreFuture<'_, ThreadSearchPage> {
        Box::pin(async {
            Err(ThreadStoreError::Unsupported {
                operation: "thread/search",
            })
        })
    }

    /// Lists turns within a stored thread.
    fn list_turns(&self, _params: ListTurnsParams) -> ThreadStoreFuture<'_, TurnPage> {
        Box::pin(async {
            Err(ThreadStoreError::Unsupported {
                operation: "list_turns",
            })
        })
    }

    /// Lists persisted items within a stored thread, optionally filtered to a turn.
    fn list_items(&self, _params: ListItemsParams) -> ThreadStoreFuture<'_, ItemPage> {
        Box::pin(async {
            Err(ThreadStoreError::Unsupported {
                operation: "list_items",
            })
        })
    }

    /// Applies a literal metadata patch and returns the updated thread.
    ///
    /// Implementations should apply the supplied fields directly. Policy such as deciding whether
    /// an append-derived preview should be emitted belongs above the store.
    fn update_thread_metadata(
        &self,
        params: UpdateThreadMetadataParams,
    ) -> ThreadStoreFuture<'_, StoredThread>;

    /// Archives a thread.
    fn archive_thread(&self, params: ArchiveThreadParams) -> ThreadStoreFuture<'_, ()>;

    /// Unarchives a thread and returns its updated metadata.
    fn unarchive_thread(&self, params: ArchiveThreadParams) -> ThreadStoreFuture<'_, StoredThread>;

    /// Deletes a thread's persisted rollout data and associated metadata.
    fn delete_thread(&self, params: DeleteThreadParams) -> ThreadStoreFuture<'_, ()>;
}
