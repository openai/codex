use crate::error_code::OVERLOADED_ERROR_CODE;
use crate::message_processor::ConnectionSessionState;
use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::OutgoingEnvelope;
use crate::outgoing_message::OutgoingError;
use crate::outgoing_message::OutgoingMessage;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::JSONRPCMessage;
use futures::SinkExt;
use futures::StreamExt;
use owo_colors::OwoColorize;
use owo_colors::Stream;
use owo_colors::Style;
use std::collections::HashMap;
use std::collections::HashSet;
use std::io::ErrorKind;
use std::io::Result as IoResult;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::io::{self};
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message as WebSocketMessage;
use tokio_util::sync::CancellationToken;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::warn;

/// Size of the bounded channels used to communicate between tasks. The value
/// is a balance between throughput and memory usage - 128 messages should be
/// plenty for an interactive CLI.
pub(crate) const CHANNEL_CAPACITY: usize = 128;

fn colorize(text: &str, style: Style) -> String {
    text.if_supports_color(Stream::Stderr, |value| value.style(style))
        .to_string()
}

#[allow(clippy::print_stderr)]
fn print_websocket_startup_banner(addr: SocketAddr) {
    let title = colorize("codex app-server (WebSockets)", Style::new().bold().cyan());
    let listening_label = colorize("listening on:", Style::new().dimmed());
    let listen_url = colorize(&format!("ws://{addr}"), Style::new().green());
    let note_label = colorize("note:", Style::new().dimmed());
    eprintln!("{title}");
    eprintln!("  {listening_label} {listen_url}");
    if addr.ip().is_loopback() {
        eprintln!(
            "  {note_label} binds localhost only (use SSH port-forwarding for remote access)"
        );
    } else {
        eprintln!(
            "  {note_label} this is a raw WS server; consider running behind TLS/auth for real remote use"
        );
    }
}

#[allow(clippy::print_stderr)]
fn print_websocket_connection(peer_addr: SocketAddr) {
    let connected_label = colorize("websocket client connected from", Style::new().dimmed());
    eprintln!("{connected_label} {peer_addr}");
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppServerTransport {
    Stdio,
    WebSocket { bind_address: SocketAddr },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum AppServerTransportParseError {
    UnsupportedListenUrl(String),
    InvalidWebSocketListenUrl(String),
}

impl std::fmt::Display for AppServerTransportParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppServerTransportParseError::UnsupportedListenUrl(listen_url) => write!(
                f,
                "unsupported --listen URL `{listen_url}`; expected `stdio://` or `ws://IP:PORT`"
            ),
            AppServerTransportParseError::InvalidWebSocketListenUrl(listen_url) => write!(
                f,
                "invalid websocket --listen URL `{listen_url}`; expected `ws://IP:PORT`"
            ),
        }
    }
}

impl std::error::Error for AppServerTransportParseError {}

impl AppServerTransport {
    pub const DEFAULT_LISTEN_URL: &'static str = "stdio://";

    pub fn from_listen_url(listen_url: &str) -> Result<Self, AppServerTransportParseError> {
        if listen_url == Self::DEFAULT_LISTEN_URL {
            return Ok(Self::Stdio);
        }

        if let Some(socket_addr) = listen_url.strip_prefix("ws://") {
            let bind_address = socket_addr.parse::<SocketAddr>().map_err(|_| {
                AppServerTransportParseError::InvalidWebSocketListenUrl(listen_url.to_string())
            })?;
            return Ok(Self::WebSocket { bind_address });
        }

        Err(AppServerTransportParseError::UnsupportedListenUrl(
            listen_url.to_string(),
        ))
    }
}

impl FromStr for AppServerTransport {
    type Err = AppServerTransportParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_listen_url(s)
    }
}

#[derive(Debug)]
pub(crate) enum TransportEvent {
    ConnectionOpened {
        connection_id: ConnectionId,
        writer: mpsc::Sender<OutgoingMessage>,
        disconnect_sender: Option<CancellationToken>,
    },
    ConnectionClosed {
        connection_id: ConnectionId,
    },
    IncomingMessage {
        connection_id: ConnectionId,
        message: JSONRPCMessage,
    },
}

pub(crate) struct ConnectionState {
    pub(crate) outbound_initialized: Arc<AtomicBool>,
    pub(crate) outbound_opted_out_notification_methods: Arc<RwLock<HashSet<String>>>,
    pub(crate) session: ConnectionSessionState,
}

impl ConnectionState {
    pub(crate) fn new(
        outbound_initialized: Arc<AtomicBool>,
        outbound_opted_out_notification_methods: Arc<RwLock<HashSet<String>>>,
    ) -> Self {
        Self {
            outbound_initialized,
            outbound_opted_out_notification_methods,
            session: ConnectionSessionState::default(),
        }
    }
}

pub(crate) struct OutboundConnectionState {
    pub(crate) initialized: Arc<AtomicBool>,
    pub(crate) opted_out_notification_methods: Arc<RwLock<HashSet<String>>>,
    pub(crate) writer: mpsc::Sender<OutgoingMessage>,
    disconnect_sender: Option<CancellationToken>,
}

impl OutboundConnectionState {
    pub(crate) fn new(
        writer: mpsc::Sender<OutgoingMessage>,
        initialized: Arc<AtomicBool>,
        opted_out_notification_methods: Arc<RwLock<HashSet<String>>>,
        disconnect_sender: Option<CancellationToken>,
    ) -> Self {
        Self {
            initialized,
            opted_out_notification_methods,
            writer,
            disconnect_sender,
        }
    }

    fn can_disconnect(&self) -> bool {
        self.disconnect_sender.is_some()
    }

    fn request_disconnect(&self) {
        if let Some(disconnect_sender) = &self.disconnect_sender {
            disconnect_sender.cancel();
        }
    }
}

pub(crate) async fn start_stdio_connection(
    transport_event_tx: mpsc::Sender<TransportEvent>,
    stdio_handles: &mut Vec<JoinHandle<()>>,
) -> IoResult<()> {
    let connection_id = ConnectionId(0);
    let (writer_tx, mut writer_rx) = mpsc::channel::<OutgoingMessage>(CHANNEL_CAPACITY);
    let writer_tx_for_reader = writer_tx.clone();
    transport_event_tx
        .send(TransportEvent::ConnectionOpened {
            connection_id,
            writer: writer_tx,
            disconnect_sender: None,
        })
        .await
        .map_err(|_| std::io::Error::new(ErrorKind::BrokenPipe, "processor unavailable"))?;

    let transport_event_tx_for_reader = transport_event_tx.clone();
    stdio_handles.push(tokio::spawn(async move {
        let stdin = io::stdin();
        let reader = BufReader::new(stdin);
        let mut lines = reader.lines();

        loop {
            match lines.next_line().await {
                Ok(Some(line)) => {
                    if !forward_incoming_message(
                        &transport_event_tx_for_reader,
                        &writer_tx_for_reader,
                        connection_id,
                        &line,
                    )
                    .await
                    {
                        break;
                    }
                }
                Ok(None) => break,
                Err(err) => {
                    error!("Failed reading stdin: {err}");
                    break;
                }
            }
        }

        let _ = transport_event_tx_for_reader
            .send(TransportEvent::ConnectionClosed { connection_id })
            .await;
        debug!("stdin reader finished (EOF)");
    }));

    stdio_handles.push(tokio::spawn(async move {
        let mut stdout = io::stdout();
        while let Some(outgoing_message) = writer_rx.recv().await {
            let Some(mut json) = serialize_outgoing_message(outgoing_message) else {
                continue;
            };
            json.push('\n');
            if let Err(err) = stdout.write_all(json.as_bytes()).await {
                error!("Failed to write to stdout: {err}");
                break;
            }
        }
        info!("stdout writer exited (channel closed)");
    }));

    Ok(())
}

pub(crate) async fn start_websocket_acceptor(
    bind_address: SocketAddr,
    transport_event_tx: mpsc::Sender<TransportEvent>,
) -> IoResult<JoinHandle<()>> {
    let listener = TcpListener::bind(bind_address).await?;
    let local_addr = listener.local_addr()?;
    print_websocket_startup_banner(local_addr);
    info!("app-server websocket listening on ws://{local_addr}");

    let connection_counter = Arc::new(AtomicU64::new(1));
    Ok(tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, peer_addr)) => {
                    print_websocket_connection(peer_addr);
                    let connection_id =
                        ConnectionId(connection_counter.fetch_add(1, Ordering::Relaxed));
                    let transport_event_tx_for_connection = transport_event_tx.clone();
                    tokio::spawn(async move {
                        run_websocket_connection(
                            connection_id,
                            stream,
                            transport_event_tx_for_connection,
                        )
                        .await;
                    });
                }
                Err(err) => {
                    error!("failed to accept websocket connection: {err}");
                }
            }
        }
    }))
}

async fn run_websocket_connection(
    connection_id: ConnectionId,
    stream: TcpStream,
    transport_event_tx: mpsc::Sender<TransportEvent>,
) {
    let websocket_stream = match accept_async(stream).await {
        Ok(stream) => stream,
        Err(err) => {
            warn!("failed to complete websocket handshake: {err}");
            return;
        }
    };

    let (writer_tx, writer_rx) = mpsc::channel::<OutgoingMessage>(CHANNEL_CAPACITY);
    let writer_tx_for_reader = writer_tx.clone();
    let disconnect_token = CancellationToken::new();
    if transport_event_tx
        .send(TransportEvent::ConnectionOpened {
            connection_id,
            writer: writer_tx,
            disconnect_sender: Some(disconnect_token.clone()),
        })
        .await
        .is_err()
    {
        return;
    }

    let (websocket_writer, websocket_reader) = websocket_stream.split();
    let (writer_control_tx, writer_control_rx) =
        mpsc::channel::<WebSocketMessage>(CHANNEL_CAPACITY);
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

async fn run_websocket_outbound_loop(
    mut websocket_writer: futures::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<TcpStream>,
        WebSocketMessage,
    >,
    mut writer_rx: mpsc::Receiver<OutgoingMessage>,
    mut writer_control_rx: mpsc::Receiver<WebSocketMessage>,
    disconnect_token: CancellationToken,
) {
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
            outgoing_message = writer_rx.recv() => {
                let Some(outgoing_message) = outgoing_message else {
                    break;
                };
                let Some(json) = serialize_outgoing_message(outgoing_message) else {
                    continue;
                };
                if websocket_writer.send(WebSocketMessage::Text(json.into())).await.is_err() {
                    break;
                }
            }
        }
    }
}

async fn run_websocket_inbound_loop(
    mut websocket_reader: futures::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<TcpStream>,
    >,
    transport_event_tx: mpsc::Sender<TransportEvent>,
    writer_tx_for_reader: mpsc::Sender<OutgoingMessage>,
    writer_control_tx: mpsc::Sender<WebSocketMessage>,
    connection_id: ConnectionId,
    disconnect_token: CancellationToken,
) {
    loop {
        tokio::select! {
            _ = disconnect_token.cancelled() => {
                break;
            }
            incoming_message = websocket_reader.next() => {
                match incoming_message {
                    Some(Ok(WebSocketMessage::Text(text))) => {
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
                    Some(Ok(WebSocketMessage::Ping(payload))) => {
                        match writer_control_tx.try_send(WebSocketMessage::Pong(payload)) {
                            Ok(()) => {}
                            Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => break,
                            Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                                warn!("websocket control queue full while replying to ping; closing connection");
                                break;
                            }
                        }
                    }
                    Some(Ok(WebSocketMessage::Pong(_))) => {}
                    Some(Ok(WebSocketMessage::Close(_))) | None => break,
                    Some(Ok(WebSocketMessage::Binary(_))) => {
                        warn!("dropping unsupported binary websocket message");
                    }
                    Some(Ok(WebSocketMessage::Frame(_))) => {}
                    Some(Err(err)) => {
                        warn!("websocket receive error: {err}");
                        break;
                    }
                }
            }
        }
    }
}

async fn forward_incoming_message(
    transport_event_tx: &mpsc::Sender<TransportEvent>,
    writer: &mpsc::Sender<OutgoingMessage>,
    connection_id: ConnectionId,
    payload: &str,
) -> bool {
    match serde_json::from_str::<JSONRPCMessage>(payload) {
        Ok(message) => {
            enqueue_incoming_message(transport_event_tx, writer, connection_id, message).await
        }
        Err(err) => {
            error!("Failed to deserialize JSONRPCMessage: {err}");
            true
        }
    }
}

async fn enqueue_incoming_message(
    transport_event_tx: &mpsc::Sender<TransportEvent>,
    writer: &mpsc::Sender<OutgoingMessage>,
    connection_id: ConnectionId,
    message: JSONRPCMessage,
) -> bool {
    let event = TransportEvent::IncomingMessage {
        connection_id,
        message,
    };
    match transport_event_tx.try_send(event) {
        Ok(()) => true,
        Err(mpsc::error::TrySendError::Closed(_)) => false,
        Err(mpsc::error::TrySendError::Full(TransportEvent::IncomingMessage {
            connection_id,
            message: JSONRPCMessage::Request(request),
        })) => {
            let overload_error = OutgoingMessage::Error(OutgoingError {
                id: request.id,
                error: JSONRPCErrorError {
                    code: OVERLOADED_ERROR_CODE,
                    message: "Server overloaded; retry later.".to_string(),
                    data: None,
                },
            });
            match writer.try_send(overload_error) {
                Ok(()) => true,
                Err(mpsc::error::TrySendError::Closed(_)) => false,
                Err(mpsc::error::TrySendError::Full(_overload_error)) => {
                    warn!(
                        "dropping overload response for connection {:?}: outbound queue is full",
                        connection_id
                    );
                    true
                }
            }
        }
        Err(mpsc::error::TrySendError::Full(event)) => transport_event_tx.send(event).await.is_ok(),
    }
}

fn serialize_outgoing_message(outgoing_message: OutgoingMessage) -> Option<String> {
    let value = match serde_json::to_value(outgoing_message) {
        Ok(value) => value,
        Err(err) => {
            error!("Failed to convert OutgoingMessage to JSON value: {err}");
            return None;
        }
    };
    match serde_json::to_string(&value) {
        Ok(json) => Some(json),
        Err(err) => {
            error!("Failed to serialize JSONRPCMessage: {err}");
            None
        }
    }
}

fn should_skip_notification_for_connection(
    connection_state: &OutboundConnectionState,
    message: &OutgoingMessage,
) -> bool {
    let Ok(opted_out_notification_methods) = connection_state.opted_out_notification_methods.read()
    else {
        warn!("failed to read outbound opted-out notifications");
        return false;
    };
    match message {
        OutgoingMessage::AppServerNotification(notification) => {
            let method = notification.to_string();
            opted_out_notification_methods.contains(method.as_str())
        }
        OutgoingMessage::Notification(notification) => {
            opted_out_notification_methods.contains(notification.method.as_str())
        }
        _ => false,
    }
}

fn disconnect_connection(
    connections: &mut HashMap<ConnectionId, OutboundConnectionState>,
    connection_id: ConnectionId,
) -> bool {
    if let Some(connection_state) = connections.remove(&connection_id) {
        connection_state.request_disconnect();
        return true;
    }
    false
}

async fn send_message_to_connection(
    connections: &mut HashMap<ConnectionId, OutboundConnectionState>,
    connection_id: ConnectionId,
    message: OutgoingMessage,
) -> bool {
    let Some(connection_state) = connections.get(&connection_id) else {
        warn!("dropping message for disconnected connection: {connection_id:?}");
        return false;
    };
    if should_skip_notification_for_connection(connection_state, &message) {
        return false;
    }

    let writer = connection_state.writer.clone();
    if connection_state.can_disconnect() {
        match writer.try_send(message) {
            Ok(()) => false,
            Err(mpsc::error::TrySendError::Full(_)) => {
                warn!(
                    "disconnecting slow connection after outbound queue filled: {connection_id:?}"
                );
                disconnect_connection(connections, connection_id)
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                disconnect_connection(connections, connection_id)
            }
        }
    } else if writer.send(message).await.is_err() {
        disconnect_connection(connections, connection_id)
    } else {
        false
    }
}

pub(crate) async fn route_outgoing_envelope(
    connections: &mut HashMap<ConnectionId, OutboundConnectionState>,
    envelope: OutgoingEnvelope,
) {
    match envelope {
        OutgoingEnvelope::ToConnection {
            connection_id,
            message,
        } => {
            let _ = send_message_to_connection(connections, connection_id, message).await;
        }
        OutgoingEnvelope::Broadcast { message } => {
            let target_connections: Vec<ConnectionId> = connections
                .iter()
                .filter_map(|(connection_id, connection_state)| {
                    if connection_state.initialized.load(Ordering::Acquire)
                        && !should_skip_notification_for_connection(connection_state, &message)
                    {
                        Some(*connection_id)
                    } else {
                        None
                    }
                })
                .collect();

            for connection_id in target_connections {
                let _ =
                    send_message_to_connection(connections, connection_id, message.clone()).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error_code::OVERLOADED_ERROR_CODE;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use tokio::time::Duration;
    use tokio::time::timeout;

    #[test]
    fn app_server_transport_parses_stdio_listen_url() {
        let transport = AppServerTransport::from_listen_url(AppServerTransport::DEFAULT_LISTEN_URL)
            .expect("stdio listen URL should parse");
        assert_eq!(transport, AppServerTransport::Stdio);
    }

    #[test]
    fn app_server_transport_parses_websocket_listen_url() {
        let transport = AppServerTransport::from_listen_url("ws://127.0.0.1:1234")
            .expect("websocket listen URL should parse");
        assert_eq!(
            transport,
            AppServerTransport::WebSocket {
                bind_address: "127.0.0.1:1234".parse().expect("valid socket address"),
            }
        );
    }

    #[test]
    fn app_server_transport_rejects_invalid_websocket_listen_url() {
        let err = AppServerTransport::from_listen_url("ws://localhost:1234")
            .expect_err("hostname bind address should be rejected");
        assert_eq!(
            err.to_string(),
            "invalid websocket --listen URL `ws://localhost:1234`; expected `ws://IP:PORT`"
        );
    }

    #[test]
    fn app_server_transport_rejects_unsupported_listen_url() {
        let err = AppServerTransport::from_listen_url("http://127.0.0.1:1234")
            .expect_err("unsupported scheme should fail");
        assert_eq!(
            err.to_string(),
            "unsupported --listen URL `http://127.0.0.1:1234`; expected `stdio://` or `ws://IP:PORT`"
        );
    }

    #[tokio::test]
    async fn enqueue_incoming_request_returns_overload_error_when_queue_is_full() {
        let connection_id = ConnectionId(42);
        let (transport_event_tx, mut transport_event_rx) = mpsc::channel(1);
        let (writer_tx, mut writer_rx) = mpsc::channel(1);

        let first_message =
            JSONRPCMessage::Notification(codex_app_server_protocol::JSONRPCNotification {
                method: "initialized".to_string(),
                params: None,
            });
        transport_event_tx
            .send(TransportEvent::IncomingMessage {
                connection_id,
                message: first_message.clone(),
            })
            .await
            .expect("queue should accept first message");

        let request = JSONRPCMessage::Request(codex_app_server_protocol::JSONRPCRequest {
            id: codex_app_server_protocol::RequestId::Integer(7),
            method: "config/read".to_string(),
            params: Some(json!({ "includeLayers": false })),
        });
        assert!(
            enqueue_incoming_message(&transport_event_tx, &writer_tx, connection_id, request).await
        );

        let queued_event = transport_event_rx
            .recv()
            .await
            .expect("first event should stay queued");
        match queued_event {
            TransportEvent::IncomingMessage {
                connection_id: queued_connection_id,
                message,
            } => {
                assert_eq!(queued_connection_id, connection_id);
                assert_eq!(message, first_message);
            }
            _ => panic!("expected queued incoming message"),
        }

        let overload = writer_rx
            .recv()
            .await
            .expect("request should receive overload error");
        let overload_json = serde_json::to_value(overload).expect("serialize overload error");
        assert_eq!(
            overload_json,
            json!({
                "id": 7,
                "error": {
                    "code": OVERLOADED_ERROR_CODE,
                    "message": "Server overloaded; retry later."
                }
            })
        );
    }

    #[tokio::test]
    async fn enqueue_incoming_response_waits_instead_of_dropping_when_queue_is_full() {
        let connection_id = ConnectionId(42);
        let (transport_event_tx, mut transport_event_rx) = mpsc::channel(1);
        let (writer_tx, _writer_rx) = mpsc::channel(1);

        let first_message =
            JSONRPCMessage::Notification(codex_app_server_protocol::JSONRPCNotification {
                method: "initialized".to_string(),
                params: None,
            });
        transport_event_tx
            .send(TransportEvent::IncomingMessage {
                connection_id,
                message: first_message.clone(),
            })
            .await
            .expect("queue should accept first message");

        let response = JSONRPCMessage::Response(codex_app_server_protocol::JSONRPCResponse {
            id: codex_app_server_protocol::RequestId::Integer(7),
            result: json!({"ok": true}),
        });
        let transport_event_tx_for_enqueue = transport_event_tx.clone();
        let writer_tx_for_enqueue = writer_tx.clone();
        let enqueue_handle = tokio::spawn(async move {
            enqueue_incoming_message(
                &transport_event_tx_for_enqueue,
                &writer_tx_for_enqueue,
                connection_id,
                response,
            )
            .await
        });

        let queued_event = transport_event_rx
            .recv()
            .await
            .expect("first event should be dequeued");
        match queued_event {
            TransportEvent::IncomingMessage {
                connection_id: queued_connection_id,
                message,
            } => {
                assert_eq!(queued_connection_id, connection_id);
                assert_eq!(message, first_message);
            }
            _ => panic!("expected queued incoming message"),
        }

        let enqueue_result = enqueue_handle.await.expect("enqueue task should not panic");
        assert!(enqueue_result);

        let forwarded_event = transport_event_rx
            .recv()
            .await
            .expect("response should be forwarded instead of dropped");
        match forwarded_event {
            TransportEvent::IncomingMessage {
                connection_id: queued_connection_id,
                message:
                    JSONRPCMessage::Response(codex_app_server_protocol::JSONRPCResponse { id, result }),
            } => {
                assert_eq!(queued_connection_id, connection_id);
                assert_eq!(id, codex_app_server_protocol::RequestId::Integer(7));
                assert_eq!(result, json!({"ok": true}));
            }
            _ => panic!("expected forwarded response message"),
        }
    }

    #[tokio::test]
    async fn enqueue_incoming_request_does_not_block_when_writer_queue_is_full() {
        let connection_id = ConnectionId(42);
        let (transport_event_tx, _transport_event_rx) = mpsc::channel(1);
        let (writer_tx, mut writer_rx) = mpsc::channel(1);

        transport_event_tx
            .send(TransportEvent::IncomingMessage {
                connection_id,
                message: JSONRPCMessage::Notification(
                    codex_app_server_protocol::JSONRPCNotification {
                        method: "initialized".to_string(),
                        params: None,
                    },
                ),
            })
            .await
            .expect("transport queue should accept first message");

        writer_tx
            .send(OutgoingMessage::Notification(
                crate::outgoing_message::OutgoingNotification {
                    method: "queued".to_string(),
                    params: None,
                },
            ))
            .await
            .expect("writer queue should accept first message");

        let request = JSONRPCMessage::Request(codex_app_server_protocol::JSONRPCRequest {
            id: codex_app_server_protocol::RequestId::Integer(7),
            method: "config/read".to_string(),
            params: Some(json!({ "includeLayers": false })),
        });

        let enqueue_result = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            enqueue_incoming_message(&transport_event_tx, &writer_tx, connection_id, request),
        )
        .await
        .expect("enqueue should not block while writer queue is full");
        assert!(enqueue_result);

        let queued_outgoing = writer_rx
            .recv()
            .await
            .expect("writer queue should still contain original message");
        let queued_json = serde_json::to_value(queued_outgoing).expect("serialize queued message");
        assert_eq!(queued_json, json!({ "method": "queued" }));
    }

    #[tokio::test]
    async fn to_connection_notification_respects_opt_out_filters() {
        let connection_id = ConnectionId(7);
        let (writer_tx, mut writer_rx) = mpsc::channel(1);
        let initialized = Arc::new(AtomicBool::new(true));
        let opted_out_notification_methods = Arc::new(RwLock::new(HashSet::from([
            "codex/event/task_started".to_string(),
        ])));

        let mut connections = HashMap::new();
        connections.insert(
            connection_id,
            OutboundConnectionState::new(
                writer_tx,
                initialized,
                opted_out_notification_methods,
                None,
            ),
        );

        route_outgoing_envelope(
            &mut connections,
            OutgoingEnvelope::ToConnection {
                connection_id,
                message: OutgoingMessage::Notification(
                    crate::outgoing_message::OutgoingNotification {
                        method: "codex/event/task_started".to_string(),
                        params: None,
                    },
                ),
            },
        )
        .await;

        assert!(
            writer_rx.try_recv().is_err(),
            "opted-out notification should be dropped"
        );
    }

    #[tokio::test]
    async fn broadcast_does_not_block_on_slow_connection() {
        let fast_connection_id = ConnectionId(1);
        let slow_connection_id = ConnectionId(2);

        let (fast_writer_tx, mut fast_writer_rx) = mpsc::channel(1);
        let (slow_writer_tx, mut slow_writer_rx) = mpsc::channel(1);
        let fast_disconnect_token = CancellationToken::new();
        let slow_disconnect_token = CancellationToken::new();

        let mut connections = HashMap::new();
        connections.insert(
            fast_connection_id,
            OutboundConnectionState::new(
                fast_writer_tx,
                Arc::new(AtomicBool::new(true)),
                Arc::new(RwLock::new(HashSet::new())),
                Some(fast_disconnect_token.clone()),
            ),
        );
        connections.insert(
            slow_connection_id,
            OutboundConnectionState::new(
                slow_writer_tx.clone(),
                Arc::new(AtomicBool::new(true)),
                Arc::new(RwLock::new(HashSet::new())),
                Some(slow_disconnect_token.clone()),
            ),
        );

        let queued_message =
            OutgoingMessage::Notification(crate::outgoing_message::OutgoingNotification {
                method: "codex/event/already-buffered".to_string(),
                params: None,
            });
        slow_writer_tx
            .try_send(queued_message)
            .expect("channel should have room");

        let broadcast_message =
            OutgoingMessage::Notification(crate::outgoing_message::OutgoingNotification {
                method: "codex/event/test".to_string(),
                params: None,
            });
        timeout(
            Duration::from_millis(100),
            route_outgoing_envelope(
                &mut connections,
                OutgoingEnvelope::Broadcast {
                    message: broadcast_message,
                },
            ),
        )
        .await
        .expect("broadcast should not block on a full writer");
        assert!(!connections.contains_key(&slow_connection_id));
        assert!(slow_disconnect_token.is_cancelled());
        assert!(!fast_disconnect_token.is_cancelled());
        let fast_message = fast_writer_rx
            .try_recv()
            .expect("fast connection should receive broadcast");
        assert!(matches!(
            fast_message,
            OutgoingMessage::Notification(crate::outgoing_message::OutgoingNotification {
                method,
                params: None,
            }) if method == "codex/event/test"
        ));

        let slow_message = slow_writer_rx
            .try_recv()
            .expect("slow connection should retain its original buffered message");
        assert!(matches!(
            slow_message,
            OutgoingMessage::Notification(crate::outgoing_message::OutgoingNotification {
                method,
                params: None,
            }) if method == "codex/event/already-buffered"
        ));
    }

    #[tokio::test]
    async fn to_connection_stdio_waits_instead_of_disconnecting_when_writer_queue_is_full() {
        let connection_id = ConnectionId(3);
        let (writer_tx, mut writer_rx) = mpsc::channel(1);
        writer_tx
            .send(OutgoingMessage::Notification(
                crate::outgoing_message::OutgoingNotification {
                    method: "queued".to_string(),
                    params: None,
                },
            ))
            .await
            .expect("channel should accept the first queued message");

        let mut connections = HashMap::new();
        connections.insert(
            connection_id,
            OutboundConnectionState::new(
                writer_tx,
                Arc::new(AtomicBool::new(true)),
                Arc::new(RwLock::new(HashSet::new())),
                None,
            ),
        );

        let route_task = tokio::spawn(async move {
            route_outgoing_envelope(
                &mut connections,
                OutgoingEnvelope::ToConnection {
                    connection_id,
                    message: OutgoingMessage::Notification(
                        crate::outgoing_message::OutgoingNotification {
                            method: "second".to_string(),
                            params: None,
                        },
                    ),
                },
            )
            .await
        });

        let first = timeout(Duration::from_millis(100), writer_rx.recv())
            .await
            .expect("first queued message should be readable")
            .expect("first queued message should exist");
        let second = timeout(Duration::from_millis(100), writer_rx.recv())
            .await
            .expect("second message should eventually be delivered")
            .expect("second message should exist");

        timeout(Duration::from_millis(100), route_task)
            .await
            .expect("routing should finish after writer drains")
            .expect("routing task should succeed");

        assert!(matches!(
            first,
            OutgoingMessage::Notification(crate::outgoing_message::OutgoingNotification {
                method,
                params: None,
            }) if method == "queued"
        ));
        assert!(matches!(
            second,
            OutgoingMessage::Notification(crate::outgoing_message::OutgoingNotification {
                method,
                params: None,
            }) if method == "second"
        ));
    }
}
