mod helpers;
mod list_threads;

use async_trait::async_trait;
use codex_protocol::ThreadId;

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
use crate::ThreadStore;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;
use crate::UpdateThreadMetadataParams;
use proto::thread_store_client::ThreadStoreClient;

#[path = "proto/codex.thread_store.v1.rs"]
mod proto;

/// gRPC-backed [`ThreadStore`] implementation for deployments whose durable thread data lives
/// outside the app-server process.
#[derive(Clone, Debug)]
pub struct RemoteThreadStore {
    endpoint: String,
}

impl RemoteThreadStore {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
        }
    }

    async fn client(&self) -> ThreadStoreResult<ThreadStoreClient<tonic::transport::Channel>> {
        ThreadStoreClient::connect(self.endpoint.clone())
            .await
            .map_err(|err| ThreadStoreError::Internal {
                message: format!("failed to connect to remote thread store: {err}"),
            })
    }
}

#[async_trait]
impl ThreadStore for RemoteThreadStore {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn create_thread(&self, _params: CreateThreadParams) -> ThreadStoreResult<()> {
        Err(not_implemented("create_thread"))
    }

    async fn resume_thread(&self, _params: ResumeThreadParams) -> ThreadStoreResult<()> {
        Err(not_implemented("resume_thread"))
    }

    async fn append_items(&self, _params: AppendThreadItemsParams) -> ThreadStoreResult<()> {
        Err(not_implemented("append_items"))
    }

    async fn persist_thread(&self, _thread_id: ThreadId) -> ThreadStoreResult<()> {
        Err(not_implemented("persist_thread"))
    }

    async fn flush_thread(&self, _thread_id: ThreadId) -> ThreadStoreResult<()> {
        Err(not_implemented("flush_thread"))
    }

    async fn shutdown_thread(&self, _thread_id: ThreadId) -> ThreadStoreResult<()> {
        Err(not_implemented("shutdown_thread"))
    }

    async fn discard_thread(&self, _thread_id: ThreadId) -> ThreadStoreResult<()> {
        Err(not_implemented("discard_thread"))
    }

    async fn load_history(
        &self,
        _params: LoadThreadHistoryParams,
    ) -> ThreadStoreResult<StoredThreadHistory> {
        Err(not_implemented("load_history"))
    }

    async fn read_thread(&self, _params: ReadThreadParams) -> ThreadStoreResult<StoredThread> {
        Err(not_implemented("read_thread"))
    }

    async fn list_threads(&self, params: ListThreadsParams) -> ThreadStoreResult<ThreadPage> {
        list_threads::list_threads(self, params).await
    }

    async fn update_thread_metadata(
        &self,
        _params: UpdateThreadMetadataParams,
    ) -> ThreadStoreResult<StoredThread> {
        Err(not_implemented("update_thread_metadata"))
    }

    async fn archive_thread(&self, _params: ArchiveThreadParams) -> ThreadStoreResult<()> {
        Err(not_implemented("archive_thread"))
    }

    async fn unarchive_thread(
        &self,
        _params: ArchiveThreadParams,
    ) -> ThreadStoreResult<StoredThread> {
        Err(not_implemented("unarchive_thread"))
    }
}

fn not_implemented(method: &str) -> ThreadStoreError {
    ThreadStoreError::Internal {
        message: format!("remote thread store does not implement {method} yet"),
    }
}
