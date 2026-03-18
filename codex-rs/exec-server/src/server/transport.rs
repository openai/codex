use std::net::SocketAddr;
use std::str::FromStr;

use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tracing::warn;

use crate::connection::JsonRpcConnection;
use crate::server::processor::run_connection;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecServerTransport {
    Stdio,
    WebSocket { bind_address: SocketAddr },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ExecServerTransportParseError {
    UnsupportedListenUrl(String),
    InvalidWebSocketListenUrl(String),
}

impl std::fmt::Display for ExecServerTransportParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecServerTransportParseError::UnsupportedListenUrl(listen_url) => write!(
                f,
                "unsupported --listen URL `{listen_url}`; expected `stdio://` or `ws://IP:PORT`"
            ),
            ExecServerTransportParseError::InvalidWebSocketListenUrl(listen_url) => write!(
                f,
                "invalid websocket --listen URL `{listen_url}`; expected `ws://IP:PORT`"
            ),
        }
    }
}

impl std::error::Error for ExecServerTransportParseError {}

impl ExecServerTransport {
    pub const DEFAULT_LISTEN_URL: &str = "ws://127.0.0.1:0";

    pub fn from_listen_url(listen_url: &str) -> Result<Self, ExecServerTransportParseError> {
        if listen_url == "stdio://" {
            return Ok(Self::Stdio);
        }

        if let Some(socket_addr) = listen_url.strip_prefix("ws://") {
            let bind_address = socket_addr.parse::<SocketAddr>().map_err(|_| {
                ExecServerTransportParseError::InvalidWebSocketListenUrl(listen_url.to_string())
            })?;
            return Ok(Self::WebSocket { bind_address });
        }

        Err(ExecServerTransportParseError::UnsupportedListenUrl(
            listen_url.to_string(),
        ))
    }
}

impl FromStr for ExecServerTransport {
    type Err = ExecServerTransportParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_listen_url(s)
    }
}

pub(crate) async fn run_transport(
    transport: ExecServerTransport,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match transport {
        ExecServerTransport::Stdio => {
            run_connection(JsonRpcConnection::from_stdio(
                tokio::io::stdin(),
                tokio::io::stdout(),
                "exec-server stdio".to_string(),
            ))
            .await;
            Ok(())
        }
        ExecServerTransport::WebSocket { bind_address } => {
            run_websocket_listener(bind_address).await
        }
    }
}

async fn run_websocket_listener(
    bind_address: SocketAddr,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let listener = TcpListener::bind(bind_address).await?;
    let local_addr = listener.local_addr()?;
    print_websocket_startup_banner(local_addr);

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

#[allow(clippy::print_stderr)]
fn print_websocket_startup_banner(addr: SocketAddr) {
    eprintln!("codex-exec-server listening on ws://{addr}");
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::ExecServerTransport;

    #[test]
    fn exec_server_transport_parses_default_websocket_listen_url() {
        let transport =
            ExecServerTransport::from_listen_url(ExecServerTransport::DEFAULT_LISTEN_URL)
                .expect("default listen URL should parse");
        assert_eq!(
            transport,
            ExecServerTransport::WebSocket {
                bind_address: "127.0.0.1:0".parse().expect("valid socket address"),
            }
        );
    }

    #[test]
    fn exec_server_transport_parses_stdio_listen_url() {
        let transport = ExecServerTransport::from_listen_url("stdio://")
            .expect("stdio listen URL should parse");
        assert_eq!(transport, ExecServerTransport::Stdio);
    }

    #[test]
    fn exec_server_transport_parses_websocket_listen_url() {
        let transport = ExecServerTransport::from_listen_url("ws://127.0.0.1:1234")
            .expect("websocket listen URL should parse");
        assert_eq!(
            transport,
            ExecServerTransport::WebSocket {
                bind_address: "127.0.0.1:1234".parse().expect("valid socket address"),
            }
        );
    }

    #[test]
    fn exec_server_transport_rejects_invalid_websocket_listen_url() {
        let err = ExecServerTransport::from_listen_url("ws://localhost:1234")
            .expect_err("hostname bind address should be rejected");
        assert_eq!(
            err.to_string(),
            "invalid websocket --listen URL `ws://localhost:1234`; expected `ws://IP:PORT`"
        );
    }

    #[test]
    fn exec_server_transport_rejects_unsupported_listen_url() {
        let err = ExecServerTransport::from_listen_url("http://127.0.0.1:1234")
            .expect_err("unsupported scheme should fail");
        assert_eq!(
            err.to_string(),
            "unsupported --listen URL `http://127.0.0.1:1234`; expected `stdio://` or `ws://IP:PORT`"
        );
    }
}
