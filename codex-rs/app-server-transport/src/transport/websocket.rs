use super::CHANNEL_CAPACITY;
use super::ConnectionOrigin;
use super::TransportEvent;
use super::forward_incoming_message;
use super::next_connection_id;
use super::serialize_outgoing_message;
use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::QueuedOutgoingMessage;
use futures::SinkExt;
use futures::StreamExt;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Bytes;
use tokio_tungstenite::tungstenite::Message as TungsteniteWebSocketMessage;
use tokio_util::sync::CancellationToken;
use tracing::warn;

/// WebSocket clients can briefly lag behind normal turn output bursts while the
/// writer task is healthy, so give them more headroom than internal channels.
const WEBSOCKET_OUTBOUND_CHANNEL_CAPACITY: usize = 32 * 1024;
const _: () = assert!(WEBSOCKET_OUTBOUND_CHANNEL_CAPACITY > CHANNEL_CAPACITY);

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
    let (writer_tx, writer_rx) =
        mpsc::channel::<QueuedOutgoingMessage>(WEBSOCKET_OUTBOUND_CHANNEL_CAPACITY);
    let writer_tx_for_reader = writer_tx.clone();
    let disconnect_token = CancellationToken::new();
    if transport_event_tx
        .send(TransportEvent::ConnectionOpened {
            connection_id,
            origin: ConnectionOrigin::UnixSocket,
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
