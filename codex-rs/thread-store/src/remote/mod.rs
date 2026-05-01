mod helpers;
mod list_threads;

use async_trait::async_trait;
use codex_protocol::ThreadId;

use crate::AppendThreadItemsParams;
use crate::ArchiveThreadParams;
use crate::CreateThreadParams;
use crate::ListThreadsParams;
use crate::LoadThreadHistoryParams;
use crate::ReadThreadByRolloutPathParams;
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
///
/// This store is still a work in progress: app-server code should call the generic
/// [`ThreadStore`] methods, and unsupported remote operations will return explicit
/// `not_implemented` errors until the remote API catches up.
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

    async fn create_thread(&self, params: CreateThreadParams) -> ThreadStoreResult<()> {
        let thread_id = params.thread_id;
        let request = proto::CreateThreadRequest {
            thread_id: thread_id.to_string(),
            forked_from_id: params.forked_from_id.map(|thread_id| thread_id.to_string()),
            source: Some(helpers::proto_session_source(&params.source)),
            base_instructions_json: helpers::base_instructions_json(&params.base_instructions)?,
            dynamic_tools_json: helpers::dynamic_tools_json(&params.dynamic_tools)?,
            event_persistence_mode: helpers::proto_event_persistence_mode(
                params.event_persistence_mode,
            )
            .into(),
            metadata_json: helpers::thread_persistence_metadata_json(&params.metadata)?,
        };
        self.client()
            .await?
            .create_thread(request)
            .await
            .map_err(|status| helpers::remote_status_to_thread_error(status, thread_id))?;
        Ok(())
    }

    async fn resume_thread(&self, params: ResumeThreadParams) -> ThreadStoreResult<()> {
        let thread_id = params.thread_id;
        let (has_history, history_json) = match params.history {
            Some(history) => (true, helpers::rollout_items_json(&history)?),
            None => (false, Vec::new()),
        };
        let request = proto::ResumeThreadRequest {
            thread_id: thread_id.to_string(),
            rollout_path: params
                .rollout_path
                .map(|path| path.to_string_lossy().into_owned()),
            history_json,
            has_history,
            include_archived: params.include_archived,
            event_persistence_mode: helpers::proto_event_persistence_mode(
                params.event_persistence_mode,
            )
            .into(),
            metadata_json: helpers::thread_persistence_metadata_json(&params.metadata)?,
        };
        self.client()
            .await?
            .resume_thread(request)
            .await
            .map_err(|status| helpers::remote_status_to_thread_error(status, thread_id))?;
        Ok(())
    }

    async fn append_items(&self, params: AppendThreadItemsParams) -> ThreadStoreResult<()> {
        let thread_id = params.thread_id;
        let request = proto::AppendThreadItemsRequest {
            thread_id: thread_id.to_string(),
            items_json: helpers::rollout_items_json(&params.items)?,
        };
        self.client()
            .await?
            .append_items(request)
            .await
            .map_err(|status| helpers::remote_status_to_thread_error(status, thread_id))?;
        Ok(())
    }

    async fn persist_thread(&self, thread_id: ThreadId) -> ThreadStoreResult<()> {
        self.client()
            .await?
            .persist_thread(helpers::proto_thread_id_request(thread_id))
            .await
            .map_err(|status| helpers::remote_status_to_thread_error(status, thread_id))?;
        Ok(())
    }

    async fn flush_thread(&self, thread_id: ThreadId) -> ThreadStoreResult<()> {
        self.client()
            .await?
            .flush_thread(helpers::proto_thread_id_request(thread_id))
            .await
            .map_err(|status| helpers::remote_status_to_thread_error(status, thread_id))?;
        Ok(())
    }

    async fn shutdown_thread(&self, thread_id: ThreadId) -> ThreadStoreResult<()> {
        self.client()
            .await?
            .shutdown_thread(helpers::proto_thread_id_request(thread_id))
            .await
            .map_err(|status| helpers::remote_status_to_thread_error(status, thread_id))?;
        Ok(())
    }

    async fn discard_thread(&self, thread_id: ThreadId) -> ThreadStoreResult<()> {
        self.client()
            .await?
            .discard_thread(helpers::proto_thread_id_request(thread_id))
            .await
            .map_err(|status| helpers::remote_status_to_thread_error(status, thread_id))?;
        Ok(())
    }

    async fn load_history(
        &self,
        params: LoadThreadHistoryParams,
    ) -> ThreadStoreResult<StoredThreadHistory> {
        let thread_id = params.thread_id;
        let response = self
            .client()
            .await?
            .load_history(proto::LoadThreadHistoryRequest {
                thread_id: thread_id.to_string(),
                include_archived: params.include_archived,
            })
            .await
            .map_err(|status| helpers::remote_status_to_thread_error(status, thread_id))?
            .into_inner();
        helpers::stored_thread_history_from_proto(response)
    }

    async fn read_thread(&self, params: ReadThreadParams) -> ThreadStoreResult<StoredThread> {
        let thread_id = params.thread_id;
        let response = self
            .client()
            .await?
            .read_thread(proto::ReadThreadRequest {
                thread_id: thread_id.to_string(),
                include_archived: params.include_archived,
                include_history: params.include_history,
            })
            .await
            .map_err(|status| helpers::remote_status_to_thread_error(status, thread_id))?
            .into_inner();
        let thread = response.thread.ok_or_else(|| ThreadStoreError::Internal {
            message: "remote thread store omitted read_thread response thread".to_string(),
        })?;
        helpers::stored_thread_from_proto(thread)
    }

    async fn read_thread_by_rollout_path(
        &self,
        _params: ReadThreadByRolloutPathParams,
    ) -> ThreadStoreResult<StoredThread> {
        Err(ThreadStoreError::Internal {
            message: "remote thread store does not support read_thread_by_rollout_path".to_string(),
        })
    }

    async fn list_threads(&self, params: ListThreadsParams) -> ThreadStoreResult<ThreadPage> {
        list_threads::list_threads(self, params).await
    }

    async fn update_thread_metadata(
        &self,
        params: UpdateThreadMetadataParams,
    ) -> ThreadStoreResult<StoredThread> {
        let thread_id = params.thread_id;
        let response = self
            .client()
            .await?
            .update_thread_metadata(proto::UpdateThreadMetadataRequest {
                thread_id: thread_id.to_string(),
                patch: Some(helpers::proto_metadata_patch(params.patch)),
                include_archived: params.include_archived,
            })
            .await
            .map_err(|status| helpers::remote_status_to_thread_error(status, thread_id))?
            .into_inner();
        let thread = response.thread.ok_or_else(|| ThreadStoreError::Internal {
            message: "remote thread store omitted update_thread_metadata response thread"
                .to_string(),
        })?;
        helpers::stored_thread_from_proto(thread)
    }

    async fn archive_thread(&self, params: ArchiveThreadParams) -> ThreadStoreResult<()> {
        let thread_id = params.thread_id;
        self.client()
            .await?
            .archive_thread(proto::ArchiveThreadRequest {
                thread_id: thread_id.to_string(),
            })
            .await
            .map_err(|status| helpers::remote_status_to_thread_error(status, thread_id))?;
        Ok(())
    }

    async fn unarchive_thread(
        &self,
        params: ArchiveThreadParams,
    ) -> ThreadStoreResult<StoredThread> {
        let thread_id = params.thread_id;
        let response = self
            .client()
            .await?
            .unarchive_thread(proto::ArchiveThreadRequest {
                thread_id: thread_id.to_string(),
            })
            .await
            .map_err(|status| helpers::remote_status_to_thread_error(status, thread_id))?
            .into_inner();
        let thread = response.thread.ok_or_else(|| ThreadStoreError::Internal {
            message: "remote thread store omitted unarchive_thread response thread".to_string(),
        })?;
        helpers::stored_thread_from_proto(thread)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use codex_protocol::ThreadId;
    use codex_protocol::models::BaseInstructions;
    use codex_protocol::protocol::SessionSource;
    use codex_protocol::protocol::ThreadMemoryMode;
    use pretty_assertions::assert_eq;
    use tokio::sync::mpsc;
    use tonic::Request;
    use tonic::Response;
    use tonic::Status;
    use tonic::transport::Server;

    use super::*;
    use crate::ThreadEventPersistenceMode;
    use crate::ThreadPersistenceMetadata;
    use proto::thread_store_server;
    use proto::thread_store_server::ThreadStoreServer;

    enum RecordedRequest {
        Create(proto::CreateThreadRequest),
        Resume(proto::ResumeThreadRequest),
    }

    struct TestServer {
        requests_tx: mpsc::UnboundedSender<RecordedRequest>,
    }

    #[tonic::async_trait]
    impl thread_store_server::ThreadStore for TestServer {
        async fn create_thread(
            &self,
            request: Request<proto::CreateThreadRequest>,
        ) -> Result<Response<proto::Empty>, Status> {
            self.requests_tx
                .send(RecordedRequest::Create(request.into_inner()))
                .expect("record create request");
            Ok(Response::new(proto::Empty {}))
        }

        async fn resume_thread(
            &self,
            request: Request<proto::ResumeThreadRequest>,
        ) -> Result<Response<proto::Empty>, Status> {
            self.requests_tx
                .send(RecordedRequest::Resume(request.into_inner()))
                .expect("record resume request");
            Ok(Response::new(proto::Empty {}))
        }

        async fn list_threads(
            &self,
            _request: Request<proto::ListThreadsRequest>,
        ) -> Result<Response<proto::ListThreadsResponse>, Status> {
            Err(Status::unimplemented("not implemented"))
        }
    }

    async fn test_store() -> (RemoteThreadStore, mpsc::UnboundedReceiver<RecordedRequest>) {
        let (requests_tx, requests_rx) = mpsc::unbounded_channel();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test server");
        let addr = listener.local_addr().expect("test server addr");

        tokio::spawn(async move {
            Server::builder()
                .add_service(ThreadStoreServer::new(TestServer { requests_tx }))
                .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
                .await
                .expect("test server");
        });

        (
            RemoteThreadStore::new(format!("http://{addr}")),
            requests_rx,
        )
    }

    #[tokio::test]
    async fn create_thread_forwards_metadata() {
        let (store, mut requests_rx) = test_store().await;
        let metadata = ThreadPersistenceMetadata {
            cwd: Some(PathBuf::from("/workspace")),
            model_provider: "test-provider".to_string(),
            memory_mode: ThreadMemoryMode::Enabled,
        };

        store
            .create_thread(CreateThreadParams {
                thread_id: ThreadId::new(),
                forked_from_id: None,
                source: SessionSource::Exec,
                base_instructions: BaseInstructions::default(),
                dynamic_tools: Vec::new(),
                metadata: metadata.clone(),
                event_persistence_mode: ThreadEventPersistenceMode::Limited,
            })
            .await
            .expect("create thread");

        let Some(RecordedRequest::Create(request)) = requests_rx.recv().await else {
            panic!("expected create request");
        };
        assert_eq!(
            serde_json::from_str::<ThreadPersistenceMetadata>(&request.metadata_json)
                .expect("metadata json"),
            metadata
        );
    }

    #[tokio::test]
    async fn resume_thread_forwards_metadata() {
        let (store, mut requests_rx) = test_store().await;
        let metadata = ThreadPersistenceMetadata {
            cwd: Some(PathBuf::from("/workspace")),
            model_provider: "test-provider".to_string(),
            memory_mode: ThreadMemoryMode::Disabled,
        };

        store
            .resume_thread(ResumeThreadParams {
                thread_id: ThreadId::new(),
                rollout_path: None,
                history: None,
                include_archived: false,
                metadata: metadata.clone(),
                event_persistence_mode: ThreadEventPersistenceMode::Limited,
            })
            .await
            .expect("resume thread");

        let Some(RecordedRequest::Resume(request)) = requests_rx.recv().await else {
            panic!("expected resume request");
        };
        assert_eq!(
            serde_json::from_str::<ThreadPersistenceMetadata>(&request.metadata_json)
                .expect("metadata json"),
            metadata
        );
    }
}
