use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::RwLock as StdRwLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;

use arc_swap::ArcSwap;
use codex_app_server_protocol::JSONRPCNotification;
use futures::FutureExt;
use futures::future::BoxFuture;
use serde_json::Value;
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use tokio::sync::RwLockReadGuard;
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
use crate::rpc::RpcCallError;
use crate::rpc::RpcClient;
use crate::rpc::RpcClientEvent;

pub(crate) mod http_client;
#[path = "client_recovery.rs"]
mod recovery;

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
    transport: RwLock<TransportState>,
    // The remote transport delivers one shared notification stream for every
    // process on the connection. Keep a local process_id -> session registry so
    // we can turn those connection-global notifications into process wakeups
    // without making notifications the source of truth for output delivery.
    sessions: StdRwLock<HashMap<ProcessId, Arc<SessionState>>>,
    // Streaming HTTP responses are keyed by a client-generated request id
    // because they share the same connection-global notification channel as
    // process output. Keep the routing table local to the client so higher
    // layers can consume body chunks like a normal byte stream.
    http_body_streams: ArcSwap<HashMap<String, mpsc::Sender<HttpRequestBodyDeltaNotification>>>,
    http_body_stream_failures: ArcSwap<HashMap<String, String>>,
    http_body_streams_write_lock: Mutex<()>,
    http_body_stream_next_id: AtomicU64,
    session_id: String,
    remote_connect_args: Option<RemoteExecServerConnectArgs>,
    next_generation_id: AtomicU64,
}

struct ClientGeneration {
    id: u64,
    client: RpcClient,
    terminal: AtomicBool,
}

enum TransportState {
    Connected(Arc<ClientGeneration>),
    Failed(String),
}

struct GenerationLease<'a> {
    generation: Arc<ClientGeneration>,
    _state: RwLockReadGuard<'a, TransportState>,
}

struct SessionRegistration {
    client: ExecServerClient,
    process_id: ProcessId,
    state: Arc<SessionState>,
    active: bool,
}

impl ClientGeneration {
    fn new(id: u64, client: RpcClient) -> Self {
        Self {
            id,
            client,
            terminal: AtomicBool::new(false),
        }
    }

    fn mark_terminal(&self) -> bool {
        !self.terminal.swap(true, Ordering::AcqRel)
    }

    fn is_terminal(&self) -> bool {
        self.terminal.load(Ordering::Acquire) || self.client.is_disconnected()
    }
}

impl Drop for SessionRegistration {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        self.client
            .inner
            .remove_session_if(&self.process_id, &self.state);
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let client = self.client.clone();
            let process_id = self.process_id.clone();
            handle.spawn(async move {
                let _ = client.terminate(&process_id).await;
            });
        }
    }
}

#[derive(Clone)]
pub struct ExecServerClient {
    inner: Arc<Inner>,
}

#[derive(Clone)]
pub(crate) struct LazyRemoteExecServerClient {
    transport_params: ExecServerTransportParams,
    client: Arc<StdMutex<Option<ExecServerClient>>>,
    connect_lock: Arc<Semaphore>,
}

impl LazyRemoteExecServerClient {
    pub(crate) fn new(transport_params: ExecServerTransportParams) -> Self {
        Self {
            transport_params,
            client: Arc::new(StdMutex::new(None)),
            connect_lock: Arc::new(Semaphore::new(/*permits*/ 1)),
        }
    }

    pub(crate) async fn get(&self) -> Result<ExecServerClient, ExecServerError> {
        if let Some(client) = self.connected_client() {
            return Ok(client);
        }

        let _connect_permit = self.connect_lock.acquire().await.map_err(|_| {
            ExecServerError::Protocol("exec-server connect lock closed".to_string())
        })?;
        if let Some(client) = self.connected_client() {
            return Ok(client);
        }

        let next_client = match self.cached_client() {
            Some(_client)
                if matches!(
                    &self.transport_params,
                    ExecServerTransportParams::WebSocketUrl { .. }
                ) =>
            {
                ExecServerClient::connect_for_transport(self.transport_params.clone()).await?
            }
            Some(client) => return Ok(client),
            None => ExecServerClient::connect_for_transport(self.transport_params.clone()).await?,
        };

        let mut cached_client = self
            .client
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *cached_client = Some(next_client.clone());
        Ok(next_client)
    }

    fn connected_client(&self) -> Option<ExecServerClient> {
        self.cached_client()
            .filter(|client| !client.is_disconnected())
    }

    fn cached_client(&self) -> Option<ExecServerClient> {
        self.client
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
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
        self.get().await?.environment_info().await
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
    async fn initialize_generation(
        generation: &ClientGeneration,
        options: ExecServerClientConnectOptions,
    ) -> Result<InitializeResponse, ExecServerError> {
        let ExecServerClientConnectOptions {
            client_name,
            initialize_timeout,
            resume_session_id,
        } = options;

        timeout(initialize_timeout, async {
            let response: InitializeResponse = generation
                .client
                .call(
                    INITIALIZE_METHOD,
                    &InitializeParams {
                        client_name,
                        resume_session_id,
                    },
                )
                .await?;
            Self::notify_initialized(generation).await?;
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

    fn create_session(&self, process_id: &ProcessId) -> Result<Session, ExecServerError> {
        let state = Arc::new(SessionState::new());
        self.inner.insert_session(process_id, Arc::clone(&state))?;
        Ok(Session {
            client: self.clone(),
            process_id: process_id.clone(),
            state,
        })
    }

    pub(crate) async fn start_process(
        &self,
        params: ExecParams,
    ) -> Result<Session, ExecServerError> {
        let generation = self.inner.generation().await?;
        let process_id = params.process_id.clone();
        let session = self.create_session(&process_id)?;
        let mut registration = SessionRegistration {
            client: self.clone(),
            process_id,
            state: Arc::clone(&session.state),
            active: true,
        };
        if let Err(error) = self
            .call_generation::<_, ExecResponse>(&generation.generation, EXEC_METHOD, &params)
            .await
        {
            return Err(error);
        }
        registration.active = false;
        Ok(session)
    }

    pub(crate) fn unregister_session(&self, process_id: &ProcessId) {
        self.inner.remove_session(process_id);
    }

    pub fn session_id(&self) -> Option<String> {
        Some(self.inner.session_id.clone())
    }

    fn is_disconnected(&self) -> bool {
        self.inner
            .transport
            .try_read()
            .is_ok_and(|state| matches!(&*state, TransportState::Failed(_)))
    }

    pub(crate) async fn connect(
        connection: JsonRpcConnection,
        options: ExecServerClientConnectOptions,
    ) -> Result<Self, ExecServerError> {
        Self::connect_with_recovery(connection, options, /*remote_connect_args*/ None).await
    }

    pub(crate) async fn connect_with_recovery(
        connection: JsonRpcConnection,
        options: ExecServerClientConnectOptions,
        remote_connect_args: Option<RemoteExecServerConnectArgs>,
    ) -> Result<Self, ExecServerError> {
        let (rpc_client, events_rx) = RpcClient::new(connection);
        let generation_id = 1;
        let generation = Arc::new(ClientGeneration::new(generation_id, rpc_client));
        let response = Self::initialize_generation(&generation, options).await?;
        let inner = Arc::new(Inner {
            transport: RwLock::new(TransportState::Connected(Arc::clone(&generation))),
            sessions: StdRwLock::new(HashMap::new()),
            http_body_streams: ArcSwap::from_pointee(HashMap::new()),
            http_body_stream_failures: ArcSwap::from_pointee(HashMap::new()),
            http_body_streams_write_lock: Mutex::new(()),
            http_body_stream_next_id: AtomicU64::new(1),
            session_id: response.session_id,
            remote_connect_args,
            next_generation_id: AtomicU64::new(generation_id + 1),
        });

        let client = Self { inner };
        client.spawn_generation_reader(generation, events_rx);
        Ok(client)
    }

    fn spawn_generation_reader(
        &self,
        generation: Arc<ClientGeneration>,
        mut events_rx: mpsc::Receiver<RpcClientEvent>,
    ) {
        let inner = Arc::downgrade(&self.inner);
        let generation = Arc::downgrade(&generation);
        tokio::spawn(async move {
            while let Some(event) = events_rx.recv().await {
                let Some(generation) = generation.upgrade() else {
                    return;
                };
                match event {
                    RpcClientEvent::Notification(notification) => {
                        if let Some(inner) = inner.upgrade()
                            && let Err(err) = handle_server_notification(&inner, notification).await
                        {
                            inner.handle_generation_disconnect(
                                Arc::clone(&generation),
                                format!("exec-server notification handling failed: {err}"),
                            );
                            return;
                        }
                    }
                    RpcClientEvent::Disconnected { reason } => {
                        if let Some(inner) = inner.upgrade() {
                            inner.handle_generation_disconnect(
                                Arc::clone(&generation),
                                disconnected_message(reason.as_deref()),
                            );
                        }
                        return;
                    }
                }
            }
        });
    }

    async fn notify_initialized(generation: &ClientGeneration) -> Result<(), ExecServerError> {
        generation
            .client
            .notify(INITIALIZED_METHOD, &serde_json::json!({}))
            .await
            .map_err(ExecServerError::from)
    }

    async fn call<P, T>(&self, method: &str, params: &P) -> Result<T, ExecServerError>
    where
        P: serde::Serialize,
        T: serde::de::DeserializeOwned,
    {
        let generation = self.inner.generation().await?;
        self.call_generation(&generation.generation, method, params)
            .await
    }

    async fn call_generation<P, T>(
        &self,
        generation: &Arc<ClientGeneration>,
        method: &str,
        params: &P,
    ) -> Result<T, ExecServerError>
    where
        P: serde::Serialize,
        T: serde::de::DeserializeOwned,
    {
        match generation.client.call(method, params).await {
            Ok(response) => Ok(response),
            Err(error) => {
                let error = ExecServerError::from(error);
                if is_transport_closed_error(&error) {
                    self.inner.handle_generation_disconnect(
                        Arc::clone(generation),
                        disconnected_message(/*reason*/ None),
                    );
                    Err(ExecServerError::Disconnected(disconnected_message(
                        /*reason*/ None,
                    )))
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
        self.publish_ready_events(&mut ordered_events)
    }

    fn publish_ready_events(&self, ordered_events: &mut OrderedSessionEvents) -> bool {
        let mut published_closed = false;
        loop {
            let next_seq = ordered_events.last_published_seq + 1;
            let Some(event) = ordered_events.pending.remove(&next_seq) else {
                break;
            };
            ordered_events.last_published_seq = next_seq;
            published_closed |= matches!(&event, ExecProcessEvent::Closed { .. });
            self.events.publish(event);
        }
        published_closed
    }

    fn last_published_seq(&self) -> u64 {
        self.ordered_events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .last_published_seq
    }

    fn recover_events(&self, response: ReadResponse) -> Result<bool, ExecServerError> {
        let ReadResponse {
            chunks,
            next_seq,
            exited,
            exit_code,
            exit_seq,
            closed,
            closed_seq,
            failure,
        } = response;
        if let Some(message) = failure {
            return Err(ExecServerError::Protocol(format!(
                "process failed while recovering: {message}"
            )));
        }
        let mut recovered = BTreeMap::new();
        for chunk in chunks {
            recovered.insert(chunk.seq, ExecProcessEvent::Output(chunk));
        }
        if exited {
            let seq = exit_seq.ok_or_else(|| {
                ExecServerError::Protocol(
                    "recovering exited process did not include its exit sequence".to_string(),
                )
            })?;
            let exit_code = exit_code.ok_or_else(|| {
                ExecServerError::Protocol(
                    "recovering exited process did not include its exit code".to_string(),
                )
            })?;
            recovered.insert(seq, ExecProcessEvent::Exited { seq, exit_code });
        }
        if closed {
            let seq = closed_seq.ok_or_else(|| {
                ExecServerError::Protocol(
                    "recovering closed process did not include its close sequence".to_string(),
                )
            })?;
            recovered.insert(seq, ExecProcessEvent::Closed { seq });
        }

        let mut ordered_events = self
            .ordered_events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let target_seq = next_seq.saturating_sub(1);
        for seq in ordered_events.last_published_seq.saturating_add(1)..=target_seq {
            if !ordered_events.pending.contains_key(&seq) && !recovered.contains_key(&seq) {
                return Err(ExecServerError::Protocol(format!(
                    "process event {seq} is no longer retained while recovering through sequence {target_seq}"
                )));
            }
        }
        for (seq, event) in recovered {
            if seq > ordered_events.last_published_seq {
                ordered_events.pending.entry(seq).or_insert(event);
            }
        }
        self.note_change(target_seq);
        Ok(self.publish_ready_events(&mut ordered_events))
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
            exit_seq: None,
            closed: true,
            closed_seq: None,
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
        loop {
            if let Some(response) = self.state.failed_response().await {
                return Ok(response);
            }
            let generation = match self.client.inner.generation().await {
                Ok(generation) => generation,
                Err(error) => {
                    if let Some(response) = self.state.failed_response().await {
                        return Ok(response);
                    }
                    return Err(error);
                }
            };
            let result = self
                .client
                .call_generation::<_, ReadResponse>(
                    &generation.generation,
                    EXEC_READ_METHOD,
                    &ReadParams {
                        process_id: self.process_id.clone(),
                        after_seq,
                        max_bytes,
                        wait_ms,
                    },
                )
                .await;
            drop(generation);
            match result {
                Ok(response) => return Ok(response),
                Err(err)
                    if is_transport_closed_error(&err)
                        && self.client.inner.remote_connect_args.is_some() =>
                {
                    continue;
                }
                Err(err) if is_transport_closed_error(&err) => {
                    let message = disconnected_message(/*reason*/ None);
                    self.state.set_failure(message.clone()).await;
                    return Ok(self.state.synthesized_failure(message));
                }
                Err(err) => return Err(err),
            }
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

    pub(crate) fn unregister(&self) {
        self.client.unregister_session(&self.process_id);
    }
}

impl Inner {
    async fn generation(&self) -> Result<GenerationLease<'_>, ExecServerError> {
        loop {
            let state = self.transport.read().await;
            match &*state {
                TransportState::Connected(generation) if !generation.is_terminal() => {
                    return Ok(GenerationLease {
                        generation: Arc::clone(generation),
                        _state: state,
                    });
                }
                TransportState::Connected(_) => {
                    drop(state);
                    tokio::task::yield_now().await;
                }
                TransportState::Failed(message) => {
                    return Err(ExecServerError::Disconnected(message.clone()));
                }
            }
        }
    }

    fn get_session(&self, process_id: &ProcessId) -> Option<Arc<SessionState>> {
        self.sessions
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(process_id)
            .cloned()
    }

    fn insert_session(
        &self,
        process_id: &ProcessId,
        session: Arc<SessionState>,
    ) -> Result<(), ExecServerError> {
        let mut sessions = self
            .sessions
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if sessions.contains_key(process_id) {
            return Err(ExecServerError::Protocol(format!(
                "session already registered for process {process_id}"
            )));
        }
        sessions.insert(process_id.clone(), session);
        Ok(())
    }

    fn remove_session(&self, process_id: &ProcessId) -> Option<Arc<SessionState>> {
        self.sessions
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(process_id)
    }

    fn remove_session_if(&self, process_id: &ProcessId, expected: &Arc<SessionState>) {
        let mut sessions = self
            .sessions
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !sessions
            .get(process_id)
            .is_some_and(|session| Arc::ptr_eq(session, expected))
        {
            return;
        }
        sessions.remove(process_id);
    }

    fn take_all_sessions(&self) -> HashMap<ProcessId, Arc<SessionState>> {
        std::mem::take(
            &mut *self
                .sessions
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
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

async fn fail_all_sessions(inner: &Inner, message: String) {
    let sessions = inner.take_all_sessions();

    for (_, session) in sessions {
        // Sessions synthesize a closed read response and emit a pushed Failed
        // event. That covers both polling consumers and streaming consumers
        // such as environment-backed MCP stdio.
        session.set_failure(message.clone()).await;
    }
}

/// Fails all in-flight work that depends on the shared JSON-RPC transport.
async fn fail_all_in_flight_work(inner: &Inner, message: String) {
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
                    inner.remove_session(&params.process_id);
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
                    inner.remove_session(&params.process_id);
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
                    inner.remove_session(&params.process_id);
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
    use codex_app_server_protocol::JSONRPCError;
    use codex_app_server_protocol::JSONRPCErrorError;
    use codex_app_server_protocol::JSONRPCMessage;
    use codex_app_server_protocol::JSONRPCNotification;
    use codex_app_server_protocol::JSONRPCResponse;
    use codex_utils_path_uri::PathUri;
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
    use tokio::sync::mpsc;
    use tokio::sync::oneshot;
    use tokio::time::Duration;
    use tokio::time::sleep;
    use tokio::time::timeout;
    use tokio_tungstenite::WebSocketStream;
    use tokio_tungstenite::accept_async;
    use tokio_tungstenite::tungstenite::Message;

    use super::ExecServerClient;
    use super::ExecServerClientConnectOptions;
    use super::LazyRemoteExecServerClient;
    use super::RemoteExecServerConnectArgs;
    use super::SessionRegistration;
    use super::SessionState;
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
    use crate::protocol::EXEC_READ_METHOD;
    use crate::protocol::EXEC_TERMINATE_METHOD;
    use crate::protocol::EXEC_WRITE_METHOD;
    use crate::protocol::EnvironmentInfo;
    use crate::protocol::ExecClosedNotification;
    use crate::protocol::ExecExitedNotification;
    use crate::protocol::ExecOutputDeltaNotification;
    use crate::protocol::ExecOutputStream;
    use crate::protocol::ExecParams;
    use crate::protocol::HttpRequestParams;
    use crate::protocol::INITIALIZE_METHOD;
    use crate::protocol::INITIALIZED_METHOD;
    use crate::protocol::InitializeResponse;
    use crate::protocol::ProcessOutputChunk;
    use crate::protocol::ReadParams;
    use crate::protocol::ReadResponse;
    use crate::protocol::ShellInfo;
    use crate::protocol::TerminateParams;
    use crate::protocol::TerminateResponse;
    use crate::protocol::WriteResponse;
    use crate::protocol::WriteStatus;

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

    async fn wait_for_generation(client: &ExecServerClient, generation_id: u64) {
        let result = timeout(Duration::from_secs(1), async {
            loop {
                let state = client.inner.transport.read().await;
                if matches!(&*state, super::TransportState::Connected(generation) if generation.id == generation_id)
                {
                    return;
                }
                drop(state);
                tokio::task::yield_now().await;
            }
        })
        .await;
        assert!(
            result.is_ok(),
            "client should connect generation {generation_id}",
        );
    }

    fn test_exec_params(process_id: &str) -> ExecParams {
        ExecParams {
            process_id: ProcessId::from(process_id),
            argv: vec!["ignored".to_string()],
            cwd: PathUri::from_path(std::env::current_dir().expect("cwd")).expect("cwd URI"),
            env_policy: None,
            env: HashMap::new(),
            tty: false,
            pipe_stdin: false,
            arg0: None,
        }
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
            .create_session(&process_id)
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
            .create_session(&process_id)
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

        assert!(matches!(
            client.inner.generation().await,
            Err(super::ExecServerError::Disconnected(_))
        ));

        drop(client);
        server.await.expect("server task should finish");
    }

    #[tokio::test]
    async fn recovery_retries_generation_that_disconnects_during_handoff() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let websocket_url = format!(
            "ws://{}",
            listener.local_addr().expect("listener should have address")
        );
        let (release_tx, release_rx) = oneshot::channel();
        let server = tokio::spawn(async move {
            let mut first = accept_websocket(&listener).await;
            complete_websocket_initialize(
                &mut first,
                "session-1",
                /*expected_resume_session_id*/ None,
            )
            .await;
            first.close(None).await.expect("first websocket closes");

            let mut second = accept_websocket(&listener).await;
            complete_websocket_initialize(
                &mut second,
                "session-1",
                /*expected_resume_session_id*/ Some("session-1"),
            )
            .await;
            second.close(None).await.expect("second websocket closes");

            let mut third = accept_websocket(&listener).await;
            complete_websocket_initialize(
                &mut third,
                "session-1",
                /*expected_resume_session_id*/ Some("session-1"),
            )
            .await;
            let request = match read_jsonrpc_websocket(&mut third).await {
                JSONRPCMessage::Request(request)
                    if request.method == super::ENVIRONMENT_INFO_METHOD =>
                {
                    request
                }
                other => panic!("expected environment info request, got {other:?}"),
            };
            write_jsonrpc_websocket(
                &mut third,
                JSONRPCMessage::Response(JSONRPCResponse {
                    id: request.id,
                    result: serde_json::to_value(EnvironmentInfo {
                        shell: ShellInfo {
                            name: "sh".to_string(),
                            path: "/bin/sh".to_string(),
                        },
                    })
                    .expect("environment info should serialize"),
                }),
            )
            .await;
            release_rx.await.expect("test should release websocket");
        });

        let mut args = RemoteExecServerConnectArgs::new(websocket_url, "test-client".to_string());
        args.connect_timeout = Duration::from_secs(1);
        args.initialize_timeout = Duration::from_secs(1);
        let client = ExecServerClient::connect_websocket(args)
            .await
            .expect("initial client should connect");

        wait_for_generation(&client, /*generation_id*/ 3).await;
        assert_eq!(
            client
                .environment_info()
                .await
                .expect("replacement generation should serve requests"),
            EnvironmentInfo {
                shell: ShellInfo {
                    name: "sh".to_string(),
                    path: "/bin/sh".to_string(),
                },
            }
        );
        drop(client);
        release_tx.send(()).expect("websocket should still be open");
        server.await.expect("server task should finish");
    }

    #[tokio::test]
    async fn process_and_http_routes_wait_for_transport_handoff() {
        let (client_stdin, server_reader) = duplex(1 << 20);
        let (mut server_writer, client_stdout) = duplex(1 << 20);
        let (terminate_seen_tx, terminate_seen_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
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
            assert!(matches!(
                read_jsonrpc_line(&mut lines).await,
                JSONRPCMessage::Notification(notification)
                    if notification.method == INITIALIZED_METHOD
            ));
            let request = match read_jsonrpc_line(&mut lines).await {
                JSONRPCMessage::Request(request) if request.method == EXEC_TERMINATE_METHOD => {
                    request
                }
                other => panic!("expected terminate request, got {other:?}"),
            };
            let params: TerminateParams =
                serde_json::from_value(request.params.expect("terminate params should exist"))
                    .expect("terminate params should deserialize");
            assert_eq!(params.process_id, ProcessId::from("cancelled"));
            write_jsonrpc_line(
                &mut server_writer,
                JSONRPCMessage::Response(JSONRPCResponse {
                    id: request.id,
                    result: serde_json::to_value(TerminateResponse { running: false })
                        .expect("terminate response should serialize"),
                }),
            )
            .await;
            terminate_seen_tx
                .send(())
                .expect("terminate request should signal");
            release_rx.await.expect("test should release server");
        });

        let client = ExecServerClient::connect(
            JsonRpcConnection::from_stdio(client_stdout, client_stdin, "handoff-test".to_string()),
            ExecServerClientConnectOptions::default(),
        )
        .await
        .expect("client should connect");
        let handoff = client.inner.transport.write().await;

        let process_client = client.clone();
        let process_task = tokio::spawn(async move {
            process_client
                .start_process(test_exec_params("after-handoff"))
                .await
        });
        let http_client = client.clone();
        let http_task = tokio::spawn(async move {
            http_client
                .http_request_stream(HttpRequestParams {
                    method: "GET".to_string(),
                    url: "https://example.test".to_string(),
                    headers: Vec::new(),
                    body: None,
                    timeout_ms: None,
                    request_id: String::new(),
                    stream_response: false,
                })
                .await
        });

        sleep(Duration::from_millis(25)).await;
        assert!(
            client
                .inner
                .get_session(&ProcessId::from("after-handoff"))
                .is_none()
        );
        assert!(client.inner.http_body_streams.load().is_empty());
        process_task.abort();
        http_task.abort();
        let (process_result, http_result) = tokio::join!(process_task, http_task);
        assert!(matches!(process_result, Err(error) if error.is_cancelled()));
        assert!(matches!(http_result, Err(error) if error.is_cancelled()));

        let state = Arc::new(SessionState::new());
        client
            .inner
            .insert_session(&ProcessId::from("cancelled"), Arc::clone(&state))
            .expect("cancelled session should register");
        drop(SessionRegistration {
            client: client.clone(),
            process_id: ProcessId::from("cancelled"),
            state,
            active: true,
        });
        assert!(
            client
                .inner
                .get_session(&ProcessId::from("cancelled"))
                .is_none()
        );

        drop(handoff);
        terminate_seen_rx
            .await
            .expect("cancelled registration should terminate the remote process");
        drop(client);
        release_tx.send(()).expect("server should still be running");
        server.await.expect("server task should finish");
    }

    #[tokio::test]
    async fn remote_websocket_client_resumes_disconnected_session() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let websocket_url = format!(
            "ws://{}",
            listener.local_addr().expect("listener should have address")
        );
        let (registered_tx, registered_rx) = oneshot::channel();
        let (disconnect_tx, disconnect_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        let server = tokio::spawn({
            async move {
                let mut first = accept_websocket(&listener).await;
                complete_websocket_initialize(
                    &mut first,
                    "session-1",
                    /*expected_resume_session_id*/ None,
                )
                .await;
                let _ = registered_rx.await;
                write_jsonrpc_websocket(
                    &mut first,
                    JSONRPCMessage::Notification(JSONRPCNotification {
                        method: EXEC_OUTPUT_DELTA_METHOD.to_string(),
                        params: Some(
                            serde_json::to_value(ExecOutputDeltaNotification {
                                process_id: ProcessId::from("resumed-process"),
                                seq: 1,
                                stream: ExecOutputStream::Stdout,
                                chunk: b"before\n".to_vec().into(),
                            })
                            .expect("output notification should serialize"),
                        ),
                    }),
                )
                .await;
                let _ = disconnect_rx.await;
                first
                    .close(None)
                    .await
                    .expect("first websocket should close");
                drop(first);

                let mut second = accept_websocket(&listener).await;
                complete_websocket_initialize(
                    &mut second,
                    "session-1",
                    /*expected_resume_session_id*/ Some("session-1"),
                )
                .await;

                for _ in 0..3 {
                    let read = match read_jsonrpc_websocket(&mut second).await {
                        JSONRPCMessage::Request(request) if request.method == EXEC_READ_METHOD => {
                            request
                        }
                        other => panic!("expected recovery read request, got {other:?}"),
                    };
                    let params: ReadParams = serde_json::from_value(
                        read.params
                            .clone()
                            .expect("recovery read params should exist"),
                    )
                    .expect("recovery read params should deserialize");
                    let message = match params.process_id.as_ref() {
                        "resumed-process" => {
                            assert_eq!(params.after_seq, Some(1));
                            JSONRPCMessage::Response(JSONRPCResponse {
                                id: read.id,
                                result: serde_json::to_value(ReadResponse {
                                    chunks: vec![ProcessOutputChunk {
                                        seq: 2,
                                        stream: ExecOutputStream::Stdout,
                                        chunk: b"during\n".to_vec().into(),
                                    }],
                                    next_seq: 3,
                                    exited: false,
                                    exit_code: None,
                                    exit_seq: None,
                                    closed: false,
                                    closed_seq: None,
                                    failure: None,
                                })
                                .expect("recovery read response should serialize"),
                            })
                        }
                        "terminal-process" => {
                            assert_eq!(params.after_seq, Some(0));
                            JSONRPCMessage::Response(JSONRPCResponse {
                                id: read.id,
                                result: serde_json::to_value(ReadResponse {
                                    chunks: vec![ProcessOutputChunk {
                                        seq: 1,
                                        stream: ExecOutputStream::Stdout,
                                        chunk: b"tail\n".to_vec().into(),
                                    }],
                                    next_seq: 4,
                                    exited: true,
                                    exit_code: Some(0),
                                    exit_seq: Some(2),
                                    closed: true,
                                    closed_seq: Some(3),
                                    failure: None,
                                })
                                .expect("terminal recovery response should serialize"),
                            })
                        }
                        "missing-process" => JSONRPCMessage::Error(JSONRPCError {
                            id: read.id,
                            error: JSONRPCErrorError {
                                code: -32600,
                                message: "unknown process".to_string(),
                                data: None,
                            },
                        }),
                        process_id => panic!("unexpected recovery process {process_id}"),
                    };
                    write_jsonrpc_websocket(&mut second, message).await;
                }

                let write = read_jsonrpc_websocket(&mut second).await;
                let write = match write {
                    JSONRPCMessage::Request(request) if request.method == EXEC_WRITE_METHOD => {
                        request
                    }
                    other => panic!("expected process write request, got {other:?}"),
                };
                write_jsonrpc_websocket(
                    &mut second,
                    JSONRPCMessage::Response(JSONRPCResponse {
                        id: write.id,
                        result: serde_json::to_value(WriteResponse {
                            status: WriteStatus::Accepted,
                        })
                        .expect("write response should serialize"),
                    }),
                )
                .await;
                let _ = release_rx.await;
            }
        });

        let client = LazyRemoteExecServerClient::new(ExecServerTransportParams::WebSocketUrl {
            websocket_url,
            connect_timeout: Duration::from_secs(1),
            initialize_timeout: Duration::from_secs(1),
        });
        let first = client.get().await.expect("first client should connect");
        let session = first
            .create_session(&ProcessId::from("resumed-process"))
            .expect("process session should register");
        let mut events = session.subscribe_events();
        let terminal_session = first
            .create_session(&ProcessId::from("terminal-process"))
            .expect("terminal process session should register");
        let mut terminal_events = terminal_session.subscribe_events();
        let missing_session = first
            .create_session(&ProcessId::from("missing-process"))
            .expect("missing process session should register");
        let mut missing_events = missing_session.subscribe_events();
        registered_tx.send(()).expect("registration should signal");
        assert_eq!(
            timeout(Duration::from_secs(1), events.recv())
                .await
                .expect("initial output should not time out")
                .expect("event stream should stay open"),
            ExecProcessEvent::Output(ProcessOutputChunk {
                seq: 1,
                stream: ExecOutputStream::Stdout,
                chunk: b"before\n".to_vec().into(),
            })
        );
        disconnect_tx.send(()).expect("disconnect should signal");
        wait_for_generation(&first, /*generation_id*/ 2).await;
        assert_eq!(
            timeout(Duration::from_secs(1), events.recv())
                .await
                .expect("recovered output should not time out")
                .expect("event stream should stay open"),
            ExecProcessEvent::Output(ProcessOutputChunk {
                seq: 2,
                stream: ExecOutputStream::Stdout,
                chunk: b"during\n".to_vec().into(),
            })
        );
        let mut recovered_terminal_events = Vec::new();
        for _ in 0..3 {
            recovered_terminal_events.push(
                timeout(Duration::from_secs(1), terminal_events.recv())
                    .await
                    .expect("terminal recovery should not time out")
                    .expect("terminal event stream should stay open"),
            );
        }
        assert_eq!(
            recovered_terminal_events,
            vec![
                ExecProcessEvent::Output(ProcessOutputChunk {
                    seq: 1,
                    stream: ExecOutputStream::Stdout,
                    chunk: b"tail\n".to_vec().into(),
                }),
                ExecProcessEvent::Exited {
                    seq: 2,
                    exit_code: 0,
                },
                ExecProcessEvent::Closed { seq: 3 },
            ]
        );
        assert_eq!(
            timeout(Duration::from_secs(1), missing_events.recv())
                .await
                .expect("missing process recovery should not time out")
                .expect("missing process event stream should stay open"),
            ExecProcessEvent::Failed(
                "failed to recover process missing-process: exec-server rejected request (-32600): unknown process"
                    .to_string()
            )
        );
        assert_eq!(
            session
                .write(b"hello\n".to_vec())
                .await
                .expect("write should use resumed session"),
            WriteResponse {
                status: WriteStatus::Accepted,
            }
        );

        let (replacement_a, replacement_b) = tokio::join!(client.get(), client.get());
        let replacement_a = replacement_a.expect("first replacement should connect");
        let replacement_b = replacement_b.expect("second replacement should reuse client");
        assert_eq!(replacement_a.session_id().as_deref(), Some("session-1"));
        assert_eq!(replacement_b.session_id().as_deref(), Some("session-1"));
        assert!(Arc::ptr_eq(&first.inner, &replacement_a.inner));
        assert!(Arc::ptr_eq(&replacement_a.inner, &replacement_b.inner));

        release_tx.send(()).expect("server should release");
        server.await.expect("server task should finish");
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
            .create_session(&noisy_process_id)
            .expect("noisy session should register");
        let quiet_session = client
            .create_session(&quiet_process_id)
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
