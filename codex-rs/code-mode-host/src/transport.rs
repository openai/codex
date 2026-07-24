use std::io;
use std::io::Write as _;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::extract::State;
use axum::extract::ws::Message;
use axum::extract::ws::WebSocket;
use axum::extract::ws::WebSocketUpgrade;
use axum::http::Request;
use axum::http::StatusCode;
use axum::http::header::ORIGIN;
use axum::middleware;
use axum::middleware::Next;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::any;
use axum::routing::get;
use codex_code_mode_protocol::host::ClientToHost;
use codex_code_mode_protocol::host::EncodedFrame;
use codex_code_mode_protocol::host::FramedReader;
use codex_code_mode_protocol::host::FramedWriter;
use codex_code_mode_protocol::host::HostToClient;
use codex_code_mode_protocol::host::MAX_FRAME_BYTES;
use futures::SinkExt;
use futures::StreamExt;
use futures::stream::SplitSink;
use futures::stream::SplitStream;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::net::TcpListener;
use tracing::info;
use tracing::warn;

use crate::HostLimits;

/// The default transport retains the standalone host's original stdio behavior.
pub const DEFAULT_LISTEN_URL: &str = "stdio";

const MAX_WEBSOCKET_FRAME_BYTES: usize = MAX_FRAME_BYTES + std::mem::size_of::<u32>();

type BoxedReader = Box<dyn AsyncRead + Send + Unpin>;
type BoxedWriter = Box<dyn AsyncWrite + Send + Unpin>;

#[derive(Debug, Clone, Eq, PartialEq)]
enum ListenTransport {
    Stdio,
    WebSocket(SocketAddr),
}

pub(crate) enum ConnectionReader {
    Framed(FramedReader<BoxedReader>),
    WebSocket(SplitStream<WebSocket>),
}

pub(crate) enum ConnectionWriter {
    Framed(FramedWriter<BoxedWriter>),
    WebSocket(SplitSink<WebSocket, Message>),
}

#[derive(Clone)]
struct WebSocketListenerState {
    limits: Arc<HostLimits>,
}

impl ConnectionReader {
    pub(crate) fn from_reader<R>(reader: R) -> Self
    where
        R: AsyncRead + Send + Unpin + 'static,
    {
        Self::Framed(FramedReader::new(Box::new(reader)))
    }

    pub(crate) async fn read(&mut self) -> io::Result<Option<ClientToHost>> {
        match self {
            Self::Framed(reader) => reader.read().await,
            Self::WebSocket(reader) => loop {
                match reader.next().await {
                    Some(Ok(Message::Binary(bytes))) => {
                        return EncodedFrame::decode_framed(&bytes).map(Some);
                    }
                    Some(Ok(Message::Text(_))) => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "code-mode websocket messages must be binary framed messages",
                        ));
                    }
                    Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
                    Some(Ok(Message::Close(_))) | None => return Ok(None),
                    Some(Err(err)) => {
                        return Err(io::Error::other(format!(
                            "failed to read code-mode websocket message: {err}"
                        )));
                    }
                }
            },
        }
    }
}

impl ConnectionWriter {
    pub(crate) fn from_writer<W>(writer: W) -> Self
    where
        W: AsyncWrite + Send + Unpin + 'static,
    {
        Self::Framed(FramedWriter::new(Box::new(writer)))
    }

    pub(crate) async fn write(&mut self, message: &HostToClient) -> io::Result<()> {
        self.write_frame(EncodedFrame::encode(message)?).await
    }

    pub(crate) async fn write_frame(&mut self, frame: EncodedFrame) -> io::Result<()> {
        match self {
            Self::Framed(writer) => writer.write_frame(&frame).await,
            Self::WebSocket(writer) => writer
                .send(Message::Binary(frame.into_framed_bytes().into()))
                .await
                .map_err(|err| {
                    io::Error::other(format!(
                        "failed to write code-mode websocket message: {err}"
                    ))
                }),
        }
    }
}

pub(crate) async fn run_transport(listen_url: &str) -> Result<()> {
    match parse_listen_url(listen_url)? {
        ListenTransport::Stdio => crate::run_stdio().await,
        ListenTransport::WebSocket(bind_address) => run_websocket_listener(bind_address).await,
    }
}

fn parse_listen_url(listen_url: &str) -> Result<ListenTransport> {
    if matches!(listen_url, "stdio" | "stdio://") {
        return Ok(ListenTransport::Stdio);
    }

    if let Some(socket_addr) = listen_url.strip_prefix("ws://") {
        return socket_addr
            .parse::<SocketAddr>()
            .map(ListenTransport::WebSocket)
            .with_context(|| {
                format!("invalid websocket --listen URL `{listen_url}`; expected `ws://IP:PORT`")
            });
    }

    anyhow::bail!(
        "unsupported --listen URL `{listen_url}`; expected `ws://IP:PORT`, `stdio`, or `stdio://`"
    );
}

async fn run_websocket_listener(bind_address: SocketAddr) -> Result<()> {
    let listener = TcpListener::bind(bind_address)
        .await
        .with_context(|| format!("failed to bind code-mode host websocket to {bind_address}"))?;
    let local_addr = listener
        .local_addr()
        .context("failed to read code-mode host websocket listen address")?;
    let state = WebSocketListenerState {
        limits: Arc::new(HostLimits::new()),
    };
    info!("codex-code-mode-host listening on ws://{local_addr}");
    println!("ws://{local_addr}");
    io::stdout()
        .flush()
        .context("failed to publish code-mode host websocket listen address")?;

    let router = Router::new()
        .route("/", any(websocket_upgrade_handler))
        .route("/readyz", get(readiness_handler))
        .layer(middleware::from_fn(reject_requests_with_origin_header))
        .with_state(state);
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .context("code-mode host websocket listener failed")
}

async fn readiness_handler() -> StatusCode {
    StatusCode::OK
}

async fn reject_requests_with_origin_header(
    request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    if request.headers().contains_key(ORIGIN) {
        warn!(
            method = %request.method(),
            uri = %request.uri(),
            "rejecting code-mode host websocket request with Origin header"
        );
        Err(StatusCode::FORBIDDEN)
    } else {
        Ok(next.run(request).await)
    }
}

async fn websocket_upgrade_handler(
    websocket: WebSocketUpgrade,
    ConnectInfo(peer_addr): ConnectInfo<SocketAddr>,
    State(state): State<WebSocketListenerState>,
) -> impl IntoResponse {
    websocket
        .max_frame_size(MAX_WEBSOCKET_FRAME_BYTES)
        .max_message_size(MAX_WEBSOCKET_FRAME_BYTES)
        .on_upgrade(move |stream| async move {
            info!(%peer_addr, "code-mode host websocket client connected");
            let (writer, reader) = stream.split();
            if let Err(err) = crate::run_connection(
                ConnectionReader::WebSocket(reader),
                ConnectionWriter::WebSocket(writer),
                state.limits,
            )
            .await
            {
                warn!(%peer_addr, "code-mode host websocket connection failed: {err:#}");
            }
        })
}

#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;
