use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::OnceLock;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;

use arc_swap::ArcSwap;
use codex_app_server_protocol::JSONRPCNotification;
use futures::FutureExt;
use futures::future::BoxFuture;
use serde_json::Value;
use tokio::sync::Mutex;
use tokio::sync::Semaphore;
use tokio::sync::mpsc;
use tokio::sync::watch;

use tokio::time::timeout;
use tracing::debug;

use crate::ProcessId;
use crate::client_api::ExecServerClientConnectOptions;
use crate::client_api::ExecServerTransportParams;
use crate::client_api::HttpClient;
use crate::client_api::RemoteExecServerConnectArgs;
use crate::client_api::StdioExecServerConnectArgs;
use crate::connection::JsonRpcConnection;
use crate::process::ExecProcessEvent;
use crate::process::ExecProcessEventLog;
use crate::process::ExecProcessEventReceiver;
use crate::protocol::ENVIRONMENT_INFO_METHOD;
use crate::protocol::EXEC_CLOSED_METHOD;
use crate::protocol::EXEC_EXITED_METHOD;
use crate::protocol::EXEC_METHOD;
use crate::protocol::EXEC_OUTPUT_DELTA_METHOD;
use crate::protocol::EXEC_READ_METHOD;
use crate::protocol::EXEC_SIGNAL_METHOD;
use crate::protocol::EXEC_TERMINATE_METHOD;
use crate::protocol::EXEC_WRITE_METHOD;
use crate::protocol::EnvironmentInfo;
use crate::protocol::ExecClosedNotification;
use crate::protocol::ExecExitedNotification;
use crate::protocol::ExecOutputDeltaNotification;
use crate::protocol::ExecParams;
use crate::protocol::ExecResponse;
use crate::protocol::FS_CANONICALIZE_METHOD;
use crate::protocol::FS_COPY_METHOD;
use crate::protocol::FS_CREATE_DIRECTORY_METHOD;
use crate::protocol::FS_GET_METADATA_METHOD;
use crate::protocol::FS_READ_DIRECTORY_METHOD;
use crate::protocol::FS_READ_FILE_METHOD;
use crate::protocol::FS_REMOVE_METHOD;
use crate::protocol::FS_WRITE_FILE_METHOD;
use crate::protocol::FsCanonicalizeParams;
use crate::protocol::FsCanonicalizeResponse;
use crate::protocol::FsCopyParams;
use crate::protocol::FsCopyResponse;
use crate::protocol::FsCreateDirectoryParams;
use crate::protocol::FsCreateDirectoryResponse;
use crate::protocol::FsGetMetadataParams;
use crate::protocol::FsGetMetadataResponse;
use crate::protocol::FsReadDirectoryParams;
use crate::protocol::FsReadDirectoryResponse;
use crate::protocol::FsReadFileParams;
use crate::protocol::FsReadFileResponse;
use crate::protocol::FsRemoveParams;
use crate::protocol::FsRemoveResponse;
use crate::protocol::FsWriteFileParams;
use crate::protocol::FsWriteFileResponse;
use crate::protocol::HTTP_REQUEST_BODY_DELTA_METHOD;
use crate::protocol::HttpRequestBodyDeltaNotification;
use crate::protocol::INITIALIZE_METHOD;
use crate::protocol::INITIALIZED_METHOD;
use crate::protocol::InitializeParams;
use crate::protocol::InitializeResponse;
use crate::protocol::ProcessOutputChunk;
use crate::protocol::ProcessSignal;
use crate::protocol::ReadParams;
use crate::protocol::ReadResponse;
use crate::protocol::SignalParams;
use crate::protocol::SignalResponse;
use crate::protocol::TerminateParams;
use crate::protocol::TerminateResponse;
use crate::protocol::WriteParams;
use crate::protocol::WriteResponse;

const ENVIRONMENT_INFO_RELOAD_PENDING: u64 = 1 << 63;
const ENVIRONMENT_INFO_LOAD_TOKEN_MASK: u64 = !ENVIRONMENT_INFO_RELOAD_PENDING;
use crate::rpc::RpcCallError;
use crate::rpc::RpcClient;
use crate::rpc::RpcClientEvent;

pub(crate) mod http_client;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const INITIALIZE_TIMEOUT: Duration = Duration::from_secs(10);
const PROCESS_EVENT_CHANNEL_CAPACITY: usize = 256;
const PROCESS_EVENT_RETAINED_BYTES: usize = 1024 * 1024;

impl Default for ExecServerClientConnectOptions {
    fn default() -> Self {
        Self {
            client_name: "codex-core".to_string(),
            initialize_timeout: INITIALIZE_TIMEOUT,
            resume_session_id: None,
        }
    }
}

impl From<RemoteExecServerConnectArgs> for ExecServerClientConnectOptions {
    fn from(value: RemoteExecServerConnectArgs) -> Self {
        Self {
            client_name: value.client_name,
            initialize_timeout: value.initialize_timeout,
            resume_session_id: value.resume_session_id,
        }
    }
}

impl From<StdioExecServerConnectArgs> for ExecServerClientConnectOptions {
    fn from(value: StdioExecServerConnectArgs) -> Self {
        Self {
            client_name: value.client_name,
            initialize_timeout: value.initialize_timeout,
            resume_session_id: value.resume_session_id,
        }
    }
}

impl RemoteExecServerConnectArgs {
    pub fn new(websocket_url: String, client_name: String) -> Self {
        Self {
            websocket_url,
            client_name,
            connect_timeout: CONNECT_TIMEOUT,
            initialize_timeout: INITIALIZE_TIMEOUT,
            resume_session_id: None,
        }
    }
}

pub(crate) struct SessionState {
    wake_tx: watch::Sender<u64>,
    events: ExecProcessEventLog,
    ordered_events: StdMutex<OrderedSessionEvents>,
    failure: Mutex<Option<String>>,
}

#[derive(Default)]
struct OrderedSessionEvents {
    last_published_seq: u64,
    // Server-side output, exit, and closed notifications are emitted by
    // different tasks and can reach the client out of order. Keep future events
    // here until all lower sequence numbers have been published.
    pending: BTreeMap<u64, ExecProcessEvent>,
}

#[derive(Clone)]
pub(crate) struct Session {
    client: ExecServerClient,
    process_id: ProcessId,
    state: Arc<SessionState>,
}

struct Inner {
    client: RpcClient,
    // The remote transport delivers one shared notification stream for every
    // process on the connection. Keep a local process_id -> session registry so
    // we can turn those connection-global notifications into process wakeups
    // without making notifications the source of truth for output delivery.
    sessions: ArcSwap<HashMap<ProcessId, Arc<SessionState>>>,
    // ArcSwap makes reads cheap on the hot notification path, but writes still
    // need serialization so concurrent register/remove operations do not
    // overwrite each other's copy-on-write updates.
    sessions_write_lock: Mutex<()>,
    // Once the transport closes, every environment operation should fail quickly
    // with the same canonical message. This client never reconnects, so the
    // latch only moves from unset to set once.
    disconnected: OnceLock<String>,
    // Streaming HTTP responses are keyed by a client-generated request id
    // because they share the same connection-global notification channel as
    // process output. Keep the routing table local to the client so higher
    // layers can consume body chunks like a normal byte stream.
    http_body_streams: ArcSwap<HashMap<String, mpsc::Sender<HttpRequestBodyDeltaNotification>>>,
    http_body_stream_failures: ArcSwap<HashMap<String, String>>,
    http_body_streams_write_lock: Mutex<()>,
    http_body_stream_next_id: AtomicU64,
    session_id: std::sync::RwLock<Option<String>>,
    reader_task: tokio::task::JoinHandle<()>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        self.reader_task.abort();
    }
}

#[derive(Clone)]
pub struct ExecServerClient {
    inner: Arc<Inner>,
}

#[derive(Clone)]
pub(crate) struct LazyRemoteExecServerClient {
    state: Arc<StdMutex<LazyRemoteExecServerClientState>>,
    transport_revision_tx: watch::Sender<u64>,
    connect_lock: Arc<Semaphore>,
    environment_info_lock: Arc<Semaphore>,
    environment_info_load_state: Arc<AtomicU64>,
}

struct LazyRemoteExecServerClientState {
    // None means we do not know how to connect to this environment yet.
    transport_params: Option<Arc<ExecServerTransportParams>>,
    client: Option<ExecServerClient>,
    environment_info: Option<EnvironmentInfo>,
}

impl LazyRemoteExecServerClient {
    pub(crate) fn new(transport_params: ExecServerTransportParams) -> Self {
        Self::with_transport(Some(transport_params))
    }

    pub(crate) fn pending() -> Self {
        Self::with_transport(/*transport_params*/ None)
    }

    fn with_transport(transport_params: Option<ExecServerTransportParams>) -> Self {
        // The revision channel wakes pending callers and cancels stale connection attempts.
        let (transport_revision_tx, _transport_revision_rx) = watch::channel(0);
        Self {
            state: Arc::new(StdMutex::new(LazyRemoteExecServerClientState {
                transport_params: transport_params.map(Arc::new),
                client: None,
                environment_info: None,
            })),
            transport_revision_tx,
            connect_lock: Arc::new(Semaphore::new(/*permits*/ 1)),
            environment_info_lock: Arc::new(Semaphore::new(/*permits*/ 1)),
            environment_info_load_state: Arc::new(AtomicU64::new(0)),
        }
    }

    pub(crate) async fn get(&self) -> Result<ExecServerClient, ExecServerError> {
        loop {
            if let Some(client) = self.connected_client() {
                return Ok(client);
            }

            // Only one caller creates and publishes a connection; followers recheck the cache.
            let _connect_permit = self.connect_lock.acquire().await.map_err(|_| {
                ExecServerError::Protocol("exec-server connect lock closed".to_string())
            })?;
            if let Some(client) = self.connected_client() {
                return Ok(client);
            }

            // Wait here until upsert provides the environment's connection details.
            let transport_params = self.wait_for_transport().await?;
            if let Some(client) = self.cached_client()
                && matches!(
                    transport_params.as_ref(),
                    ExecServerTransportParams::StdioCommand { .. }
                )
            {
                return Ok(client);
            }
            let mut transport_revision_rx = self.transport_revision_tx.subscribe();
            if !self.is_current_transport(&transport_params) {
                continue;
            }
            // Stop waiting on an obsolete endpoint as soon as a replacement arrives.
            let next_client = tokio::select! {
                result = ExecServerClient::connect_for_transport(transport_params.as_ref().clone()) => {
                    match result {
                        Ok(client) => client,
                        Err(_err) if !self.is_current_transport(&transport_params) => continue,
                        Err(err) => return Err(err),
                    }
                }
                changed = transport_revision_rx.changed() => {
                    changed.map_err(|_| ExecServerError::Closed)?;
                    continue;
                }
            };

            // Publish only if this connection still belongs to the current transport.
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state
                .transport_params
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, &transport_params))
            {
                state.client = Some(next_client.clone());
                return Ok(next_client);
            }
        }
    }

    pub(crate) fn set_transport(&self, transport_params: ExecServerTransportParams) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state
            .transport_params
            .as_deref()
            .is_some_and(|current| current == &transport_params)
        {
            return;
        }
        state.transport_params = Some(Arc::new(transport_params));
        state.client = None;
        state.environment_info = None;
        drop(state);
        // Notify after releasing the state lock so woken callers can inspect it immediately.
        self.transport_revision_tx
            .send_modify(|revision| *revision += 1);
    }

    pub(crate) fn transport_params(&self) -> Option<Arc<ExecServerTransportParams>> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .transport_params
            .clone()
    }

    pub(crate) async fn wait_for_transport(
        &self,
    ) -> Result<Arc<ExecServerTransportParams>, ExecServerError> {
        let mut transport_revision_rx = self.transport_revision_tx.subscribe();
        loop {
            // This only waits for connection details; it does not connect yet.
            if let Some(transport_params) = self.transport_params() {
                return Ok(transport_params);
            }
            transport_revision_rx
                .changed()
                .await
                .map_err(|_| ExecServerError::Closed)?;
        }
    }

    fn is_current_transport(&self, transport_params: &Arc<ExecServerTransportParams>) -> bool {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .transport_params
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, transport_params))
    }

    fn connected_client(&self) -> Option<ExecServerClient> {
        self.cached_client()
            .filter(|client| !client.is_disconnected())
    }

    fn cached_client(&self) -> Option<ExecServerClient> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .client
            .clone()
    }
}

impl HttpClient for LazyRemoteExecServerClient {
    fn http_request(
        &self,
        params: crate::HttpRequestParams,
    ) -> BoxFuture<'_, Result<crate::HttpRequestResponse, ExecServerError>> {
        async move { self.get().await?.http_request(params).await }.boxed()
    }

    fn http_request_stream(
        &self,
        params: crate::HttpRequestParams,
    ) -> BoxFuture<
        '_,
        Result<(crate::HttpRequestResponse, crate::HttpResponseBodyStream), ExecServerError>,
    > {
        async move { self.get().await?.http_request_stream(params).await }.boxed()
    }
}

impl LazyRemoteExecServerClient {
    pub(crate) async fn environment_info(&self) -> Result<EnvironmentInfo, ExecServerError> {
        if let Some(info) = self.current_environment_info() {
            return Ok(info);
        }
        let _info_permit =
            self.environment_info_lock.acquire().await.map_err(|_| {
                ExecServerError::Protocol("environment info lock closed".to_string())
            })?;
        loop {
            if let Some(info) = self.current_environment_info() {
                return Ok(info);
            }
            let transport_params = self.wait_for_transport().await?;
            let client = self.get().await?;
            let mut transport_revision_rx = self.transport_revision_tx.subscribe();
            if !self.is_current_transport(&transport_params) {
                continue;
            }
            let info = tokio::select! {
                result = client.environment_info() => {
                    match result {
                        Ok(info) => info,
                        Err(_err) if !self.is_current_transport(&transport_params) => continue,
                        Err(err) => return Err(err),
                    }
                }
                changed = transport_revision_rx.changed() => {
                    changed.map_err(|_| ExecServerError::Closed)?;
                    continue;
                }
            };
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state
                .transport_params
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, &transport_params))
            {
                state.environment_info = Some(info.clone());
                return Ok(info);
            }
        }
    }

    pub(crate) fn current_environment_info(&self) -> Option<EnvironmentInfo> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .environment_info
            .clone()
    }

    pub(crate) fn try_begin_environment_info_load(&self) -> Option<u64> {
        if self.current_environment_info().is_some() {
            return None;
        }
        let token = self.environment_info_load_token();
        let mut state = self.environment_info_load_state.load(Ordering::Acquire);
        loop {
            if state == 0 {
                match self.environment_info_load_state.compare_exchange(
                    0,
                    token,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => return Some(token),
                    Err(next_state) => state = next_state,
                }
            } else {
                let pending_state = state | ENVIRONMENT_INFO_RELOAD_PENDING;
                if pending_state == state {
                    return None;
                }
                match self.environment_info_load_state.compare_exchange(
                    state,
                    pending_state,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => return None,
                    Err(next_state) => state = next_state,
                }
            }
        }
    }

    pub(crate) fn finish_environment_info_load(&self, token: u64) -> bool {
        let mut state = self.environment_info_load_state.load(Ordering::Acquire);
        loop {
            debug_assert_eq!(state & ENVIRONMENT_INFO_LOAD_TOKEN_MASK, token);
            let reload_requested = state & ENVIRONMENT_INFO_RELOAD_PENDING != 0;
            match self.environment_info_load_state.compare_exchange(
                state,
                0,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return reload_requested,
                Err(next_state) => state = next_state,
            }
        }
    }

    pub(crate) fn environment_info_load_token(&self) -> u64 {
        self.transport_revision_tx
            .borrow()
            .wrapping_add(1)
            .min(ENVIRONMENT_INFO_LOAD_TOKEN_MASK)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ExecServerError {
    #[error("failed to spawn exec-server: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("timed out connecting to exec-server websocket `{url}` after {timeout:?}")]
    WebSocketConnectTimeout { url: String, timeout: Duration },
    #[error("failed to connect to exec-server websocket `{url}`: {source}")]
    WebSocketConnect {
        url: String,
        #[source]
        source: tokio_tungstenite::tungstenite::Error,
    },
    #[error("timed out waiting for exec-server initialize handshake after {timeout:?}")]
    InitializeTimedOut { timeout: Duration },
    #[error("exec-server transport closed")]
    Closed,
    #[error("{0}")]
    Disconnected(String),
    #[error("failed to serialize or deserialize exec-server JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("HTTP request failed: {0}")]
    HttpRequest(String),
    #[error("exec-server protocol error: {0}")]
    Protocol(String),
    #[error("exec-server rejected request ({code}): {message}")]
    Server { code: i64, message: String },
    #[error("environment registry request failed ({status}{code_suffix}): {message}", code_suffix = .code.as_ref().map(|code| format!(", {code}")).unwrap_or_default())]
    EnvironmentRegistryHttp {
        status: reqwest::StatusCode,
        code: Option<String>,
        message: String,
    },
    #[error("environment registry configuration error: {0}")]
    EnvironmentRegistryConfig(String),
    #[error("environment registry authentication error: {0}")]
    EnvironmentRegistryAuth(String),
    #[error("environment registry request failed: {0}")]
    EnvironmentRegistryRequest(#[from] reqwest::Error),
}

impl ExecServerClient {
    pub async fn initialize(
        &self,
        options: ExecServerClientConnectOptions,
    ) -> Result<InitializeResponse, ExecServerError> {
        let ExecServerClientConnectOptions {
            client_name,
            initialize_timeout,
            resume_session_id,
        } = options;

        timeout(initialize_timeout, async {
            let response: InitializeResponse = self
                .inner
                .client
                .call(
                    INITIALIZE_METHOD,
                    &InitializeParams {
                        client_name,
                        resume_session_id,
                    },
                )
                .await?;
            {
                let mut session_id = self
                    .inner
                    .session_id
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                *session_id = Some(response.session_id.clone());
            }
            self.notify_initialized().await?;
            Ok(response)
        })
        .await
        .map_err(|_| ExecServerError::InitializeTimedOut {
            timeout: initialize_timeout,
        })?
    }

    pub async fn exec(&self, params: ExecParams) -> Result<ExecResponse, ExecServerError> {
        self.call(EXEC_METHOD, &params).await
    }

    pub async fn environment_info(&self) -> Result<EnvironmentInfo, ExecServerError> {
        self.call(ENVIRONMENT_INFO_METHOD, &()).await
    }

    pub async fn read(&self, params: ReadParams) -> Result<ReadResponse, ExecServerError> {
        self.call(EXEC_READ_METHOD, &params).await
    }

    pub async fn write(
        &self,
        process_id: &ProcessId,
        chunk: Vec<u8>,
    ) -> Result<WriteResponse, ExecServerError> {
        self.call(
            EXEC_WRITE_METHOD,
            &WriteParams {
                process_id: process_id.clone(),
                chunk: chunk.into(),
            },
        )
        .await
    }

    pub async fn signal(
        &self,
        process_id: &ProcessId,
        signal: ProcessSignal,
    ) -> Result<(), ExecServerError> {
        let _response: SignalResponse = self
            .call(
                EXEC_SIGNAL_METHOD,
                &SignalParams {
                    process_id: process_id.clone(),
                    signal,
                },
            )
            .await?;
        Ok(())
    }

    pub async fn terminate(
        &self,
        process_id: &ProcessId,
    ) -> Result<TerminateResponse, ExecServerError> {
        self.call(
            EXEC_TERMINATE_METHOD,
            &TerminateParams {
                process_id: process_id.clone(),
            },
        )
        .await
    }

    pub async fn fs_read_file(
        &self,
        params: FsReadFileParams,
    ) -> Result<FsReadFileResponse, ExecServerError> {
        self.call(FS_READ_FILE_METHOD, &params).await
    }

    pub async fn fs_write_file(
        &self,
        params: FsWriteFileParams,
    ) -> Result<FsWriteFileResponse, ExecServerError> {
        self.call(FS_WRITE_FILE_METHOD, &params).await
    }

    pub async fn fs_create_directory(
        &self,
        params: FsCreateDirectoryParams,
    ) -> Result<FsCreateDirectoryResponse, ExecServerError> {
        self.call(FS_CREATE_DIRECTORY_METHOD, &params).await
    }

    pub async fn fs_get_metadata(
        &self,
        params: FsGetMetadataParams,
    ) -> Result<FsGetMetadataResponse, ExecServerError> {
        self.call(FS_GET_METADATA_METHOD, &params).await
    }

    pub async fn fs_canonicalize(
        &self,
        params: FsCanonicalizeParams,
    ) -> Result<FsCanonicalizeResponse, ExecServerError> {
        self.call(FS_CANONICALIZE_METHOD, &params).await
    }

    pub async fn fs_read_directory(
        &self,
        params: FsReadDirectoryParams,
    ) -> Result<FsReadDirectoryResponse, ExecServerError> {
        self.call(FS_READ_DIRECTORY_METHOD, &params).await
    }

    pub async fn fs_remove(
        &self,
        params: FsRemoveParams,
    ) -> Result<FsRemoveResponse, ExecServerError> {
        self.call(FS_REMOVE_METHOD, &params).await
    }

    pub async fn fs_copy(&self, params: FsCopyParams) -> Result<FsCopyResponse, ExecServerError> {
        self.call(FS_COPY_METHOD, &params).await
    }

    pub(crate) async fn register_session(
        &self,
        process_id: &ProcessId,
    ) -> Result<Session, ExecServerError> {
        let state = Arc::new(SessionState::new());
        self.inner
            .insert_session(process_id, Arc::clone(&state))
            .await?;
        Ok(Session {
            client: self.clone(),
            process_id: process_id.clone(),
            state,
        })
    }

    pub(crate) async fn unregister_session(&self, process_id: &ProcessId) {
        self.inner.remove_session(process_id).await;
    }

    pub fn session_id(&self) -> Option<String> {
        self.inner
            .session_id
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn is_disconnected(&self) -> bool {
        self.inner.disconnected.get().is_some() || self.inner.client.is_disconnected()
    }

    pub(crate) async fn connect(
        connection: JsonRpcConnection,
        options: ExecServerClientConnectOptions,
    ) -> Result<Self, ExecServerError> {
        let (rpc_client, mut events_rx) = RpcClient::new(connection);
        let inner = Arc::new_cyclic(|weak| {
            let weak = weak.clone();
            let reader_task = tokio::spawn(async move {
                while let Some(event) = events_rx.recv().await {
                    match event {
                        RpcClientEvent::Notification(notification) => {
                            if let Some(inner) = weak.upgrade()
                                && let Err(err) =
                                    handle_server_notification(&inner, notification).await
                            {
                                let message = record_disconnected(
                                    &inner,
                                    format!("exec-server notification handling failed: {err}"),
                                );
                                fail_all_in_flight_work(&inner, message).await;
                                return;
                            }
                        }
                        RpcClientEvent::Disconnected { reason } => {
                            if let Some(inner) = weak.upgrade() {
                                let message = record_disconnected(
                                    &inner,
                                    disconnected_message(reason.as_deref()),
                                );
                                fail_all_in_flight_work(&inner, message).await;
                            }
                            return;
                        }
                    }
                }
            });

            Inner {
                client: rpc_client,
                sessions: ArcSwap::from_pointee(HashMap::new()),
                sessions_write_lock: Mutex::new(()),
                disconnected: OnceLock::new(),
                http_body_streams: ArcSwap::from_pointee(HashMap::new()),
                http_body_stream_failures: ArcSwap::from_pointee(HashMap::new()),
                http_body_streams_write_lock: Mutex::new(()),
                http_body_stream_next_id: AtomicU64::new(1),
                session_id: std::sync::RwLock::new(None),
                reader_task,
            }
        });

        let client = Self { inner };
        client.initialize(options).await?;
        Ok(client)
    }

    async fn notify_initialized(&self) -> Result<(), ExecServerError> {
        self.inner
            .client
            .notify(INITIALIZED_METHOD, &serde_json::json!({}))
            .await
            .map_err(ExecServerError::Json)
    }

    async fn call<P, T>(&self, method: &str, params: &P) -> Result<T, ExecServerError>
    where
        P: serde::Serialize,
        T: serde::de::DeserializeOwned,
    {
        // Reject new work before allocating a JSON-RPC request id. MCP tool
        // calls, process writes, and fs operations all pass through here, so
        // this is the shared low-level failure path after environment disconnect.
        if let Some(error) = self.inner.disconnected_error() {
            return Err(error);
        }

        match self.inner.client.call(method, params).await {
            Ok(response) => Ok(response),
            Err(error) => {
                let error = ExecServerError::from(error);
                if is_transport_closed_error(&error) {
                    // A call can race with disconnect after the preflight
                    // check. Only the reader task drains sessions so queued
                    // process notifications stay ordered before disconnect.
                    let message = disconnected_message(/*reason*/ None);
                    let message = record_disconnected(&self.inner, message);
                    Err(ExecServerError::Disconnected(message))
                } else {
                    Err(error)
                }
            }
        }
    }
}

impl From<RpcCallError> for ExecServerError {
    fn from(value: RpcCallError) -> Self {
        match value {
            RpcCallError::Closed => Self::Closed,
            RpcCallError::Json(err) => Self::Json(err),
            RpcCallError::Server(error) => Self::Server {
                code: error.code,
                message: error.message,
            },
        }
    }
}

impl SessionState {
    fn new() -> Self {
        let (wake_tx, _wake_rx) = watch::channel(0);
        Self {
            wake_tx,
            events: ExecProcessEventLog::new(
                PROCESS_EVENT_CHANNEL_CAPACITY,
                PROCESS_EVENT_RETAINED_BYTES,
            ),
            ordered_events: StdMutex::new(OrderedSessionEvents::default()),
            failure: Mutex::new(None),
        }
    }

    pub(crate) fn subscribe(&self) -> watch::Receiver<u64> {
        self.wake_tx.subscribe()
    }

    pub(crate) fn subscribe_events(&self) -> ExecProcessEventReceiver {
        self.events.subscribe()
    }

    fn note_change(&self, seq: u64) {
        let next = (*self.wake_tx.borrow()).max(seq);
        let _ = self.wake_tx.send(next);
    }

    /// Publishes a process event only when all earlier sequenced events have
    /// already been published.
    ///
    /// Returns `true` only when this call actually publishes the ordered
    /// `Closed` event. The caller uses that signal to remove the session route
    /// after the terminal event is visible to subscribers, rather than when a
    /// possibly-early closed notification first arrives.
    fn publish_ordered_event(&self, event: ExecProcessEvent) -> bool {
        let Some(seq) = event.seq() else {
            self.events.publish(event);
            return false;
        };

        let mut ready = Vec::new();
        {
            let mut ordered_events = self
                .ordered_events
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            // We have already delivered this sequence number or moved past it,
            // so accepting it again would duplicate output or lifecycle events.
            if seq <= ordered_events.last_published_seq {
                return false;
            }

            ordered_events.pending.entry(seq).or_insert(event);
            loop {
                let next_seq = ordered_events.last_published_seq + 1;
                let Some(event) = ordered_events.pending.remove(&next_seq) else {
                    break;
                };
                ordered_events.last_published_seq += 1;
                ready.push(event);
            }
        }

        let mut published_closed = false;
        for event in ready {
            published_closed |= matches!(&event, ExecProcessEvent::Closed { .. });
            self.events.publish(event);
        }
        published_closed
    }

    async fn set_failure(&self, message: String) {
        let mut failure = self.failure.lock().await;
        let should_publish = failure.is_none();
        if should_publish {
            *failure = Some(message.clone());
        }
        drop(failure);
        let next = (*self.wake_tx.borrow()).saturating_add(1);
        let _ = self.wake_tx.send(next);
        if should_publish {
            let _ = self.publish_ordered_event(ExecProcessEvent::Failed(message));
        }
    }

    async fn failed_response(&self) -> Option<ReadResponse> {
        self.failure
            .lock()
            .await
            .clone()
            .map(|message| self.synthesized_failure(message))
    }

    fn synthesized_failure(&self, message: String) -> ReadResponse {
        let next_seq = (*self.wake_tx.borrow()).saturating_add(1);
        ReadResponse {
            chunks: Vec::new(),
            next_seq,
            exited: true,
            exit_code: None,
            closed: true,
            failure: Some(message),
        }
    }
}

impl Session {
    pub(crate) fn process_id(&self) -> &ProcessId {
        &self.process_id
    }

    pub(crate) fn subscribe_wake(&self) -> watch::Receiver<u64> {
        self.state.subscribe()
    }

    pub(crate) fn subscribe_events(&self) -> ExecProcessEventReceiver {
        self.state.subscribe_events()
    }

    pub(crate) async fn read(
        &self,
        after_seq: Option<u64>,
        max_bytes: Option<usize>,
        wait_ms: Option<u64>,
    ) -> Result<ReadResponse, ExecServerError> {
        if let Some(response) = self.state.failed_response().await {
            return Ok(response);
        }

        match self
            .client
            .read(ReadParams {
                process_id: self.process_id.clone(),
                after_seq,
                max_bytes,
                wait_ms,
            })
            .await
        {
            Ok(response) => Ok(response),
            Err(err) if is_transport_closed_error(&err) => {
                let message = disconnected_message(/*reason*/ None);
                self.state.set_failure(message.clone()).await;
                Ok(self.state.synthesized_failure(message))
            }
            Err(err) => Err(err),
        }
    }

    pub(crate) async fn write(&self, chunk: Vec<u8>) -> Result<WriteResponse, ExecServerError> {
        self.client.write(&self.process_id, chunk).await
    }

    pub(crate) async fn signal(&self, signal: ProcessSignal) -> Result<(), ExecServerError> {
        self.client.signal(&self.process_id, signal).await
    }

    pub(crate) async fn terminate(&self) -> Result<(), ExecServerError> {
        self.client.terminate(&self.process_id).await?;
        Ok(())
    }

    pub(crate) async fn unregister(&self) {
        self.client.unregister_session(&self.process_id).await;
    }
}

impl Inner {
    fn disconnected_error(&self) -> Option<ExecServerError> {
        self.disconnected
            .get()
            .cloned()
            .map(ExecServerError::Disconnected)
    }

    fn set_disconnected(&self, message: String) -> Option<String> {
        match self.disconnected.set(message.clone()) {
            Ok(()) => Some(message),
            Err(_) => None,
        }
    }

    fn get_session(&self, process_id: &ProcessId) -> Option<Arc<SessionState>> {
        self.sessions.load().get(process_id).cloned()
    }

    async fn insert_session(
        &self,
        process_id: &ProcessId,
        session: Arc<SessionState>,
    ) -> Result<(), ExecServerError> {
        let _sessions_write_guard = self.sessions_write_lock.lock().await;
        // Do not register a process session that can never receive environment
        // notifications. Without this check, remote MCP startup could create a
        // dead session and wait for process output that will never arrive.
        if let Some(error) = self.disconnected_error() {
            return Err(error);
        }
        let sessions = self.sessions.load();
        if sessions.contains_key(process_id) {
            return Err(ExecServerError::Protocol(format!(
                "session already registered for process {process_id}"
            )));
        }
        let mut next_sessions = sessions.as_ref().clone();
        next_sessions.insert(process_id.clone(), session);
        self.sessions.store(Arc::new(next_sessions));
        Ok(())
    }

    async fn remove_session(&self, process_id: &ProcessId) -> Option<Arc<SessionState>> {
        let _sessions_write_guard = self.sessions_write_lock.lock().await;
        let sessions = self.sessions.load();
        let session = sessions.get(process_id).cloned();
        session.as_ref()?;
        let mut next_sessions = sessions.as_ref().clone();
        next_sessions.remove(process_id);
        self.sessions.store(Arc::new(next_sessions));
        session
    }

    async fn take_all_sessions(&self) -> HashMap<ProcessId, Arc<SessionState>> {
        let _sessions_write_guard = self.sessions_write_lock.lock().await;
        let sessions = self.sessions.load();
        let drained_sessions = sessions.as_ref().clone();
        self.sessions.store(Arc::new(HashMap::new()));
        drained_sessions
    }
}

fn disconnected_message(reason: Option<&str>) -> String {
    match reason {
        Some(reason) => format!("exec-server transport disconnected: {reason}"),
        None => "exec-server transport disconnected".to_string(),
    }
}

fn is_transport_closed_error(error: &ExecServerError) -> bool {
    matches!(
        error,
        ExecServerError::Closed | ExecServerError::Disconnected(_)
    ) || matches!(
        error,
        ExecServerError::Server {
            code: -32000,
            message,
        } if message == "JSON-RPC transport closed"
    )
}

fn record_disconnected(inner: &Arc<Inner>, message: String) -> String {
    // The first observer records the canonical disconnect reason. Session
    // draining stays with the reader task so it can preserve notification
    // ordering before publishing the terminal failure.
    if let Some(message) = inner.set_disconnected(message.clone()) {
        message
    } else {
        inner.disconnected.get().cloned().unwrap_or(message)
    }
}

async fn fail_all_sessions(inner: &Arc<Inner>, message: String) {
    let sessions = inner.take_all_sessions().await;

    for (_, session) in sessions {
        // Sessions synthesize a closed read response and emit a pushed Failed
        // event. That covers both polling consumers and streaming consumers
        // such as environment-backed MCP stdio.
        session.set_failure(message.clone()).await;
    }
}

/// Fails all in-flight work that depends on the shared JSON-RPC transport.
async fn fail_all_in_flight_work(inner: &Arc<Inner>, message: String) {
    fail_all_sessions(inner, message.clone()).await;
    inner.fail_all_http_body_streams(message).await;
}

async fn handle_server_notification(
    inner: &Arc<Inner>,
    notification: JSONRPCNotification,
) -> Result<(), ExecServerError> {
    match notification.method.as_str() {
        EXEC_OUTPUT_DELTA_METHOD => {
            let params: ExecOutputDeltaNotification =
                serde_json::from_value(notification.params.unwrap_or(Value::Null))?;
            if let Some(session) = inner.get_session(&params.process_id) {
                session.note_change(params.seq);
                let published_closed =
                    session.publish_ordered_event(ExecProcessEvent::Output(ProcessOutputChunk {
                        seq: params.seq,
                        stream: params.stream,
                        chunk: params.chunk,
                    }));
                if published_closed {
                    inner.remove_session(&params.process_id).await;
                }
            }
        }
        EXEC_EXITED_METHOD => {
            let params: ExecExitedNotification =
                serde_json::from_value(notification.params.unwrap_or(Value::Null))?;
            if let Some(session) = inner.get_session(&params.process_id) {
                session.note_change(params.seq);
                let published_closed = session.publish_ordered_event(ExecProcessEvent::Exited {
                    seq: params.seq,
                    exit_code: params.exit_code,
                });
                if published_closed {
                    inner.remove_session(&params.process_id).await;
                }
            }
        }
        EXEC_CLOSED_METHOD => {
            let params: ExecClosedNotification =
                serde_json::from_value(notification.params.unwrap_or(Value::Null))?;
            if let Some(session) = inner.get_session(&params.process_id) {
                session.note_change(params.seq);
                // Closed is terminal, but it can arrive before tail output or
                // exited. Keep routing this process until the ordered publisher
                // says Closed has actually been delivered.
                let published_closed =
                    session.publish_ordered_event(ExecProcessEvent::Closed { seq: params.seq });
                if published_closed {
                    inner.remove_session(&params.process_id).await;
                }
            }
        }
        HTTP_REQUEST_BODY_DELTA_METHOD => {
            inner
                .handle_http_body_delta_notification(notification.params)
                .await?;
        }
        other => {
            debug!("ignoring unknown exec-server notification: {other}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use codex_app_server_protocol::JSONRPCMessage;
    use codex_app_server_protocol::JSONRPCNotification;
    use codex_app_server_protocol::JSONRPCResponse;
    use futures::SinkExt;
    use futures::StreamExt;
    use pretty_assertions::assert_eq;
    use std::collections::HashMap;
    #[cfg(unix)]
    use std::path::Path;
    #[cfg(unix)]
    use std::process::Command;
    use std::sync::Arc;
    use tokio::io::AsyncBufReadExt;
    use tokio::io::AsyncWrite;
    use tokio::io::AsyncWriteExt;
    use tokio::io::BufReader;
    use tokio::io::duplex;
    use tokio::net::TcpListener;
    use tokio::net::TcpStream;
    use tokio::sync::Barrier;
    use tokio::sync::mpsc;
    use tokio::sync::oneshot;
    use tokio::time::Duration;
    #[cfg(unix)]
    use tokio::time::sleep;
    use tokio::time::timeout;
    use tokio_tungstenite::WebSocketStream;
    use tokio_tungstenite::accept_async;
    use tokio_tungstenite::tungstenite::Message;

    use super::ExecServerClient;
    use super::ExecServerClientConnectOptions;
    use super::LazyRemoteExecServerClient;
    use crate::ProcessId;
    #[cfg(not(windows))]
    use crate::client_api::DEFAULT_REMOTE_EXEC_SERVER_INITIALIZE_TIMEOUT;
    use crate::client_api::ExecServerTransportParams;
    use crate::client_api::StdioExecServerCommand;
    use crate::client_api::StdioExecServerConnectArgs;
    use crate::connection::JsonRpcConnection;
    use crate::process::ExecProcessEvent;
    use crate::protocol::EXEC_CLOSED_METHOD;
    use crate::protocol::EXEC_EXITED_METHOD;
    use crate::protocol::EXEC_OUTPUT_DELTA_METHOD;
    use crate::protocol::ExecClosedNotification;
    use crate::protocol::ExecExitedNotification;
    use crate::protocol::ExecOutputDeltaNotification;
    use crate::protocol::ExecOutputStream;
    use crate::protocol::INITIALIZE_METHOD;
    use crate::protocol::INITIALIZED_METHOD;
    use crate::protocol::InitializeResponse;
    use crate::protocol::ProcessOutputChunk;

    async fn read_jsonrpc_line<R>(lines: &mut tokio::io::Lines<BufReader<R>>) -> JSONRPCMessage
    where
        R: tokio::io::AsyncRead + Unpin,
    {
        let line = timeout(Duration::from_secs(1), lines.next_line())
            .await
            .expect("json-rpc read should not time out")
            .expect("json-rpc read should succeed")
            .expect("json-rpc connection should stay open");
        serde_json::from_str(&line).expect("json-rpc line should parse")
    }

    async fn write_jsonrpc_line<W>(writer: &mut W, message: JSONRPCMessage)
    where
        W: AsyncWrite + Unpin,
    {
        let encoded = serde_json::to_string(&message).expect("json-rpc message should serialize");
        writer
            .write_all(format!("{encoded}\n").as_bytes())
            .await
            .expect("json-rpc line should write");
    }

    async fn accept_websocket(listener: &TcpListener) -> WebSocketStream<TcpStream> {
        let (stream, _) = listener.accept().await.expect("listener should accept");
        accept_async(stream)
            .await
            .expect("websocket handshake should succeed")
    }

    async fn read_jsonrpc_websocket(websocket: &mut WebSocketStream<TcpStream>) -> JSONRPCMessage {
        loop {
            match timeout(Duration::from_secs(1), websocket.next())
                .await
                .expect("json-rpc websocket read should not time out")
                .expect("websocket should stay open")
                .expect("websocket frame should read")
            {
                Message::Text(text) => {
                    return serde_json::from_str(text.as_ref())
                        .expect("json-rpc text frame should parse");
                }
                Message::Binary(bytes) => {
                    return serde_json::from_slice(bytes.as_ref())
                        .expect("json-rpc binary frame should parse");
                }
                Message::Ping(_) | Message::Pong(_) => {}
                other => panic!("expected json-rpc websocket frame, got {other:?}"),
            }
        }
    }

    async fn write_jsonrpc_websocket(
        websocket: &mut WebSocketStream<TcpStream>,
        message: JSONRPCMessage,
    ) {
        let encoded = serde_json::to_string(&message).expect("json-rpc should serialize");
        websocket
            .send(Message::Text(encoded.into()))
            .await
            .expect("json-rpc websocket frame should write");
    }

    async fn complete_websocket_initialize(
        websocket: &mut WebSocketStream<TcpStream>,
        session_id: &str,
        expected_resume_session_id: Option<&str>,
    ) {
        let initialize = read_jsonrpc_websocket(websocket).await;
        let request = match initialize {
            JSONRPCMessage::Request(request) if request.method == INITIALIZE_METHOD => request,
            other => panic!("expected initialize request, got {other:?}"),
        };
        let params: crate::protocol::InitializeParams =
            serde_json::from_value(request.params.expect("initialize params should exist"))
                .expect("initialize params should deserialize");
        assert_eq!(
            params.resume_session_id.as_deref(),
            expected_resume_session_id
        );
        write_jsonrpc_websocket(
            websocket,
            JSONRPCMessage::Response(JSONRPCResponse {
                id: request.id,
                result: serde_json::to_value(InitializeResponse {
                    session_id: session_id.to_string(),
                })
                .expect("initialize response should serialize"),
            }),
        )
        .await;

        let initialized = read_jsonrpc_websocket(websocket).await;
        match initialized {
            JSONRPCMessage::Notification(notification)
                if notification.method == INITIALIZED_METHOD => {}
            other => panic!("expected initialized notification, got {other:?}"),
        }
    }

    async fn wait_for_disconnect(client: &ExecServerClient) {
        timeout(Duration::from_secs(1), async {
            loop {
                if client.is_disconnected() {
                    return;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("client should observe disconnect");
    }

    #[cfg(not(windows))]
    #[tokio::test]
    async fn connect_stdio_command_initializes_json_rpc_client() {
        let client = ExecServerClient::connect_stdio_command(StdioExecServerConnectArgs {
            command: StdioExecServerCommand {
                program: "sh".to_string(),
                args: vec![
                    "-c".to_string(),
                    "read _line; printf '%s\\n' '{\"id\":1,\"result\":{\"sessionId\":\"stdio-test\"}}'; read _line; sleep 60".to_string(),
                ],
                env: HashMap::new(),
                cwd: None,
            },
            client_name: "stdio-test-client".to_string(),
            initialize_timeout: Duration::from_secs(1),
            resume_session_id: None,
        })
        .await
        .expect("stdio client should connect");

        assert_eq!(client.session_id().as_deref(), Some("stdio-test"));
    }

    #[cfg(not(windows))]
    #[tokio::test]
    async fn connect_for_transport_initializes_stdio_command() {
        let client = ExecServerClient::connect_for_transport(
            ExecServerTransportParams::StdioCommand {
                command: StdioExecServerCommand {
                    program: "sh".to_string(),
                    args: vec![
                        "-c".to_string(),
                        "read _line; printf '%s\\n' '{\"id\":1,\"result\":{\"sessionId\":\"stdio-test\"}}'; read _line; sleep 60".to_string(),
                    ],
                    env: HashMap::new(),
                    cwd: None,
                },
                initialize_timeout: DEFAULT_REMOTE_EXEC_SERVER_INITIALIZE_TIMEOUT,
            },
        )
        .await
        .expect("stdio transport should connect");

        assert_eq!(client.session_id().as_deref(), Some("stdio-test"));
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn connect_stdio_command_initializes_json_rpc_client_on_windows() {
        let client = ExecServerClient::connect_stdio_command(StdioExecServerConnectArgs {
            command: StdioExecServerCommand {
                program: "powershell".to_string(),
                args: vec![
                    "-NoProfile".to_string(),
                    "-Command".to_string(),
                    "$null = [Console]::In.ReadLine(); [Console]::Out.WriteLine('{\"id\":1,\"result\":{\"sessionId\":\"stdio-test\"}}'); $null = [Console]::In.ReadLine(); Start-Sleep -Seconds 60".to_string(),
                ],
                env: HashMap::new(),
                cwd: None,
            },
            client_name: "stdio-test-client".to_string(),
            initialize_timeout: Duration::from_secs(1),
            resume_session_id: None,
        })
        .await
        .expect("stdio client should connect");

        assert_eq!(client.session_id().as_deref(), Some("stdio-test"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn dropping_stdio_client_terminates_spawned_process() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let pid_file = tempdir.path().join("server.pid");
        let child_pid_file = tempdir.path().join("server-child.pid");
        let stdio_script = format!(
            "read _line; \
             echo \"$$\" > {}; \
             sleep 60 >/dev/null 2>&1 & echo \"$!\" > {}; \
             printf '%s\\n' '{{\"id\":1,\"result\":{{\"sessionId\":\"stdio-test\"}}}}'; \
             read _line; \
             wait",
            shell_quote(pid_file.as_path()),
            shell_quote(child_pid_file.as_path()),
        );

        let client = ExecServerClient::connect_stdio_command(StdioExecServerConnectArgs {
            command: StdioExecServerCommand {
                program: "sh".to_string(),
                args: vec!["-c".to_string(), stdio_script],
                env: HashMap::new(),
                cwd: None,
            },
            client_name: "stdio-test-client".to_string(),
            initialize_timeout: Duration::from_secs(1),
            resume_session_id: None,
        })
        .await
        .expect("stdio client should connect");
        let server_pid = read_pid_file(pid_file.as_path()).await;
        let child_pid = read_pid_file(child_pid_file.as_path()).await;
        assert!(
            process_exists(server_pid),
            "spawned stdio process should be running before client drop"
        );
        assert!(
            process_exists(child_pid),
            "spawned stdio child process should be running before client drop"
        );

        drop(client);

        wait_for_process_exit(server_pid).await;
        wait_for_process_exit(child_pid).await;
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn malformed_stdio_message_terminates_spawned_process() {
        let tempdir = tempfile::tempdir().expect("tempdir should be created");
        let pid_file = tempdir.path().join("server.pid");
        let stdio_script = format!(
            "read _line; \
             echo \"$$\" > {}; \
             printf '%s\\n' 'not-json'; \
             sleep 60",
            shell_quote(pid_file.as_path()),
        );

        let result = ExecServerClient::connect_stdio_command(StdioExecServerConnectArgs {
            command: StdioExecServerCommand {
                program: "sh".to_string(),
                args: vec!["-c".to_string(), stdio_script],
                env: HashMap::new(),
                cwd: None,
            },
            client_name: "stdio-test-client".to_string(),
            initialize_timeout: Duration::from_secs(1),
            resume_session_id: None,
        })
        .await;
        assert!(result.is_err(), "malformed stdio server should not connect");

        let server_pid = read_pid_file(pid_file.as_path()).await;
        wait_for_process_exit(server_pid).await;
    }

    #[cfg(unix)]
    async fn read_pid_file(path: &Path) -> u32 {
        for _ in 0..20 {
            if let Ok(contents) = std::fs::read_to_string(path) {
                return contents
                    .trim()
                    .parse()
                    .expect("pid file should contain a pid");
            }
            sleep(Duration::from_millis(50)).await;
        }
        panic!("pid file {} should be written", path.display());
    }

    #[cfg(unix)]
    async fn wait_for_process_exit(pid: u32) {
        for _ in 0..20 {
            if !process_exists(pid) {
                return;
            }
            sleep(Duration::from_millis(100)).await;
        }
        panic!("process {pid} should exit");
    }

    #[cfg(unix)]
    fn process_exists(pid: u32) -> bool {
        Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .status()
            .is_ok_and(|status| status.success())
    }

    #[cfg(unix)]
    fn shell_quote(path: &Path) -> String {
        let value = path.to_string_lossy();
        format!("'{}'", value.replace('\'', "'\\''"))
    }

    #[tokio::test]
    async fn process_events_are_delivered_in_seq_order_when_notifications_are_reordered() {
        let (client_stdin, server_reader) = duplex(1 << 20);
        let (mut server_writer, client_stdout) = duplex(1 << 20);
        let (notifications_tx, mut notifications_rx) = mpsc::channel(16);
        let server = tokio::spawn(async move {
            let mut lines = BufReader::new(server_reader).lines();
            let initialize = read_jsonrpc_line(&mut lines).await;
            let request = match initialize {
                JSONRPCMessage::Request(request) if request.method == INITIALIZE_METHOD => request,
                other => panic!("expected initialize request, got {other:?}"),
            };
            write_jsonrpc_line(
                &mut server_writer,
                JSONRPCMessage::Response(JSONRPCResponse {
                    id: request.id,
                    result: serde_json::to_value(InitializeResponse {
                        session_id: "session-1".to_string(),
                    })
                    .expect("initialize response should serialize"),
                }),
            )
            .await;

            let initialized = read_jsonrpc_line(&mut lines).await;
            match initialized {
                JSONRPCMessage::Notification(notification)
                    if notification.method == INITIALIZED_METHOD => {}
                other => panic!("expected initialized notification, got {other:?}"),
            }

            while let Some(message) = notifications_rx.recv().await {
                write_jsonrpc_line(&mut server_writer, message).await;
            }
        });

        let client = ExecServerClient::connect(
            JsonRpcConnection::from_stdio(
                client_stdout,
                client_stdin,
                "test-exec-server-client".to_string(),
            ),
            ExecServerClientConnectOptions::default(),
        )
        .await
        .expect("client should connect");

        let process_id = ProcessId::from("reordered");
        let session = client
            .register_session(&process_id)
            .await
            .expect("session should register");
        let mut events = session.subscribe_events();

        for message in [
            JSONRPCMessage::Notification(JSONRPCNotification {
                method: EXEC_CLOSED_METHOD.to_string(),
                params: Some(
                    serde_json::to_value(ExecClosedNotification {
                        process_id: process_id.clone(),
                        seq: 4,
                    })
                    .expect("closed notification should serialize"),
                ),
            }),
            JSONRPCMessage::Notification(JSONRPCNotification {
                method: EXEC_OUTPUT_DELTA_METHOD.to_string(),
                params: Some(
                    serde_json::to_value(ExecOutputDeltaNotification {
                        process_id: process_id.clone(),
                        seq: 1,
                        stream: ExecOutputStream::Stdout,
                        chunk: b"one".to_vec().into(),
                    })
                    .expect("output notification should serialize"),
                ),
            }),
            JSONRPCMessage::Notification(JSONRPCNotification {
                method: EXEC_EXITED_METHOD.to_string(),
                params: Some(
                    serde_json::to_value(ExecExitedNotification {
                        process_id: process_id.clone(),
                        seq: 3,
                        exit_code: 0,
                    })
                    .expect("exit notification should serialize"),
                ),
            }),
            JSONRPCMessage::Notification(JSONRPCNotification {
                method: EXEC_OUTPUT_DELTA_METHOD.to_string(),
                params: Some(
                    serde_json::to_value(ExecOutputDeltaNotification {
                        process_id: process_id.clone(),
                        seq: 2,
                        stream: ExecOutputStream::Stderr,
                        chunk: b"two".to_vec().into(),
                    })
                    .expect("output notification should serialize"),
                ),
            }),
        ] {
            notifications_tx
                .send(message)
                .await
                .expect("notification should queue");
        }

        let mut delivered = Vec::new();
        for _ in 0..4 {
            delivered.push(
                timeout(Duration::from_secs(1), events.recv())
                    .await
                    .expect("process event should not time out")
                    .expect("process event stream should stay open"),
            );
        }

        assert_eq!(
            delivered,
            vec![
                ExecProcessEvent::Output(ProcessOutputChunk {
                    seq: 1,
                    stream: ExecOutputStream::Stdout,
                    chunk: b"one".to_vec().into(),
                }),
                ExecProcessEvent::Output(ProcessOutputChunk {
                    seq: 2,
                    stream: ExecOutputStream::Stderr,
                    chunk: b"two".to_vec().into(),
                }),
                ExecProcessEvent::Exited {
                    seq: 3,
                    exit_code: 0,
                },
                ExecProcessEvent::Closed { seq: 4 },
            ]
        );

        drop(notifications_tx);
        drop(client);
        server.await.expect("server task should finish");
    }

    #[tokio::test]
    async fn transport_disconnect_fails_sessions_and_rejects_new_sessions() {
        let (client_stdin, server_reader) = duplex(1 << 20);
        let (mut server_writer, client_stdout) = duplex(1 << 20);
        let (disconnect_tx, disconnect_rx) = oneshot::channel();
        let server = tokio::spawn(async move {
            let mut lines = BufReader::new(server_reader).lines();
            let initialize = read_jsonrpc_line(&mut lines).await;
            let request = match initialize {
                JSONRPCMessage::Request(request) if request.method == INITIALIZE_METHOD => request,
                other => panic!("expected initialize request, got {other:?}"),
            };
            write_jsonrpc_line(
                &mut server_writer,
                JSONRPCMessage::Response(JSONRPCResponse {
                    id: request.id,
                    result: serde_json::to_value(InitializeResponse {
                        session_id: "session-1".to_string(),
                    })
                    .expect("initialize response should serialize"),
                }),
            )
            .await;

            let initialized = read_jsonrpc_line(&mut lines).await;
            match initialized {
                JSONRPCMessage::Notification(notification)
                    if notification.method == INITIALIZED_METHOD => {}
                other => panic!("expected initialized notification, got {other:?}"),
            }

            let _ = disconnect_rx.await;
            drop(server_writer);
        });

        let client = ExecServerClient::connect(
            JsonRpcConnection::from_stdio(
                client_stdout,
                client_stdin,
                "test-exec-server-client".to_string(),
            ),
            ExecServerClientConnectOptions::default(),
        )
        .await
        .expect("client should connect");

        let process_id = ProcessId::from("disconnect");
        let session = client
            .register_session(&process_id)
            .await
            .expect("session should register");
        let mut events = session.subscribe_events();

        disconnect_tx.send(()).expect("disconnect should signal");

        let event = timeout(Duration::from_secs(1), events.recv())
            .await
            .expect("session failure should not time out")
            .expect("session event stream should stay open");
        let ExecProcessEvent::Failed(message) = event else {
            panic!("expected session failure after disconnect, got {event:?}");
        };
        assert_eq!(message, "exec-server transport disconnected");

        let response = session
            .read(
                /*after_seq*/ None, /*max_bytes*/ None, /*wait_ms*/ None,
            )
            .await
            .expect("disconnected session read should synthesize a response");
        assert_eq!(
            response.failure.as_deref(),
            Some("exec-server transport disconnected")
        );
        assert!(response.closed);

        let new_session = client.register_session(&ProcessId::from("new")).await;
        assert!(matches!(
            new_session,
            Err(super::ExecServerError::Disconnected(_))
        ));

        drop(client);
        server.await.expect("server task should finish");
    }

    #[tokio::test]
    async fn remote_websocket_client_replaces_disconnected_client_with_fresh_session() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let websocket_url = format!(
            "ws://{}",
            listener.local_addr().expect("listener should have address")
        );
        let server = tokio::spawn({
            async move {
                let mut first = accept_websocket(&listener).await;
                complete_websocket_initialize(
                    &mut first,
                    "session-1",
                    /*expected_resume_session_id*/ None,
                )
                .await;
                first
                    .close(None)
                    .await
                    .expect("first websocket should close");

                let mut second = accept_websocket(&listener).await;
                complete_websocket_initialize(
                    &mut second,
                    "session-2",
                    /*expected_resume_session_id*/ None,
                )
                .await;
            }
        });

        let client = LazyRemoteExecServerClient::new(ExecServerTransportParams::WebSocketUrl {
            websocket_url,
            connect_timeout: Duration::from_secs(1),
            initialize_timeout: Duration::from_secs(1),
        });
        let first = client.get().await.expect("first client should connect");
        wait_for_disconnect(&first).await;

        let (replacement_a, replacement_b) = tokio::join!(client.get(), client.get());
        let replacement_a = replacement_a.expect("first replacement should connect");
        let replacement_b = replacement_b.expect("second replacement should reuse client");
        assert_eq!(replacement_a.session_id().as_deref(), Some("session-2"));
        assert_eq!(replacement_b.session_id().as_deref(), Some("session-2"));
        assert!(Arc::ptr_eq(&replacement_a.inner, &replacement_b.inner));

        server.await.expect("server task should finish");
    }

    #[tokio::test]
    async fn remote_websocket_client_cancels_stale_connection_after_transport_update() {
        let stale_listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("stale listener should bind");
        let stale_websocket_url = format!(
            "ws://{}",
            stale_listener
                .local_addr()
                .expect("stale listener should have address")
        );
        let (stale_accepted_tx, stale_accepted_rx) = oneshot::channel();
        let (release_stale_tx, release_stale_rx) = oneshot::channel();
        let stale_server = tokio::spawn(async move {
            let _stale_websocket = accept_websocket(&stale_listener).await;
            stale_accepted_tx
                .send(())
                .expect("stale connection should signal");
            let _ = release_stale_rx.await;
        });

        let ready_listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("ready listener should bind");
        let ready_websocket_url = format!(
            "ws://{}",
            ready_listener
                .local_addr()
                .expect("ready listener should have address")
        );
        let ready_server = tokio::spawn(async move {
            let mut websocket = accept_websocket(&ready_listener).await;
            complete_websocket_initialize(
                &mut websocket,
                "ready-session",
                /*expected_resume_session_id*/ None,
            )
            .await;
        });

        let client = LazyRemoteExecServerClient::new(ExecServerTransportParams::WebSocketUrl {
            websocket_url: stale_websocket_url,
            connect_timeout: Duration::from_secs(1),
            initialize_timeout: Duration::from_secs(60),
        });
        let connecting_client = client.clone();
        let connect_task = tokio::spawn(async move { connecting_client.get().await });
        stale_accepted_rx
            .await
            .expect("stale connection should be accepted");

        client.set_transport(ExecServerTransportParams::WebSocketUrl {
            websocket_url: ready_websocket_url,
            connect_timeout: Duration::from_secs(1),
            initialize_timeout: Duration::from_secs(1),
        });

        let connected = timeout(Duration::from_secs(1), connect_task)
            .await
            .expect("updated transport should not wait for the stale timeout")
            .expect("connection task should not panic")
            .expect("updated transport should connect");
        assert_eq!(connected.session_id().as_deref(), Some("ready-session"));

        release_stale_tx
            .send(())
            .expect("stale server should be released");
        stale_server.await.expect("stale server should finish");
        ready_server.await.expect("ready server should finish");
    }

    #[tokio::test]
    async fn environment_info_load_preserves_concurrent_reload_handoff() {
        for _ in 0..100 {
            let client = Arc::new(LazyRemoteExecServerClient::new(
                ExecServerTransportParams::WebSocketUrl {
                    websocket_url: "ws://127.0.0.1:8765".to_string(),
                    connect_timeout: Duration::from_secs(1),
                    initialize_timeout: Duration::from_secs(1),
                },
            ));
            let first_token = client
                .try_begin_environment_info_load()
                .expect("first load should start");
            let barrier = Arc::new(Barrier::new(2));
            let contender = {
                let barrier = Arc::clone(&barrier);
                let client = Arc::clone(&client);
                tokio::spawn(async move {
                    barrier.wait().await;
                    client.try_begin_environment_info_load()
                })
            };

            barrier.wait().await;
            let reload_requested = client.finish_environment_info_load(first_token);
            let contender_token = contender.await.expect("contender should not panic");

            assert!(reload_requested || contender_token.is_some());
        }
    }

    #[tokio::test]
    async fn wake_notifications_do_not_block_other_sessions() {
        let (client_stdin, server_reader) = duplex(1 << 20);
        let (mut server_writer, client_stdout) = duplex(1 << 20);
        let (notifications_tx, mut notifications_rx) = mpsc::channel(16);
        let server = tokio::spawn(async move {
            let mut lines = BufReader::new(server_reader).lines();
            let initialize = read_jsonrpc_line(&mut lines).await;
            let request = match initialize {
                JSONRPCMessage::Request(request) if request.method == INITIALIZE_METHOD => request,
                other => panic!("expected initialize request, got {other:?}"),
            };
            write_jsonrpc_line(
                &mut server_writer,
                JSONRPCMessage::Response(JSONRPCResponse {
                    id: request.id,
                    result: serde_json::to_value(InitializeResponse {
                        session_id: "session-1".to_string(),
                    })
                    .expect("initialize response should serialize"),
                }),
            )
            .await;

            let initialized = read_jsonrpc_line(&mut lines).await;
            match initialized {
                JSONRPCMessage::Notification(notification)
                    if notification.method == INITIALIZED_METHOD => {}
                other => panic!("expected initialized notification, got {other:?}"),
            }

            while let Some(message) = notifications_rx.recv().await {
                write_jsonrpc_line(&mut server_writer, message).await;
            }
        });

        let client = ExecServerClient::connect(
            JsonRpcConnection::from_stdio(
                client_stdout,
                client_stdin,
                "test-exec-server-client".to_string(),
            ),
            ExecServerClientConnectOptions::default(),
        )
        .await
        .expect("client should connect");

        let noisy_process_id = ProcessId::from("noisy");
        let quiet_process_id = ProcessId::from("quiet");
        let _noisy_session = client
            .register_session(&noisy_process_id)
            .await
            .expect("noisy session should register");
        let quiet_session = client
            .register_session(&quiet_process_id)
            .await
            .expect("quiet session should register");
        let mut quiet_wake_rx = quiet_session.subscribe_wake();

        for seq in 0..=4096 {
            notifications_tx
                .send(JSONRPCMessage::Notification(JSONRPCNotification {
                    method: EXEC_OUTPUT_DELTA_METHOD.to_string(),
                    params: Some(
                        serde_json::to_value(ExecOutputDeltaNotification {
                            process_id: noisy_process_id.clone(),
                            seq,
                            stream: ExecOutputStream::Stdout,
                            chunk: b"x".to_vec().into(),
                        })
                        .expect("output notification should serialize"),
                    ),
                }))
                .await
                .expect("output notification should queue");
        }

        notifications_tx
            .send(JSONRPCMessage::Notification(JSONRPCNotification {
                method: EXEC_EXITED_METHOD.to_string(),
                params: Some(
                    serde_json::to_value(ExecExitedNotification {
                        process_id: quiet_process_id,
                        seq: 1,
                        exit_code: 17,
                    })
                    .expect("exit notification should serialize"),
                ),
            }))
            .await
            .expect("exit notification should queue");

        timeout(Duration::from_secs(1), quiet_wake_rx.changed())
            .await
            .expect("quiet session should receive wake before timeout")
            .expect("quiet wake channel should stay open");
        assert_eq!(*quiet_wake_rx.borrow(), 1);

        drop(notifications_tx);
        drop(client);
        server.await.expect("server task should finish");
    }
}
