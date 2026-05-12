use std::collections::HashMap;

use codex_app_server_protocol::JSONRPCMessage;
use futures::SinkExt;
use futures::StreamExt;
use prost::Message as ProstMessage;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;
use tracing::debug;
use tracing::warn;
use uuid::Uuid;

use crate::ExecServerError;
use crate::connection::CHANNEL_CAPACITY;
use crate::connection::JsonRpcConnection;
use crate::connection::JsonRpcConnectionEvent;
use crate::connection::JsonRpcTransport;
use crate::relay_proto::RelayData;
use crate::relay_proto::RelayMessageFrame;
use crate::relay_proto::RelayResume;
use crate::relay_proto::relay_message_frame;
use crate::server::ConnectionProcessor;

const RELAY_MESSAGE_FRAME_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum RelayFrameBodyKind {
    Data,
    Ack,
    Resume,
    Reset,
    Heartbeat,
}

impl RelayMessageFrame {
    fn data(stream_id: String, seq: u32, payload: Vec<u8>) -> Self {
        Self {
            version: RELAY_MESSAGE_FRAME_VERSION,
            stream_id,
            ack: 0,
            ack_bits: 0,
            body: Some(relay_message_frame::Body::Data(RelayData {
                seq,
                segment_index: 0,
                segment_count: 1,
                payload,
            })),
        }
    }

    fn resume(stream_id: String) -> Self {
        Self {
            version: RELAY_MESSAGE_FRAME_VERSION,
            stream_id,
            ack: 0,
            ack_bits: 0,
            body: Some(relay_message_frame::Body::Resume(RelayResume {
                next_seq: 0,
            })),
        }
    }

    fn validate(&self) -> Result<RelayFrameBodyKind, ExecServerError> {
        if self.version != RELAY_MESSAGE_FRAME_VERSION {
            return Err(ExecServerError::Protocol(format!(
                "unsupported relay message frame version {}",
                self.version
            )));
        }
        if self.stream_id.trim().is_empty() {
            return Err(ExecServerError::Protocol(
                "relay message frame is missing stream_id".to_string(),
            ));
        }
        match self.body.as_ref() {
            Some(relay_message_frame::Body::Data(data)) => {
                if data.segment_index != 0 || data.segment_count != 1 || data.payload.is_empty() {
                    return Err(ExecServerError::Protocol(
                        "relay data message frame is missing required fields".to_string(),
                    ));
                }
                Ok(RelayFrameBodyKind::Data)
            }
            Some(relay_message_frame::Body::AckFrame(_)) => Ok(RelayFrameBodyKind::Ack),
            Some(relay_message_frame::Body::Resume(_)) => Ok(RelayFrameBodyKind::Resume),
            Some(relay_message_frame::Body::Reset(reset)) => {
                if reset.reason.is_empty() {
                    return Err(ExecServerError::Protocol(
                        "relay reset message frame is missing reason".to_string(),
                    ));
                }
                Ok(RelayFrameBodyKind::Reset)
            }
            Some(relay_message_frame::Body::Heartbeat(_)) => Ok(RelayFrameBodyKind::Heartbeat),
            None => Err(ExecServerError::Protocol(
                "relay message frame is missing body".to_string(),
            )),
        }
    }

    fn into_jsonrpc_message(self) -> Result<JSONRPCMessage, ExecServerError> {
        let kind = self.validate()?;
        if kind != RelayFrameBodyKind::Data {
            return Err(ExecServerError::Protocol(
                "expected relay data message frame".to_string(),
            ));
        }
        let payload = match self.body {
            Some(relay_message_frame::Body::Data(data)) => data.payload,
            _ => Vec::new(),
        };
        serde_json::from_slice(&payload).map_err(ExecServerError::Json)
    }

    fn into_reset_reason(self) -> Option<String> {
        match self.body {
            Some(relay_message_frame::Body::Reset(reset)) if !reset.reason.is_empty() => {
                Some(reset.reason)
            }
            _ => None,
        }
    }
}

fn encode_relay_message_frame(frame: &RelayMessageFrame) -> Vec<u8> {
    frame.encode_to_vec()
}

fn decode_relay_message_frame(payload: &[u8]) -> Result<RelayMessageFrame, ExecServerError> {
    RelayMessageFrame::decode(payload)
        .map_err(|err| ExecServerError::Protocol(format!("invalid relay message frame: {err}")))
}

fn jsonrpc_payload(message: &JSONRPCMessage) -> Result<Vec<u8>, ExecServerError> {
    serde_json::to_vec(message).map_err(ExecServerError::Json)
}

pub(crate) fn harness_connection_from_websocket<S>(
    stream: WebSocketStream<S>,
    connection_label: String,
) -> JsonRpcConnection
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let stream_id = Uuid::new_v4().to_string();
    let (mut websocket_writer, mut websocket_reader) = stream.split();
    let (outgoing_tx, mut outgoing_rx) = mpsc::channel(CHANNEL_CAPACITY);
    let (incoming_tx, incoming_rx) = mpsc::channel(CHANNEL_CAPACITY);
    let (disconnected_tx, disconnected_rx) = watch::channel(false);

    let reader_label = connection_label;
    let reader_stream_id = stream_id.clone();
    let incoming_tx_for_reader = incoming_tx;
    let disconnected_tx_for_reader = disconnected_tx.clone();
    let reader_task = tokio::spawn(async move {
        loop {
            match websocket_reader.next().await {
                Some(Ok(Message::Binary(payload))) => {
                    let frame = match decode_relay_message_frame(payload.as_ref()) {
                        Ok(frame) => frame,
                        Err(err) => {
                            let _ = incoming_tx_for_reader
                                .send(JsonRpcConnectionEvent::MalformedMessage {
                                    reason: format!(
                                        "failed to parse relay message frame from {reader_label}: {err}"
                                    ),
                                })
                                .await;
                            continue;
                        }
                    };
                    if frame.stream_id != reader_stream_id {
                        continue;
                    }
                    let kind = match frame.validate() {
                        Ok(kind) => kind,
                        Err(err) => {
                            let _ = incoming_tx_for_reader
                                .send(JsonRpcConnectionEvent::MalformedMessage {
                                    reason: err.to_string(),
                                })
                                .await;
                            continue;
                        }
                    };
                    match kind {
                        RelayFrameBodyKind::Data => match frame.into_jsonrpc_message() {
                            Ok(message) => {
                                if incoming_tx_for_reader
                                    .send(JsonRpcConnectionEvent::Message(message))
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            Err(err) => {
                                let _ = incoming_tx_for_reader
                                    .send(JsonRpcConnectionEvent::MalformedMessage {
                                        reason: err.to_string(),
                                    })
                                    .await;
                            }
                        },
                        RelayFrameBodyKind::Reset => {
                            let _ = disconnected_tx_for_reader.send(true);
                            let _ = incoming_tx_for_reader
                                .send(JsonRpcConnectionEvent::Disconnected {
                                    reason: frame.into_reset_reason(),
                                })
                                .await;
                            break;
                        }
                        RelayFrameBodyKind::Ack
                        | RelayFrameBodyKind::Resume
                        | RelayFrameBodyKind::Heartbeat => {}
                    }
                }
                Some(Ok(Message::Close(_))) | None => {
                    let _ = disconnected_tx_for_reader.send(true);
                    let _ = incoming_tx_for_reader
                        .send(JsonRpcConnectionEvent::Disconnected { reason: None })
                        .await;
                    break;
                }
                Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_))) => {}
                Some(Ok(Message::Text(_))) => {
                    let _ = incoming_tx_for_reader
                        .send(JsonRpcConnectionEvent::MalformedMessage {
                            reason: "relay exec-server transport expects binary protobuf frames"
                                .to_string(),
                        })
                        .await;
                }
                Some(Err(err)) => {
                    let _ = disconnected_tx_for_reader.send(true);
                    let _ = incoming_tx_for_reader
                        .send(JsonRpcConnectionEvent::Disconnected {
                            reason: Some(format!(
                                "failed to read relay websocket frame from {reader_label}: {err}"
                            )),
                        })
                        .await;
                    break;
                }
            }
        }
    });

    let writer_task = tokio::spawn(async move {
        let resume = RelayMessageFrame::resume(stream_id.clone());
        if websocket_writer
            .send(Message::Binary(encode_relay_message_frame(&resume).into()))
            .await
            .is_err()
        {
            let _ = disconnected_tx.send(true);
            return;
        }

        let mut next_seq = 0u32;
        while let Some(message) = outgoing_rx.recv().await {
            let payload = match jsonrpc_payload(&message) {
                Ok(payload) => payload,
                Err(err) => {
                    warn!("failed to serialize JSON-RPC payload for relay transport: {err}");
                    break;
                }
            };
            let frame = RelayMessageFrame::data(stream_id.clone(), next_seq, payload);
            next_seq = next_seq.wrapping_add(1);
            if websocket_writer
                .send(Message::Binary(encode_relay_message_frame(&frame).into()))
                .await
                .is_err()
            {
                let _ = disconnected_tx.send(true);
                break;
            }
        }
    });

    JsonRpcConnection {
        outgoing_tx,
        incoming_rx,
        disconnected_rx,
        task_handles: vec![reader_task, writer_task],
        transport: JsonRpcTransport::Plain,
    }
}

pub(crate) async fn run_multiplexed_executor<S>(
    stream: WebSocketStream<S>,
    processor: ConnectionProcessor,
) where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (mut websocket_writer, mut websocket_reader) = stream.split();
    let (physical_outgoing_tx, mut physical_outgoing_rx) =
        mpsc::channel::<Vec<u8>>(CHANNEL_CAPACITY);
    let writer_task = tokio::spawn(async move {
        while let Some(encoded) = physical_outgoing_rx.recv().await {
            if websocket_writer
                .send(Message::Binary(encoded.into()))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    let mut streams: HashMap<String, VirtualStream> = HashMap::new();
    loop {
        let frame = match websocket_reader.next().await {
            Some(Ok(Message::Binary(payload))) => {
                match decode_relay_message_frame(payload.as_ref()) {
                    Ok(frame) => frame,
                    Err(err) => {
                        warn!("dropping malformed relay message frame from harness: {err}");
                        continue;
                    }
                }
            }
            Some(Ok(Message::Close(_))) | None => break,
            Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_))) => continue,
            Some(Ok(Message::Text(_))) => {
                warn!("dropping non-binary relay message frame from harness");
                continue;
            }
            Some(Err(err)) => {
                debug!("multiplexed executor websocket read failed: {err}");
                break;
            }
        };

        let kind = match frame.validate() {
            Ok(kind) => kind,
            Err(err) => {
                warn!("dropping invalid relay message frame: {err}");
                continue;
            }
        };

        match kind {
            RelayFrameBodyKind::Data => {
                let stream_id = frame.stream_id.clone();
                let message = match frame.into_jsonrpc_message() {
                    Ok(message) => message,
                    Err(err) => {
                        warn!("dropping malformed relay data message frame: {err}");
                        continue;
                    }
                };
                let stream = streams.entry(stream_id.clone()).or_insert_with(|| {
                    spawn_virtual_stream(
                        stream_id.clone(),
                        processor.clone(),
                        physical_outgoing_tx.clone(),
                    )
                });
                if stream
                    .incoming_tx
                    .send(JsonRpcConnectionEvent::Message(message))
                    .await
                    .is_err()
                {
                    streams.remove(&stream_id);
                }
            }
            RelayFrameBodyKind::Reset => {
                if let Some(stream) = streams.remove(&frame.stream_id) {
                    stream.disconnect(frame.into_reset_reason()).await;
                }
            }
            RelayFrameBodyKind::Ack
            | RelayFrameBodyKind::Resume
            | RelayFrameBodyKind::Heartbeat => {}
        }
    }

    for (_stream_id, stream) in streams {
        stream.disconnect(/*reason*/ None).await;
    }
    drop(physical_outgoing_tx);
    let _ = writer_task.await;
}

struct VirtualStream {
    incoming_tx: mpsc::Sender<JsonRpcConnectionEvent>,
    disconnected_tx: watch::Sender<bool>,
}

impl VirtualStream {
    async fn disconnect(self, reason: Option<String>) {
        let _ = self.disconnected_tx.send(true);
        let _ = self
            .incoming_tx
            .send(JsonRpcConnectionEvent::Disconnected { reason })
            .await;
    }
}

fn spawn_virtual_stream(
    stream_id: String,
    processor: ConnectionProcessor,
    physical_outgoing_tx: mpsc::Sender<Vec<u8>>,
) -> VirtualStream {
    let (json_outgoing_tx, mut json_outgoing_rx) = mpsc::channel(CHANNEL_CAPACITY);
    let (incoming_tx, incoming_rx) = mpsc::channel(CHANNEL_CAPACITY);
    let (disconnected_tx, disconnected_rx) = watch::channel(false);

    let writer_stream_id = stream_id;
    let writer_task = tokio::spawn(async move {
        let mut next_seq = 0u32;
        while let Some(message) = json_outgoing_rx.recv().await {
            let payload = match jsonrpc_payload(&message) {
                Ok(payload) => payload,
                Err(err) => {
                    warn!("failed to serialize virtual stream JSON-RPC payload: {err}");
                    break;
                }
            };
            let frame = RelayMessageFrame::data(writer_stream_id.clone(), next_seq, payload);
            next_seq = next_seq.wrapping_add(1);
            if physical_outgoing_tx
                .send(encode_relay_message_frame(&frame))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    let connection = JsonRpcConnection {
        outgoing_tx: json_outgoing_tx,
        incoming_rx,
        disconnected_rx,
        task_handles: vec![writer_task],
        transport: JsonRpcTransport::Plain,
    };
    tokio::spawn(async move {
        processor.run_connection(connection).await;
    });

    VirtualStream {
        incoming_tx,
        disconnected_tx,
    }
}
