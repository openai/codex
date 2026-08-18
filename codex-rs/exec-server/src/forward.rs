use std::time::Duration;

use bytes::Bytes;
use codex_http_client::HttpClientFactory;
use codex_websocket_client::WebSocketConnector;
use futures::Sink;
use futures::SinkExt;
use futures::StreamExt;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::protocol::frame::Frame;
use tokio_tungstenite::tungstenite::protocol::frame::coding::Data;
use tokio_tungstenite::tungstenite::protocol::frame::coding::OpCode;
use tokio_util::task::AbortOnDropHandle;
use tracing::warn;

use crate::ExecServerError;
use crate::ExecServerTelemetry;
use crate::client_api::DEFAULT_REMOTE_EXEC_SERVER_CONNECT_TIMEOUT;
use crate::noise_relay::message_framing::MAX_NOISE_JSONRPC_MESSAGE_LEN;
use crate::noise_relay::message_framing::frame_message;
use crate::noise_relay::stream_handler::NoiseOutboundMessage;
use crate::noise_relay::stream_handler::NoiseStreamConnection;
use crate::noise_relay::stream_handler::NoiseStreamHandler;
use crate::telemetry::ConnectionTransport;

// Existing exec-server listeners accept 64 MiB messages but only 16 MiB frames.
const WEBSOCKET_FRAGMENT_LEN: usize = 8 * 1024 * 1024;
const WEBSOCKET_CLOSE_TIMEOUT: Duration = Duration::from_secs(1);

async fn send_websocket_message<S>(websocket: &mut S, mut payload: Bytes) -> Result<(), S::Error>
where
    S: Sink<Message> + Unpin,
{
    if payload.len() <= WEBSOCKET_FRAGMENT_LEN {
        return websocket.send(Message::Binary(payload)).await;
    }

    let mut opcode = OpCode::Data(Data::Binary);
    while !payload.is_empty() {
        let chunk = payload.split_to(payload.len().min(WEBSOCKET_FRAGMENT_LEN));
        let is_final = payload.is_empty();
        websocket
            .send(Message::Frame(Frame::message(chunk, opcode, is_final)))
            .await?;
        opcode = OpCode::Data(Data::Continue);
    }
    Ok(())
}

/// Copies authenticated remote messages to an independently owned executor.
#[derive(Clone)]
pub(crate) struct Forwarder {
    websocket_url: String,
    connector: WebSocketConnector,
    telemetry: ExecServerTelemetry,
}

impl Forwarder {
    pub(crate) fn new(
        websocket_url: String,
        http_client_factory: &HttpClientFactory,
        telemetry: ExecServerTelemetry,
    ) -> Result<Self, ExecServerError> {
        let url = url::Url::parse(&websocket_url)
            .map_err(|error| ExecServerError::WebSocketConfiguration(error.to_string()))?;
        if !matches!(url.scheme(), "ws" | "wss") || url.host_str().is_none() {
            return Err(ExecServerError::WebSocketConfiguration(
                "forward destination must be a ws:// or wss:// URL".to_string(),
            ));
        }
        let connector = WebSocketConnector::new(http_client_factory)
            .map_err(|error| ExecServerError::WebSocketConfiguration(error.to_string()))?
            .with_tcp_nodelay();
        Ok(Self {
            websocket_url,
            connector,
            telemetry,
        })
    }

    pub(crate) async fn run_connection(self, mut remote: NoiseStreamConnection<Bytes, Bytes>) {
        let mut writer_task = AbortOnDropHandle::new(remote.writer_task);
        let _metrics = self
            .telemetry
            .connection_started(ConnectionTransport::Relay);
        let connect = async {
            let request = self.websocket_url.as_str().into_client_request()?;
            self.connector
                .connect(
                    request,
                    WebSocketConfig::default()
                        .max_message_size(Some(MAX_NOISE_JSONRPC_MESSAGE_LEN))
                        .max_frame_size(Some(MAX_NOISE_JSONRPC_MESSAGE_LEN)),
                )
                .await
        };
        let connected = tokio::select! {
            biased;
            _ = remote.disconnected_rx.wait_for(|disconnected| *disconnected) => None,
            result = timeout(DEFAULT_REMOTE_EXEC_SERVER_CONNECT_TIMEOUT, connect) => {
                match result {
                    Ok(Ok((websocket, _))) => Some(websocket),
                    Ok(Err(_)) => {
                        warn!("failed to connect to forwarded exec-server");
                        None
                    }
                    Err(_) => {
                        warn!("timed out connecting to forwarded exec-server");
                        None
                    }
                }
            }
        };
        let drain_outgoing = if let Some(websocket) = connected {
            let (mut destination_tx, mut destination_rx) = websocket.split();
            let to_destination = async {
                while let Some(payload) = remote.incoming_rx.recv().await {
                    if send_websocket_message(&mut destination_tx, payload)
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            };
            let from_destination = async {
                while let Some(Ok(message)) = destination_rx.next().await {
                    let payload = match message {
                        Message::Text(_) | Message::Binary(_) => message.into_data(),
                        Message::Close(_) => return true,
                        Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => continue,
                    };
                    if remote.outgoing_tx.send(payload).await.is_err() {
                        break;
                    }
                }
                false
            };
            let (drain_outgoing, received_close) = tokio::select! {
                _ = remote.disconnected_rx.wait_for(|disconnected| *disconnected) => (false, false),
                _ = to_destination => (false, false),
                received_close = from_destination => (true, received_close),
            };
            if received_close && let Ok(mut websocket) = destination_tx.reunite(destination_rx) {
                // Reuniting drops any canceled application send before flushing
                // Tungstenite's automatically queued Close acknowledgement.
                tokio::select! {
                    _ = timeout(WEBSOCKET_CLOSE_TIMEOUT, websocket.flush()) => {},
                    _ = remote.disconnected_rx.wait_for(|disconnected| *disconnected) => {},
                }
            }
            drain_outgoing
        } else {
            false
        };
        // Preserve messages received before the destination's Close. Never wait
        // for a dead remote, and let owner shutdown interrupt a blocked drain.
        drop(remote.outgoing_tx);
        if drain_outgoing {
            tokio::select! {
                _ = &mut writer_task => return,
                _ = remote.disconnected_rx.wait_for(|disconnected| *disconnected) => {},
            }
        }
        writer_task.abort();
        let _ = writer_task.await;
    }
}

impl NoiseStreamHandler for Forwarder {
    type Incoming = Bytes;
    type Outgoing = Bytes;

    fn decode(payload: Bytes) -> Result<Bytes, ExecServerError> {
        Ok(payload)
    }

    fn encode(payload: Bytes) -> Result<NoiseOutboundMessage, ExecServerError> {
        Ok(NoiseOutboundMessage {
            framed: frame_message(&payload)?,
            trace: None,
        })
    }

    async fn run_connection(self, connection: NoiseStreamConnection<Bytes, Bytes>) {
        Forwarder::run_connection(self, connection).await;
    }
}

#[cfg(test)]
#[path = "forward_tests.rs"]
mod tests;
