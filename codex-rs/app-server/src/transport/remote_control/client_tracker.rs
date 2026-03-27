use super::CHANNEL_CAPACITY;
use super::TransportEvent;
use super::next_connection_id;
use super::protocol::ClientEnvelope;
pub use super::protocol::ClientEvent;
pub use super::protocol::ClientId;
use super::protocol::PongStatus;
use super::protocol::ServerEvent;
use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::QueuedOutgoingMessage;
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
    clients: HashMap<ClientId, ClientState>,
    join_set: JoinSet<ClientId>,
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
            join_set: JoinSet::new(),
            server_event_tx,
            transport_event_tx,
            shutdown_token: shutdown_token.child_token(),
        }
    }

    pub(crate) async fn bookkeep_join_set(&mut self) -> Option<ClientId> {
        while let Some(join_result) = self.join_set.join_next().await {
            let Ok(client_id) = join_result else {
                continue;
            };
            return Some(client_id);
        }
        futures::future::pending().await
    }

    pub(crate) async fn shutdown(&mut self) {
        self.shutdown_token.cancel();

        while let Some(client_id) = self.clients.keys().next().cloned() {
            let _ = self.close_client(&client_id).await;
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
            seq_id,
            cursor: _,
        } = client_envelope;
        match event {
            ClientEvent::ClientMessage { message } => {
                let is_initialize = remote_control_message_starts_connection(&message);
                if let Some(seq_id) = seq_id
                    && let Some(client) = self.clients.get(&client_id)
                    && client
                        .last_inbound_seq_id
                        .is_some_and(|last_seq_id| last_seq_id >= seq_id)
                    && !is_initialize
                {
                    return Ok(());
                }

                if is_initialize && self.clients.contains_key(&client_id) {
                    self.close_client(&client_id).await?;
                }

                if let Some(connection_id) = self.clients.get_mut(&client_id).map(|client| {
                    client.last_activity_at = Instant::now();
                    if let Some(seq_id) = seq_id {
                        client.last_inbound_seq_id = Some(seq_id);
                    }
                    client.connection_id
                }) {
                    self.transport_event_tx
                        .send(TransportEvent::IncomingMessage {
                            connection_id,
                            message,
                        })
                        .await
                        .map_err(|_| Stopped)?;
                    return Ok(());
                }

                if !is_initialize {
                    return Ok(());
                }

                let connection_id = next_connection_id();
                let (writer_tx, writer_rx) =
                    mpsc::channel::<QueuedOutgoingMessage>(CHANNEL_CAPACITY);
                let disconnect_token = self.shutdown_token.child_token();
                self.transport_event_tx
                    .send(TransportEvent::ConnectionOpened {
                        connection_id,
                        writer: writer_tx,
                        disconnect_sender: Some(disconnect_token.clone()),
                    })
                    .await
                    .map_err(|_| Stopped)?;

                let (status_tx, status_rx) = watch::channel(PongStatus::Active);
                self.join_set.spawn(Self::run_client_outbound(
                    client_id.clone(),
                    self.server_event_tx.clone(),
                    writer_rx,
                    status_rx,
                    disconnect_token.clone(),
                ));
                self.clients.insert(
                    client_id,
                    ClientState {
                        connection_id,
                        disconnect_token,
                        last_activity_at: Instant::now(),
                        last_inbound_seq_id: seq_id,
                        status_tx,
                    },
                );
                self.send_transport_event(TransportEvent::IncomingMessage {
                    connection_id,
                    message,
                })
                .await
            }
            ClientEvent::Ack => Ok(()),
            ClientEvent::Ping => {
                if let Some(client) = self.clients.get_mut(&client_id) {
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
                        write_complete_tx: None,
                    };
                    let _ = server_event_tx.send(server_envelope).await;
                });
                Ok(())
            }
            ClientEvent::ClientClosed => self.close_client(&client_id).await,
        }
    }

    async fn run_client_outbound(
        client_id: ClientId,
        server_event_tx: mpsc::Sender<QueuedServerEnvelope>,
        mut writer_rx: mpsc::Receiver<QueuedOutgoingMessage>,
        mut status_rx: watch::Receiver<PongStatus>,
        disconnect_token: CancellationToken,
    ) -> ClientId {
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
                    write_complete_tx,
                }) => send_result,
            };
            if send_result.is_err() {
                break;
            }
        }
        client_id
    }

    pub(crate) async fn close_expired_clients(&mut self) -> Result<Vec<ClientId>, Stopped> {
        let now = Instant::now();
        let expired_client_ids: Vec<ClientId> = self
            .clients
            .iter()
            .filter_map(|(client_id, client)| {
                (!remote_control_client_is_alive(client, now)).then_some(client_id.clone())
            })
            .collect();
        for client_id in &expired_client_ids {
            self.close_client(client_id).await?;
        }
        Ok(expired_client_ids)
    }

    pub(super) async fn close_client(&mut self, client_id: &ClientId) -> Result<(), Stopped> {
        let Some(client) = self.clients.remove(client_id) else {
            return Ok(());
        };
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
        assert_eq!(closed_client_id, ClientId("client-1".to_string()));
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
}
