use super::CHANNEL_CAPACITY;
use super::TransportEvent;
use super::next_connection_id;
use super::protocol::ClientEnvelope;
pub use super::protocol::ClientEvent;
pub use super::protocol::ClientId;
use super::protocol::PongStatus;
use super::protocol::ServerEvent;
use super::protocol::StreamId;
use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::QueuedOutgoingMessage;
use crate::transport::ConnectionOrigin;
use crate::transport::remote_control::QueuedServerEnvelope;
use codex_app_server_protocol::JSONRPCMessage;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tokio::time::Duration;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

const REMOTE_CONTROL_CLIENT_IDLE_TIMEOUT: Duration = Duration::from_secs(10 * 60);
pub(crate) const REMOTE_CONTROL_IDLE_SWEEP_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug)]
pub(crate) struct Stopped;

struct ClientState {
    connection_id: ConnectionId,
    disconnect_token: CancellationToken,
    last_activity_at: Instant,
    last_inbound_seq_id: Option<u64>,
    status_tx: watch::Sender<PongStatus>,
}

pub(crate) struct ClientTracker {
    clients: HashMap<(ClientId, StreamId), ClientState>,
    legacy_stream_ids: HashMap<ClientId, StreamId>,
    join_set: JoinSet<(ClientId, StreamId)>,
    server_event_tx: mpsc::Sender<QueuedServerEnvelope>,
    transport_event_tx: mpsc::Sender<TransportEvent>,
    shutdown_token: CancellationToken,
}

impl ClientTracker {
    pub(crate) fn new(
        server_event_tx: mpsc::Sender<QueuedServerEnvelope>,
        transport_event_tx: mpsc::Sender<TransportEvent>,
        shutdown_token: &CancellationToken,
    ) -> Self {
        Self {
            clients: HashMap::new(),
            legacy_stream_ids: HashMap::new(),
            join_set: JoinSet::new(),
            server_event_tx,
            transport_event_tx,
            shutdown_token: shutdown_token.child_token(),
        }
    }

    pub(crate) async fn bookkeep_join_set(&mut self) -> Option<(ClientId, StreamId)> {
        while let Some(join_result) = self.join_set.join_next().await {
            let Ok(client_key) = join_result else {
                continue;
            };
            return Some(client_key);
        }
        futures::future::pending().await
    }

    pub(crate) async fn shutdown(&mut self) {
        self.shutdown_token.cancel();

        while let Some(client_key) = self.clients.keys().next().cloned() {
            let _ = self.close_client(&client_key).await;
        }

        self.drain_join_set().await;
    }

    async fn drain_join_set(&mut self) {
        while self.join_set.join_next().await.is_some() {}
    }

    pub(crate) async fn handle_message(
        &mut self,
        client_envelope: ClientEnvelope,
    ) -> Result<(), Stopped> {
        let ClientEnvelope {
            client_id,
            event,
            stream_id,
            seq_id,
            cursor: _,
        } = client_envelope;
        let is_legacy_stream_id = stream_id.is_none();
        let is_initialize = matches!(&event, ClientEvent::ClientMessage { message } if remote_control_message_starts_connection(message));
        let stream_id = match stream_id {
            Some(stream_id) => stream_id,
            None if is_initialize => {
                // TODO(ruslan): delete this fallback once all clients are updated to send stream_id.
                self.legacy_stream_ids
                    .remove(&client_id)
                    .unwrap_or_else(StreamId::new_random)
            }
            None => self
                .legacy_stream_ids
                .get(&client_id)
                .cloned()
                .unwrap_or_else(|| {
                    if matches!(&event, ClientEvent::Ping) {
                        StreamId::new_random()
                    } else {
                        StreamId(String::new())
                    }
                }),
        };
        if stream_id.0.is_empty() {
            return Ok(());
        }
        let client_key = (client_id.clone(), stream_id.clone());
        match event {
            ClientEvent::ClientMessage { message } => {
                if let Some(seq_id) = seq_id
                    && let Some(client) = self.clients.get(&client_key)
                    && client
                        .last_inbound_seq_id
                        .is_some_and(|last_seq_id| last_seq_id >= seq_id)
                    && !is_initialize
                {
                    return Ok(());
                }

                if is_initialize && self.clients.contains_key(&client_key) {
                    self.close_client(&client_key).await?;
                }

                if let Some(connection_id) = self.clients.get_mut(&client_key).map(|client| {
                    client.last_activity_at = Instant::now();
                    if let Some(seq_id) = seq_id {
                        client.last_inbound_seq_id = Some(seq_id);
                    }
                    client.connection_id
                }) {
                    self.send_transport_event(TransportEvent::IncomingMessage {
                        connection_id,
                        message,
                    })
                    .await?;
                    return Ok(());
                }

                if !is_initialize {
                    return Ok(());
                }

                let connection_id = next_connection_id();
                let (writer_tx, writer_rx) =
                    mpsc::channel::<QueuedOutgoingMessage>(CHANNEL_CAPACITY);
                let disconnect_token = self.shutdown_token.child_token();
                self.send_transport_event(TransportEvent::ConnectionOpened {
                    connection_id,
                    origin: ConnectionOrigin::RemoteControl,
                    writer: writer_tx,
                    disconnect_sender: Some(disconnect_token.clone()),
                })
                .await?;

                let (status_tx, status_rx) = watch::channel(PongStatus::Active);
                self.join_set.spawn(Self::run_client_outbound(
                    client_id.clone(),
                    stream_id.clone(),
                    self.server_event_tx.clone(),
                    writer_rx,
                    status_rx,
                    disconnect_token.clone(),
                ));
                self.clients.insert(
                    client_key,
                    ClientState {
                        connection_id,
                        disconnect_token,
                        last_activity_at: Instant::now(),
                        last_inbound_seq_id: if is_legacy_stream_id { None } else { seq_id },
                        status_tx,
                    },
                );
                if is_legacy_stream_id {
                    self.legacy_stream_ids.insert(client_id.clone(), stream_id);
                }
                self.send_transport_event(TransportEvent::IncomingMessage {
                    connection_id,
                    message,
                })
                .await
            }
            ClientEvent::Ack => Ok(()),
            ClientEvent::Ping => {
                if let Some(client) = self.clients.get_mut(&client_key) {
                    client.last_activity_at = Instant::now();
                    let _ = client.status_tx.send(PongStatus::Active);
                    return Ok(());
                }

                let server_event_tx = self.server_event_tx.clone();
                tokio::spawn(async move {
                    let server_envelope = QueuedServerEnvelope {
                        event: ServerEvent::Pong {
                            status: PongStatus::Unknown,
                        },
                        client_id,
                        stream_id,
                        write_complete_tx: None,
                    };
                    let _ = server_event_tx.send(server_envelope).await;
                });
                Ok(())
            }
            ClientEvent::ClientClosed => self.close_client(&client_key).await,
        }
    }

    async fn run_client_outbound(
        client_id: ClientId,
        stream_id: StreamId,
        server_event_tx: mpsc::Sender<QueuedServerEnvelope>,
        mut writer_rx: mpsc::Receiver<QueuedOutgoingMessage>,
        mut status_rx: watch::Receiver<PongStatus>,
        disconnect_token: CancellationToken,
    ) -> (ClientId, StreamId) {
        loop {
            let (event, write_complete_tx) = tokio::select! {
                _ = disconnect_token.cancelled() => {
                    break;
                }
                queued_message = writer_rx.recv() => {
                    let Some(queued_message) = queued_message else {
                        break;
                    };
                    let event = ServerEvent::ServerMessage {
                        message: Box::new(queued_message.message),
                    };
                    (event, queued_message.write_complete_tx)
                }
                changed = status_rx.changed() => {
                    if changed.is_err() {
                        break;
                    }
                    let event = ServerEvent::Pong { status: status_rx.borrow().clone() };
                    (event, None)
                }
            };
            let send_result = tokio::select! {
                _ = disconnect_token.cancelled() => {
                    break;
                }
                send_result = server_event_tx.send(QueuedServerEnvelope {
                    event,
                    client_id: client_id.clone(),
                    stream_id: stream_id.clone(),
                    write_complete_tx,
                }) => send_result,
            };
            if send_result.is_err() {
                break;
            }
        }
        (client_id, stream_id)
    }

    pub(crate) async fn close_expired_clients(
        &mut self,
    ) -> Result<Vec<(ClientId, StreamId)>, Stopped> {
        let now = Instant::now();
        let expired_client_ids: Vec<(ClientId, StreamId)> = self
            .clients
            .iter()
            .filter_map(|(client_key, client)| {
                (!remote_control_client_is_alive(client, now)).then_some(client_key.clone())
            })
            .collect();
        for client_key in &expired_client_ids {
            self.close_client(client_key).await?;
        }
        Ok(expired_client_ids)
    }

    pub(super) async fn close_client(
        &mut self,
        client_key: &(ClientId, StreamId),
    ) -> Result<(), Stopped> {
        let Some(client) = self.clients.remove(client_key) else {
            return Ok(());
        };
        if self
            .legacy_stream_ids
            .get(&client_key.0)
            .is_some_and(|stream_id| stream_id == &client_key.1)
        {
            self.legacy_stream_ids.remove(&client_key.0);
        }
        client.disconnect_token.cancel();
        self.send_transport_event(TransportEvent::ConnectionClosed {
            connection_id: client.connection_id,
        })
        .await
    }

    async fn send_transport_event(&self, event: TransportEvent) -> Result<(), Stopped> {
        self.transport_event_tx
            .send(event)
            .await
            .map_err(|_| Stopped)
    }
}

fn remote_control_message_starts_connection(message: &JSONRPCMessage) -> bool {
    matches!(
        message,
        JSONRPCMessage::Request(codex_app_server_protocol::JSONRPCRequest { method, .. })
            if method == "initialize"
    )
}

fn remote_control_client_is_alive(client: &ClientState, now: Instant) -> bool {
    now.duration_since(client.last_activity_at) < REMOTE_CONTROL_CLIENT_IDLE_TIMEOUT
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outgoing_message::OutgoingMessage;
    use crate::transport::remote_control::protocol::ClientEnvelope;
    use crate::transport::remote_control::protocol::ClientEvent;
    use codex_app_server_protocol::ConfigWarningNotification;
    use codex_app_server_protocol::JSONRPCRequest;
    use codex_app_server_protocol::RequestId;
    use codex_app_server_protocol::ServerNotification;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use tokio::time::timeout;

    fn initialize_envelope(client_id: &str) -> ClientEnvelope {
        initialize_envelope_with_stream_id(client_id, /*stream_id*/ None)
    }

    fn initialize_envelope_with_stream_id(
        client_id: &str,
        stream_id: Option<&str>,
    ) -> ClientEnvelope {
        ClientEnvelope {
            event: ClientEvent::ClientMessage {
                message: JSONRPCMessage::Request(JSONRPCRequest {
                    id: RequestId::Integer(1),
                    method: "initialize".to_string(),
                    params: Some(json!({
                        "clientInfo": {
                            "name": "remote-test-client",
                            "version": "0.1.0"
                        }
                    })),
                    trace: None,
                }),
            },
            client_id: ClientId(client_id.to_string()),
            stream_id: stream_id.map(|stream_id| StreamId(stream_id.to_string())),
            seq_id: Some(0),
            cursor: None,
        }
    }

    #[tokio::test]
    async fn cancelled_outbound_task_emits_connection_closed() {
        let (server_event_tx, _server_event_rx) = mpsc::channel(CHANNEL_CAPACITY);
        let (transport_event_tx, mut transport_event_rx) = mpsc::channel(CHANNEL_CAPACITY);
        let shutdown_token = CancellationToken::new();
        let mut client_tracker =
            ClientTracker::new(server_event_tx, transport_event_tx, &shutdown_token);

        client_tracker
            .handle_message(initialize_envelope("client-1"))
            .await
            .expect("initialize should open client");

        let (connection_id, disconnect_sender) = match transport_event_rx
            .recv()
            .await
            .expect("connection opened should be sent")
        {
            TransportEvent::ConnectionOpened {
                connection_id,
                disconnect_sender: Some(disconnect_sender),
                ..
            } => (connection_id, disconnect_sender),
            other => panic!("expected connection opened, got {other:?}"),
        };
        match transport_event_rx
            .recv()
            .await
            .expect("initialize should be forwarded")
        {
            TransportEvent::IncomingMessage {
                connection_id: incoming_connection_id,
                ..
            } => assert_eq!(incoming_connection_id, connection_id),
            other => panic!("expected incoming initialize, got {other:?}"),
        }

        disconnect_sender.cancel();
        let closed_client_id = timeout(Duration::from_secs(1), client_tracker.bookkeep_join_set())
            .await
            .expect("bookkeeping should process the closed task")
            .expect("closed task should return client id");
        assert_eq!(closed_client_id.0, ClientId("client-1".to_string()));
        client_tracker
            .close_client(&closed_client_id)
            .await
            .expect("closed client should emit connection closed");

        match transport_event_rx
            .recv()
            .await
            .expect("connection closed should be sent")
        {
            TransportEvent::ConnectionClosed {
                connection_id: closed_connection_id,
            } => assert_eq!(closed_connection_id, connection_id),
            other => panic!("expected connection closed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn shutdown_cancels_blocked_outbound_forwarding() {
        let (server_event_tx, _server_event_rx) = mpsc::channel(1);
        let (transport_event_tx, mut transport_event_rx) = mpsc::channel(CHANNEL_CAPACITY);
        let shutdown_token = CancellationToken::new();
        let mut client_tracker =
            ClientTracker::new(server_event_tx.clone(), transport_event_tx, &shutdown_token);

        server_event_tx
            .send(QueuedServerEnvelope {
                event: ServerEvent::Pong {
                    status: PongStatus::Unknown,
                },
                client_id: ClientId("queued-client".to_string()),
                stream_id: StreamId("queued-stream".to_string()),
                write_complete_tx: None,
            })
            .await
            .expect("server event queue should accept prefill");

        client_tracker
            .handle_message(initialize_envelope("client-1"))
            .await
            .expect("initialize should open client");

        let writer = match transport_event_rx
            .recv()
            .await
            .expect("connection opened should be sent")
        {
            TransportEvent::ConnectionOpened { writer, .. } => writer,
            other => panic!("expected connection opened, got {other:?}"),
        };
        let _ = transport_event_rx
            .recv()
            .await
            .expect("initialize should be forwarded");

        writer
            .send(QueuedOutgoingMessage::new(
                OutgoingMessage::AppServerNotification(ServerNotification::ConfigWarning(
                    ConfigWarningNotification {
                        summary: "test".to_string(),
                        details: None,
                        path: None,
                        range: None,
                    },
                )),
            ))
            .await
            .expect("writer should accept queued message");

        timeout(Duration::from_secs(1), client_tracker.shutdown())
            .await
            .expect("shutdown should not hang on blocked server forwarding");
    }

    #[tokio::test]
    async fn initialize_with_new_stream_id_opens_new_connection_for_same_client() {
        let (server_event_tx, _server_event_rx) = mpsc::channel(CHANNEL_CAPACITY);
        let (transport_event_tx, mut transport_event_rx) = mpsc::channel(CHANNEL_CAPACITY);
        let shutdown_token = CancellationToken::new();
        let mut client_tracker =
            ClientTracker::new(server_event_tx, transport_event_tx, &shutdown_token);

        client_tracker
            .handle_message(initialize_envelope_with_stream_id(
                "client-1",
                Some("stream-1"),
            ))
            .await
            .expect("first initialize should open client");
        let first_connection_id = match transport_event_rx.recv().await.expect("open event") {
            TransportEvent::ConnectionOpened { connection_id, .. } => connection_id,
            other => panic!("expected connection opened, got {other:?}"),
        };
        let _ = transport_event_rx.recv().await.expect("initialize event");

        client_tracker
            .handle_message(initialize_envelope_with_stream_id(
                "client-1",
                Some("stream-2"),
            ))
            .await
            .expect("second initialize should open client");
        let second_connection_id = match transport_event_rx.recv().await.expect("open event") {
            TransportEvent::ConnectionOpened { connection_id, .. } => connection_id,
            other => panic!("expected connection opened, got {other:?}"),
        };

        assert_ne!(first_connection_id, second_connection_id);
    }

    #[tokio::test]
    async fn legacy_initialize_without_stream_id_resets_inbound_seq_id() {
        let (server_event_tx, _server_event_rx) = mpsc::channel(CHANNEL_CAPACITY);
        let (transport_event_tx, mut transport_event_rx) = mpsc::channel(CHANNEL_CAPACITY);
        let shutdown_token = CancellationToken::new();
        let mut client_tracker =
            ClientTracker::new(server_event_tx, transport_event_tx, &shutdown_token);

        client_tracker
            .handle_message(initialize_envelope("client-1"))
            .await
            .expect("initialize should open client");
        let connection_id = match transport_event_rx.recv().await.expect("open event") {
            TransportEvent::ConnectionOpened { connection_id, .. } => connection_id,
            other => panic!("expected connection opened, got {other:?}"),
        };
        let _ = transport_event_rx.recv().await.expect("initialize event");

        client_tracker
            .handle_message(ClientEnvelope {
                event: ClientEvent::ClientMessage {
                    message: JSONRPCMessage::Notification(
                        codex_app_server_protocol::JSONRPCNotification {
                            method: "initialized".to_string(),
                            params: None,
                        },
                    ),
                },
                client_id: ClientId("client-1".to_string()),
                stream_id: None,
                seq_id: Some(0),
                cursor: None,
            })
            .await
            .expect("legacy followup should be forwarded");

        match transport_event_rx.recv().await.expect("followup event") {
            TransportEvent::IncomingMessage {
                connection_id: incoming_connection_id,
                ..
            } => assert_eq!(incoming_connection_id, connection_id),
            other => panic!("expected incoming message, got {other:?}"),
        }
    }
}
