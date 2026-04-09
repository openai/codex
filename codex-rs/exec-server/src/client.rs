use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwap;
use codex_app_server_protocol::JSONRPCNotification;
use serde_json::Value;
use tokio::sync::Mutex;
use tokio::sync::watch;

use tokio::time::timeout;
use tokio_tungstenite::connect_async;
use tracing::debug;

use crate::ProcessId;
use crate::client_api::ExecServerClientConnectOptions;
use crate::client_api::RemoteExecServerConnectArgs;
use crate::connection::JsonRpcConnection;
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
use crate::protocol::INITIALIZE_METHOD;
use crate::protocol::INITIALIZED_METHOD;
use crate::protocol::InitializeParams;
use crate::protocol::InitializeResponse;
use crate::protocol::ReadParams;
use crate::protocol::ReadResponse;
use crate::protocol::TerminateParams;
use crate::protocol::TerminateResponse;
use crate::protocol::WriteParams;
use crate::protocol::WriteResponse;
use crate::rpc::RpcCallError;
use crate::rpc::RpcClient;
use crate::rpc::RpcClientEvent;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const INITIALIZE_TIMEOUT: Duration = Duration::from_secs(10);

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
    failure: Mutex<Option<String>>,
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
    #[error("failed to serialize or deserialize exec-server JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("exec-server protocol error: {0}")]
    Protocol(String),
    #[error("exec-server rejected request ({code}): {message}")]
    Server { code: i64, message: String },
}

impl ExecServerClient {
    pub async fn connect_websocket(
        args: RemoteExecServerConnectArgs,
    ) -> Result<Self, ExecServerError> {
        let websocket_url = args.websocket_url.clone();
        let connect_timeout = args.connect_timeout;
        let (stream, _) = timeout(connect_timeout, connect_async(websocket_url.as_str()))
            .await
            .map_err(|_| ExecServerError::WebSocketConnectTimeout {
                url: websocket_url.clone(),
                timeout: connect_timeout,
            })?
            .map_err(|source| ExecServerError::WebSocketConnect {
                url: websocket_url.clone(),
                source,
            })?;

        Self::connect(
            JsonRpcConnection::from_websocket(
                stream,
                format!("exec-server websocket {websocket_url}"),
            ),
            args.into(),
        )
        .await
    }

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
        self.inner
            .client
            .call(EXEC_METHOD, &params)
            .await
            .map_err(Into::into)
    }

    pub async fn read(&self, params: ReadParams) -> Result<ReadResponse, ExecServerError> {
        self.inner
            .client
            .call(EXEC_READ_METHOD, &params)
            .await
            .map_err(Into::into)
    }

    pub async fn write(
        &self,
        process_id: &ProcessId,
        chunk: Vec<u8>,
    ) -> Result<WriteResponse, ExecServerError> {
        self.inner
            .client
            .call(
                EXEC_WRITE_METHOD,
                &WriteParams {
                    process_id: process_id.clone(),
                    chunk: chunk.into(),
                },
            )
            .await
            .map_err(Into::into)
    }

    pub async fn terminate(
        &self,
        process_id: &ProcessId,
    ) -> Result<TerminateResponse, ExecServerError> {
        self.inner
            .client
            .call(
                EXEC_TERMINATE_METHOD,
                &TerminateParams {
                    process_id: process_id.clone(),
                },
            )
            .await
            .map_err(Into::into)
    }

    pub async fn fs_read_file(
        &self,
        params: FsReadFileParams,
    ) -> Result<FsReadFileResponse, ExecServerError> {
        self.inner
            .client
            .call(FS_READ_FILE_METHOD, &params)
            .await
            .map_err(Into::into)
    }

    pub async fn fs_write_file(
        &self,
        params: FsWriteFileParams,
    ) -> Result<FsWriteFileResponse, ExecServerError> {
        self.inner
            .client
            .call(FS_WRITE_FILE_METHOD, &params)
            .await
            .map_err(Into::into)
    }

    pub async fn fs_create_directory(
        &self,
        params: FsCreateDirectoryParams,
    ) -> Result<FsCreateDirectoryResponse, ExecServerError> {
        self.inner
            .client
            .call(FS_CREATE_DIRECTORY_METHOD, &params)
            .await
            .map_err(Into::into)
    }

    pub async fn fs_get_metadata(
        &self,
        params: FsGetMetadataParams,
    ) -> Result<FsGetMetadataResponse, ExecServerError> {
        self.inner
            .client
            .call(FS_GET_METADATA_METHOD, &params)
            .await
            .map_err(Into::into)
    }

    pub async fn fs_read_directory(
        &self,
        params: FsReadDirectoryParams,
    ) -> Result<FsReadDirectoryResponse, ExecServerError> {
        self.inner
            .client
            .call(FS_READ_DIRECTORY_METHOD, &params)
            .await
            .map_err(Into::into)
    }

    pub async fn fs_remove(
        &self,
        params: FsRemoveParams,
    ) -> Result<FsRemoveResponse, ExecServerError> {
        self.inner
            .client
            .call(FS_REMOVE_METHOD, &params)
            .await
            .map_err(Into::into)
    }

    pub async fn fs_copy(&self, params: FsCopyParams) -> Result<FsCopyResponse, ExecServerError> {
        self.inner
            .client
            .call(FS_COPY_METHOD, &params)
            .await
            .map_err(Into::into)
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

    async fn connect(
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
                                fail_all_sessions(
                                    &inner,
                                    format!("exec-server notification handling failed: {err}"),
                                )
                                .await;
                                return;
                            }
                        }
                        RpcClientEvent::Disconnected { reason } => {
                            if let Some(inner) = weak.upgrade() {
                                fail_all_sessions(&inner, disconnected_message(reason.as_deref()))
                                    .await;
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
            failure: Mutex::new(None),
        }
    }

    pub(crate) fn subscribe(&self) -> watch::Receiver<u64> {
        self.wake_tx.subscribe()
    }

    fn note_change(&self, seq: u64) {
        let next = (*self.wake_tx.borrow()).max(seq);
        let _ = self.wake_tx.send(next);
    }

    async fn set_failure(&self, message: String) {
        let mut failure = self.failure.lock().await;
        if failure.is_none() {
            *failure = Some(message);
        }
        drop(failure);
        let next = (*self.wake_tx.borrow()).saturating_add(1);
        let _ = self.wake_tx.send(next);
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

    pub(crate) async fn terminate(&self) -> Result<(), ExecServerError> {
        self.client.terminate(&self.process_id).await?;
        Ok(())
    }

    pub(crate) async fn unregister(&self) {
        self.client.unregister_session(&self.process_id).await;
    }
}

impl Inner {
    fn get_session(&self, process_id: &ProcessId) -> Option<Arc<SessionState>> {
        self.sessions.load().get(process_id).cloned()
    }

    async fn insert_session(
        &self,
        process_id: &ProcessId,
        session: Arc<SessionState>,
    ) -> Result<(), ExecServerError> {
        let _sessions_write_guard = self.sessions_write_lock.lock().await;
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
    matches!(error, ExecServerError::Closed)
        || matches!(
            error,
            ExecServerError::Server {
                code: -32000,
                message,
            } if message == "JSON-RPC transport closed"
        )
}

async fn fail_all_sessions(inner: &Arc<Inner>, message: String) {
    let sessions = inner.take_all_sessions().await;

    for (_, session) in sessions {
        session.set_failure(message.clone()).await;
    }
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
            }
        }
        EXEC_EXITED_METHOD => {
            let params: ExecExitedNotification =
                serde_json::from_value(notification.params.unwrap_or(Value::Null))?;
            if let Some(session) = inner.get_session(&params.process_id) {
                session.note_change(params.seq);
            }
        }
        EXEC_CLOSED_METHOD => {
            let params: ExecClosedNotification =
                serde_json::from_value(notification.params.unwrap_or(Value::Null))?;
            // Closed is the terminal lifecycle event for this process, so drop
            // the routing entry before forwarding it.
            let session = inner.remove_session(&params.process_id).await;
            if let Some(session) = session {
                session.note_change(params.seq);
            }
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
    use tokio::io::AsyncBufReadExt;
    use tokio::io::AsyncWrite;
    use tokio::io::AsyncWriteExt;
    use tokio::io::BufReader;
    use tokio::io::duplex;
    use tokio::sync::mpsc;
    use tokio::time::Duration;
    use tokio::time::timeout;

    use super::ExecServerClient;
    use super::ExecServerClientConnectOptions;
    use crate::ProcessId;
    use crate::connection::JsonRpcConnection;
    use crate::protocol::EXEC_EXITED_METHOD;
    use crate::protocol::EXEC_OUTPUT_DELTA_METHOD;
    use crate::protocol::ExecExitedNotification;
    use crate::protocol::ExecOutputDeltaNotification;
    use crate::protocol::ExecOutputStream;
    use crate::protocol::INITIALIZE_METHOD;
    use crate::protocol::INITIALIZED_METHOD;
    use crate::protocol::InitializeResponse;

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
