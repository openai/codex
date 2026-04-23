use super::CHANNEL_CAPACITY;
use super::ConnectionOrigin;
use super::TransportEvent;
use super::auth::WebsocketAuthPolicy;
use super::auth::authorize_upgrade;
use super::auth::should_warn_about_unauthenticated_non_loopback_listener;
use super::forward_incoming_message;
use super::next_connection_id;
use super::serialize_outgoing_message;
use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::QueuedOutgoingMessage;
use axum::Router;
use axum::body::Body;
use axum::body::Bytes;
use axum::extract::ConnectInfo;
use axum::extract::State;
use axum::extract::ws::Message as AxumWebSocketMessage;
use axum::extract::ws::WebSocketUpgrade;
use axum::http::HeaderMap;
use axum::http::Request;
use axum::http::StatusCode;
use axum::http::header::ORIGIN;
use axum::middleware;
use axum::middleware::Next;
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::any;
use axum::routing::get;
use futures::SinkExt;
use futures::StreamExt;
use owo_colors::OwoColorize;
use owo_colors::Stream;
use owo_colors::Style;
use std::io::Result as IoResult;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message as TungsteniteWebSocketMessage;
use tokio_util::sync::CancellationToken;
use tracing::error;
use tracing::info;
use tracing::warn;

fn colorize(text: &str, style: Style) -> String {
    text.if_supports_color(Stream::Stderr, |value| value.style(style))
        .to_string()
}

#[allow(clippy::print_stderr)]
fn print_websocket_startup_banner(addr: SocketAddr) {
    let title = colorize("codex app-server (WebSockets)", Style::new().bold().cyan());
    let listening_label = colorize("listening on:", Style::new().dimmed());
    let listen_url = colorize(&format!("ws://{addr}"), Style::new().green());
    let ready_label = colorize("readyz:", Style::new().dimmed());
    let ready_url = colorize(&format!("http://{addr}/readyz"), Style::new().green());
    let health_label = colorize("healthz:", Style::new().dimmed());
    let health_url = colorize(&format!("http://{addr}/healthz"), Style::new().green());
    let note_label = colorize("note:", Style::new().dimmed());
    eprintln!("{title}");
    eprintln!("  {listening_label} {listen_url}");
    eprintln!("  {ready_label} {ready_url}");
    eprintln!("  {health_label} {health_url}");
    if addr.ip().is_loopback() {
        eprintln!(
            "  {note_label} binds localhost only (use SSH port-forwarding for remote access)"
        );
    } else {
        eprintln!(
            "  {note_label} websocket auth is opt-in in this build; configure `--ws-auth ...` before real remote use"
        );
    }
}

#[derive(Clone)]
struct WebSocketListenerState {
    transport_event_tx: mpsc::Sender<TransportEvent>,
    auth_policy: Arc<WebsocketAuthPolicy>,
}

async fn health_check_handler() -> StatusCode {
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
            "rejecting websocket listener request with Origin header"
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
    headers: HeaderMap,
) -> impl IntoResponse {
    if let Err(err) = authorize_upgrade(&headers, state.auth_policy.as_ref()) {
        warn!(
            %peer_addr,
            message = err.message(),
            "rejecting websocket client during upgrade"
        );
        return (err.status_code(), err.message()).into_response();
    }
    info!(%peer_addr, "websocket client connected");
    websocket
        .on_upgrade(move |stream| async move {
            let (websocket_writer, websocket_reader) = stream.split();
            run_websocket_connection(websocket_writer, websocket_reader, state.transport_event_tx)
                .await;
        })
        .into_response()
}

pub(crate) async fn start_websocket_acceptor(
    bind_address: SocketAddr,
    transport_event_tx: mpsc::Sender<TransportEvent>,
    shutdown_token: CancellationToken,
    auth_policy: WebsocketAuthPolicy,
) -> IoResult<JoinHandle<()>> {
    if should_warn_about_unauthenticated_non_loopback_listener(bind_address, &auth_policy) {
        warn!(
            %bind_address,
            "starting non-loopback websocket listener without auth; websocket auth is opt-in for now and will become the default in a future release"
        );
    }
    let listener = TcpListener::bind(bind_address).await?;
    let local_addr = listener.local_addr()?;
    print_websocket_startup_banner(local_addr);
    info!("app-server websocket listening on ws://{local_addr}");

    let router = Router::new()
        .route("/readyz", get(health_check_handler))
        .route("/healthz", get(health_check_handler))
        .fallback(any(websocket_upgrade_handler))
        .layer(middleware::from_fn(reject_requests_with_origin_header))
        .with_state(WebSocketListenerState {
            transport_event_tx,
            auth_policy: Arc::new(auth_policy),
        });
    let server = axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        shutdown_token.cancelled().await;
    });
    Ok(tokio::spawn(async move {
        if let Err(err) = server.await {
            error!("websocket acceptor failed: {err}");
        }
        info!("websocket acceptor shutting down");
    }))
}

pub(crate) async fn run_websocket_connection<M, SinkError, StreamError>(
    websocket_writer: impl futures::sink::Sink<M, Error = SinkError> + Send + 'static,
    websocket_reader: impl futures::stream::Stream<Item = Result<M, StreamError>> + Send + 'static,
    transport_event_tx: mpsc::Sender<TransportEvent>,
) where
    M: AppServerWebSocketMessage + Send + 'static,
    SinkError: Send + 'static,
    StreamError: std::fmt::Display + Send + 'static,
{
    let connection_id = next_connection_id();
    let (writer_tx, writer_rx) = mpsc::channel::<QueuedOutgoingMessage>(CHANNEL_CAPACITY);
    let writer_tx_for_reader = writer_tx.clone();
    let disconnect_token = CancellationToken::new();
    if transport_event_tx
        .send(TransportEvent::ConnectionOpened {
            connection_id,
            origin: ConnectionOrigin::WebSocket,
            writer: writer_tx,
            disconnect_sender: Some(disconnect_token.clone()),
        })
        .await
        .is_err()
    {
        return;
    }

    let (writer_control_tx, writer_control_rx) = mpsc::channel::<M>(CHANNEL_CAPACITY);
    let mut outbound_task = tokio::spawn(run_websocket_outbound_loop(
        websocket_writer,
        writer_rx,
        writer_control_rx,
        disconnect_token.clone(),
    ));
    let mut inbound_task = tokio::spawn(run_websocket_inbound_loop(
        websocket_reader,
        transport_event_tx.clone(),
        writer_tx_for_reader,
        writer_control_tx,
        connection_id,
        disconnect_token.clone(),
    ));

    tokio::select! {
        _ = &mut outbound_task => {
            disconnect_token.cancel();
            inbound_task.abort();
        }
        _ = &mut inbound_task => {
            disconnect_token.cancel();
            outbound_task.abort();
        }
    }

    let _ = transport_event_tx
        .send(TransportEvent::ConnectionClosed { connection_id })
        .await;
}

pub(crate) enum IncomingWebSocketMessage {
    Text(String),
    Binary,
    Ping(Bytes),
    Pong,
    Close,
}

/// Converts concrete WebSocket message types into the small message surface the
/// app-server transport needs, and constructs the only outbound frames it
/// sends directly.
pub(crate) trait AppServerWebSocketMessage: Sized {
    fn text(text: String) -> Self;
    fn pong(payload: Bytes) -> Self;
    fn into_incoming(self) -> Option<IncomingWebSocketMessage>;
}

impl AppServerWebSocketMessage for AxumWebSocketMessage {
    fn text(text: String) -> Self {
        Self::Text(text.into())
    }

    fn pong(payload: Bytes) -> Self {
        Self::Pong(payload)
    }

    fn into_incoming(self) -> Option<IncomingWebSocketMessage> {
        Some(match self {
            Self::Text(text) => IncomingWebSocketMessage::Text(text.to_string()),
            Self::Binary(_) => IncomingWebSocketMessage::Binary,
            Self::Ping(payload) => IncomingWebSocketMessage::Ping(payload),
            Self::Pong(_) => IncomingWebSocketMessage::Pong,
            Self::Close(_) => IncomingWebSocketMessage::Close,
        })
    }
}

impl AppServerWebSocketMessage for TungsteniteWebSocketMessage {
    fn text(text: String) -> Self {
        Self::Text(text.into())
    }

    fn pong(payload: Bytes) -> Self {
        Self::Pong(payload)
    }

    fn into_incoming(self) -> Option<IncomingWebSocketMessage> {
        Some(match self {
            Self::Text(text) => IncomingWebSocketMessage::Text(text.to_string()),
            Self::Binary(_) => IncomingWebSocketMessage::Binary,
            Self::Ping(payload) => IncomingWebSocketMessage::Ping(payload),
            Self::Pong(_) => IncomingWebSocketMessage::Pong,
            Self::Close(_) => IncomingWebSocketMessage::Close,
            Self::Frame(_) => return None,
        })
    }
}

async fn run_websocket_outbound_loop<M, SinkError>(
    websocket_writer: impl futures::sink::Sink<M, Error = SinkError> + Send + 'static,
    mut writer_rx: mpsc::Receiver<QueuedOutgoingMessage>,
    mut writer_control_rx: mpsc::Receiver<M>,
    disconnect_token: CancellationToken,
) where
    M: AppServerWebSocketMessage + Send + 'static,
    SinkError: Send + 'static,
{
    tokio::pin!(websocket_writer);
    loop {
        tokio::select! {
            _ = disconnect_token.cancelled() => {
                break;
            }
            message = writer_control_rx.recv() => {
                let Some(message) = message else {
                    break;
                };
                if websocket_writer.send(message).await.is_err() {
                    break;
                }
            }
            queued_message = writer_rx.recv() => {
                let Some(queued_message) = queued_message else {
                    break;
                };
                let Some(json) = serialize_outgoing_message(queued_message.message) else {
                    continue;
                };
                if websocket_writer.send(M::text(json)).await.is_err() {
                    break;
                }
                if let Some(write_complete_tx) = queued_message.write_complete_tx {
                    let _ = write_complete_tx.send(());
                }
            }
        }
    }
}

async fn run_websocket_inbound_loop<M, StreamError>(
    websocket_reader: impl futures::stream::Stream<Item = Result<M, StreamError>> + Send + 'static,
    transport_event_tx: mpsc::Sender<TransportEvent>,
    writer_tx_for_reader: mpsc::Sender<QueuedOutgoingMessage>,
    writer_control_tx: mpsc::Sender<M>,
    connection_id: ConnectionId,
    disconnect_token: CancellationToken,
) where
    M: AppServerWebSocketMessage + Send + 'static,
    StreamError: std::fmt::Display + Send + 'static,
{
    tokio::pin!(websocket_reader);
    loop {
        tokio::select! {
            _ = disconnect_token.cancelled() => {
                break;
            }
            incoming_message = websocket_reader.next() => {
                match incoming_message {
                    Some(Ok(message)) => match message.into_incoming() {
                        Some(IncomingWebSocketMessage::Text(text)) => {
                            if !forward_incoming_message(
                                &transport_event_tx,
                                &writer_tx_for_reader,
                                connection_id,
                                &text,
                            )
                            .await
                            {
                                break;
                            }
                        }
                        Some(IncomingWebSocketMessage::Ping(payload)) => {
                            match writer_control_tx.try_send(M::pong(payload)) {
                                Ok(()) => {}
                                Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => break,
                                Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                                    warn!("websocket control queue full while replying to ping; closing connection");
                                    break;
                                }
                            }
                        }
                        Some(IncomingWebSocketMessage::Pong) => {}
                        Some(IncomingWebSocketMessage::Close) => break,
                        Some(IncomingWebSocketMessage::Binary) => {
                            warn!("dropping unsupported binary websocket message");
                        }
                        None => {}
                    },
                    None => break,
                    Some(Err(err)) => {
                        warn!("websocket receive error: {err}");
                        break;
                    }
                }
            }
        }
    }
}
