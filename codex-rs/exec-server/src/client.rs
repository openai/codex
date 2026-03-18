use std::sync::Arc;
use std::time::Duration;

use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::time::timeout;
use tokio_tungstenite::connect_async;
use tracing::warn;

use crate::client_api::ExecServerClientConnectOptions;
use crate::client_api::RemoteExecServerConnectArgs;
use crate::connection::JsonRpcConnection;
use crate::protocol::INITIALIZE_METHOD;
use crate::protocol::INITIALIZED_METHOD;
use crate::protocol::InitializeParams;
use crate::protocol::InitializeResponse;
use crate::rpc::RpcCallError;
use crate::rpc::RpcClient;
use crate::rpc::RpcClientEvent;

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
        let backend = LocalBackend::new(crate::server::ExecServerHandler::new());
        let inner = Arc::new(Inner {
            backend: ClientBackend::InProcess(backend),
            reader_task: tokio::spawn(async {}),
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

    async fn connect(
        connection: JsonRpcConnection,
        options: ExecServerClientConnectOptions,
    ) -> Result<Self, ExecServerError> {
        let (rpc_client, mut events_rx) = RpcClient::new(connection);
        let reader_task = tokio::spawn(async move {
            while let Some(event) = events_rx.recv().await {
                match event {
                    RpcClientEvent::Notification(notification) => {
                        warn!(
                            "ignoring unexpected exec-server notification during stub phase: {}",
                            notification.method
                        );
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

        let client = Self {
            inner: Arc::new(Inner {
                backend: ClientBackend::Remote(rpc_client),
                reader_task,
            }),
        };
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
