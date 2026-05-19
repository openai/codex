use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::OnceLock;
use std::sync::atomic::AtomicU64;
use std::time::Duration;

use arc_swap::ArcSwap;
use codex_app_server_protocol::JSONRPCNotification;
use futures::FutureExt;
use futures::future::BoxFuture;
use serde_json::Value;
use tokio::sync::Mutex;
use tokio::sync::Notify;
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
use crate::client_transport::ENVIRONMENT_CLIENT_NAME;
use crate::connection::JsonRpcConnection;
use crate::process::ExecProcessEvent;
use crate::process::ExecProcessEventLog;
use crate::process::ExecProcessEventReceiver;
use crate::protocol::EXEC_CLOSED_METHOD;
use crate::protocol::EXEC_EXITED_METHOD;
use crate::protocol::EXEC_METHOD;
use crate::protocol::EXEC_OUTPUT_DELTA_METHOD;
use crate::protocol::EXEC_READ_METHOD;
use crate::protocol::EXEC_TERMINATE_METHOD;
use crate::protocol::EXEC_WRITE_METHOD;
use crate::protocol::ExecClosedNotification;
use crate::protocol::ExecExitedNotification;
use crate::protocol::ExecOutputDeltaNotification;
use crate::protocol::ExecParams;
use crate::protocol::ExecResponse;
use crate::protocol::FS_COPY_METHOD;
use crate::protocol::FS_CREATE_DIRECTORY_METHOD;
use crate::protocol::FS_GET_METADATA_METHOD;
use crate::protocol::FS_READ_DIRECTORY_METHOD;
use crate::protocol::FS_READ_FILE_METHOD;
use crate::protocol::FS_REMOVE_METHOD;
use crate::protocol::FS_WRITE_FILE_METHOD;
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
use crate::protocol::ReadParams;
use crate::protocol::ReadResponse;
use crate::protocol::TerminateParams;
use crate::protocol::TerminateResponse;
use crate::protocol::WriteParams;
use crate::protocol::WriteResponse;
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

pub(crate) struct ProcessSession {
    wake_tx: watch::Sender<u64>,
    events: ExecProcessEventLog,
    ordered_events: StdMutex<OrderedSessionEvents>,
    failure: Mutex<Option<String>>,
    disconnect_behavior: ProcessSessionDisconnectBehavior,
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
pub(crate) struct ProcessSessionHandle {
    control: ProcessSessionControl,
    process_id: ProcessId,
    session: Arc<ProcessSession>,
}

#[derive(Clone)]
enum ProcessSessionControl {
    #[cfg(test)]
    // Direct connections are used by one-shot callers and focused client tests.
    Connection(ExecServerConnection),
    // Remote environments use the logical client so process sessions survive
    // connection replacement across reconnect.
    RemoteClient(RemoteExecServerClient),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProcessSessionDisconnectBehavior {
    Fail,
    Preserve,
}

struct Inner {
    rpc_client: RpcClient,
    // The remote transport delivers one shared notification stream for every
    // process on the connection. Keep a local process_id -> session route map
    // so we can turn those connection-global notifications into process wakeups
    // without making notifications the source of truth for output delivery.
    process_session_routes: ArcSwap<HashMap<ProcessId, Arc<ProcessSession>>>,
    // ArcSwap makes reads cheap on the hot notification path, but writes still
    // need serialization so concurrent register/remove operations do not
    // overwrite each other's copy-on-write updates.
    process_session_routes_write_lock: Mutex<()>,
    // Once the transport closes, every executor operation should fail quickly
    // with the same canonical message. This connection never reconnects, so
    // the latch only moves from unset to set once.
    disconnected: OnceLock<String>,
    // Streaming HTTP responses are keyed by a client-generated request id
    // because they share the same connection-global notification channel as
    // process output. Keep the routing table local to the connection so higher
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
pub struct ExecServerConnection {
    inner: Arc<Inner>,
}

#[derive(Clone)]
pub(crate) struct RemoteExecServerClient {
    transport_params: ExecServerTransportParams,
    session: Arc<StdMutex<RemoteExecServerSession>>,
}

// Shared state for one logical remote exec-server client. The logical client
// owns resumable session identity and durable process session state; individual
// ExecServerConnection values only bind that state to one live transport.
struct RemoteExecServerSession {
    connection: Option<ExecServerConnection>,
    connection_attempt: Option<Arc<Notify>>,
    logical_session_id: Option<String>,
    terminal_error: Option<TerminalReconnectError>,
    process_sessions: HashMap<ProcessId, Arc<ProcessSession>>,
}

enum RemoteExecServerConnectionAction {
    Ready(ExecServerConnection),
    Wait(BoxFuture<'static, ()>),
    Connect {
        connection_attempt: Arc<Notify>,
        resume_session_id: Option<String>,
        process_sessions: Vec<(ProcessId, Arc<ProcessSession>)>,
    },
}

#[derive(Clone)]
struct TerminalReconnectError {
    code: i64,
    message: String,
}

impl RemoteExecServerClient {
    pub(crate) fn new(transport_params: ExecServerTransportParams) -> Self {
        Self {
            transport_params,
            session: Arc::new(StdMutex::new(RemoteExecServerSession {
                connection: None,
                connection_attempt: None,
                logical_session_id: None,
                terminal_error: None,
                process_sessions: HashMap::new(),
            })),
        }
    }

    pub(crate) async fn connection(&self) -> Result<ExecServerConnection, ExecServerError> {
        loop {
            match self.next_connection_action()? {
                RemoteExecServerConnectionAction::Ready(connection) => return Ok(connection),
                RemoteExecServerConnectionAction::Wait(connection_attempt) => {
                    connection_attempt.await;
                }
                RemoteExecServerConnectionAction::Connect {
                    connection_attempt,
                    resume_session_id,
                    process_sessions,
                } => {
                    let connection = self
                        .connect_and_rebind(resume_session_id.clone(), process_sessions)
                        .await;
                    return self.finish_connection_attempt(
                        connection_attempt,
                        resume_session_id,
                        connection,
                    );
                }
            }
        }
    }

    pub(crate) async fn register_process_session(
        &self,
        process_id: &ProcessId,
    ) -> Result<ProcessSessionHandle, ExecServerError> {
        let process_session = Arc::new(ProcessSession::new(
            ProcessSessionDisconnectBehavior::Preserve,
        ));
        {
            let mut session = self.lock_session();
            if session.process_sessions.contains_key(process_id) {
                return Err(ExecServerError::Protocol(format!(
                    "session already registered for process {process_id}"
                )));
            }
            session
                .process_sessions
                .insert(process_id.clone(), Arc::clone(&process_session));
        }

        let connection = self.connection().await?;
        if let Err(err) = connection
            .register_process_session_route(process_id, Arc::clone(&process_session))
            .await
        {
            self.unregister_process_session(process_id).await;
            return Err(err);
        }

        Ok(ProcessSessionHandle {
            control: ProcessSessionControl::RemoteClient(self.clone()),
            process_id: process_id.clone(),
            session: process_session,
        })
    }

    async fn read(&self, params: ReadParams) -> Result<ReadResponse, ExecServerError> {
        let connection = self.connection().await?;
        match connection.read(params.clone()).await {
            Ok(response) => Ok(response),
            Err(err) if is_transport_closed_error(&err) && self.supports_reconnect() => {
                self.connection().await?.read(params).await
            }
            Err(err) => Err(err),
        }
    }

    async fn write(
        &self,
        process_id: &ProcessId,
        chunk: Vec<u8>,
    ) -> Result<WriteResponse, ExecServerError> {
        self.connection().await?.write(process_id, chunk).await
    }

    async fn terminate(
        &self,
        process_id: &ProcessId,
    ) -> Result<TerminateResponse, ExecServerError> {
        self.connection().await?.terminate(process_id).await
    }

    async fn unregister_process_session(&self, process_id: &ProcessId) {
        let connection = {
            let mut session = self.lock_session();
            session.process_sessions.remove(process_id);
            session.connection.clone()
        };
        if let Some(connection) = connection {
            connection.unregister_process_session(process_id).await;
        }
    }

    fn next_connection_action(&self) -> Result<RemoteExecServerConnectionAction, ExecServerError> {
        let mut session = self.lock_session();
        if let Some(error) = &session.terminal_error {
            return Err(error.to_exec_server_error());
        }

        if let Some(connection) = &session.connection {
            if let Some(error) = connection.disconnected_error() {
                if !self.supports_reconnect() {
                    return Err(error);
                }
            } else {
                return Ok(RemoteExecServerConnectionAction::Ready(connection.clone()));
            }
        }

        if let Some(connection_attempt) = &session.connection_attempt {
            let connection_attempt = Arc::clone(connection_attempt).notified_owned();
            return Ok(RemoteExecServerConnectionAction::Wait(
                connection_attempt.boxed(),
            ));
        }

        let connection_attempt = Arc::new(Notify::new());
        let resume_session_id = session.logical_session_id.clone();
        let process_sessions = session
            .process_sessions
            .iter()
            .map(|(process_id, process_session)| (process_id.clone(), Arc::clone(process_session)))
            .collect();
        session.connection_attempt = Some(Arc::clone(&connection_attempt));
        Ok(RemoteExecServerConnectionAction::Connect {
            connection_attempt,
            resume_session_id,
            process_sessions,
        })
    }

    async fn connect_and_rebind(
        &self,
        resume_session_id: Option<String>,
        process_sessions: Vec<(ProcessId, Arc<ProcessSession>)>,
    ) -> Result<ExecServerConnection, ExecServerError> {
        let connection = self.connect(resume_session_id).await?;
        for (process_id, process_session) in process_sessions {
            connection
                .register_process_session_route(&process_id, process_session)
                .await?;
        }
        Ok(connection)
    }

    fn finish_connection_attempt(
        &self,
        connection_attempt: Arc<Notify>,
        resume_session_id: Option<String>,
        connection: Result<ExecServerConnection, ExecServerError>,
    ) -> Result<ExecServerConnection, ExecServerError> {
        let mut session = self.lock_session();
        if let Err(err) = &connection {
            if resume_session_id.is_some()
                && let Some(terminal_error) = TerminalReconnectError::from_error(err)
            {
                session.terminal_error = Some(terminal_error);
            }
        } else if let Ok(connection) = &connection {
            session.logical_session_id = connection.session_id();
            session.connection = Some(connection.clone());
        }
        session.connection_attempt = None;
        connection_attempt.notify_waiters();
        connection
    }

    fn lock_session(&self) -> std::sync::MutexGuard<'_, RemoteExecServerSession> {
        self.session
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    async fn connect(
        &self,
        resume_session_id: Option<String>,
    ) -> Result<ExecServerConnection, ExecServerError> {
        match &self.transport_params {
            ExecServerTransportParams::WebSocketUrl {
                websocket_url,
                connect_timeout,
                initialize_timeout,
            } => {
                ExecServerConnection::connect_websocket(RemoteExecServerConnectArgs {
                    websocket_url: websocket_url.clone(),
                    client_name: ENVIRONMENT_CLIENT_NAME.to_string(),
                    connect_timeout: *connect_timeout,
                    initialize_timeout: *initialize_timeout,
                    resume_session_id,
                })
                .await
            }
            ExecServerTransportParams::StdioCommand { .. } => {
                ExecServerConnection::connect_for_transport(self.transport_params.clone()).await
            }
        }
    }

    fn supports_reconnect(&self) -> bool {
        matches!(
            &self.transport_params,
            ExecServerTransportParams::WebSocketUrl { .. }
        )
    }
}

impl HttpClient for RemoteExecServerClient {
    fn http_request(
        &self,
        params: crate::HttpRequestParams,
    ) -> BoxFuture<'_, Result<crate::HttpRequestResponse, ExecServerError>> {
        async move { self.connection().await?.http_request(params).await }.boxed()
    }

    fn http_request_stream(
        &self,
        params: crate::HttpRequestParams,
    ) -> BoxFuture<
        '_,
        Result<(crate::HttpRequestResponse, crate::HttpResponseBodyStream), ExecServerError>,
    > {
        async move { self.connection().await?.http_request_stream(params).await }.boxed()
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
    #[error("executor registry request failed ({status}{code_suffix}): {message}", code_suffix = .code.as_ref().map(|code| format!(", {code}")).unwrap_or_default())]
    ExecutorRegistryHttp {
        status: reqwest::StatusCode,
        code: Option<String>,
        message: String,
    },
    #[error("executor registry configuration error: {0}")]
    ExecutorRegistryConfig(String),
    #[error("executor registry authentication error: {0}")]
    ExecutorRegistryAuth(String),
    #[error("executor registry request failed: {0}")]
    ExecutorRegistryRequest(#[from] reqwest::Error),
}

impl ExecServerConnection {
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
                .rpc_client
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

    #[cfg(test)]
    pub(crate) async fn register_process_session(
        &self,
        process_id: &ProcessId,
    ) -> Result<ProcessSessionHandle, ExecServerError> {
        let session = Arc::new(ProcessSession::new(ProcessSessionDisconnectBehavior::Fail));
        self.register_process_session_route(process_id, Arc::clone(&session))
            .await?;
        Ok(ProcessSessionHandle {
            control: ProcessSessionControl::Connection(self.clone()),
            process_id: process_id.clone(),
            session,
        })
    }

    async fn register_process_session_route(
        &self,
        process_id: &ProcessId,
        session: Arc<ProcessSession>,
    ) -> Result<(), ExecServerError> {
        self.inner
            .insert_process_session_route(process_id, session)
            .await
    }

    pub(crate) async fn unregister_process_session(&self, process_id: &ProcessId) {
        self.inner.remove_process_session_route(process_id).await;
    }

    pub fn session_id(&self) -> Option<String> {
        self.inner
            .session_id
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn disconnected_error(&self) -> Option<ExecServerError> {
        self.inner.disconnected_error()
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
                rpc_client,
                process_session_routes: ArcSwap::from_pointee(HashMap::new()),
                process_session_routes_write_lock: Mutex::new(()),
                disconnected: OnceLock::new(),
                http_body_streams: ArcSwap::from_pointee(HashMap::new()),
                http_body_stream_failures: ArcSwap::from_pointee(HashMap::new()),
                http_body_streams_write_lock: Mutex::new(()),
                http_body_stream_next_id: AtomicU64::new(1),
                session_id: std::sync::RwLock::new(None),
                reader_task,
            }
        });

        let connection = Self { inner };
        connection.initialize(options).await?;
        Ok(connection)
    }

    async fn notify_initialized(&self) -> Result<(), ExecServerError> {
        self.inner
            .rpc_client
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
        // this is the shared low-level failure path after executor disconnect.
        if let Some(error) = self.inner.disconnected_error() {
            return Err(error);
        }

        match self.inner.rpc_client.call(method, params).await {
            Ok(response) => Ok(response),
            Err(error) => {
                let error = ExecServerError::from(error);
                if is_transport_closed_error(&error) {
                    // A call can race with disconnect after the preflight
                    // check. Only the reader task drains routes so queued
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

impl ProcessSession {
    fn new(disconnect_behavior: ProcessSessionDisconnectBehavior) -> Self {
        let (wake_tx, _wake_rx) = watch::channel(0);
        Self {
            wake_tx,
            events: ExecProcessEventLog::new(
                PROCESS_EVENT_CHANNEL_CAPACITY,
                PROCESS_EVENT_RETAINED_BYTES,
            ),
            ordered_events: StdMutex::new(OrderedSessionEvents::default()),
            failure: Mutex::new(None),
            disconnect_behavior,
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

impl ProcessSessionHandle {
    pub(crate) fn process_id(&self) -> &ProcessId {
        &self.process_id
    }

    pub(crate) fn subscribe_wake(&self) -> watch::Receiver<u64> {
        self.session.subscribe()
    }

    pub(crate) fn subscribe_events(&self) -> ExecProcessEventReceiver {
        self.session.subscribe_events()
    }

    pub(crate) async fn read(
        &self,
        after_seq: Option<u64>,
        max_bytes: Option<usize>,
        wait_ms: Option<u64>,
    ) -> Result<ReadResponse, ExecServerError> {
        if let Some(response) = self.session.failed_response().await {
            return Ok(response);
        }

        let params = ReadParams {
            process_id: self.process_id.clone(),
            after_seq,
            max_bytes,
            wait_ms,
        };
        match self.control.read(params).await {
            Ok(response) => Ok(response),
            Err(err)
                if is_transport_closed_error(&err)
                    && self.session.disconnect_behavior
                        == ProcessSessionDisconnectBehavior::Fail =>
            {
                let message = disconnected_message(/*reason*/ None);
                self.session.set_failure(message.clone()).await;
                Ok(self.session.synthesized_failure(message))
            }
            Err(err) => Err(err),
        }
    }

    pub(crate) async fn write(&self, chunk: Vec<u8>) -> Result<WriteResponse, ExecServerError> {
        self.control.write(&self.process_id, chunk).await
    }

    pub(crate) async fn terminate(&self) -> Result<(), ExecServerError> {
        self.control.terminate(&self.process_id).await?;
        Ok(())
    }

    pub(crate) async fn unregister(&self) {
        self.control
            .unregister_process_session(&self.process_id)
            .await;
    }
}

impl ProcessSessionControl {
    async fn read(&self, params: ReadParams) -> Result<ReadResponse, ExecServerError> {
        match self {
            #[cfg(test)]
            Self::Connection(connection) => connection.read(params).await,
            Self::RemoteClient(client) => client.read(params).await,
        }
    }

    async fn write(
        &self,
        process_id: &ProcessId,
        chunk: Vec<u8>,
    ) -> Result<WriteResponse, ExecServerError> {
        match self {
            #[cfg(test)]
            Self::Connection(connection) => connection.write(process_id, chunk).await,
            Self::RemoteClient(client) => client.write(process_id, chunk).await,
        }
    }

    async fn terminate(
        &self,
        process_id: &ProcessId,
    ) -> Result<TerminateResponse, ExecServerError> {
        match self {
            #[cfg(test)]
            Self::Connection(connection) => connection.terminate(process_id).await,
            Self::RemoteClient(client) => client.terminate(process_id).await,
        }
    }

    async fn unregister_process_session(&self, process_id: &ProcessId) {
        match self {
            #[cfg(test)]
            Self::Connection(connection) => connection.unregister_process_session(process_id).await,
            Self::RemoteClient(client) => client.unregister_process_session(process_id).await,
        }
    }
}

impl TerminalReconnectError {
    fn from_error(error: &ExecServerError) -> Option<Self> {
        match error {
            ExecServerError::Server { code, message } if *code == -32600 => Some(Self {
                code: *code,
                message: message.clone(),
            }),
            _ => None,
        }
    }

    fn to_exec_server_error(&self) -> ExecServerError {
        ExecServerError::Server {
            code: self.code,
            message: self.message.clone(),
        }
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

    fn get_process_session_route(&self, process_id: &ProcessId) -> Option<Arc<ProcessSession>> {
        self.process_session_routes.load().get(process_id).cloned()
    }

    async fn insert_process_session_route(
        &self,
        process_id: &ProcessId,
        session: Arc<ProcessSession>,
    ) -> Result<(), ExecServerError> {
        let _routes_write_guard = self.process_session_routes_write_lock.lock().await;
        // Do not register a process session that can never receive executor
        // notifications. Without this check, remote MCP startup could create a
        // dead session and wait for process output that will never arrive.
        if let Some(error) = self.disconnected_error() {
            return Err(error);
        }
        let routes = self.process_session_routes.load();
        if let Some(existing_session) = routes.get(process_id) {
            if Arc::ptr_eq(existing_session, &session) {
                return Ok(());
            }
            return Err(ExecServerError::Protocol(format!(
                "session already registered for process {process_id}"
            )));
        }
        let mut next_routes = routes.as_ref().clone();
        next_routes.insert(process_id.clone(), session);
        self.process_session_routes.store(Arc::new(next_routes));
        Ok(())
    }

    async fn remove_process_session_route(
        &self,
        process_id: &ProcessId,
    ) -> Option<Arc<ProcessSession>> {
        let _routes_write_guard = self.process_session_routes_write_lock.lock().await;
        let routes = self.process_session_routes.load();
        let session = routes.get(process_id).cloned();
        session.as_ref()?;
        let mut next_routes = routes.as_ref().clone();
        next_routes.remove(process_id);
        self.process_session_routes.store(Arc::new(next_routes));
        session
    }

    async fn take_all_process_session_routes(&self) -> HashMap<ProcessId, Arc<ProcessSession>> {
        let _routes_write_guard = self.process_session_routes_write_lock.lock().await;
        let routes = self.process_session_routes.load();
        let drained_routes = routes.as_ref().clone();
        self.process_session_routes.store(Arc::new(HashMap::new()));
        drained_routes
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
    // The first observer records the canonical disconnect reason. Process
    // session route draining stays with the reader task so it can preserve
    // notification ordering before publishing the terminal failure.
    if let Some(message) = inner.set_disconnected(message.clone()) {
        message
    } else {
        inner.disconnected.get().cloned().unwrap_or(message)
    }
}

async fn fail_all_process_sessions(inner: &Arc<Inner>, message: String) {
    let routes = inner.take_all_process_session_routes().await;

    for (_, session) in routes {
        // One-shot sessions synthesize a closed read response and emit a
        // pushed Failed event. Reconnecting remote sessions keep their local
        // event state so a reattached client can bind them again.
        if session.disconnect_behavior == ProcessSessionDisconnectBehavior::Fail {
            session.set_failure(message.clone()).await;
        }
    }
}

/// Fails all in-flight work that depends on the shared JSON-RPC transport.
async fn fail_all_in_flight_work(inner: &Arc<Inner>, message: String) {
    fail_all_process_sessions(inner, message.clone()).await;
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
            if let Some(session) = inner.get_process_session_route(&params.process_id) {
                session.note_change(params.seq);
                let published_closed =
                    session.publish_ordered_event(ExecProcessEvent::Output(ProcessOutputChunk {
                        seq: params.seq,
                        stream: params.stream,
                        chunk: params.chunk,
                    }));
                if published_closed {
                    inner.remove_process_session_route(&params.process_id).await;
                }
            }
        }
        EXEC_EXITED_METHOD => {
            let params: ExecExitedNotification =
                serde_json::from_value(notification.params.unwrap_or(Value::Null))?;
            if let Some(session) = inner.get_process_session_route(&params.process_id) {
                session.note_change(params.seq);
                let published_closed = session.publish_ordered_event(ExecProcessEvent::Exited {
                    seq: params.seq,
                    exit_code: params.exit_code,
                });
                if published_closed {
                    inner.remove_process_session_route(&params.process_id).await;
                }
            }
        }
        EXEC_CLOSED_METHOD => {
            let params: ExecClosedNotification =
                serde_json::from_value(notification.params.unwrap_or(Value::Null))?;
            if let Some(session) = inner.get_process_session_route(&params.process_id) {
                session.note_change(params.seq);
                // Closed is terminal, but it can arrive before tail output or
                // exited. Keep routing this process until the ordered publisher
                // says Closed has actually been delivered.
                let published_closed =
                    session.publish_ordered_event(ExecProcessEvent::Closed { seq: params.seq });
                if published_closed {
                    inner.remove_process_session_route(&params.process_id).await;
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
    use pretty_assertions::assert_eq;
    use std::collections::HashMap;
    #[cfg(unix)]
    use std::path::Path;
    #[cfg(unix)]
    use std::process::Command;
    use tokio::io::AsyncBufReadExt;
    use tokio::io::AsyncWrite;
    use tokio::io::AsyncWriteExt;
    use tokio::io::BufReader;
    use tokio::io::duplex;
    use tokio::sync::mpsc;
    use tokio::sync::oneshot;
    use tokio::time::Duration;
    #[cfg(unix)]
    use tokio::time::sleep;
    use tokio::time::timeout;

    use super::ExecServerClientConnectOptions;
    use super::ExecServerConnection;
    use crate::ProcessId;
    #[cfg(not(windows))]
    use crate::client_api::DEFAULT_REMOTE_EXEC_SERVER_INITIALIZE_TIMEOUT;
    #[cfg(not(windows))]
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

    #[cfg(not(windows))]
    #[tokio::test]
    async fn connect_stdio_command_initializes_json_rpc_client() {
        let client = ExecServerConnection::connect_stdio_command(StdioExecServerConnectArgs {
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
        let client = ExecServerConnection::connect_for_transport(
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
        let client = ExecServerConnection::connect_stdio_command(StdioExecServerConnectArgs {
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

        let client = ExecServerConnection::connect_stdio_command(StdioExecServerConnectArgs {
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

        let result = ExecServerConnection::connect_stdio_command(StdioExecServerConnectArgs {
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

        let client = ExecServerConnection::connect(
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
            .register_process_session(&process_id)
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

        let client = ExecServerConnection::connect(
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
            .register_process_session(&process_id)
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

        let new_session = client
            .register_process_session(&ProcessId::from("new"))
            .await;
        assert!(matches!(
            new_session,
            Err(super::ExecServerError::Disconnected(_))
        ));

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

        let client = ExecServerConnection::connect(
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
            .register_process_session(&noisy_process_id)
            .await
            .expect("noisy session should register");
        let quiet_session = client
            .register_process_session(&quiet_process_id)
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
