mod archive_thread;
mod helpers;
mod list_threads;
mod read_thread;
mod unarchive_thread;
mod update_thread_metadata;

#[cfg(test)]
mod test_support;

use async_trait::async_trait;
use codex_rollout::RolloutConfig;

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
use crate::ThreadStore;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;
use crate::UpdateThreadMetadataParams;

/// Local filesystem/SQLite-backed implementation of [`ThreadStore`].
#[derive(Clone, Debug)]
pub struct LocalThreadStore {
    pub(super) config: RolloutConfig,
}

impl LocalThreadStore {
    /// Create a local store from the rollout configuration used by existing local persistence.
    pub fn new(config: RolloutConfig) -> Self {
        Self { config }
    }

    /// Read a local rollout-backed thread by path.
    pub async fn read_thread_by_rollout_path(
        &self,
        rollout_path: std::path::PathBuf,
        include_archived: bool,
        include_history: bool,
    ) -> ThreadStoreResult<StoredThread> {
        read_thread::read_thread_by_rollout_path(
            self,
            rollout_path,
            include_archived,
            include_history,
        )
        .await
    }
}

#[async_trait]
impl ThreadStore for LocalThreadStore {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn create_thread(
        &self,
        _params: CreateThreadParams,
    ) -> ThreadStoreResult<Box<dyn ThreadRecorder>> {
        unsupported("create_thread")
    }

    async fn resume_thread_recorder(
        &self,
        _params: ResumeThreadRecorderParams,
    ) -> ThreadStoreResult<Box<dyn ThreadRecorder>> {
        unsupported("resume_thread_recorder")
    }

    async fn append_items(&self, _params: AppendThreadItemsParams) -> ThreadStoreResult<()> {
        unsupported("append_items")
    }

    async fn load_history(
        &self,
        _params: LoadThreadHistoryParams,
    ) -> ThreadStoreResult<StoredThreadHistory> {
        unsupported("load_history")
    }

    async fn read_thread(&self, params: ReadThreadParams) -> ThreadStoreResult<StoredThread> {
        read_thread::read_thread(self, params).await
    }

    async fn list_threads(&self, params: ListThreadsParams) -> ThreadStoreResult<ThreadPage> {
        list_threads::list_threads(self, params).await
    }

    async fn update_thread_metadata(
        &self,
        params: UpdateThreadMetadataParams,
    ) -> ThreadStoreResult<StoredThread> {
        update_thread_metadata::update_thread_metadata(self, params).await
    }

    async fn archive_thread(&self, params: ArchiveThreadParams) -> ThreadStoreResult<()> {
        archive_thread::archive_thread(self, params).await
    }

    async fn unarchive_thread(
        &self,
        params: ArchiveThreadParams,
    ) -> ThreadStoreResult<StoredThread> {
        unarchive_thread::unarchive_thread(self, params).await
    }
}

fn unsupported<T>(operation: &str) -> ThreadStoreResult<T> {
    Err(ThreadStoreError::Internal {
        message: format!("local thread store does not implement {operation} in this slice"),
    })
}
