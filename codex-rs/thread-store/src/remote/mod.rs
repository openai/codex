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
