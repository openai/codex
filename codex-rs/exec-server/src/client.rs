use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwap;
use codex_app_server_protocol::FsCopyParams;
use codex_app_server_protocol::FsCopyResponse;
use codex_app_server_protocol::FsCreateDirectoryParams;
use codex_app_server_protocol::FsCreateDirectoryResponse;
use codex_app_server_protocol::FsGetMetadataParams;
use codex_app_server_protocol::FsGetMetadataResponse;
use codex_app_server_protocol::FsReadDirectoryParams;
use codex_app_server_protocol::FsReadDirectoryResponse;
use codex_app_server_protocol::FsReadFileParams;
use codex_app_server_protocol::FsReadFileResponse;
use codex_app_server_protocol::FsRemoveParams;
use codex_app_server_protocol::FsRemoveResponse;
use codex_app_server_protocol::FsWriteFileParams;
use codex_app_server_protocol::FsWriteFileResponse;
use codex_app_server_protocol::JSONRPCNotification;
use serde_json::Value;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_tungstenite::connect_async;
use tracing::debug;

use crate::client_api::ExecServerClientConnectOptions;
use crate::client_api::RemoteExecServerConnectArgs;
use crate::connection::JsonRpcConnection;
use crate::process::ExecSessionEvent;
use crate::process::SESSION_EVENT_CHANNEL_CAPACITY;
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
        }
    }
}

impl From<RemoteExecServerConnectArgs> for ExecServerClientConnectOptions {
    fn from(value: RemoteExecServerConnectArgs) -> Self {
        Self {
            client_name: value.client_name,
            initialize_timeout: value.initialize_timeout,
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
        }
    }
}

struct Inner {
    client: RpcClient,
    // The remote transport delivers one shared notification stream for every
    // process on the connection. Keep a local process_id -> sender registry so
    // we can demux those connection-global notifications into the single
    // process-scoped event channel returned by ExecBackend::start().
    sessions: ArcSwap<HashMap<String, mpsc::Sender<ExecSessionEvent>>>,
    // ArcSwap makes reads cheap on the hot notification path, but writes still
    // need serialization so concurrent register/remove operations do not
    // overwrite each other's copy-on-write updates.
    sessions_write_lock: Mutex<()>,
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
        } = options;

        timeout(initialize_timeout, async {
            let response = self
                .inner
                .client
                .call(INITIALIZE_METHOD, &InitializeParams { client_name })
                .await?;
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
        process_id: &str,
        chunk: Vec<u8>,
    ) -> Result<WriteResponse, ExecServerError> {
        self.inner
            .client
            .call(
                EXEC_WRITE_METHOD,
                &WriteParams {
                    process_id: process_id.to_string(),
                    chunk: chunk.into(),
                },
            )
            .await
            .map_err(Into::into)
    }

    pub async fn terminate(&self, process_id: &str) -> Result<TerminateResponse, ExecServerError> {
        self.inner
            .client
            .call(
                EXEC_TERMINATE_METHOD,
                &TerminateParams {
                    process_id: process_id.to_string(),
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
        process_id: &str,
    ) -> Result<mpsc::Receiver<ExecSessionEvent>, ExecServerError> {
        let (events_tx, events_rx) = mpsc::channel(SESSION_EVENT_CHANNEL_CAPACITY);
        let _sessions_write_guard = self.inner.sessions_write_lock.lock().await;
        let sessions = self.inner.sessions.load();
        if sessions.contains_key(process_id) {
            return Err(ExecServerError::Protocol(format!(
                "session already registered for process {process_id}"
            )));
        }
        let mut next_sessions = sessions.as_ref().clone();
        next_sessions.insert(process_id.to_string(), events_tx);
        self.inner.sessions.store(Arc::new(next_sessions));
        Ok(events_rx)
    }

    pub(crate) async fn unregister_session(&self, process_id: &str) {
        let _sessions_write_guard = self.inner.sessions_write_lock.lock().await;
        let sessions = self.inner.sessions.load();
        if !sessions.contains_key(process_id) {
            return;
        }
        let mut next_sessions = sessions.as_ref().clone();
        next_sessions.remove(process_id);
        self.inner.sessions.store(Arc::new(next_sessions));
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
                                let _ = err;
                                return;
                            }
                        }
                        RpcClientEvent::Disconnected { reason } => {
                            let _ = reason;
                            return;
                        }
                    }
                }
            });

            Inner {
                client: rpc_client,
                sessions: ArcSwap::from_pointee(HashMap::new()),
                sessions_write_lock: Mutex::new(()),
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

async fn handle_server_notification(
    inner: &Arc<Inner>,
    notification: JSONRPCNotification,
) -> Result<(), ExecServerError> {
    match notification.method.as_str() {
        EXEC_OUTPUT_DELTA_METHOD => {
            let params: ExecOutputDeltaNotification =
                serde_json::from_value(notification.params.unwrap_or(Value::Null))?;
            // Remote exec-server notifications are connection-global, so route
            // each event to the single local receiver that owns this process.
            let events_tx = inner.sessions.load().get(&params.process_id).cloned();
            if let Some(events_tx) = events_tx {
                let _ = events_tx
                    .send(ExecSessionEvent::Output {
                        seq: params.seq,
                        stream: params.stream,
                        chunk: params.chunk.into_inner(),
                    })
                    .await;
            }
        }
        EXEC_EXITED_METHOD => {
            let params: ExecExitedNotification =
                serde_json::from_value(notification.params.unwrap_or(Value::Null))?;
            let events_tx = inner.sessions.load().get(&params.process_id).cloned();
            if let Some(events_tx) = events_tx {
                let _ = events_tx
                    .send(ExecSessionEvent::Exited {
                        seq: params.seq,
                        exit_code: params.exit_code,
                    })
                    .await;
            }
        }
        EXEC_CLOSED_METHOD => {
            let params: ExecClosedNotification =
                serde_json::from_value(notification.params.unwrap_or(Value::Null))?;
            let events_tx = {
                let _sessions_write_guard = inner.sessions_write_lock.lock().await;
                let sessions = inner.sessions.load();
                let events_tx = sessions.get(&params.process_id).cloned();
                if events_tx.is_some() {
                    // Closed is the terminal lifecycle event for this process,
                    // so drop the routing entry before forwarding it.
                    let mut next_sessions = sessions.as_ref().clone();
                    next_sessions.remove(&params.process_id);
                    inner.sessions.store(Arc::new(next_sessions));
                }
                events_tx
            };
            if let Some(events_tx) = events_tx {
                let _ = events_tx
                    .send(ExecSessionEvent::Closed { seq: params.seq })
                    .await;
            }
        }
        other => {
            debug!("ignoring unknown exec-server notification: {other}");
        }
    }
    Ok(())
}
