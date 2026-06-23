use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::OnceLock;
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
use tokio::sync::OnceCell;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio_util::task::AbortOnDropHandle;

use tokio::time::timeout;
use tracing::debug;

use crate::ProcessId;
use crate::client_api::ExecServerClientConnectOptions;
use crate::client_api::ExecServerTransportParams;
use crate::client_api::HttpClient;
use crate::client_api::RemoteExecServerConnectArgs;
use crate::client_api::StdioExecServerConnectArgs;
use crate::client_transport::ExecServerReconnectStrategy;
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
use crate::protocol::FS_CLOSE_METHOD;
use crate::protocol::FS_COPY_METHOD;
use crate::protocol::FS_CREATE_DIRECTORY_METHOD;
use crate::protocol::FS_GET_METADATA_METHOD;
use crate::protocol::FS_OPEN_METHOD;
use crate::protocol::FS_READ_BLOCK_METHOD;
use crate::protocol::FS_READ_DIRECTORY_METHOD;
use crate::protocol::FS_READ_FILE_METHOD;
use crate::protocol::FS_REMOVE_METHOD;
use crate::protocol::FS_WRITE_FILE_METHOD;
use crate::protocol::FsCanonicalizeParams;
use crate::protocol::FsCanonicalizeResponse;
use crate::protocol::FsCloseParams;
use crate::protocol::FsCloseResponse;
use crate::protocol::FsCopyParams;
use crate::protocol::FsCopyResponse;
use crate::protocol::FsCreateDirectoryParams;
use crate::protocol::FsCreateDirectoryResponse;
use crate::protocol::FsGetMetadataParams;
use crate::protocol::FsGetMetadataResponse;
use crate::protocol::FsOpenParams;
use crate::protocol::FsOpenResponse;
use crate::protocol::FsReadBlockParams;
use crate::protocol::FsReadBlockResponse;
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
    recoverable: AtomicBool,
    next_write_id: AtomicU64,
}

#[derive(Default)]
struct OrderedSessionEvents {
    last_published_seq: u64,
    exit_published: bool,
    closed_published: bool,
    // Server-side output, exit, and closed notifications are emitted by
    // different tasks and can reach the client out of order. Keep future events
    // here until all lower sequence numbers have been published.
    pending: BTreeMap<u64, ExecProcessEvent>,
    failure: Option<String>,
}

#[derive(Clone)]
pub(crate) struct Session {
    client: ExecServerClient,
    process_id: ProcessId,
    state: Arc<SessionState>,
}

struct Inner {
    connection: StdMutex<ConnectionState>,
    connection_changed: watch::Sender<()>,
    // The remote transport delivers one shared notification stream for every
    // process on the connection. Keep a local process_id -> session registry so
    // we can turn those connection-global notifications into process wakeups
    // without making notifications the source of truth for output delivery.
    sessions: ArcSwap<HashMap<ProcessId, Arc<SessionState>>>,
    // ArcSwap makes reads cheap on the hot notification path, but writes still
    // need serialization so concurrent register/remove operations do not
    // overwrite each other's copy-on-write updates.
    sessions_write_lock: StdMutex<()>,
    // Streaming HTTP responses are keyed by a client-generated request id
    // because they share the same connection-global notification channel as
    // process output. Keep the routing table local to the client so higher
    // layers can consume body chunks like a normal byte stream.
    http_body_streams: ArcSwap<HashMap<String, mpsc::Sender<HttpRequestBodyDeltaNotification>>>,
    http_body_stream_failures: ArcSwap<HashMap<String, String>>,
    http_body_streams_write_lock: Mutex<()>,
    http_body_stream_next_id: AtomicU64,
    session_id: OnceLock<String>,
    reconnect_strategy: Option<ExecServerReconnectStrategy>,
}

struct ConnectionState {
    status: ConnectionStatus,
    active_process_starts: usize,
}

enum ConnectionStatus {
    Connected(Arc<RpcClient>),
    Recovering,
    Failed(String),
}

#[derive(Clone)]
pub struct ExecServerClient {
    inner: Arc<Inner>,
}

struct ActiveProcessStart {
    inner: Arc<Inner>,
}

impl Drop for ActiveProcessStart {
    fn drop(&mut self) {
        self.inner.finish_process_start();
    }
}

type ConnectionResult = Result<ExecServerClient, Arc<ExecServerError>>;
type ConnectionAttempt = OnceCell<ConnectionResult>;

#[derive(Clone)]
pub(crate) struct LazyRemoteExecServerClient {
    transport_params: ExecServerTransportParams,
    // Saves the first startup result so callers share it and failures remain final.
    startup: Arc<ConnectionAttempt>,
    // The latest successful client, replaced whenever reconnecting succeeds.
    current_client: Arc<StdMutex<Option<ExecServerClient>>>,
    reconnect: Arc<StdMutex<Option<Arc<ConnectionAttempt>>>>,
}

impl LazyRemoteExecServerClient {
    pub(crate) fn new(transport_params: ExecServerTransportParams) -> Self {
        Self {
            transport_params,
            startup: Arc::new(ConnectionAttempt::new()),
            current_client: Arc::new(StdMutex::new(None)),
            reconnect: Arc::new(StdMutex::new(None)),
        }
    }

    pub(crate) fn start_connecting(&self) -> Option<AbortOnDropHandle<()>> {
        // Stdio starts a process, so keep it lazy until the environment is used.
        if matches!(
            self.transport_params,
            ExecServerTransportParams::StdioCommand { .. }
        ) {
            return None;
        }
        let client = self.clone();
        Some(AbortOnDropHandle::new(tokio::spawn(async move {
            if let Err(error) = client.wait_until_ready().await {
                debug!(%error, "exec-server environment startup failed");
            }
        })))
    }

    pub(crate) fn startup_finished(&self) -> bool {
        self.startup.get().is_some()
    }

    pub(crate) async fn wait_until_ready(&self) -> Result<(), ExecServerError> {
        self.initial_client().await.map(drop)
    }

    pub(crate) async fn get(&self) -> Result<ExecServerClient, ExecServerError> {
        if let Some(client) = self.connected_client() {
            return Ok(client);
        }

        let Some(cached_client) = self.cached_client() else {
            let client = self.initial_client().await?;
            if !client.is_disconnected() || !self.can_reconnect() {
                return Ok(client);
            }
            return self.reconnect().await;
        };

        if !self.can_reconnect() {
            return Ok(cached_client);
        }

        self.reconnect().await
    }

    async fn initial_client(&self) -> Result<ExecServerClient, ExecServerError> {
        // The first caller starts the work; every other caller waits for that same result.
        let result = self
            .startup
            .get_or_init(|| connect_once(self.transport_params.clone()))
            .await;
        match result {
            Ok(client) => {
                let mut current_client = self
                    .current_client
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if current_client.is_none() {
                    *current_client = Some(client.clone());
                }
                Ok(client.clone())
            }
            Err(error) => Err(ExecServerError::ConnectionAttempt(Arc::clone(error))),
        }
    }

    async fn reconnect(&self) -> Result<ExecServerClient, ExecServerError> {
        // Callers handling the same outage share one reconnect attempt.
        let attempt = {
            let mut reconnect = self
                .reconnect
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(client) = self.connected_client() {
                return Ok(client);
            }
            reconnect
                .get_or_insert_with(|| Arc::new(ConnectionAttempt::new()))
                .clone()
        };
        let result = attempt
            .get_or_init(|| async {
                let result = connect_once(self.transport_params.clone()).await;
                if let Ok(client) = &result {
                    *self
                        .current_client
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(client.clone());
                }
                result
            })
            .await;
        let mut reconnect = self
            .reconnect
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Forget only this completed attempt so a later operation can retry after failure.
        if reconnect
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, &attempt))
        {
            *reconnect = None;
        }
        result.clone().map_err(ExecServerError::ConnectionAttempt)
    }

    fn connected_client(&self) -> Option<ExecServerClient> {
        self.cached_client()
            .filter(|client| !client.is_disconnected())
    }

    fn cached_client(&self) -> Option<ExecServerClient> {
        self.current_client
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn can_reconnect(&self) -> bool {
        matches!(
            self.transport_params,
            ExecServerTransportParams::WebSocketUrl { .. }
                | ExecServerTransportParams::NoiseRendezvous { .. }
        )
    }
}

async fn connect_once(transport_params: ExecServerTransportParams) -> ConnectionResult {
    ExecServerClient::connect_for_transport(transport_params)
        .await
        .map_err(Arc::new)
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
    #[error("exec-server connection attempt failed: {0}")]
    ConnectionAttempt(#[source] Arc<ExecServerError>),
}

impl ExecServerClient {
    pub async fn initialize(
        &self,
        options: ExecServerClientConnectOptions,
    ) -> Result<InitializeResponse, ExecServerError> {
        let rpc_client = self.inner.rpc_client().await?;
        self.initialize_rpc(&rpc_client, options).await
    }

    async fn initialize_rpc(
        &self,
        rpc_client: &RpcClient,
        options: ExecServerClientConnectOptions,
    ) -> Result<InitializeResponse, ExecServerError> {
        let ExecServerClientConnectOptions {
            client_name,
            initialize_timeout,
            resume_session_id,
        } = options;

        timeout(initialize_timeout, async {
            let response: InitializeResponse = rpc_client
                .call(
                    INITIALIZE_METHOD,
                    &InitializeParams {
                        client_name,
                        resume_session_id,
                    },
                )
                .await?;
            let session_id = self
                .inner
                .session_id
                .get_or_init(|| response.session_id.clone());
            if session_id != &response.session_id {
                return Err(ExecServerError::Protocol(format!(
                    "exec-server initialized an unexpected session {}",
                    response.session_id
                )));
            }
            rpc_client
                .notify(INITIALIZED_METHOD, &serde_json::json!({}))
                .await?;
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
        write_id: String,
    ) -> Result<WriteResponse, ExecServerError> {
        self.call(
            EXEC_WRITE_METHOD,
            &WriteParams {
                process_id: process_id.clone(),
                chunk: chunk.into(),
                write_id,
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

    pub async fn fs_open(&self, params: FsOpenParams) -> Result<FsOpenResponse, ExecServerError> {
        self.call(FS_OPEN_METHOD, &params).await
    }

    pub async fn fs_read_block(
        &self,
        params: FsReadBlockParams,
    ) -> Result<FsReadBlockResponse, ExecServerError> {
        self.call(FS_READ_BLOCK_METHOD, &params).await
    }

    pub async fn fs_close(
        &self,
        params: FsCloseParams,
    ) -> Result<FsCloseResponse, ExecServerError> {
        self.call(FS_CLOSE_METHOD, &params).await
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

    pub(crate) async fn start_process(
        &self,
        params: ExecParams,
    ) -> Result<Session, ExecServerError> {
        loop {
            let rpc_client = self.inner.rpc_client().await?;
            if !self.inner.begin_process_start(&rpc_client) {
                continue;
            }

            let process_id = params.process_id.clone();
            let state = Arc::new(SessionState::new(/*recoverable*/ false));
            if let Err(error) = self.inner.insert_session(&process_id, Arc::clone(&state)) {
                self.inner.finish_process_start();
                return Err(error);
            }
            let active_start = ActiveProcessStart {
                inner: Arc::clone(&self.inner),
            };
            let client = self.clone();
            let (result_tx, result_rx) = tokio::sync::oneshot::channel();
            tokio::spawn(async move {
                let _active_start = active_start;
                match client
                    .call_rpc::<_, ExecResponse>(&rpc_client, EXEC_METHOD, &params)
                    .await
                {
                    Ok(_) => {
                        state.recoverable.store(true, Ordering::Release);
                        let session = Session {
                            client: client.clone(),
                            process_id: process_id.clone(),
                            state: Arc::clone(&state),
                        };
                        if result_tx.send(Ok(session)).is_err() {
                            state.recoverable.store(false, Ordering::Release);
                            tokio::spawn(async move {
                                cleanup_process_start(&client, &process_id, &state).await;
                            });
                        }
                    }
                    Err(error) => {
                        if is_transport_closed_error(&error) {
                            tokio::spawn(async move {
                                cleanup_process_start(&client, &process_id, &state).await;
                            });
                        } else {
                            client.inner.remove_session_if(&process_id, &state);
                        }
                        let _ = result_tx.send(Err(error));
                    }
                }
            });
            return result_rx.await.map_err(|_| {
                ExecServerError::Protocol("process start task stopped unexpectedly".to_string())
            })?;
        }
    }

    #[cfg(test)]
    pub(crate) async fn register_session(
        &self,
        process_id: &ProcessId,
    ) -> Result<Session, ExecServerError> {
        let state = Arc::new(SessionState::new(/*recoverable*/ true));
        self.inner.insert_session(process_id, Arc::clone(&state))?;
        Ok(Session {
            client: self.clone(),
            process_id: process_id.clone(),
            state,
        })
    }

    pub fn session_id(&self) -> Option<String> {
        self.inner.session_id.get().cloned()
    }

    fn is_disconnected(&self) -> bool {
        self.inner.is_failed()
    }

    pub(crate) async fn connect(
        connection: JsonRpcConnection,
        options: ExecServerClientConnectOptions,
    ) -> Result<Self, ExecServerError> {
        Self::connect_with_recovery(connection, options, /*reconnect_strategy*/ None).await
    }

    pub(crate) async fn connect_with_recovery(
        connection: JsonRpcConnection,
        options: ExecServerClientConnectOptions,
        reconnect_strategy: Option<ExecServerReconnectStrategy>,
    ) -> Result<Self, ExecServerError> {
        let (rpc_client, events_rx) = RpcClient::new(connection);
        let rpc_client = Arc::new(rpc_client);
        let session_id = OnceLock::new();
        let (connection_changed, _connection_changed_rx) = watch::channel(());
        let inner = Arc::new(Inner {
            connection: StdMutex::new(ConnectionState {
                status: ConnectionStatus::Connected(Arc::clone(&rpc_client)),
                active_process_starts: 0,
            }),
            connection_changed,
            sessions: ArcSwap::from_pointee(HashMap::new()),
            sessions_write_lock: StdMutex::new(()),
            http_body_streams: ArcSwap::from_pointee(HashMap::new()),
            http_body_stream_failures: ArcSwap::from_pointee(HashMap::new()),
            http_body_streams_write_lock: Mutex::new(()),
            http_body_stream_next_id: AtomicU64::new(1),
            session_id,
            reconnect_strategy,
        });
        let client = Self { inner };
        // An explicit resume can redirect notifications from running processes
        // before initialize returns. Drain them immediately so a burst cannot
        // fill the bounded event channel and block the initialize response.
        client.spawn_rpc_reader(&rpc_client, events_rx);
        client.initialize_rpc(&rpc_client, options).await?;
        Ok(client)
    }

    async fn call<P, T>(&self, method: &str, params: &P) -> Result<T, ExecServerError>
    where
        P: serde::Serialize,
        T: serde::de::DeserializeOwned,
    {
        let rpc_client = self.inner.rpc_client().await?;
        self.call_rpc(&rpc_client, method, params).await
    }

    async fn call_rpc<P, T>(
        &self,
        rpc_client: &Arc<RpcClient>,
        method: &str,
        params: &P,
    ) -> Result<T, ExecServerError>
    where
        P: serde::Serialize,
        T: serde::de::DeserializeOwned,
    {
        match rpc_client.call(method, params).await {
            Ok(response) => Ok(response),
            Err(error) => {
                let error = ExecServerError::from(error);
                if is_transport_closed_error(&error) {
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

async fn cleanup_process_start(
    client: &ExecServerClient,
    process_id: &ProcessId,
    state: &Arc<SessionState>,
) {
    loop {
        match client.terminate(process_id).await {
            Ok(_) => break,
            Err(error) if is_transport_closed_error(&error) && !client.inner.is_failed() => {
                continue;
            }
            Err(_) => break,
        }
    }
    client.inner.remove_session_if(process_id, state);
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
    fn new(recoverable: bool) -> Self {
        let (wake_tx, _wake_rx) = watch::channel(0);
        Self {
            wake_tx,
            events: ExecProcessEventLog::new(
                PROCESS_EVENT_CHANNEL_CAPACITY,
                PROCESS_EVENT_RETAINED_BYTES,
            ),
            ordered_events: StdMutex::new(OrderedSessionEvents::default()),
            recoverable: AtomicBool::new(recoverable),
            next_write_id: AtomicU64::new(1),
        }
    }

    pub(crate) fn subscribe(&self) -> watch::Receiver<u64> {
        self.wake_tx.subscribe()
    }

    pub(crate) fn subscribe_events(&self) -> ExecProcessEventReceiver {
        self.events.subscribe()
    }

    fn note_change(&self, seq: u64) {
        self.wake_tx
            .send_modify(|current| *current = (*current).max(seq));
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
        if ordered_events.failure.is_some()
            || ordered_events.closed_published
            || seq <= ordered_events.last_published_seq
        {
            return false;
        }

        ordered_events.pending.entry(seq).or_insert(event);
        self.publish_ready(&mut ordered_events)
    }

    fn publish_ready(&self, ordered_events: &mut OrderedSessionEvents) -> bool {
        let mut published_closed = false;
        loop {
            let next_seq = ordered_events.last_published_seq.saturating_add(1);
            let Some(event) = ordered_events.pending.remove(&next_seq) else {
                break;
            };
            ordered_events.last_published_seq = next_seq;
            ordered_events.exit_published |= matches!(&event, ExecProcessEvent::Exited { .. });
            let is_closed = matches!(&event, ExecProcessEvent::Closed { .. });
            ordered_events.closed_published |= is_closed;
            published_closed |= is_closed;
            self.events.publish(event);
        }
        published_closed
    }

    fn set_failure(&self, message: String) {
        let mut ordered_events = self
            .ordered_events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if ordered_events.failure.is_some() || ordered_events.closed_published {
            return;
        }
        ordered_events.failure = Some(message.clone());
        ordered_events.pending.clear();
        self.events.publish(ExecProcessEvent::Failed(message));
        drop(ordered_events);
        self.wake_tx
            .send_modify(|current| *current = current.saturating_add(1));
    }

    fn failed_response(&self) -> Option<ReadResponse> {
        self.ordered_events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .failure
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
            sandbox_denied: false,
        }
    }

    fn next_write_id(&self) -> String {
        self.next_write_id
            .fetch_add(1, Ordering::Relaxed)
            .to_string()
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
            if let Some(response) = self.state.failed_response() {
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
                Ok(response) => return Ok(response),
                Err(error)
                    if is_transport_closed_error(&error) && !self.client.inner.is_failed() =>
                {
                    continue;
                }
                Err(error) if is_transport_closed_error(&error) => {
                    if let Some(response) = self.state.failed_response() {
                        return Ok(response);
                    }
                    let message = error.to_string();
                    self.state.set_failure(message.clone());
                    return Ok(self.state.synthesized_failure(message));
                }
                Err(error) => return Err(error),
            }
        }
    }

    pub(crate) async fn write(&self, chunk: Vec<u8>) -> Result<WriteResponse, ExecServerError> {
        let write_id = self.state.next_write_id();
        loop {
            match self
                .client
                .write(&self.process_id, chunk.clone(), write_id.clone())
                .await
            {
                Ok(response) => return Ok(response),
                Err(error)
                    if is_transport_closed_error(&error) && !self.client.inner.is_failed() =>
                {
                    continue;
                }
                Err(error) => return Err(error),
            }
        }
    }

    pub(crate) async fn signal(&self, signal: ProcessSignal) -> Result<(), ExecServerError> {
        self.client.signal(&self.process_id, signal).await
    }

    pub(crate) async fn terminate(&self) -> Result<(), ExecServerError> {
        self.client.terminate(&self.process_id).await?;
        Ok(())
    }

    pub(crate) async fn unregister(&self) {
        self.client
            .inner
            .remove_session_if(&self.process_id, &self.state);
    }
}

impl Inner {
    fn get_session(&self, process_id: &ProcessId) -> Option<Arc<SessionState>> {
        self.sessions.load().get(process_id).cloned()
    }

    fn insert_session(
        &self,
        process_id: &ProcessId,
        session: Arc<SessionState>,
    ) -> Result<(), ExecServerError> {
        let _sessions_write_guard = self
            .sessions_write_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Do not register a process session that can never receive environment
        // notifications. Without this check, remote MCP startup could create a
        // dead session and wait for process output that will never arrive.
        if let Some(message) = self.failure_message() {
            return Err(ExecServerError::Disconnected(message));
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

    fn remove_session_if(&self, process_id: &ProcessId, expected: &Arc<SessionState>) {
        let _sessions_write_guard = self
            .sessions_write_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let sessions = self.sessions.load();
        if !sessions
            .get(process_id)
            .is_some_and(|session| Arc::ptr_eq(session, expected))
        {
            return;
        }
        let mut next_sessions = sessions.as_ref().clone();
        next_sessions.remove(process_id);
        self.sessions.store(Arc::new(next_sessions));
    }

    fn take_all_sessions(&self) -> HashMap<ProcessId, Arc<SessionState>> {
        let _sessions_write_guard = self
            .sessions_write_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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

fn fail_all_sessions(inner: &Arc<Inner>, message: String) {
    let sessions = inner.take_all_sessions();

    for (_, session) in sessions {
        // Sessions synthesize a closed read response and emit a pushed Failed
        // event. That covers both polling consumers and streaming consumers
        // such as environment-backed MCP stdio.
        session.set_failure(message.clone());
    }
}

/// Fails all in-flight work that depends on the shared JSON-RPC transport.
async fn fail_all_in_flight_work(inner: &Arc<Inner>, message: String) {
    fail_all_sessions(inner, message.clone());
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
                    inner.remove_session_if(&params.process_id, &session);
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
                    inner.remove_session_if(&params.process_id, &session);
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
                    inner.remove_session_if(&params.process_id, &session);
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
    use tokio::sync::mpsc;
    use tokio::sync::oneshot;
    use tokio::sync::watch;
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
    use crate::client_api::RemoteExecServerConnectArgs;
    use crate::client_api::StdioExecServerCommand;
    use crate::client_api::StdioExecServerConnectArgs;
    use crate::connection::JsonRpcConnection;
    use crate::process::ExecProcessEvent;
    use crate::protocol::EXEC_CLOSED_METHOD;
    use crate::protocol::EXEC_EXITED_METHOD;
    use crate::protocol::EXEC_OUTPUT_DELTA_METHOD;
    use crate::protocol::EXEC_READ_METHOD;
    use crate::protocol::EXEC_WRITE_METHOD;
    use crate::protocol::ExecClosedNotification;
    use crate::protocol::ExecExitedNotification;
    use crate::protocol::ExecOutputDeltaNotification;
    use crate::protocol::ExecOutputStream;
    use crate::protocol::INITIALIZE_METHOD;
    use crate::protocol::INITIALIZED_METHOD;
    use crate::protocol::InitializeResponse;
    use crate::protocol::ProcessOutputChunk;
    use crate::protocol::ReadResponse;
    use crate::protocol::WriteParams;
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
    async fn remote_websocket_client_resumes_session() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let websocket_url = format!(
            "ws://{}",
            listener.local_addr().expect("listener should have address")
        );
        let (resumed_tx, resumed_rx) = oneshot::channel();
        let (finish_tx, finish_rx) = oneshot::channel();
        let server = tokio::spawn(async move {
            let mut first = accept_websocket(&listener).await;
            complete_websocket_initialize(
                &mut first,
                "session-1",
                /*expected_resume_session_id*/ None,
            )
            .await;
            first.close(None).await.expect("websocket should close");

            let mut resumed = accept_websocket(&listener).await;
            complete_websocket_initialize(
                &mut resumed,
                "session-1",
                /*expected_resume_session_id*/ Some("session-1"),
            )
            .await;
            resumed_tx.send(()).expect("resume should signal");
            finish_rx.await.expect("test should finish");
        });

        let client = LazyRemoteExecServerClient::new(ExecServerTransportParams::WebSocketUrl {
            websocket_url,
            connect_timeout: Duration::from_secs(1),
            initialize_timeout: Duration::from_secs(1),
        });
        let stable_client = client.get().await.expect("client should connect");
        timeout(Duration::from_secs(1), resumed_rx)
            .await
            .expect("session resume should not time out")
            .expect("session resume should signal");
        let reused_client = client.get().await.expect("client should stay connected");
        assert_eq!(stable_client.session_id().as_deref(), Some("session-1"));
        assert!(Arc::ptr_eq(&stable_client.inner, &reused_client.inner));
        finish_tx.send(()).expect("test should finish");
        server.await.expect("server task should finish");
    }

    #[tokio::test]
    async fn session_write_retries_same_write_id_after_recovery() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let websocket_url = format!(
            "ws://{}",
            listener.local_addr().expect("listener should have address")
        );
        let (finish_tx, finish_rx) = oneshot::channel();
        let server = tokio::spawn(async move {
            let mut first = accept_websocket(&listener).await;
            complete_websocket_initialize(
                &mut first,
                "session-1",
                /*expected_resume_session_id*/ None,
            )
            .await;

            let first_write = read_jsonrpc_websocket(&mut first).await;
            let first_write = match first_write {
                JSONRPCMessage::Request(request) if request.method == EXEC_WRITE_METHOD => request,
                other => panic!("expected first process/write request, got {other:?}"),
            };
            let first_write_params: WriteParams =
                serde_json::from_value(first_write.params.expect("write params should exist"))
                    .expect("write params should deserialize");
            assert_eq!(first_write_params.process_id.as_str(), "proc-write");
            assert_eq!(first_write_params.chunk.into_inner(), b"hello\n".to_vec());
            let write_id = first_write_params.write_id;
            assert!(!write_id.is_empty());
            drop(first);

            let mut resumed = accept_websocket(&listener).await;
            complete_websocket_initialize(
                &mut resumed,
                "session-1",
                /*expected_resume_session_id*/ Some("session-1"),
            )
            .await;

            let recovery_read = read_jsonrpc_websocket(&mut resumed).await;
            let recovery_read = match recovery_read {
                JSONRPCMessage::Request(request) if request.method == EXEC_READ_METHOD => request,
                other => panic!("expected recovery process/read request, got {other:?}"),
            };
            write_jsonrpc_websocket(
                &mut resumed,
                JSONRPCMessage::Response(JSONRPCResponse {
                    id: recovery_read.id,
                    result: serde_json::to_value(ReadResponse {
                        chunks: Vec::new(),
                        next_seq: 1,
                        exited: false,
                        exit_code: None,
                        closed: false,
                        failure: None,
                        sandbox_denied: false,
                    })
                    .expect("read response should serialize"),
                }),
            )
            .await;

            let retried_write = read_jsonrpc_websocket(&mut resumed).await;
            let retried_write = match retried_write {
                JSONRPCMessage::Request(request) if request.method == EXEC_WRITE_METHOD => request,
                other => panic!("expected retried process/write request, got {other:?}"),
            };
            let retried_write_params: WriteParams =
                serde_json::from_value(retried_write.params.expect("write params should exist"))
                    .expect("write params should deserialize");
            assert_eq!(retried_write_params.process_id.as_str(), "proc-write");
            assert_eq!(retried_write_params.chunk.into_inner(), b"hello\n".to_vec());
            assert_eq!(retried_write_params.write_id, write_id);
            write_jsonrpc_websocket(
                &mut resumed,
                JSONRPCMessage::Response(JSONRPCResponse {
                    id: retried_write.id,
                    result: serde_json::to_value(WriteResponse {
                        status: WriteStatus::Accepted,
                    })
                    .expect("write response should serialize"),
                }),
            )
            .await;

            finish_rx.await.expect("test should finish");
        });

        let client = LazyRemoteExecServerClient::new(ExecServerTransportParams::WebSocketUrl {
            websocket_url,
            connect_timeout: Duration::from_secs(1),
            initialize_timeout: Duration::from_secs(1),
        });
        let stable_client = client.get().await.expect("client should connect");
        let session = stable_client
            .register_session(&ProcessId::from("proc-write"))
            .await
            .expect("session should register");

        let response = timeout(Duration::from_secs(2), session.write(b"hello\n".to_vec()))
            .await
            .expect("write should not time out")
            .expect("write should recover");
        assert_eq!(
            response,
            WriteResponse {
                status: WriteStatus::Accepted
            }
        );

        finish_tx.send(()).expect("test should finish");
        server.await.expect("server task should finish");
    }

    #[tokio::test]
    async fn explicit_resume_drains_notifications_before_initialize_response() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let websocket_url = format!(
            "ws://{}",
            listener.local_addr().expect("listener should have address")
        );
        let (initialized_tx, initialized_rx) = oneshot::channel();
        let (finish_tx, finish_rx) = oneshot::channel();
        let server = tokio::spawn(async move {
            let mut websocket = accept_websocket(&listener).await;
            let initialize = read_jsonrpc_websocket(&mut websocket).await;
            let request = match initialize {
                JSONRPCMessage::Request(request) if request.method == INITIALIZE_METHOD => request,
                other => panic!("expected initialize request, got {other:?}"),
            };
            let params: crate::protocol::InitializeParams =
                serde_json::from_value(request.params.expect("initialize params should exist"))
                    .expect("initialize params should deserialize");
            assert_eq!(params.resume_session_id.as_deref(), Some("session-1"));

            for seq in 1..=256 {
                write_jsonrpc_websocket(
                    &mut websocket,
                    JSONRPCMessage::Notification(JSONRPCNotification {
                        method: EXEC_OUTPUT_DELTA_METHOD.to_string(),
                        params: Some(
                            serde_json::to_value(ExecOutputDeltaNotification {
                                process_id: ProcessId::from("busy-process"),
                                seq,
                                stream: ExecOutputStream::Stdout,
                                chunk: b"output".to_vec().into(),
                            })
                            .expect("output notification should serialize"),
                        ),
                    }),
                )
                .await;
            }
            write_jsonrpc_websocket(
                &mut websocket,
                JSONRPCMessage::Response(JSONRPCResponse {
                    id: request.id,
                    result: serde_json::to_value(InitializeResponse {
                        session_id: "session-1".to_string(),
                    })
                    .expect("initialize response should serialize"),
                }),
            )
            .await;

            let initialized = read_jsonrpc_websocket(&mut websocket).await;
            match initialized {
                JSONRPCMessage::Notification(notification)
                    if notification.method == INITIALIZED_METHOD => {}
                other => panic!("expected initialized notification, got {other:?}"),
            }
            initialized_tx
                .send(())
                .expect("initialized notification should signal");
            finish_rx.await.expect("test should finish");
        });

        let client = timeout(
            Duration::from_secs(1),
            ExecServerClient::connect_websocket(RemoteExecServerConnectArgs {
                websocket_url,
                client_name: "test-client".to_string(),
                connect_timeout: Duration::from_secs(1),
                initialize_timeout: Duration::from_secs(1),
                resume_session_id: Some("session-1".to_string()),
            }),
        )
        .await
        .expect("explicit resume should not time out")
        .expect("explicit resume should connect");
        assert_eq!(client.session_id().as_deref(), Some("session-1"));

        timeout(Duration::from_secs(1), initialized_rx)
            .await
            .expect("initialized notification should not time out")
            .expect("initialized notification should signal");
        finish_tx.send(()).expect("test should finish");
        server.await.expect("server task should finish");
    }

    #[tokio::test]
    async fn initial_connection_is_shared_by_all_waiters() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let websocket_url = format!(
            "ws://{}",
            listener.local_addr().expect("listener should have address")
        );
        let server = tokio::spawn(async move {
            let mut connection = accept_websocket(&listener).await;
            complete_websocket_initialize(
                &mut connection,
                "startup-session",
                /*expected_resume_session_id*/ None,
            )
            .await;
            timeout(Duration::from_secs(1), connection.next())
                .await
                .expect("client should close after the test");
        });
        let client = LazyRemoteExecServerClient::new(ExecServerTransportParams::WebSocketUrl {
            websocket_url,
            connect_timeout: Duration::from_secs(1),
            initialize_timeout: Duration::from_secs(1),
        });

        assert!(!client.startup_finished());
        let _startup_task = client.start_connecting();
        let (ready, first, second) =
            tokio::join!(client.wait_until_ready(), client.get(), client.get());
        ready.expect("background startup should finish");
        let first = first.expect("first waiter should receive the client");
        let second = second.expect("second waiter should receive the same client");

        assert!(client.startup_finished());
        assert_eq!(first.session_id().as_deref(), Some("startup-session"));
        assert!(Arc::ptr_eq(&first.inner, &second.inner));

        drop(first);
        drop(second);
        drop(client);
        server.await.expect("server task should finish");
    }

    #[tokio::test]
    async fn terminal_stdio_startup_failure_is_remembered() {
        let client = LazyRemoteExecServerClient::new(ExecServerTransportParams::StdioCommand {
            command: StdioExecServerCommand {
                program: "codex-missing-exec-server-for-test".to_string(),
                args: Vec::new(),
                env: HashMap::new(),
                cwd: None,
            },
            initialize_timeout: Duration::from_secs(1),
        });

        assert!(client.start_connecting().is_none());
        assert!(!client.startup_finished());
        let first = match client.get().await {
            Ok(_) => panic!("missing executable should fail"),
            Err(error) => error,
        };
        assert!(client.startup_finished());
        let second = match client.get().await {
            Ok(_) => panic!("burned environment should stay failed"),
            Err(error) => error,
        };

        let (
            super::ExecServerError::ConnectionAttempt(first),
            super::ExecServerError::ConnectionAttempt(second),
        ) = (first, second)
        else {
            panic!("expected saved connection failures");
        };
        assert!(Arc::ptr_eq(&first, &second));
    }

    #[tokio::test]
    async fn failed_reconnect_does_not_burn_environment() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let websocket_url = format!(
            "ws://{}",
            listener.local_addr().expect("listener should have address")
        );
        let (replacement_initialized_tx, replacement_initialized_rx) = oneshot::channel();
        let (allow_replacement_tx, allow_replacement_rx) = watch::channel(false);
        let server = tokio::spawn(async move {
            let mut first = accept_websocket(&listener).await;
            complete_websocket_initialize(
                &mut first,
                "startup-session",
                /*expected_resume_session_id*/ None,
            )
            .await;
            first
                .close(None)
                .await
                .expect("startup websocket should close");

            let successful_reconnect = loop {
                let (stream, _) = listener.accept().await.expect("reconnect should arrive");
                if *allow_replacement_rx.borrow() {
                    break stream;
                }
                let mut failed_reconnect = stream;
                failed_reconnect
                    .write_all(b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n")
                    .await
                    .expect("failed handshake response should write");
            };
            let mut successful_reconnect = accept_async(successful_reconnect)
                .await
                .expect("replacement websocket handshake should succeed");
            complete_websocket_initialize(
                &mut successful_reconnect,
                "replacement-session",
                /*expected_resume_session_id*/ None,
            )
            .await;
            replacement_initialized_tx
                .send(())
                .expect("replacement initialization should be observed");
            timeout(Duration::from_secs(1), successful_reconnect.next())
                .await
                .expect("client should close after the test");
        });
        let client = LazyRemoteExecServerClient::new(ExecServerTransportParams::WebSocketUrl {
            websocket_url,
            connect_timeout: Duration::from_secs(1),
            initialize_timeout: Duration::from_secs(1),
        });

        let initial = client.get().await.expect("startup should connect");
        timeout(Duration::from_secs(1), async {
            while !initial.is_disconnected() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("client should observe disconnect");
        let failed_reconnect = match client.get().await {
            Ok(_) => panic!("first lazy reconnect should fail"),
            Err(error) => error,
        };
        assert!(matches!(
            failed_reconnect,
            super::ExecServerError::ConnectionAttempt(_)
        ));
        allow_replacement_tx
            .send(true)
            .expect("server should allow a fresh client");
        let replacement = client.get().await.expect("later reconnect should succeed");

        assert_eq!(
            replacement.session_id().as_deref(),
            Some("replacement-session")
        );
        replacement_initialized_rx
            .await
            .expect("server should observe replacement initialization");

        drop(initial);
        drop(replacement);
        drop(client);
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
