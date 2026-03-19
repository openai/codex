use std::net::SocketAddr;

use tokio::net::TcpListener;
use tokio::io;
use tokio_tungstenite::accept_async;
use tracing::warn;

use crate::connection::JsonRpcConnection;
use crate::server::processor::run_connection;

pub const DEFAULT_LISTEN_URL: &str = "ws://127.0.0.1:0";

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
                "unsupported --listen URL `{listen_url}`; expected `ws://IP:PORT`"
            ),
            ExecServerListenUrlParseError::InvalidWebSocketListenUrl(listen_url) => write!(
                f,
                "invalid websocket --listen URL `{listen_url}`; expected `ws://IP:PORT`"
            ),
        }
    }
}

impl std::error::Error for ExecServerListenUrlParseError {}

#[derive(Debug, Clone, Eq, PartialEq)]
enum ListenAddress {
    Websocket(SocketAddr),
    Stdio,
}

fn parse_listen_url(listen_url: &str) -> Result<ListenAddress, ExecServerListenUrlParseError> {
    if let Some(socket_addr) = listen_url.strip_prefix("ws://") {
        return socket_addr
            .parse::<SocketAddr>()
            .map(ListenAddress::Websocket)
            .map_err(|_| {
                ExecServerListenUrlParseError::InvalidWebSocketListenUrl(listen_url.to_string())
            });
    }
    if listen_url == "stdio://" {
        return Ok(ListenAddress::Stdio);
    }

    Err(ExecServerListenUrlParseError::UnsupportedListenUrl(
        listen_url.to_string(),
    ))
}

pub(crate) async fn run_transport(
    listen_url: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match parse_listen_url(listen_url)? {
        ListenAddress::Websocket(bind_address) => run_websocket_listener(bind_address).await,
        ListenAddress::Stdio => run_stdio_listener().await,
    }
}

async fn run_stdio_listener() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    run_connection(JsonRpcConnection::from_stdio(
        io::stdin(),
        io::stdout(),
        "exec-server stdio".to_string(),
    ))
    .await;
    Ok(())
}

async fn run_websocket_listener(
    bind_address: SocketAddr,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let listener = TcpListener::bind(bind_address).await?;
    let local_addr = listener.local_addr()?;
    let listen_message = format!("codex-exec-server listening on ws://{local_addr}");
    tracing::info!("{}", listen_message);
    eprintln!("{listen_message}");

    loop {
        let (stream, peer_addr) = listener.accept().await?;
        tokio::spawn(async move {
            match accept_async(stream).await {
                Ok(websocket) => {
                    run_connection(JsonRpcConnection::from_websocket(
                        websocket,
                        format!("exec-server websocket {peer_addr}"),
                    ))
                    .await;
                }
                Err(err) => {
                    warn!(
                        "failed to accept exec-server websocket connection from {peer_addr}: {err}"
                    );
                }
            }
        });
    }
}

#[cfg(test)]
#[path = "transport_tests.rs"]
mod transport_tests;
