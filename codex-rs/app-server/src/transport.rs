use crate::error_code::OVERLOADED_ERROR_CODE;
use crate::message_processor::ConnectionSessionState;
use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::OutgoingEnvelope;
use crate::outgoing_message::OutgoingError;
use crate::outgoing_message::OutgoingMessage;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::ServerRequest;
use codex_core::AuthManager;
use codex_core::default_client::build_reqwest_client;
use codex_utils_rustls_provider::ensure_rustls_crypto_provider;
use futures::SinkExt;
use futures::StreamExt;
use owo_colors::OwoColorize;
use owo_colors::Stream;
use owo_colors::Style;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::collections::HashSet;
use std::io::ErrorKind;
use std::io::Result as IoResult;
use std::net::SocketAddr;
use std::path::Path;
use std::path::PathBuf;
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
use tokio::time::Duration;
use tokio::time::Instant;
use tokio::time::MissedTickBehavior;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::accept_async_with_config;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message as WebSocketMessage;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::{self};
use tokio_util::sync::CancellationToken;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::warn;

/// Size of the bounded channels used to communicate between tasks. The value
/// is a balance between throughput and memory usage - 128 messages should be
/// plenty for an interactive CLI.
pub(crate) const CHANNEL_CAPACITY: usize = 128;
const REMOTE_CONTROL_CLIENT_IDLE_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const REMOTE_CONTROL_IDLE_SWEEP_INTERVAL: Duration = Duration::from_secs(30);
const REMOTE_CONTROL_ENROLL_TIMEOUT: Duration = Duration::from_secs(30);
const REMOTE_CONTROL_RECONNECT_INITIAL_BACKOFF: Duration = Duration::from_secs(1);
const REMOTE_CONTROL_RECONNECT_MAX_BACKOFF: Duration = Duration::from_secs(30);
const REMOTE_CONTROL_PROTOCOL_VERSION: &str = "2";
const REMOTE_CONTROL_SERVER_NAME: &str = "codex-app-server";
const REMOTE_CONTROL_ACCOUNT_ID_HEADER: &str = "chatgpt-account-id";
const REMOTE_CONTROL_STATE_FILE: &str = "remote_control.toml";

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppServerTransport {
    Stdio,
    WebSocket { bind_address: SocketAddr },
    Headless,
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
    pub(crate) outbound_experimental_api_enabled: Arc<AtomicBool>,
    pub(crate) outbound_opted_out_notification_methods: Arc<RwLock<HashSet<String>>>,
    pub(crate) session: ConnectionSessionState,
}

impl ConnectionState {
    pub(crate) fn new(
        outbound_initialized: Arc<AtomicBool>,
        outbound_experimental_api_enabled: Arc<AtomicBool>,
        outbound_opted_out_notification_methods: Arc<RwLock<HashSet<String>>>,
    ) -> Self {
        Self {
            outbound_initialized,
            outbound_experimental_api_enabled,
            outbound_opted_out_notification_methods,
            session: ConnectionSessionState::default(),
        }
    }
}

pub(crate) struct OutboundConnectionState {
    pub(crate) initialized: Arc<AtomicBool>,
    pub(crate) experimental_api_enabled: Arc<AtomicBool>,
    pub(crate) opted_out_notification_methods: Arc<RwLock<HashSet<String>>>,
    pub(crate) writer: mpsc::Sender<OutgoingMessage>,
    disconnect_sender: Option<CancellationToken>,
}

impl OutboundConnectionState {
    pub(crate) fn new(
        writer: mpsc::Sender<OutgoingMessage>,
        initialized: Arc<AtomicBool>,
        experimental_api_enabled: Arc<AtomicBool>,
        opted_out_notification_methods: Arc<RwLock<HashSet<String>>>,
        disconnect_sender: Option<CancellationToken>,
    ) -> Self {
        Self {
            initialized,
            experimental_api_enabled,
            opted_out_notification_methods,
            writer,
            disconnect_sender,
        }
    }

    fn can_disconnect(&self) -> bool {
        self.disconnect_sender.is_some()
    }

    pub(crate) fn request_disconnect(&self) {
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

static CONNECTION_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

fn next_connection_id() -> ConnectionId {
    ConnectionId(CONNECTION_ID_COUNTER.fetch_add(1, Ordering::Relaxed))
}

pub(crate) async fn start_websocket_acceptor(
    bind_address: SocketAddr,
    transport_event_tx: mpsc::Sender<TransportEvent>,
    shutdown_token: CancellationToken,
) -> IoResult<JoinHandle<()>> {
    let listener = TcpListener::bind(bind_address).await?;
    let local_addr = listener.local_addr()?;
    print_websocket_startup_banner(local_addr);
    info!("app-server websocket listening on ws://{local_addr}");

    Ok(tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = shutdown_token.cancelled() => {
                    info!("websocket acceptor shutting down");
                    break;
                }
                accept_result = listener.accept() => {
                    match accept_result {
                        Ok((stream, peer_addr)) => {
                            info!(%peer_addr, "websocket client connected");
                            let connection_id = next_connection_id();
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
            }
        }
    }))
}

async fn run_websocket_connection(
    connection_id: ConnectionId,
    stream: TcpStream,
    transport_event_tx: mpsc::Sender<TransportEvent>,
) {
    let websocket_stream =
        match accept_async_with_config(stream, Some(WebSocketConfig::default())).await {
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
    let message = filter_outgoing_message_for_connection(connection_state, message);
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

fn filter_outgoing_message_for_connection(
    connection_state: &OutboundConnectionState,
    message: OutgoingMessage,
) -> OutgoingMessage {
    let experimental_api_enabled = connection_state
        .experimental_api_enabled
        .load(Ordering::Acquire);
    match message {
        OutgoingMessage::Request(ServerRequest::CommandExecutionRequestApproval {
            request_id,
            mut params,
        }) => {
            if !experimental_api_enabled {
                params.strip_experimental_fields();
            }
            OutgoingMessage::Request(ServerRequest::CommandExecutionRequestApproval {
                request_id,
                params,
            })
        }
        _ => message,
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ClientId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientActivityState {
    Foreground,
    Background,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientEvent {
    ClientMessage {
        #[serde(rename = "client_id", alias = "clientId")]
        client_id: ClientId,
        message: JSONRPCMessage,
    },
    Ping {
        #[serde(rename = "client_id", alias = "clientId")]
        client_id: ClientId,
        #[serde(skip_serializing_if = "Option::is_none")]
        state: Option<ClientActivityState>,
    },
    ClientClosed {
        #[serde(rename = "client_id", alias = "clientId")]
        client_id: ClientId,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerEvent {
    ServerMessage {
        #[serde(rename = "client_id")]
        client_id: ClientId,
        message: Box<OutgoingMessage>,
    },
    Pong {
        #[serde(rename = "client_id")]
        client_id: ClientId,
        status: PongStatus,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PongStatus {
    Active,
    Unknown,
}

struct RemoteControlClientState {
    connection_id: ConnectionId,
    disconnect_token: CancellationToken,
    last_activity_at: Instant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RemoteControlTarget {
    websocket_url: String,
    enroll_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RemoteControlEnrollment {
    server_id: String,
    server_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RemoteControlConnectionAuth {
    bearer_token: String,
    account_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
struct RemoteControlStateToml {
    #[serde(default)]
    enrollments: Vec<PersistedRemoteControlEnrollment>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct PersistedRemoteControlEnrollment {
    websocket_url: String,
    account_id: Option<String>,
    server_id: String,
    server_name: String,
}

#[derive(Debug, Serialize)]
struct EnrollRemoteServerRequest<'a> {
    name: &'a str,
    os: &'a str,
    arch: &'a str,
    app_server_version: &'a str,
}

#[derive(Debug, Deserialize)]
struct EnrollRemoteServerResponse {
    server_id: String,
}

fn remote_control_state_path(codex_home: &Path) -> PathBuf {
    codex_home.join(REMOTE_CONTROL_STATE_FILE)
}

fn matches_persisted_remote_control_enrollment(
    entry: &PersistedRemoteControlEnrollment,
    remote_control_target: &RemoteControlTarget,
    account_id: Option<&str>,
) -> bool {
    entry.websocket_url == remote_control_target.websocket_url
        && entry.account_id.as_deref() == account_id
}

async fn load_remote_control_state(state_path: &Path) -> IoResult<RemoteControlStateToml> {
    let contents = match tokio::fs::read_to_string(state_path).await {
        Ok(contents) => contents,
        Err(err) if err.kind() == ErrorKind::NotFound => {
            return Ok(RemoteControlStateToml::default());
        }
        Err(err) => return Err(err),
    };

    toml::from_str(&contents).map_err(|err| {
        std::io::Error::new(
            ErrorKind::InvalidData,
            format!(
                "failed to parse remote control state `{}`: {err}",
                state_path.display()
            ),
        )
    })
}

async fn write_remote_control_state(
    state_path: &Path,
    state: &RemoteControlStateToml,
) -> IoResult<()> {
    if let Some(parent) = state_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let serialized = toml::to_string(state).map_err(std::io::Error::other)?;
    tokio::fs::write(state_path, serialized).await
}

async fn load_persisted_remote_control_enrollment(
    state_path: &Path,
    remote_control_target: &RemoteControlTarget,
    account_id: Option<&str>,
) -> Option<RemoteControlEnrollment> {
    let state = match load_remote_control_state(state_path).await {
        Ok(state) => state,
        Err(err) => {
            warn!("{err}");
            return None;
        }
    };

    state
        .enrollments
        .into_iter()
        .find(|entry| {
            matches_persisted_remote_control_enrollment(entry, remote_control_target, account_id)
        })
        .map(|entry| RemoteControlEnrollment {
            server_id: entry.server_id,
            server_name: entry.server_name,
        })
}

async fn update_persisted_remote_control_enrollment(
    state_path: &Path,
    remote_control_target: &RemoteControlTarget,
    account_id: Option<&str>,
    enrollment: Option<&RemoteControlEnrollment>,
) -> IoResult<()> {
    let mut state = match load_remote_control_state(state_path).await {
        Ok(state) => state,
        Err(err) if err.kind() == ErrorKind::InvalidData => {
            warn!("{err}");
            RemoteControlStateToml::default()
        }
        Err(err) => return Err(err),
    };

    state.enrollments.retain(|entry| {
        !matches_persisted_remote_control_enrollment(entry, remote_control_target, account_id)
    });

    if let Some(enrollment) = enrollment {
        state.enrollments.push(PersistedRemoteControlEnrollment {
            websocket_url: remote_control_target.websocket_url.clone(),
            account_id: account_id.map(str::to_owned),
            server_id: enrollment.server_id.clone(),
            server_name: enrollment.server_name.clone(),
        });
    }

    if state.enrollments.is_empty() {
        match tokio::fs::remove_file(state_path).await {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err),
        }
    } else {
        write_remote_control_state(state_path, &state).await
    }
}

pub(crate) async fn start_remote_control(
    remote_control_url: String,
    codex_home: PathBuf,
    auth_manager: Arc<AuthManager>,
    transport_event_tx: mpsc::Sender<TransportEvent>,
    shutdown_token: CancellationToken,
) -> IoResult<JoinHandle<()>> {
    let remote_control_url = normalize_remote_control_url(&remote_control_url)?;
    let remote_control_state_path = remote_control_state_path(&codex_home);
    Ok(tokio::spawn(async move {
        let local_shutdown_token = shutdown_token.child_token();
        let (client_event_tx, client_event_rx) = mpsc::channel(CHANNEL_CAPACITY);
        let (server_event_tx, server_event_rx) = mpsc::channel(CHANNEL_CAPACITY);
        let (writer_exited_tx, writer_exited_rx) = mpsc::channel(CHANNEL_CAPACITY);

        let mut websocket_task = tokio::spawn(run_remote_control_websocket_loop(
            remote_control_url,
            remote_control_state_path,
            auth_manager,
            client_event_tx,
            server_event_rx,
            local_shutdown_token.clone(),
        ));
        let mut manager_task = tokio::spawn(run_remote_control_manager(
            transport_event_tx,
            client_event_rx,
            server_event_tx,
            writer_exited_tx,
            writer_exited_rx,
            local_shutdown_token.clone(),
        ));

        tokio::select! {
            _ = local_shutdown_token.cancelled() => {}
            _ = &mut websocket_task => {
                local_shutdown_token.cancel();
            }
            _ = &mut manager_task => {
                local_shutdown_token.cancel();
            }
        }

        let _ = websocket_task.await;
        let _ = manager_task.await;
    }))
}

fn normalize_remote_control_url(remote_control_url: &str) -> IoResult<RemoteControlTarget> {
    let remote_control_url = remote_control_url.trim_end_matches('/');

    if remote_control_url.starts_with("ws://") || remote_control_url.starts_with("wss://") {
        return Ok(RemoteControlTarget {
            websocket_url: remote_control_url.to_string(),
            enroll_url: None,
        });
    }

    if let Some(rest) = remote_control_url.strip_prefix("http://") {
        return Ok(normalize_http_remote_control_url(rest, "http://", "ws://"));
    }
    if let Some(rest) = remote_control_url.strip_prefix("https://") {
        return Ok(normalize_http_remote_control_url(
            rest, "https://", "wss://",
        ));
    }

    Err(std::io::Error::new(
        ErrorKind::InvalidInput,
        format!(
            "invalid remote control URL `{remote_control_url}`; expected ws://, wss://, http://, or https://"
        ),
    ))
}

fn normalize_http_remote_control_url(
    rest: &str,
    http_scheme: &str,
    websocket_scheme: &str,
) -> RemoteControlTarget {
    let rest = if let Some(rest) = rest.strip_suffix("/remote/control/server/enroll") {
        format!("{rest}/remote/control/server")
    } else if rest.ends_with("/remote/control/server") {
        rest.to_string()
    } else if let Some(rest) = rest.strip_suffix("/server/enroll") {
        format!("{rest}/server")
    } else if rest.ends_with("/server") {
        rest.to_string()
    } else {
        format!("{rest}/remote/control/server")
    };

    RemoteControlTarget {
        websocket_url: format!("{websocket_scheme}{rest}"),
        enroll_url: Some(format!("{http_scheme}{rest}/enroll")),
    }
}

async fn run_remote_control_manager(
    transport_event_tx: mpsc::Sender<TransportEvent>,
    mut client_event_rx: mpsc::Receiver<ClientEvent>,
    server_event_tx: mpsc::Sender<ServerEvent>,
    writer_exited_tx: mpsc::Sender<ClientId>,
    mut writer_exited_rx: mpsc::Receiver<ClientId>,
    shutdown_token: CancellationToken,
) {
    let mut clients = HashMap::<ClientId, RemoteControlClientState>::new();
    let mut idle_sweep = tokio::time::interval(REMOTE_CONTROL_IDLE_SWEEP_INTERVAL);
    idle_sweep.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            _ = shutdown_token.cancelled() => {
                break;
            }
            _ = idle_sweep.tick() => {
                if !close_expired_remote_control_clients(&transport_event_tx, &mut clients).await {
                    break;
                }
            }
            writer_exited = writer_exited_rx.recv() => {
                let Some(client_id) = writer_exited else {
                    break;
                };
                if !close_remote_control_client(&transport_event_tx, &mut clients, &client_id).await {
                    break;
                }
            }
            client_event = client_event_rx.recv() => {
                let Some(client_event) = client_event else {
                    break;
                };
                match client_event {
                    ClientEvent::ClientMessage { client_id, message } => {
                        if let Some(connection_id) = clients.get_mut(&client_id).map(|client| {
                            client.last_activity_at = Instant::now();
                            client.connection_id
                        }) {
                            if transport_event_tx
                                .send(TransportEvent::IncomingMessage {
                                    connection_id,
                                    message,
                                })
                                .await
                                .is_err()
                            {
                                break;
                            }
                            continue;
                        }

                        if !remote_control_message_starts_connection(&message) {
                            continue;
                        }

                        let connection_id = next_connection_id();
                        let (writer_tx, writer_rx) = mpsc::channel::<OutgoingMessage>(CHANNEL_CAPACITY);
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
                            break;
                        }

                        tokio::spawn(run_remote_control_client_outbound(
                            client_id.clone(),
                            writer_rx,
                            server_event_tx.clone(),
                            writer_exited_tx.clone(),
                            disconnect_token.clone(),
                        ));
                        clients.insert(
                            client_id,
                            RemoteControlClientState {
                                connection_id,
                                disconnect_token,
                                last_activity_at: Instant::now(),
                            },
                        );
                        if transport_event_tx
                            .send(TransportEvent::IncomingMessage {
                                connection_id,
                                message,
                            })
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    ClientEvent::Ping { client_id, .. } => {
                        let status = match clients.get_mut(&client_id) {
                            Some(client) => {
                                client.last_activity_at = Instant::now();
                                PongStatus::Active
                            }
                            None => PongStatus::Unknown,
                        };

                        if server_event_tx
                            .send(ServerEvent::Pong { client_id, status })
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    ClientEvent::ClientClosed { client_id } => {
                        if !close_remote_control_client(&transport_event_tx, &mut clients, &client_id).await {
                            break;
                        }
                    }
                }
            }
        }
    }

    while let Some(client_id) = clients.keys().next().cloned() {
        if !close_remote_control_client(&transport_event_tx, &mut clients, &client_id).await {
            break;
        }
    }
}

fn remote_control_message_starts_connection(message: &JSONRPCMessage) -> bool {
    matches!(
        message,
        JSONRPCMessage::Request(codex_app_server_protocol::JSONRPCRequest { method, .. })
            if method == "initialize"
    )
}

fn remote_control_client_is_alive(client: &RemoteControlClientState, now: Instant) -> bool {
    now.duration_since(client.last_activity_at) < REMOTE_CONTROL_CLIENT_IDLE_TIMEOUT
}

async fn close_expired_remote_control_clients(
    transport_event_tx: &mpsc::Sender<TransportEvent>,
    clients: &mut HashMap<ClientId, RemoteControlClientState>,
) -> bool {
    let now = Instant::now();
    let expired_client_ids: Vec<ClientId> = clients
        .iter()
        .filter_map(|(client_id, client)| {
            (!remote_control_client_is_alive(client, now)).then_some(client_id.clone())
        })
        .collect();
    for client_id in expired_client_ids {
        if !close_remote_control_client(transport_event_tx, clients, &client_id).await {
            return false;
        }
    }
    true
}

async fn close_remote_control_client(
    transport_event_tx: &mpsc::Sender<TransportEvent>,
    clients: &mut HashMap<ClientId, RemoteControlClientState>,
    client_id: &ClientId,
) -> bool {
    let Some(client) = clients.remove(client_id) else {
        return true;
    };
    client.disconnect_token.cancel();
    transport_event_tx
        .send(TransportEvent::ConnectionClosed {
            connection_id: client.connection_id,
        })
        .await
        .is_ok()
}

async fn run_remote_control_client_outbound(
    client_id: ClientId,
    mut writer_rx: mpsc::Receiver<OutgoingMessage>,
    server_event_tx: mpsc::Sender<ServerEvent>,
    writer_exited_tx: mpsc::Sender<ClientId>,
    disconnect_token: CancellationToken,
) {
    loop {
        tokio::select! {
            _ = disconnect_token.cancelled() => {
                break;
            }
            outgoing_message = writer_rx.recv() => {
                let Some(outgoing_message) = outgoing_message else {
                    break;
                };
                if server_event_tx
                    .send(ServerEvent::ServerMessage {
                        client_id: client_id.clone(),
                        message: Box::new(outgoing_message),
                    })
                    .await
                    .is_err()
                {
                    break;
                }
            }
        }
    }

    let _ = writer_exited_tx.send(client_id).await;
}

async fn load_remote_control_auth(
    auth_manager: &AuthManager,
) -> IoResult<RemoteControlConnectionAuth> {
    let auth = match auth_manager.auth().await {
        Some(auth) => auth,
        None => {
            auth_manager.reload();
            auth_manager.auth().await.ok_or_else(|| {
                std::io::Error::new(
                    ErrorKind::PermissionDenied,
                    "remote control requires ChatGPT authentication",
                )
            })?
        }
    };

    if !auth.is_chatgpt_auth() {
        return Err(std::io::Error::new(
            ErrorKind::PermissionDenied,
            "remote control requires ChatGPT authentication; API key auth is not supported",
        ));
    }

    Ok(RemoteControlConnectionAuth {
        bearer_token: auth.get_token().map_err(std::io::Error::other)?,
        account_id: auth.get_account_id(),
    })
}

pub(crate) async fn validate_remote_control_auth(auth_manager: &AuthManager) -> IoResult<()> {
    load_remote_control_auth(auth_manager).await.map(|_| ())
}

async fn enroll_remote_control_server(
    remote_control_target: &RemoteControlTarget,
    auth: &RemoteControlConnectionAuth,
) -> IoResult<RemoteControlEnrollment> {
    let Some(enroll_url) = remote_control_target.enroll_url.as_deref() else {
        return Err(std::io::Error::other(
            "remote control enrollment requires an HTTP(S) URL",
        ));
    };

    let request = EnrollRemoteServerRequest {
        name: REMOTE_CONTROL_SERVER_NAME,
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        app_server_version: env!("CARGO_PKG_VERSION"),
    };
    let client = build_reqwest_client();
    let mut http_request = client
        .post(enroll_url)
        .timeout(REMOTE_CONTROL_ENROLL_TIMEOUT)
        .bearer_auth(&auth.bearer_token)
        .json(&request);
    if let Some(account_id) = auth.account_id.as_deref() {
        http_request = http_request.header(REMOTE_CONTROL_ACCOUNT_ID_HEADER, account_id);
    }

    let response = http_request.send().await.map_err(|err| {
        std::io::Error::other(format!(
            "failed to enroll remote control server at `{enroll_url}`: {err}"
        ))
    })?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(std::io::Error::other(format!(
            "remote control server enrollment failed at `{enroll_url}`: HTTP {status} {body}"
        )));
    }

    let enrollment = response
        .json::<EnrollRemoteServerResponse>()
        .await
        .map_err(|err| {
            std::io::Error::other(format!(
                "failed to parse remote control enrollment response from `{enroll_url}`: {err}"
            ))
        })?;

    Ok(RemoteControlEnrollment {
        server_id: enrollment.server_id,
        server_name: REMOTE_CONTROL_SERVER_NAME.to_string(),
    })
}

fn set_remote_control_header(
    headers: &mut tungstenite::http::HeaderMap,
    name: &'static str,
    value: &str,
) -> IoResult<()> {
    let header_value = HeaderValue::from_str(value).map_err(|err| {
        std::io::Error::new(
            ErrorKind::InvalidInput,
            format!("invalid remote control header `{name}`: {err}"),
        )
    })?;
    headers.insert(name, header_value);
    Ok(())
}

fn build_remote_control_websocket_request(
    websocket_url: &str,
    enrollment: &RemoteControlEnrollment,
    auth: &RemoteControlConnectionAuth,
) -> IoResult<tungstenite::http::Request<()>> {
    let mut request = websocket_url.into_client_request().map_err(|err| {
        std::io::Error::new(
            ErrorKind::InvalidInput,
            format!("invalid remote control websocket URL `{websocket_url}`: {err}"),
        )
    })?;
    let headers = request.headers_mut();
    set_remote_control_header(headers, "x-codex-server-id", &enrollment.server_id)?;
    set_remote_control_header(headers, "x-codex-name", &enrollment.server_name)?;
    set_remote_control_header(
        headers,
        "x-codex-protocol-version",
        REMOTE_CONTROL_PROTOCOL_VERSION,
    )?;
    set_remote_control_header(
        headers,
        "authorization",
        &format!("Bearer {}", auth.bearer_token),
    )?;
    if let Some(account_id) = auth.account_id.as_deref() {
        set_remote_control_header(headers, REMOTE_CONTROL_ACCOUNT_ID_HEADER, account_id)?;
    }
    Ok(request)
}

async fn connect_remote_control_websocket(
    remote_control_target: &RemoteControlTarget,
    remote_control_state_path: &Path,
    auth_manager: &AuthManager,
    enrollment: &mut Option<RemoteControlEnrollment>,
) -> IoResult<WebSocketStream<MaybeTlsStream<TcpStream>>> {
    ensure_rustls_crypto_provider();

    if remote_control_target.enroll_url.is_none() {
        return connect_async(remote_control_target.websocket_url.as_str())
            .await
            .map(|(websocket_stream, _response)| websocket_stream)
            .map_err(|err| {
                std::io::Error::other(format!(
                    "failed to connect app-server remote control websocket `{}`: {err}",
                    remote_control_target.websocket_url
                ))
            });
    }

    let auth = load_remote_control_auth(auth_manager).await?;
    if enrollment.is_none() {
        *enrollment = load_persisted_remote_control_enrollment(
            remote_control_state_path,
            remote_control_target,
            auth.account_id.as_deref(),
        )
        .await;
    }

    if enrollment.is_none() {
        let new_enrollment = enroll_remote_control_server(remote_control_target, &auth).await?;
        if let Err(err) = update_persisted_remote_control_enrollment(
            remote_control_state_path,
            remote_control_target,
            auth.account_id.as_deref(),
            Some(&new_enrollment),
        )
        .await
        {
            warn!(
                "failed to persist remote control enrollment in `{}`: {err}",
                remote_control_state_path.display()
            );
        }
        *enrollment = Some(new_enrollment);
    }

    let enrollment_ref = match enrollment.as_ref() {
        Some(enrollment) => enrollment,
        None => {
            return Err(std::io::Error::other(
                "missing remote control enrollment after enrollment step",
            ));
        }
    };
    let request = build_remote_control_websocket_request(
        &remote_control_target.websocket_url,
        enrollment_ref,
        &auth,
    )?;

    match connect_async(request).await {
        Ok((websocket_stream, _response)) => Ok(websocket_stream),
        Err(err) => {
            if matches!(
                &err,
                tungstenite::Error::Http(response) if response.status().as_u16() == 404
            ) {
                if let Err(clear_err) = update_persisted_remote_control_enrollment(
                    remote_control_state_path,
                    remote_control_target,
                    auth.account_id.as_deref(),
                    None,
                )
                .await
                {
                    warn!(
                        "failed to clear stale remote control enrollment in `{}`: {clear_err}",
                        remote_control_state_path.display()
                    );
                }
                *enrollment = None;
            }
            Err(std::io::Error::other(format!(
                "failed to connect app-server remote control websocket `{}`: {err}",
                remote_control_target.websocket_url
            )))
        }
    }
}

async fn run_remote_control_websocket_loop(
    remote_control_target: RemoteControlTarget,
    remote_control_state_path: PathBuf,
    auth_manager: Arc<AuthManager>,
    client_event_tx: mpsc::Sender<ClientEvent>,
    mut server_event_rx: mpsc::Receiver<ServerEvent>,
    shutdown_token: CancellationToken,
) {
    let mut reconnect_backoff = REMOTE_CONTROL_RECONNECT_INITIAL_BACKOFF;
    let mut wait_before_connect = false;
    let mut enrollment = None::<RemoteControlEnrollment>;
    let mut pending_server_event = None::<ServerEvent>;

    loop {
        let slept_before_connect = if wait_before_connect {
            tokio::select! {
                _ = shutdown_token.cancelled() => {
                    break;
                }
                _ = tokio::time::sleep(reconnect_backoff) => {}
            }
            true
        } else {
            wait_before_connect = true;
            false
        };

        let websocket_stream = tokio::select! {
            _ = shutdown_token.cancelled() => {
                break;
            }
            connect_result = connect_remote_control_websocket(
                &remote_control_target,
                remote_control_state_path.as_path(),
                auth_manager.as_ref(),
                &mut enrollment,
            ) => {
                match connect_result {
                    Ok(websocket_stream) => {
                        reconnect_backoff = REMOTE_CONTROL_RECONNECT_INITIAL_BACKOFF;
                        info!(
                            "connected to app-server remote control websocket: {}",
                            remote_control_target.websocket_url
                        );
                        websocket_stream
                    }
                    Err(err) => {
                        warn!("{err}");
                        if slept_before_connect {
                            reconnect_backoff = reconnect_backoff
                                .saturating_mul(2)
                                .min(REMOTE_CONTROL_RECONNECT_MAX_BACKOFF);
                        }
                        continue;
                    }
                }
            }
        };

        let (mut websocket_writer, mut websocket_reader) = websocket_stream.split();
        loop {
            if let Some(server_event) = pending_server_event.take() {
                let payload = match serde_json::to_string(&server_event) {
                    Ok(payload) => payload,
                    Err(err) => {
                        error!("failed to serialize remote-control server event: {err}");
                        continue;
                    }
                };
                if let Err(err) = websocket_writer
                    .send(WebSocketMessage::Text(payload.into()))
                    .await
                {
                    warn!("remote control websocket send failed: {err}");
                    pending_server_event = Some(server_event);
                    break;
                }
                continue;
            }

            tokio::select! {
                _ = shutdown_token.cancelled() => {
                    return;
                }
                incoming_message = websocket_reader.next() => {
                    match incoming_message {
                        Some(Ok(WebSocketMessage::Text(text))) => {
                            match serde_json::from_str::<ClientEvent>(&text) {
                                Ok(client_event) => {
                                    if client_event_tx.send(client_event).await.is_err() {
                                        return;
                                    }
                                }
                                Err(_) => {
                                warn!("failed to deserialize remote-control client event");
                                }
                            }
                        }
                        Some(Ok(WebSocketMessage::Ping(payload))) => {
                            if let Err(err) = websocket_writer.send(WebSocketMessage::Pong(payload)).await {
                                warn!("remote control websocket pong failed: {err}");
                                break;
                            }
                        }
                        Some(Ok(WebSocketMessage::Pong(_))) => {}
                        Some(Ok(WebSocketMessage::Binary(_))) => {
                            warn!("dropping unsupported binary remote-control websocket message");
                        }
                        Some(Ok(WebSocketMessage::Frame(_))) => {}
                        Some(Ok(WebSocketMessage::Close(_))) | None => {
                            warn!("remote control websocket disconnected");
                            break;
                        }
                        Some(Err(err)) => {
                            warn!("remote control websocket receive error: {err}");
                            break;
                        }
                    }
                }
                server_event = server_event_rx.recv() => {
                    let Some(server_event) = server_event else {
                        return;
                    };
                    pending_server_event = Some(server_event);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error_code::OVERLOADED_ERROR_CODE;
    use codex_core::CodexAuth;
    use codex_core::test_support::auth_manager_from_auth;
    use codex_core::test_support::auth_manager_from_auth_with_home;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use tokio::io::AsyncBufReadExt;
    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncWriteExt;
    use tokio::io::BufReader;
    use tokio::time::timeout;
    use tokio_tungstenite::WebSocketStream;
    use tokio_tungstenite::accept_async;
    use tokio_tungstenite::accept_hdr_async;

    fn absolute_path(path: &str) -> AbsolutePathBuf {
        AbsolutePathBuf::from_absolute_path(path).expect("absolute path")
    }

    fn remote_control_auth_manager() -> Arc<AuthManager> {
        auth_manager_from_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
    }

    fn remote_control_auth_manager_with_home(codex_home: &TempDir) -> Arc<AuthManager> {
        auth_manager_from_auth_with_home(
            CodexAuth::create_dummy_chatgpt_auth_for_testing(),
            codex_home.path().to_path_buf(),
        )
    }

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
    async fn validate_remote_control_auth_rejects_api_key_auth() {
        let auth_manager = auth_manager_from_auth(CodexAuth::from_api_key("sk-test"));

        let err = validate_remote_control_auth(auth_manager.as_ref())
            .await
            .expect_err("API key auth should be rejected");

        assert_eq!(
            err.to_string(),
            "remote control requires ChatGPT authentication; API key auth is not supported"
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
                Arc::new(AtomicBool::new(true)),
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
    async fn command_execution_request_approval_strips_experimental_fields_without_capability() {
        let connection_id = ConnectionId(8);
        let (writer_tx, mut writer_rx) = mpsc::channel(1);

        let mut connections = HashMap::new();
        connections.insert(
            connection_id,
            OutboundConnectionState::new(
                writer_tx,
                Arc::new(AtomicBool::new(true)),
                Arc::new(AtomicBool::new(false)),
                Arc::new(RwLock::new(HashSet::new())),
                None,
            ),
        );

        route_outgoing_envelope(
            &mut connections,
            OutgoingEnvelope::ToConnection {
                connection_id,
                message: OutgoingMessage::Request(ServerRequest::CommandExecutionRequestApproval {
                    request_id: codex_app_server_protocol::RequestId::Integer(1),
                    params: codex_app_server_protocol::CommandExecutionRequestApprovalParams {
                        thread_id: "thr_123".to_string(),
                        turn_id: "turn_123".to_string(),
                        item_id: "call_123".to_string(),
                        approval_id: None,
                        reason: Some("Need extra read access".to_string()),
                        network_approval_context: None,
                        command: Some("cat file".to_string()),
                        cwd: Some(PathBuf::from("/tmp")),
                        command_actions: None,
                        additional_permissions: Some(
                            codex_app_server_protocol::AdditionalPermissionProfile {
                                network: None,
                                file_system: Some(
                                    codex_app_server_protocol::AdditionalFileSystemPermissions {
                                        read: Some(vec![absolute_path("/tmp/allowed")]),
                                        write: None,
                                    },
                                ),
                                macos: None,
                            },
                        ),
                        proposed_execpolicy_amendment: None,
                        proposed_network_policy_amendments: None,
                        available_decisions: None,
                    },
                }),
            },
        )
        .await;

        let message = writer_rx
            .recv()
            .await
            .expect("request should be delivered to the connection");
        let json = serde_json::to_value(message).expect("request should serialize");
        assert_eq!(json["params"].get("additionalPermissions"), None);
    }

    #[tokio::test]
    async fn command_execution_request_approval_keeps_experimental_fields_with_capability() {
        let connection_id = ConnectionId(9);
        let (writer_tx, mut writer_rx) = mpsc::channel(1);

        let mut connections = HashMap::new();
        connections.insert(
            connection_id,
            OutboundConnectionState::new(
                writer_tx,
                Arc::new(AtomicBool::new(true)),
                Arc::new(AtomicBool::new(true)),
                Arc::new(RwLock::new(HashSet::new())),
                None,
            ),
        );

        route_outgoing_envelope(
            &mut connections,
            OutgoingEnvelope::ToConnection {
                connection_id,
                message: OutgoingMessage::Request(ServerRequest::CommandExecutionRequestApproval {
                    request_id: codex_app_server_protocol::RequestId::Integer(1),
                    params: codex_app_server_protocol::CommandExecutionRequestApprovalParams {
                        thread_id: "thr_123".to_string(),
                        turn_id: "turn_123".to_string(),
                        item_id: "call_123".to_string(),
                        approval_id: None,
                        reason: Some("Need extra read access".to_string()),
                        network_approval_context: None,
                        command: Some("cat file".to_string()),
                        cwd: Some(PathBuf::from("/tmp")),
                        command_actions: None,
                        additional_permissions: Some(
                            codex_app_server_protocol::AdditionalPermissionProfile {
                                network: None,
                                file_system: Some(
                                    codex_app_server_protocol::AdditionalFileSystemPermissions {
                                        read: Some(vec![absolute_path("/tmp/allowed")]),
                                        write: None,
                                    },
                                ),
                                macos: None,
                            },
                        ),
                        proposed_execpolicy_amendment: None,
                        proposed_network_policy_amendments: None,
                        available_decisions: None,
                    },
                }),
            },
        )
        .await;

        let message = writer_rx
            .recv()
            .await
            .expect("request should be delivered to the connection");
        let json = serde_json::to_value(message).expect("request should serialize");
        let allowed_path = absolute_path("/tmp/allowed").to_string_lossy().into_owned();
        assert_eq!(
            json["params"]["additionalPermissions"],
            json!({
                "network": null,
                "fileSystem": {
                    "read": [allowed_path],
                    "write": null,
                },
                "macos": null,
            })
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

    #[test]
    fn normalize_remote_control_url_rewrites_http_schemes() {
        assert_eq!(
            normalize_remote_control_url("ws://example.com/control").expect("valid ws url"),
            RemoteControlTarget {
                websocket_url: "ws://example.com/control".to_string(),
                enroll_url: None,
            }
        );
        assert_eq!(
            normalize_remote_control_url("wss://example.com/control").expect("valid wss url"),
            RemoteControlTarget {
                websocket_url: "wss://example.com/control".to_string(),
                enroll_url: None,
            }
        );
        assert_eq!(
            normalize_remote_control_url("http://example.com/backend-api/wham")
                .expect("valid http prefix"),
            RemoteControlTarget {
                websocket_url: "ws://example.com/backend-api/wham/remote/control/server"
                    .to_string(),
                enroll_url: Some(
                    "http://example.com/backend-api/wham/remote/control/server/enroll".to_string(),
                ),
            }
        );
        assert_eq!(
            normalize_remote_control_url(
                "https://example.com/backend-api/wham/remote/control/server"
            )
            .expect("valid https full path"),
            RemoteControlTarget {
                websocket_url: "wss://example.com/backend-api/wham/remote/control/server"
                    .to_string(),
                enroll_url: Some(
                    "https://example.com/backend-api/wham/remote/control/server/enroll".to_string(),
                ),
            }
        );
        assert_eq!(
            normalize_remote_control_url("http://example.com/legacy/server")
                .expect("valid legacy http url"),
            RemoteControlTarget {
                websocket_url: "ws://example.com/legacy/server".to_string(),
                enroll_url: Some("http://example.com/legacy/server/enroll".to_string()),
            }
        );
    }

    #[test]
    fn normalize_remote_control_url_rejects_unsupported_schemes() {
        let err = normalize_remote_control_url("ftp://example.com/control")
            .expect_err("unsupported scheme should fail");
        assert_eq!(
            err.to_string(),
            "invalid remote control URL `ftp://example.com/control`; expected ws://, wss://, http://, or https://"
        );
    }

    #[tokio::test]
    async fn persisted_remote_control_enrollment_round_trips_by_target_and_account() {
        let codex_home = TempDir::new().expect("temp dir should create");
        let state_path = remote_control_state_path(codex_home.path());
        let first_target = normalize_remote_control_url("http://example.com/remote/control")
            .expect("first target should parse");
        let second_target = normalize_remote_control_url("http://example.com/other/control")
            .expect("second target should parse");
        let first_enrollment = RemoteControlEnrollment {
            server_id: "srv_e_first".to_string(),
            server_name: REMOTE_CONTROL_SERVER_NAME.to_string(),
        };
        let second_enrollment = RemoteControlEnrollment {
            server_id: "srv_e_second".to_string(),
            server_name: REMOTE_CONTROL_SERVER_NAME.to_string(),
        };

        update_persisted_remote_control_enrollment(
            state_path.as_path(),
            &first_target,
            Some("account-a"),
            Some(&first_enrollment),
        )
        .await
        .expect("first enrollment should persist");
        update_persisted_remote_control_enrollment(
            state_path.as_path(),
            &second_target,
            Some("account-a"),
            Some(&second_enrollment),
        )
        .await
        .expect("second enrollment should persist");

        assert_eq!(
            load_persisted_remote_control_enrollment(
                state_path.as_path(),
                &first_target,
                Some("account-a"),
            )
            .await,
            Some(first_enrollment.clone())
        );
        assert_eq!(
            load_persisted_remote_control_enrollment(
                state_path.as_path(),
                &first_target,
                Some("account-b"),
            )
            .await,
            None
        );
        assert_eq!(
            load_persisted_remote_control_enrollment(
                state_path.as_path(),
                &second_target,
                Some("account-a"),
            )
            .await,
            Some(second_enrollment)
        );
    }

    #[tokio::test]
    async fn clearing_persisted_remote_control_enrollment_removes_only_matching_entry() {
        let codex_home = TempDir::new().expect("temp dir should create");
        let state_path = remote_control_state_path(codex_home.path());
        let first_target = normalize_remote_control_url("http://example.com/remote/control")
            .expect("first target should parse");
        let second_target = normalize_remote_control_url("http://example.com/other/control")
            .expect("second target should parse");
        let first_enrollment = RemoteControlEnrollment {
            server_id: "srv_e_first".to_string(),
            server_name: REMOTE_CONTROL_SERVER_NAME.to_string(),
        };
        let second_enrollment = RemoteControlEnrollment {
            server_id: "srv_e_second".to_string(),
            server_name: REMOTE_CONTROL_SERVER_NAME.to_string(),
        };

        update_persisted_remote_control_enrollment(
            state_path.as_path(),
            &first_target,
            Some("account-a"),
            Some(&first_enrollment),
        )
        .await
        .expect("first enrollment should persist");
        update_persisted_remote_control_enrollment(
            state_path.as_path(),
            &second_target,
            Some("account-a"),
            Some(&second_enrollment),
        )
        .await
        .expect("second enrollment should persist");

        update_persisted_remote_control_enrollment(
            state_path.as_path(),
            &first_target,
            Some("account-a"),
            None,
        )
        .await
        .expect("matching enrollment should clear");

        assert_eq!(
            load_persisted_remote_control_enrollment(
                state_path.as_path(),
                &first_target,
                Some("account-a"),
            )
            .await,
            None
        );
        assert_eq!(
            load_persisted_remote_control_enrollment(
                state_path.as_path(),
                &second_target,
                Some("account-a"),
            )
            .await,
            Some(second_enrollment)
        );
    }

    #[test]
    fn remote_control_client_is_alive_respects_activity_timeout() {
        let base = Instant::now();
        let client = RemoteControlClientState {
            connection_id: ConnectionId(11),
            disconnect_token: CancellationToken::new(),
            last_activity_at: base,
        };

        assert!(remote_control_client_is_alive(
            &client,
            base + REMOTE_CONTROL_CLIENT_IDLE_TIMEOUT - Duration::from_millis(1)
        ));
        assert!(!remote_control_client_is_alive(
            &client,
            base + REMOTE_CONTROL_CLIENT_IDLE_TIMEOUT
        ));
    }

    #[tokio::test]
    async fn remote_control_transport_manages_virtual_clients_and_routes_messages() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let remote_control_url = format!(
            "ws://{}",
            listener
                .local_addr()
                .expect("listener should have a local addr")
        );
        let codex_home = TempDir::new().expect("temp dir should create");
        let (transport_event_tx, mut transport_event_rx) =
            mpsc::channel::<TransportEvent>(CHANNEL_CAPACITY);
        let shutdown_token = CancellationToken::new();
        let remote_handle = start_remote_control(
            remote_control_url,
            codex_home.path().to_path_buf(),
            remote_control_auth_manager(),
            transport_event_tx,
            shutdown_token.clone(),
        )
        .await
        .expect("remote control should start");
        let mut websocket = accept_remote_control_connection(&listener).await;

        let client_id = ClientId("client-1".to_string());
        send_client_event(
            &mut websocket,
            ClientEvent::Ping {
                client_id: client_id.clone(),
                state: Some(ClientActivityState::Foreground),
            },
        )
        .await;
        assert_eq!(
            read_server_event(&mut websocket).await,
            json!({
                "type": "pong",
                "client_id": "client-1",
                "status": "unknown",
            })
        );

        send_client_event(
            &mut websocket,
            ClientEvent::ClientMessage {
                client_id: client_id.clone(),
                message: JSONRPCMessage::Notification(
                    codex_app_server_protocol::JSONRPCNotification {
                        method: "initialized".to_string(),
                        params: None,
                    },
                ),
            },
        )
        .await;
        assert!(
            timeout(Duration::from_millis(100), transport_event_rx.recv())
                .await
                .is_err(),
            "non-initialize client messages should be ignored before connection creation"
        );

        let initialize_message =
            JSONRPCMessage::Request(codex_app_server_protocol::JSONRPCRequest {
                id: codex_app_server_protocol::RequestId::Integer(1),
                method: "initialize".to_string(),
                params: Some(json!({
                    "clientInfo": {
                        "name": "remote-test-client",
                        "version": "0.1.0"
                    }
                })),
            });
        send_client_event(
            &mut websocket,
            ClientEvent::ClientMessage {
                client_id: client_id.clone(),
                message: initialize_message.clone(),
            },
        )
        .await;

        let (connection_id, writer) =
            match timeout(Duration::from_secs(5), transport_event_rx.recv())
                .await
                .expect("connection open should arrive in time")
                .expect("connection open should exist")
            {
                TransportEvent::ConnectionOpened {
                    connection_id,
                    writer,
                    ..
                } => (connection_id, writer),
                other => panic!("expected connection open event, got {other:?}"),
            };

        match timeout(Duration::from_secs(5), transport_event_rx.recv())
            .await
            .expect("initialize message should arrive in time")
            .expect("initialize message should exist")
        {
            TransportEvent::IncomingMessage {
                connection_id: incoming_connection_id,
                message,
            } => {
                assert_eq!(incoming_connection_id, connection_id);
                assert_eq!(message, initialize_message);
            }
            other => panic!("expected initialize incoming message, got {other:?}"),
        }

        let followup_message =
            JSONRPCMessage::Notification(codex_app_server_protocol::JSONRPCNotification {
                method: "initialized".to_string(),
                params: None,
            });
        send_client_event(
            &mut websocket,
            ClientEvent::ClientMessage {
                client_id: client_id.clone(),
                message: followup_message.clone(),
            },
        )
        .await;
        match timeout(Duration::from_secs(5), transport_event_rx.recv())
            .await
            .expect("followup message should arrive in time")
            .expect("followup message should exist")
        {
            TransportEvent::IncomingMessage {
                connection_id: incoming_connection_id,
                message,
            } => {
                assert_eq!(incoming_connection_id, connection_id);
                assert_eq!(message, followup_message);
            }
            other => panic!("expected followup incoming message, got {other:?}"),
        }

        send_client_event(
            &mut websocket,
            ClientEvent::Ping {
                client_id: client_id.clone(),
                state: Some(ClientActivityState::Foreground),
            },
        )
        .await;
        assert_eq!(
            read_server_event(&mut websocket).await,
            json!({
                "type": "pong",
                "client_id": "client-1",
                "status": "active",
            })
        );

        writer
            .send(OutgoingMessage::Notification(
                crate::outgoing_message::OutgoingNotification {
                    method: "codex/event/test".to_string(),
                    params: Some(json!({ "ok": true })),
                },
            ))
            .await
            .expect("remote writer should accept outgoing message");
        assert_eq!(
            read_server_event(&mut websocket).await,
            json!({
                "type": "server_message",
                "client_id": "client-1",
                "message": {
                    "method": "codex/event/test",
                    "params": {
                        "ok": true,
                    }
                }
            })
        );

        send_client_event(
            &mut websocket,
            ClientEvent::ClientClosed {
                client_id: client_id.clone(),
            },
        )
        .await;
        match timeout(Duration::from_secs(5), transport_event_rx.recv())
            .await
            .expect("connection close should arrive in time")
            .expect("connection close should exist")
        {
            TransportEvent::ConnectionClosed {
                connection_id: closed_connection_id,
            } => {
                assert_eq!(closed_connection_id, connection_id);
            }
            other => panic!("expected connection close event, got {other:?}"),
        }

        send_client_event(
            &mut websocket,
            ClientEvent::Ping {
                client_id,
                state: Some(ClientActivityState::Foreground),
            },
        )
        .await;
        assert_eq!(
            read_server_event(&mut websocket).await,
            json!({
                "type": "pong",
                "client_id": "client-1",
                "status": "unknown",
            })
        );

        shutdown_token.cancel();
        let _ = remote_handle.await;
    }

    #[tokio::test]
    async fn remote_control_transport_reconnects_after_disconnect() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let remote_control_url = format!(
            "ws://{}",
            listener
                .local_addr()
                .expect("listener should have a local addr")
        );
        let codex_home = TempDir::new().expect("temp dir should create");
        let (transport_event_tx, mut transport_event_rx) =
            mpsc::channel::<TransportEvent>(CHANNEL_CAPACITY);
        let shutdown_token = CancellationToken::new();
        let remote_handle = start_remote_control(
            remote_control_url,
            codex_home.path().to_path_buf(),
            remote_control_auth_manager(),
            transport_event_tx,
            shutdown_token.clone(),
        )
        .await
        .expect("remote control should start");

        let mut first_websocket = accept_remote_control_connection(&listener).await;
        first_websocket
            .close(None)
            .await
            .expect("first websocket should close");
        drop(first_websocket);

        let mut second_websocket = accept_remote_control_connection(&listener).await;
        send_client_event(
            &mut second_websocket,
            ClientEvent::ClientMessage {
                client_id: ClientId("client-2".to_string()),
                message: JSONRPCMessage::Request(codex_app_server_protocol::JSONRPCRequest {
                    id: codex_app_server_protocol::RequestId::Integer(2),
                    method: "initialize".to_string(),
                    params: Some(json!({
                        "clientInfo": {
                            "name": "remote-test-client",
                            "version": "0.1.0"
                        }
                    })),
                }),
            },
        )
        .await;

        match timeout(Duration::from_secs(5), transport_event_rx.recv())
            .await
            .expect("reconnected initialize should arrive in time")
            .expect("reconnected initialize should exist")
        {
            TransportEvent::ConnectionOpened { .. } => {}
            other => panic!("expected connection open after reconnect, got {other:?}"),
        }

        shutdown_token.cancel();
        let _ = remote_handle.await;
    }

    #[tokio::test]
    async fn remote_control_http_mode_enrolls_before_connecting() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let remote_control_url = format!(
            "http://{}/backend-api/wham",
            listener
                .local_addr()
                .expect("listener should have a local addr")
        );
        let codex_home = TempDir::new().expect("temp dir should create");
        let (transport_event_tx, mut transport_event_rx) =
            mpsc::channel::<TransportEvent>(CHANNEL_CAPACITY);
        let shutdown_token = CancellationToken::new();
        let remote_handle = start_remote_control(
            remote_control_url,
            codex_home.path().to_path_buf(),
            remote_control_auth_manager(),
            transport_event_tx,
            shutdown_token.clone(),
        )
        .await
        .expect("remote control should start");

        let enroll_request = accept_http_request(&listener).await;
        assert_eq!(
            enroll_request.request_line,
            "POST /backend-api/wham/remote/control/server/enroll HTTP/1.1"
        );
        assert_eq!(
            enroll_request.headers.get("authorization"),
            Some(&"Bearer Access Token".to_string())
        );
        assert_eq!(
            enroll_request.headers.get(REMOTE_CONTROL_ACCOUNT_ID_HEADER),
            Some(&"account_id".to_string())
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&enroll_request.body)
                .expect("enroll body should deserialize"),
            json!({
                "name": REMOTE_CONTROL_SERVER_NAME,
                "os": std::env::consts::OS,
                "arch": std::env::consts::ARCH,
                "app_server_version": env!("CARGO_PKG_VERSION"),
            })
        );
        respond_with_json(enroll_request.stream, json!({ "server_id": "srv_e_test" })).await;

        let (handshake_request, mut websocket) =
            accept_remote_control_backend_connection(&listener).await;
        assert_eq!(
            handshake_request.path,
            "/backend-api/wham/remote/control/server"
        );
        assert_eq!(
            handshake_request.headers.get("authorization"),
            Some(&"Bearer Access Token".to_string())
        );
        assert_eq!(
            handshake_request
                .headers
                .get(REMOTE_CONTROL_ACCOUNT_ID_HEADER),
            Some(&"account_id".to_string())
        );
        assert_eq!(
            handshake_request.headers.get("x-codex-server-id"),
            Some(&"srv_e_test".to_string())
        );
        assert_eq!(
            handshake_request.headers.get("x-codex-name"),
            Some(&REMOTE_CONTROL_SERVER_NAME.to_string())
        );
        assert_eq!(
            handshake_request.headers.get("x-codex-protocol-version"),
            Some(&REMOTE_CONTROL_PROTOCOL_VERSION.to_string())
        );

        let backend_client_id = ClientId("backend-test-client".to_string());
        let writer = {
            let initialize_message =
                JSONRPCMessage::Request(codex_app_server_protocol::JSONRPCRequest {
                    id: codex_app_server_protocol::RequestId::Integer(11),
                    method: "initialize".to_string(),
                    params: Some(json!({
                        "clientInfo": {
                            "name": "remote-backend-client",
                            "version": "0.1.0"
                        }
                    })),
                });
            send_client_event(
                &mut websocket,
                ClientEvent::ClientMessage {
                    client_id: backend_client_id.clone(),
                    message: initialize_message.clone(),
                },
            )
            .await;

            let (connection_id, writer) =
                match timeout(Duration::from_secs(5), transport_event_rx.recv())
                    .await
                    .expect("connection open should arrive in time")
                    .expect("connection open should exist")
                {
                    TransportEvent::ConnectionOpened {
                        connection_id,
                        writer,
                        ..
                    } => (connection_id, writer),
                    other => panic!("expected connection open event, got {other:?}"),
                };

            match timeout(Duration::from_secs(5), transport_event_rx.recv())
                .await
                .expect("initialize message should arrive in time")
                .expect("initialize message should exist")
            {
                TransportEvent::IncomingMessage {
                    connection_id: incoming_connection_id,
                    message,
                } => {
                    assert_eq!(incoming_connection_id, connection_id);
                    assert_eq!(message, initialize_message);
                }
                other => panic!("expected initialize incoming message, got {other:?}"),
            }
            writer
        };

        writer
            .send(OutgoingMessage::Response(
                crate::outgoing_message::OutgoingResponse {
                    id: codex_app_server_protocol::RequestId::Integer(11),
                    result: json!({
                        "userAgent": "codex-test-agent"
                    }),
                },
            ))
            .await
            .expect("remote writer should accept initialize response");
        assert_eq!(
            read_server_event(&mut websocket).await,
            json!({
                "type": "server_message",
                "client_id": backend_client_id.0.clone(),
                "message": {
                    "id": 11,
                    "result": {
                        "userAgent": "codex-test-agent",
                    }
                }
            })
        );

        writer
            .send(OutgoingMessage::Notification(
                crate::outgoing_message::OutgoingNotification {
                    method: "codex/event/test".to_string(),
                    params: Some(json!({ "backend": true })),
                },
            ))
            .await
            .expect("remote writer should accept outgoing message");
        assert_eq!(
            read_server_event(&mut websocket).await,
            json!({
                "type": "server_message",
                "client_id": backend_client_id.0.clone(),
                "message": {
                    "method": "codex/event/test",
                    "params": {
                        "backend": true,
                    }
                }
            })
        );

        shutdown_token.cancel();
        let _ = remote_handle.await;
    }

    #[tokio::test]
    async fn remote_control_http_mode_reuses_persisted_enrollment_before_reenrolling() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let remote_control_url = format!(
            "http://{}/backend-api/wham",
            listener
                .local_addr()
                .expect("listener should have a local addr")
        );
        let codex_home = TempDir::new().expect("temp dir should create");
        let remote_control_target =
            normalize_remote_control_url(&remote_control_url).expect("target should parse");
        let persisted_enrollment = RemoteControlEnrollment {
            server_id: "srv_e_persisted".to_string(),
            server_name: REMOTE_CONTROL_SERVER_NAME.to_string(),
        };
        update_persisted_remote_control_enrollment(
            remote_control_state_path(codex_home.path()).as_path(),
            &remote_control_target,
            Some("account_id"),
            Some(&persisted_enrollment),
        )
        .await
        .expect("persisted enrollment should save");

        let (transport_event_tx, _transport_event_rx) =
            mpsc::channel::<TransportEvent>(CHANNEL_CAPACITY);
        let shutdown_token = CancellationToken::new();
        let remote_handle = start_remote_control(
            remote_control_url,
            codex_home.path().to_path_buf(),
            remote_control_auth_manager_with_home(&codex_home),
            transport_event_tx,
            shutdown_token.clone(),
        )
        .await
        .expect("remote control should start");

        let (handshake_request, _websocket) =
            accept_remote_control_backend_connection(&listener).await;
        assert_eq!(
            handshake_request.path,
            "/backend-api/wham/remote/control/server"
        );
        assert_eq!(
            handshake_request.headers.get("x-codex-server-id"),
            Some(&persisted_enrollment.server_id)
        );
        assert_eq!(
            load_persisted_remote_control_enrollment(
                remote_control_state_path(codex_home.path()).as_path(),
                &remote_control_target,
                Some("account_id"),
            )
            .await,
            Some(persisted_enrollment)
        );

        shutdown_token.cancel();
        let _ = remote_handle.await;
    }

    #[tokio::test]
    async fn remote_control_http_mode_clears_stale_persisted_enrollment_after_404() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let remote_control_url = format!(
            "http://{}/backend-api/wham",
            listener
                .local_addr()
                .expect("listener should have a local addr")
        );
        let codex_home = TempDir::new().expect("temp dir should create");
        let state_path = remote_control_state_path(codex_home.path());
        let remote_control_target =
            normalize_remote_control_url(&remote_control_url).expect("target should parse");
        let stale_enrollment = RemoteControlEnrollment {
            server_id: "srv_e_stale".to_string(),
            server_name: REMOTE_CONTROL_SERVER_NAME.to_string(),
        };
        let refreshed_enrollment = RemoteControlEnrollment {
            server_id: "srv_e_refreshed".to_string(),
            server_name: REMOTE_CONTROL_SERVER_NAME.to_string(),
        };
        update_persisted_remote_control_enrollment(
            state_path.as_path(),
            &remote_control_target,
            Some("account_id"),
            Some(&stale_enrollment),
        )
        .await
        .expect("stale enrollment should save");

        let (transport_event_tx, _transport_event_rx) =
            mpsc::channel::<TransportEvent>(CHANNEL_CAPACITY);
        let shutdown_token = CancellationToken::new();
        let remote_handle = start_remote_control(
            remote_control_url,
            codex_home.path().to_path_buf(),
            remote_control_auth_manager_with_home(&codex_home),
            transport_event_tx,
            shutdown_token.clone(),
        )
        .await
        .expect("remote control should start");

        let websocket_request = accept_http_request(&listener).await;
        assert_eq!(
            websocket_request.request_line,
            "GET /backend-api/wham/remote/control/server HTTP/1.1"
        );
        assert_eq!(
            websocket_request.headers.get("x-codex-server-id"),
            Some(&stale_enrollment.server_id)
        );
        respond_with_status(websocket_request.stream, "404 Not Found", "").await;

        let enroll_request = accept_http_request(&listener).await;
        assert_eq!(
            enroll_request.request_line,
            "POST /backend-api/wham/remote/control/server/enroll HTTP/1.1"
        );
        respond_with_json(
            enroll_request.stream,
            json!({ "server_id": refreshed_enrollment.server_id }),
        )
        .await;

        let (handshake_request, _websocket) =
            accept_remote_control_backend_connection(&listener).await;
        assert_eq!(
            handshake_request.headers.get("x-codex-server-id"),
            Some(&refreshed_enrollment.server_id)
        );
        assert_eq!(
            load_persisted_remote_control_enrollment(
                state_path.as_path(),
                &remote_control_target,
                Some("account_id"),
            )
            .await,
            Some(refreshed_enrollment)
        );

        shutdown_token.cancel();
        let _ = remote_handle.await;
    }

    #[derive(Debug)]
    struct CapturedHttpRequest {
        stream: TcpStream,
        request_line: String,
        headers: BTreeMap<String, String>,
        body: String,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct CapturedWebSocketRequest {
        path: String,
        headers: BTreeMap<String, String>,
    }

    async fn accept_remote_control_connection(
        listener: &TcpListener,
    ) -> WebSocketStream<TcpStream> {
        let (stream, _) = timeout(Duration::from_secs(5), listener.accept())
            .await
            .expect("remote control should connect in time")
            .expect("listener accept should succeed");
        accept_async(stream)
            .await
            .expect("websocket handshake should succeed")
    }

    async fn accept_http_request(listener: &TcpListener) -> CapturedHttpRequest {
        let (stream, _) = timeout(Duration::from_secs(5), listener.accept())
            .await
            .expect("HTTP request should arrive in time")
            .expect("listener accept should succeed");
        let mut reader = BufReader::new(stream);

        let mut request_line = String::new();
        reader
            .read_line(&mut request_line)
            .await
            .expect("request line should read");
        let request_line = request_line.trim_end_matches("\r\n").to_string();

        let mut headers = BTreeMap::new();
        loop {
            let mut line = String::new();
            reader
                .read_line(&mut line)
                .await
                .expect("header line should read");
            if line == "\r\n" {
                break;
            }
            let line = line.trim_end_matches("\r\n");
            let (name, value) = line.split_once(':').expect("header should contain colon");
            headers.insert(name.to_ascii_lowercase(), value.trim().to_string());
        }

        let content_length = headers
            .get("content-length")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(0);
        let mut body = vec![0; content_length];
        reader
            .read_exact(&mut body)
            .await
            .expect("request body should read");

        CapturedHttpRequest {
            stream: reader.into_inner(),
            request_line,
            headers,
            body: String::from_utf8(body).expect("body should be utf-8"),
        }
    }

    async fn respond_with_json(mut stream: TcpStream, body: serde_json::Value) {
        let body = body.to_string();
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        stream
            .write_all(response.as_bytes())
            .await
            .expect("response should write");
        stream.flush().await.expect("response should flush");
    }

    async fn respond_with_status(mut stream: TcpStream, status: &str, body: &str) {
        let response = format!(
            "HTTP/1.1 {status}\r\ncontent-type: text/plain\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        stream
            .write_all(response.as_bytes())
            .await
            .expect("response should write");
        stream.flush().await.expect("response should flush");
    }

    async fn accept_remote_control_backend_connection(
        listener: &TcpListener,
    ) -> (CapturedWebSocketRequest, WebSocketStream<TcpStream>) {
        let (stream, _) = timeout(Duration::from_secs(5), listener.accept())
            .await
            .expect("websocket request should arrive in time")
            .expect("listener accept should succeed");
        let captured_request = Arc::new(std::sync::Mutex::new(None::<CapturedWebSocketRequest>));
        let captured_request_for_callback = captured_request.clone();
        let websocket = accept_hdr_async(
            stream,
            move |request: &tungstenite::handshake::server::Request,
                  response: tungstenite::handshake::server::Response| {
                let headers = request
                    .headers()
                    .iter()
                    .map(|(name, value)| {
                        (
                            name.as_str().to_ascii_lowercase(),
                            value
                                .to_str()
                                .expect("header should be valid utf-8")
                                .to_string(),
                        )
                    })
                    .collect::<BTreeMap<_, _>>();
                *captured_request_for_callback
                    .lock()
                    .expect("capture lock should acquire") = Some(CapturedWebSocketRequest {
                    path: request.uri().path().to_string(),
                    headers,
                });
                Ok(response)
            },
        )
        .await
        .expect("websocket handshake should succeed");
        let captured_request = captured_request
            .lock()
            .expect("capture lock should acquire")
            .clone()
            .expect("websocket request should be captured");
        (captured_request, websocket)
    }

    async fn send_client_event(
        websocket: &mut WebSocketStream<TcpStream>,
        client_event: ClientEvent,
    ) {
        let payload = serde_json::to_string(&client_event).expect("client event should serialize");
        websocket
            .send(WebSocketMessage::Text(payload.into()))
            .await
            .expect("client event should send");
    }

    async fn read_server_event(websocket: &mut WebSocketStream<TcpStream>) -> serde_json::Value {
        loop {
            let frame = timeout(Duration::from_secs(5), websocket.next())
                .await
                .expect("server event should arrive in time")
                .expect("websocket should stay open")
                .expect("websocket frame should be readable");
            match frame {
                WebSocketMessage::Text(text) => {
                    return serde_json::from_str(text.as_ref())
                        .expect("server event should deserialize");
                }
                WebSocketMessage::Ping(payload) => {
                    websocket
                        .send(WebSocketMessage::Pong(payload))
                        .await
                        .expect("websocket pong should send");
                }
                WebSocketMessage::Pong(_) => {}
                WebSocketMessage::Close(frame) => {
                    panic!("unexpected websocket close frame: {frame:?}");
                }
                WebSocketMessage::Binary(_) => {
                    panic!("unexpected binary websocket frame");
                }
                WebSocketMessage::Frame(_) => {}
            }
        }
    }
}
