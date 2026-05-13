use axum::Router;
use axum::extract::ConnectInfo;
use axum::extract::State;
use axum::extract::ws::WebSocketUpgrade;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::any;
use axum::routing::get;
use std::io::Write as _;
use std::net::SocketAddr;
use tokio::io;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::net::TcpListener;
use tracing::info;

use crate::ExecServerRuntimePaths;
use crate::connection::JsonRpcConnection;
use crate::server::processor::ConnectionProcessor;

pub const DEFAULT_LISTEN_URL: &str = "ws://127.0.0.1:0";

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum ExecServerListenTransport {
    WebSocket(SocketAddr),
    Stdio,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ExecServerListenUrlParseError {
    UnsupportedListenUrl(String),
    InvalidWebSocketListenUrl(String),
}

impl std::fmt::Display for ExecServerListenUrlParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecServerListenUrlParseError::UnsupportedListenUrl(listen_url) => write!(
                f,
                "unsupported --listen URL `{listen_url}`; expected `ws://IP:PORT` or `stdio`"
            ),
            ExecServerListenUrlParseError::InvalidWebSocketListenUrl(listen_url) => write!(
                f,
                "invalid websocket --listen URL `{listen_url}`; expected `ws://IP:PORT`"
            ),
        }
    }
}

impl std::error::Error for ExecServerListenUrlParseError {}

pub(crate) fn parse_listen_url(
    listen_url: &str,
) -> Result<ExecServerListenTransport, ExecServerListenUrlParseError> {
    if matches!(listen_url, "stdio" | "stdio://") {
        return Ok(ExecServerListenTransport::Stdio);
    }

    if let Some(socket_addr) = listen_url.strip_prefix("ws://") {
        return socket_addr
            .parse::<SocketAddr>()
            .map(ExecServerListenTransport::WebSocket)
            .map_err(|_| {
                ExecServerListenUrlParseError::InvalidWebSocketListenUrl(listen_url.to_string())
            });
    }

    Err(ExecServerListenUrlParseError::UnsupportedListenUrl(
        listen_url.to_string(),
    ))
}

pub(crate) async fn run_transport(
    listen_url: &str,
    runtime_paths: ExecServerRuntimePaths,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match parse_listen_url(listen_url)? {
        ExecServerListenTransport::WebSocket(bind_address) => {
            run_websocket_listener(bind_address, runtime_paths).await
        }
        ExecServerListenTransport::Stdio => run_stdio_connection(runtime_paths).await,
    }
}

async fn run_stdio_connection(
    runtime_paths: ExecServerRuntimePaths,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    run_stdio_connection_with_io(io::stdin(), io::stdout(), runtime_paths).await
}

async fn run_stdio_connection_with_io<R, W>(
    reader: R,
    writer: W,
    runtime_paths: ExecServerRuntimePaths,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let processor = ConnectionProcessor::new(runtime_paths);
    tracing::info!("codex-exec-server listening on stdio");
    processor
        .run_connection(JsonRpcConnection::from_stdio(
            reader,
            writer,
            "exec-server stdio".to_string(),
        ))
        .await;
    Ok(())
}

async fn run_websocket_listener(
    bind_address: SocketAddr,
    runtime_paths: ExecServerRuntimePaths,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let listener = TcpListener::bind(bind_address).await?;
    let local_addr = listener.local_addr()?;
    let processor = ConnectionProcessor::new(runtime_paths);
    info!("codex-exec-server listening on ws://{local_addr}");
    println!("ws://{local_addr}");
    std::io::stdout().flush()?;

    let router = Router::new()
        .route("/", any(websocket_upgrade_handler))
        .route("/readyz", get(readiness_handler))
        .with_state(ExecServerWebSocketState { processor });
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}

#[derive(Clone)]
struct ExecServerWebSocketState {
    processor: ConnectionProcessor,
}

async fn readiness_handler() -> StatusCode {
    StatusCode::OK
}

async fn websocket_upgrade_handler(
    websocket: WebSocketUpgrade,
    ConnectInfo(peer_addr): ConnectInfo<SocketAddr>,
    State(state): State<ExecServerWebSocketState>,
) -> impl IntoResponse {
    info!(%peer_addr, "exec-server websocket client connected");
    websocket.on_upgrade(move |stream| async move {
        state
            .processor
            .run_connection(JsonRpcConnection::from_axum_websocket(
                stream,
                format!("exec-server websocket {peer_addr}"),
            ))
            .await;
    })
}

#[cfg(test)]
#[path = "transport_tests.rs"]
mod transport_tests;
