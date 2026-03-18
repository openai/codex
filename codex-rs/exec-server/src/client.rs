use std::sync::Arc;
use std::time::Duration;

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
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_tungstenite::connect_async;
use tracing::debug;
use tracing::warn;

use crate::client_api::ExecServerClientConnectOptions;
use crate::client_api::ExecServerEvent;
use crate::client_api::RemoteExecServerConnectArgs;
use crate::connection::JsonRpcConnection;
use crate::protocol::EXEC_EXITED_METHOD;
use crate::protocol::EXEC_METHOD;
use crate::protocol::EXEC_OUTPUT_DELTA_METHOD;
use crate::protocol::EXEC_READ_METHOD;
use crate::protocol::EXEC_TERMINATE_METHOD;
use crate::protocol::EXEC_WRITE_METHOD;
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
use crate::rpc::RpcNotificationSender;
use crate::rpc::RpcServerOutboundMessage;

mod local_backend;
use local_backend::LocalBackend;

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

enum ClientBackend {
    Remote(RpcClient),
    InProcess(LocalBackend),
}

impl ClientBackend {
    fn as_local(&self) -> Option<&LocalBackend> {
        match self {
            ClientBackend::Remote(_) => None,
            ClientBackend::InProcess(backend) => Some(backend),
        }
    }

    fn as_remote(&self) -> Option<&RpcClient> {
        match self {
            ClientBackend::Remote(client) => Some(client),
            ClientBackend::InProcess(_) => None,
        }
    }
}

struct Inner {
    backend: ClientBackend,
    events_tx: broadcast::Sender<ExecServerEvent>,
    reader_task: tokio::task::JoinHandle<()>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        if let Some(backend) = self.backend.as_local()
            && let Ok(handle) = tokio::runtime::Handle::try_current()
        {
            let backend = backend.clone();
            handle.spawn(async move {
                backend.shutdown().await;
            });
        }
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
    pub async fn connect_in_process(
        options: ExecServerClientConnectOptions,
    ) -> Result<Self, ExecServerError> {
        let (outgoing_tx, mut outgoing_rx) = mpsc::channel::<RpcServerOutboundMessage>(256);
        let backend = LocalBackend::new(crate::server::ExecServerHandler::new(
            RpcNotificationSender::new(outgoing_tx),
        ));
        let inner = Arc::new_cyclic(|weak| {
            let weak = weak.clone();
            let reader_task = tokio::spawn(async move {
                while let Some(message) = outgoing_rx.recv().await {
                    if let Some(inner) = weak.upgrade()
                        && let Err(err) = handle_in_process_outbound_message(&inner, message).await
                    {
                        warn!(
                            "in-process exec-server client closing after unexpected response: {err}"
                        );
                        return;
                    }
                }
            });

            Inner {
                backend: ClientBackend::InProcess(backend),
                events_tx: broadcast::channel(256).0,
                reader_task,
            }
        });

        let client = Self { inner };
        client.initialize(options).await?;
        Ok(client)
    }

    pub async fn connect_stdio<R, W>(
        stdin: W,
        stdout: R,
        options: ExecServerClientConnectOptions,
    ) -> Result<Self, ExecServerError>
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        Self::connect(
            JsonRpcConnection::from_stdio(stdout, stdin, "exec-server stdio".to_string()),
            options,
        )
        .await
    }

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

    pub fn event_receiver(&self) -> broadcast::Receiver<ExecServerEvent> {
        self.inner.events_tx.subscribe()
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
            let response = if let Some(backend) = self.inner.backend.as_local() {
                backend.initialize().await?
            } else {
                let params = InitializeParams { client_name };
                let Some(remote) = self.inner.backend.as_remote() else {
                    return Err(ExecServerError::Protocol(
                        "remote backend missing during initialize".to_string(),
                    ));
                };
                remote.call(INITIALIZE_METHOD, &params).await?
            };
            self.notify_initialized().await?;
            Ok(response)
        })
        .await
        .map_err(|_| ExecServerError::InitializeTimedOut {
            timeout: initialize_timeout,
        })?
    }

    pub async fn exec(&self, params: ExecParams) -> Result<ExecResponse, ExecServerError> {
        if let Some(backend) = self.inner.backend.as_local() {
            return backend.exec(params).await;
        }
        let Some(remote) = self.inner.backend.as_remote() else {
            return Err(ExecServerError::Protocol(
                "remote backend missing during exec".to_string(),
            ));
        };
        remote.call(EXEC_METHOD, &params).await.map_err(Into::into)
    }

    pub async fn read(&self, params: ReadParams) -> Result<ReadResponse, ExecServerError> {
        if let Some(backend) = self.inner.backend.as_local() {
            return backend.exec_read(params).await;
        }
        let Some(remote) = self.inner.backend.as_remote() else {
            return Err(ExecServerError::Protocol(
                "remote backend missing during read".to_string(),
            ));
        };
        remote
            .call(EXEC_READ_METHOD, &params)
            .await
            .map_err(Into::into)
    }

    pub async fn write(
        &self,
        process_id: &str,
        chunk: Vec<u8>,
    ) -> Result<WriteResponse, ExecServerError> {
        let params = WriteParams {
            process_id: process_id.to_string(),
            chunk: chunk.into(),
        };
        if let Some(backend) = self.inner.backend.as_local() {
            return backend.exec_write(params).await;
        }
        let Some(remote) = self.inner.backend.as_remote() else {
            return Err(ExecServerError::Protocol(
                "remote backend missing during write".to_string(),
            ));
        };
        remote
            .call(EXEC_WRITE_METHOD, &params)
            .await
            .map_err(Into::into)
    }

    pub async fn terminate(&self, process_id: &str) -> Result<TerminateResponse, ExecServerError> {
        let params = TerminateParams {
            process_id: process_id.to_string(),
        };
        if let Some(backend) = self.inner.backend.as_local() {
            return backend.terminate(params).await;
        }
        let Some(remote) = self.inner.backend.as_remote() else {
            return Err(ExecServerError::Protocol(
                "remote backend missing during terminate".to_string(),
            ));
        };
        remote
            .call(EXEC_TERMINATE_METHOD, &params)
            .await
            .map_err(Into::into)
    }

    pub async fn fs_read_file(
        &self,
        params: FsReadFileParams,
    ) -> Result<FsReadFileResponse, ExecServerError> {
        if let Some(backend) = self.inner.backend.as_local() {
            return backend.fs_read_file(params).await;
        }
        let Some(remote) = self.inner.backend.as_remote() else {
            return Err(ExecServerError::Protocol(
                "remote backend missing during fs/readFile".to_string(),
            ));
        };
        remote
            .call(FS_READ_FILE_METHOD, &params)
            .await
            .map_err(Into::into)
    }

    pub async fn fs_write_file(
        &self,
        params: FsWriteFileParams,
    ) -> Result<FsWriteFileResponse, ExecServerError> {
        if let Some(backend) = self.inner.backend.as_local() {
            return backend.fs_write_file(params).await;
        }
        let Some(remote) = self.inner.backend.as_remote() else {
            return Err(ExecServerError::Protocol(
                "remote backend missing during fs/writeFile".to_string(),
            ));
        };
        remote
            .call(FS_WRITE_FILE_METHOD, &params)
            .await
            .map_err(Into::into)
    }

    pub async fn fs_create_directory(
        &self,
        params: FsCreateDirectoryParams,
    ) -> Result<FsCreateDirectoryResponse, ExecServerError> {
        if let Some(backend) = self.inner.backend.as_local() {
            return backend.fs_create_directory(params).await;
        }
        let Some(remote) = self.inner.backend.as_remote() else {
            return Err(ExecServerError::Protocol(
                "remote backend missing during fs/createDirectory".to_string(),
            ));
        };
        remote
            .call(FS_CREATE_DIRECTORY_METHOD, &params)
            .await
            .map_err(Into::into)
    }

    pub async fn fs_get_metadata(
        &self,
        params: FsGetMetadataParams,
    ) -> Result<FsGetMetadataResponse, ExecServerError> {
        if let Some(backend) = self.inner.backend.as_local() {
            return backend.fs_get_metadata(params).await;
        }
        let Some(remote) = self.inner.backend.as_remote() else {
            return Err(ExecServerError::Protocol(
                "remote backend missing during fs/getMetadata".to_string(),
            ));
        };
        remote
            .call(FS_GET_METADATA_METHOD, &params)
            .await
            .map_err(Into::into)
    }

    pub async fn fs_read_directory(
        &self,
        params: FsReadDirectoryParams,
    ) -> Result<FsReadDirectoryResponse, ExecServerError> {
        if let Some(backend) = self.inner.backend.as_local() {
            return backend.fs_read_directory(params).await;
        }
        let Some(remote) = self.inner.backend.as_remote() else {
            return Err(ExecServerError::Protocol(
                "remote backend missing during fs/readDirectory".to_string(),
            ));
        };
        remote
            .call(FS_READ_DIRECTORY_METHOD, &params)
            .await
            .map_err(Into::into)
    }

    pub async fn fs_remove(
        &self,
        params: FsRemoveParams,
    ) -> Result<FsRemoveResponse, ExecServerError> {
        if let Some(backend) = self.inner.backend.as_local() {
            return backend.fs_remove(params).await;
        }
        let Some(remote) = self.inner.backend.as_remote() else {
            return Err(ExecServerError::Protocol(
                "remote backend missing during fs/remove".to_string(),
            ));
        };
        remote
            .call(FS_REMOVE_METHOD, &params)
            .await
            .map_err(Into::into)
    }

    pub async fn fs_copy(&self, params: FsCopyParams) -> Result<FsCopyResponse, ExecServerError> {
        if let Some(backend) = self.inner.backend.as_local() {
            return backend.fs_copy(params).await;
        }
        let Some(remote) = self.inner.backend.as_remote() else {
            return Err(ExecServerError::Protocol(
                "remote backend missing during fs/copy".to_string(),
            ));
        };
        remote
            .call(FS_COPY_METHOD, &params)
            .await
            .map_err(Into::into)
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
                                warn!("exec-server client closing after protocol error: {err}");
                                return;
                            }
                        }
                        RpcClientEvent::Disconnected { reason } => {
                            if let Some(reason) = reason {
                                warn!("exec-server client transport disconnected: {reason}");
                            }
                            return;
                        }
                    }
                }
            });

            Inner {
                backend: ClientBackend::Remote(rpc_client),
                events_tx: broadcast::channel(256).0,
                reader_task,
            }
        });

        let client = Self { inner };
        client.initialize(options).await?;
        Ok(client)
    }

    async fn notify_initialized(&self) -> Result<(), ExecServerError> {
        match &self.inner.backend {
            ClientBackend::Remote(client) => client
                .notify(INITIALIZED_METHOD, &serde_json::json!({}))
                .await
                .map_err(ExecServerError::Json),
            ClientBackend::InProcess(backend) => backend.initialized().await,
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

async fn handle_in_process_outbound_message(
    inner: &Arc<Inner>,
    message: RpcServerOutboundMessage,
) -> Result<(), ExecServerError> {
    match message {
        RpcServerOutboundMessage::Response { .. } | RpcServerOutboundMessage::Error { .. } => Err(
            ExecServerError::Protocol("unexpected in-process RPC response".to_string()),
        ),
        RpcServerOutboundMessage::Notification(notification) => {
            handle_server_notification(inner, notification).await
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
            let _ = inner.events_tx.send(ExecServerEvent::OutputDelta(params));
        }
        EXEC_EXITED_METHOD => {
            let params: ExecExitedNotification =
                serde_json::from_value(notification.params.unwrap_or(Value::Null))?;
            let _ = inner.events_tx.send(ExecServerEvent::Exited(params));
        }
        other => {
            debug!("ignoring unknown exec-server notification: {other}");
        }
    }
    Ok(())
}
