mod helpers;
mod list_threads;

use async_trait::async_trait;

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
use crate::ThreadReadSource;
use crate::ThreadRecorder;
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
    async fn create_thread(
        &self,
        _params: CreateThreadParams,
    ) -> ThreadStoreResult<Box<dyn ThreadRecorder>> {
        Err(not_implemented("create_thread"))
    }

    async fn resume_thread_recorder(
        &self,
        _params: ResumeThreadRecorderParams,
    ) -> ThreadStoreResult<Box<dyn ThreadRecorder>> {
        Err(not_implemented("resume_thread_recorder"))
    }

    async fn append_items(&self, _params: AppendThreadItemsParams) -> ThreadStoreResult<()> {
        Err(not_implemented("append_items"))
    }

    async fn load_history(
        &self,
        params: LoadThreadHistoryParams,
    ) -> ThreadStoreResult<StoredThreadHistory> {
        Err(unsupported_or_not_implemented(
            "load_history",
            &params.source,
        ))
    }

    async fn read_thread(&self, params: ReadThreadParams) -> ThreadStoreResult<StoredThread> {
        Err(unsupported_or_not_implemented(
            "read_thread",
            &params.source,
        ))
    }

    async fn list_threads(&self, params: ListThreadsParams) -> ThreadStoreResult<ThreadPage> {
        list_threads::list_threads(self, params).await
    }

    async fn set_thread_name(&self, _params: SetThreadNameParams) -> ThreadStoreResult<()> {
        Err(not_implemented("set_thread_name"))
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

fn unsupported_or_not_implemented(method: &str, source: &ThreadReadSource) -> ThreadStoreError {
    match source {
        ThreadReadSource::LocalPath { .. } => ThreadStoreError::InvalidRequest {
            message: format!(
                "remote thread store does not support local rollout paths for {method}"
            ),
        },
        ThreadReadSource::ThreadId { .. } => not_implemented(method),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::ReadThreadParams;
    use crate::RemoteThreadStore;
    use crate::ThreadReadSource;
    use crate::ThreadStore;
    use crate::ThreadStoreError;

    #[tokio::test]
    async fn read_thread_rejects_local_paths() {
        let store = RemoteThreadStore::new("http://localhost:1");

        let err = store
            .read_thread(ReadThreadParams {
                source: ThreadReadSource::LocalPath {
                    rollout_path: PathBuf::from("/tmp/rollout.jsonl"),
                },
                include_history: false,
            })
            .await
            .expect_err("local paths should not be supported by remote store");

        assert!(matches!(
            err,
            ThreadStoreError::InvalidRequest { message }
                if message == "remote thread store does not support local rollout paths for read_thread"
        ));
    }
}
