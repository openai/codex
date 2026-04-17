mod archive_thread;
mod helpers;
mod list_threads;
mod read_thread;
mod unarchive_thread;

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
use crate::SetThreadNameParams;
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
}

#[async_trait]
impl ThreadStore for LocalThreadStore {
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

    async fn set_thread_name(&self, _params: SetThreadNameParams) -> ThreadStoreResult<()> {
        unsupported("set_thread_name")
    }

    async fn update_thread_metadata(
        &self,
        _params: UpdateThreadMetadataParams,
    ) -> ThreadStoreResult<StoredThread> {
        unsupported("update_thread_metadata")
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
