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
use crate::ThreadRecorder;
use crate::ThreadStore;
use crate::ThreadStoreError;
use crate::ThreadStoreResult;
use crate::UpdateThreadMetadataParams;
use proto::thread_store_client::ThreadStoreClient;
use tonic::codegen::InterceptedService;
use tonic::metadata::BinaryMetadataValue;
use tonic::service::Interceptor;
use tonic::transport::Channel;
use tonic::transport::Endpoint;

#[path = "proto/codex.thread_store.v1.rs"]
mod proto;

/// Metadata key used to forward the app-server's opaque identity key to remote contracts.
pub const IDENTITY_KEY_HEADER: &str = "x-codex-app-server-identity-key-bin";

#[derive(Clone, Debug)]
struct IdentityKeyInterceptor {
    identity_key: Option<Vec<u8>>,
}

impl Interceptor for IdentityKeyInterceptor {
    fn call(
        &mut self,
        mut request: tonic::Request<()>,
    ) -> Result<tonic::Request<()>, tonic::Status> {
        if let Some(identity_key) = &self.identity_key {
            request.metadata_mut().insert_bin(
                IDENTITY_KEY_HEADER,
                BinaryMetadataValue::from_bytes(identity_key),
            );
        }
        Ok(request)
    }
}

type RemoteThreadStoreClient =
    ThreadStoreClient<InterceptedService<Channel, IdentityKeyInterceptor>>;

/// gRPC-backed [`ThreadStore`] implementation for deployments whose durable thread data lives
/// outside the app-server process.
#[derive(Clone, Debug)]
pub struct RemoteThreadStore {
    endpoint: String,
    identity_key: Option<Vec<u8>>,
}

impl RemoteThreadStore {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            identity_key: None,
        }
    }

    pub fn new_with_identity_key(
        endpoint: impl Into<String>,
        identity_key: Option<Vec<u8>>,
    ) -> Self {
        Self {
            endpoint: endpoint.into(),
            identity_key,
        }
    }

    async fn client(&self) -> ThreadStoreResult<RemoteThreadStoreClient> {
        let channel = Endpoint::new(self.endpoint.clone())
            .map_err(|err| ThreadStoreError::InvalidRequest {
                message: format!("invalid remote thread store endpoint: {err}"),
            })?
            .connect()
            .await
            .map_err(|err| ThreadStoreError::Internal {
                message: format!("failed to connect to remote thread store: {err}"),
            })?;
        Ok(ThreadStoreClient::with_interceptor(
            channel,
            IdentityKeyInterceptor {
                identity_key: self.identity_key.clone(),
            },
        ))
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
