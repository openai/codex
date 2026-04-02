pub(crate) mod auth;

use crate::error_code::OVERLOADED_ERROR_CODE;
use crate::message_processor::ConnectionSessionState;
use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::OutgoingEnvelope;
use crate::outgoing_message::OutgoingError;
use crate::outgoing_message::OutgoingMessage;
use crate::outgoing_message::QueuedOutgoingMessage;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::ServerRequest;
use std::collections::HashMap;
use std::collections::HashSet;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::error;
use tracing::warn;

/// Size of the bounded channels used to communicate between tasks. The value
/// is a balance between throughput and memory usage - 128 messages should be
/// plenty for an interactive CLI.
pub(crate) const CHANNEL_CAPACITY: usize = 128;

mod stdio;
mod websocket;

pub(crate) use stdio::start_stdio_connection;
pub(crate) use websocket::start_websocket_acceptor;

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
        writer: mpsc::Sender<QueuedOutgoingMessage>,
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
    pub(crate) writer: mpsc::Sender<QueuedOutgoingMessage>,
    disconnect_sender: Option<CancellationToken>,
}

impl OutboundConnectionState {
    pub(crate) fn new(
        writer: mpsc::Sender<QueuedOutgoingMessage>,
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

async fn forward_incoming_message(
    transport_event_tx: &mpsc::Sender<TransportEvent>,
    writer: &mpsc::Sender<QueuedOutgoingMessage>,
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
    writer: &mpsc::Sender<QueuedOutgoingMessage>,
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
            match writer.try_send(QueuedOutgoingMessage::new(overload_error)) {
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
    write_complete_tx: Option<tokio::sync::oneshot::Sender<()>>,
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
    let queued_message = QueuedOutgoingMessage {
        message,
        write_complete_tx,
    };
    if connection_state.can_disconnect() {
        match writer.try_send(queued_message) {
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
    } else if writer.send(queued_message).await.is_err() {
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
            write_complete_tx,
        } => {
            let _ =
                send_message_to_connection(connections, connection_id, message, write_complete_tx)
                    .await;
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
                let _ = send_message_to_connection(
                    connections,
                    connection_id,
                    message.clone(),
                    /*write_complete_tx*/ None,
                )
                .await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error_code::OVERLOADED_ERROR_CODE;
    use codex_app_server_protocol::ConfigWarningNotification;
    use codex_app_server_protocol::ServerNotification;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::path::PathBuf;
    use tokio::time::Duration;
    use tokio::time::timeout;

    fn absolute_path(path: &str) -> AbsolutePathBuf {
        AbsolutePathBuf::from_absolute_path(path).expect("absolute path")
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
            trace: None,
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
        let overload_json =
            serde_json::to_value(overload.message).expect("serialize overload error");
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
            .send(QueuedOutgoingMessage::new(
                OutgoingMessage::AppServerNotification(ServerNotification::ConfigWarning(
                    ConfigWarningNotification {
                        summary: "queued".to_string(),
                        details: None,
                        path: None,
                        range: None,
                    },
                )),
            ))
            .await
            .expect("writer queue should accept first message");

        let request = JSONRPCMessage::Request(codex_app_server_protocol::JSONRPCRequest {
            id: codex_app_server_protocol::RequestId::Integer(7),
            method: "config/read".to_string(),
            params: Some(json!({ "includeLayers": false })),
            trace: None,
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
        let queued_json =
            serde_json::to_value(queued_outgoing.message).expect("serialize queued message");
        assert_eq!(
            queued_json,
            json!({
                "method": "configWarning",
                "params": {
                    "summary": "queued",
                    "details": null,
                },
            })
        );
    }

    #[tokio::test]
    async fn to_connection_notification_respects_opt_out_filters() {
        let connection_id = ConnectionId(7);
        let (writer_tx, mut writer_rx) = mpsc::channel(1);
        let initialized = Arc::new(AtomicBool::new(true));
        let opted_out_notification_methods =
            Arc::new(RwLock::new(HashSet::from(["configWarning".to_string()])));

        let mut connections = HashMap::new();
        connections.insert(
            connection_id,
            OutboundConnectionState::new(
                writer_tx,
                initialized,
                Arc::new(AtomicBool::new(true)),
                opted_out_notification_methods,
                /*disconnect_sender*/ None,
            ),
        );

        route_outgoing_envelope(
            &mut connections,
            OutgoingEnvelope::ToConnection {
                connection_id,
                message: OutgoingMessage::AppServerNotification(ServerNotification::ConfigWarning(
                    ConfigWarningNotification {
                        summary: "task_started".to_string(),
                        details: None,
                        path: None,
                        range: None,
                    },
                )),
                write_complete_tx: None,
            },
        )
        .await;

        assert!(
            writer_rx.try_recv().is_err(),
            "opted-out notification should be dropped"
        );
    }

    #[tokio::test]
    async fn to_connection_notifications_are_dropped_for_opted_out_clients() {
        let connection_id = ConnectionId(10);
        let (writer_tx, mut writer_rx) = mpsc::channel(1);

        let mut connections = HashMap::new();
        connections.insert(
            connection_id,
            OutboundConnectionState::new(
                writer_tx,
                Arc::new(AtomicBool::new(true)),
                Arc::new(AtomicBool::new(true)),
                Arc::new(RwLock::new(HashSet::from(["configWarning".to_string()]))),
                /*disconnect_sender*/ None,
            ),
        );

        route_outgoing_envelope(
            &mut connections,
            OutgoingEnvelope::ToConnection {
                connection_id,
                message: OutgoingMessage::AppServerNotification(ServerNotification::ConfigWarning(
                    ConfigWarningNotification {
                        summary: "task_started".to_string(),
                        details: None,
                        path: None,
                        range: None,
                    },
                )),
                write_complete_tx: None,
            },
        )
        .await;

        assert!(
            writer_rx.try_recv().is_err(),
            "opted-out notifications should not reach clients"
        );
    }

    #[tokio::test]
    async fn to_connection_notifications_are_preserved_for_non_opted_out_clients() {
        let connection_id = ConnectionId(11);
        let (writer_tx, mut writer_rx) = mpsc::channel(1);

        let mut connections = HashMap::new();
        connections.insert(
            connection_id,
            OutboundConnectionState::new(
                writer_tx,
                Arc::new(AtomicBool::new(true)),
                Arc::new(AtomicBool::new(true)),
                Arc::new(RwLock::new(HashSet::new())),
                /*disconnect_sender*/ None,
            ),
        );

        route_outgoing_envelope(
            &mut connections,
            OutgoingEnvelope::ToConnection {
                connection_id,
                message: OutgoingMessage::AppServerNotification(ServerNotification::ConfigWarning(
                    ConfigWarningNotification {
                        summary: "task_started".to_string(),
                        details: None,
                        path: None,
                        range: None,
                    },
                )),
                write_complete_tx: None,
            },
        )
        .await;

        let message = writer_rx
            .recv()
            .await
            .expect("notification should reach non-opted-out clients");
        assert!(matches!(
            message.message,
            OutgoingMessage::AppServerNotification(ServerNotification::ConfigWarning(
                ConfigWarningNotification { summary, .. }
            )) if summary == "task_started"
        ));
    }

    #[tokio::test]
    async fn command_execution_request_approval_strips_additional_permissions_without_capability() {
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
                /*disconnect_sender*/ None,
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
                                        entries: None,
                                    },
                                ),
                            },
                        ),
                        proposed_execpolicy_amendment: None,
                        proposed_network_policy_amendments: None,
                        available_decisions: None,
                    },
                }),
                write_complete_tx: None,
            },
        )
        .await;

        let message = writer_rx
            .recv()
            .await
            .expect("request should be delivered to the connection");
        let json = serde_json::to_value(message.message).expect("request should serialize");
        assert_eq!(json["params"].get("additionalPermissions"), None);
    }

    #[tokio::test]
    async fn command_execution_request_approval_keeps_additional_permissions_with_capability() {
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
                /*disconnect_sender*/ None,
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
                                        entries: None,
                                    },
                                ),
                            },
                        ),
                        proposed_execpolicy_amendment: None,
                        proposed_network_policy_amendments: None,
                        available_decisions: None,
                    },
                }),
                write_complete_tx: None,
            },
        )
        .await;

        let message = writer_rx
            .recv()
            .await
            .expect("request should be delivered to the connection");
        let json = serde_json::to_value(message.message).expect("request should serialize");
        let allowed_path = absolute_path("/tmp/allowed").to_string_lossy().into_owned();
        assert_eq!(
            json["params"]["additionalPermissions"],
            json!({
                "network": null,
                "fileSystem": {
                    "read": [allowed_path],
                    "write": null,
                    "entries": null,
                },
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

        let queued_message = OutgoingMessage::AppServerNotification(
            ServerNotification::ConfigWarning(ConfigWarningNotification {
                summary: "already-buffered".to_string(),
                details: None,
                path: None,
                range: None,
            }),
        );
        slow_writer_tx
            .try_send(QueuedOutgoingMessage::new(queued_message))
            .expect("channel should have room");

        let broadcast_message = OutgoingMessage::AppServerNotification(
            ServerNotification::ConfigWarning(ConfigWarningNotification {
                summary: "test".to_string(),
                details: None,
                path: None,
                range: None,
            }),
        );
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
        .expect("broadcast should return even when one connection is slow");
        assert!(!connections.contains_key(&slow_connection_id));
        assert!(slow_disconnect_token.is_cancelled());
        assert!(!fast_disconnect_token.is_cancelled());
        let fast_message = fast_writer_rx
            .try_recv()
            .expect("fast connection should receive the broadcast notification");
        assert!(matches!(
            fast_message.message,
            OutgoingMessage::AppServerNotification(ServerNotification::ConfigWarning(
                ConfigWarningNotification { summary, .. }
            )) if summary == "test"
        ));

        let slow_message = slow_writer_rx
            .try_recv()
            .expect("slow connection should retain its original buffered message");
        assert!(matches!(
            slow_message.message,
            OutgoingMessage::AppServerNotification(ServerNotification::ConfigWarning(
                ConfigWarningNotification { summary, .. }
            )) if summary == "already-buffered"
        ));
    }

    #[tokio::test]
    async fn to_connection_stdio_waits_instead_of_disconnecting_when_writer_queue_is_full() {
        let connection_id = ConnectionId(3);
        let (writer_tx, mut writer_rx) = mpsc::channel(1);
        writer_tx
            .send(QueuedOutgoingMessage::new(
                OutgoingMessage::AppServerNotification(ServerNotification::ConfigWarning(
                    ConfigWarningNotification {
                        summary: "queued".to_string(),
                        details: None,
                        path: None,
                        range: None,
                    },
                )),
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
                /*disconnect_sender*/ None,
            ),
        );

        let route_task = tokio::spawn(async move {
            route_outgoing_envelope(
                &mut connections,
                OutgoingEnvelope::ToConnection {
                    connection_id,
                    message: OutgoingMessage::AppServerNotification(
                        ServerNotification::ConfigWarning(ConfigWarningNotification {
                            summary: "second".to_string(),
                            details: None,
                            path: None,
                            range: None,
                        }),
                    ),
                    write_complete_tx: None,
                },
            )
            .await
        });

        let first = timeout(Duration::from_millis(100), writer_rx.recv())
            .await
            .expect("first queued message should be readable")
            .expect("first queued message should exist");
        timeout(Duration::from_millis(100), route_task)
            .await
            .expect("routing should finish after the first queued message is drained")
            .expect("routing task should succeed");

        assert!(matches!(
            first.message,
            OutgoingMessage::AppServerNotification(ServerNotification::ConfigWarning(
                ConfigWarningNotification { summary, .. }
            )) if summary == "queued"
        ));
        let second = writer_rx
            .try_recv()
            .expect("second notification should be delivered once the queue has room");
        assert!(matches!(
            second.message,
            OutgoingMessage::AppServerNotification(ServerNotification::ConfigWarning(
                ConfigWarningNotification { summary, .. }
            )) if summary == "second"
        ));
    }
}
