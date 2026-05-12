use std::collections::HashMap;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_app_server_protocol::JSONRPCMessage;
use futures::SinkExt;
use futures::StreamExt;
use serde::Deserialize;
use serde::Serialize;
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
use crate::server::ConnectionProcessor;

const RELAY_MESSAGE_FRAME_VERSION: u32 = 1;
#[derive(Debug, Clone, Copy, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum RelayMessageFrameKind {
    Data,
    Ack,
    Resume,
    Reset,
    Heartbeat,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RelayMessageFrame {
    version: u32,
    stream_id: String,
    kind: RelayMessageFrameKind,
    ack: u32,
    ack_bits: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    seq: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    segment_index: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    segment_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    payload_base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

impl RelayMessageFrame {
    fn data(stream_id: String, seq: u32, payload: Vec<u8>) -> Self {
        Self {
            version: RELAY_MESSAGE_FRAME_VERSION,
            stream_id,
            kind: RelayMessageFrameKind::Data,
            ack: 0,
            ack_bits: 0,
            seq: Some(seq),
            segment_index: Some(0),
            segment_count: Some(1),
            payload_base64: Some(BASE64_STANDARD.encode(payload)),
            reason: None,
        }
    }

    fn resume(stream_id: String) -> Self {
        Self {
            version: RELAY_MESSAGE_FRAME_VERSION,
            stream_id,
            kind: RelayMessageFrameKind::Resume,
            ack: 0,
            ack_bits: 0,
            seq: None,
            segment_index: None,
            segment_count: None,
            payload_base64: None,
            reason: None,
        }
    }

    fn validate(&self) -> Result<(), ExecServerError> {
        if self.version != RELAY_MESSAGE_FRAME_VERSION {
            return Err(ExecServerError::Protocol(format!(
                "unsupported relay message frame version {}",
                self.version
            )));
        }
        if self.stream_id.trim().is_empty() {
            return Err(ExecServerError::Protocol(
                "relay message frame is missing streamId".to_string(),
            ));
        }
        if self.kind == RelayMessageFrameKind::Data
            && (self.seq.is_none()
                || self.segment_index != Some(0)
                || self.segment_count != Some(1)
                || self.payload_base64.is_none())
        {
            return Err(ExecServerError::Protocol(
                "relay data message frame is missing required fields".to_string(),
            ));
        }
        if self.kind == RelayMessageFrameKind::Reset && self.reason.is_none() {
            return Err(ExecServerError::Protocol(
                "relay reset message frame is missing reason".to_string(),
            ));
        }
        Ok(())
    }

    fn into_jsonrpc_message(self) -> Result<JSONRPCMessage, ExecServerError> {
        self.validate()?;
        if self.kind != RelayMessageFrameKind::Data {
            return Err(ExecServerError::Protocol(
                "expected relay data message frame".to_string(),
            ));
        }
        let payload = BASE64_STANDARD
            .decode(self.payload_base64.unwrap_or_default())
            .map_err(|err| ExecServerError::Protocol(format!("invalid payloadBase64: {err}")))?;
        serde_json::from_slice(&payload).map_err(ExecServerError::Json)
    }
}

fn serialize_relay_message_frame(frame: &RelayMessageFrame) -> Result<String, ExecServerError> {
    serde_json::to_string(frame).map_err(ExecServerError::Json)
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
                Some(Ok(Message::Text(text))) => {
                    let frame = match serde_json::from_str::<RelayMessageFrame>(text.as_ref()) {
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
                    match frame.kind {
                        RelayMessageFrameKind::Data => match frame.into_jsonrpc_message() {
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
                        RelayMessageFrameKind::Reset => {
                            let _ = disconnected_tx_for_reader.send(true);
                            let _ = incoming_tx_for_reader
                                .send(JsonRpcConnectionEvent::Disconnected {
                                    reason: frame.reason,
                                })
                                .await;
                            break;
                        }
                        RelayMessageFrameKind::Ack
                        | RelayMessageFrameKind::Resume
                        | RelayMessageFrameKind::Heartbeat => {}
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
                Some(Ok(Message::Binary(_))) => {
                    let _ = incoming_tx_for_reader
                        .send(JsonRpcConnectionEvent::MalformedMessage {
                            reason: "relay exec-server transport expects JSON text frames"
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
        match serialize_relay_message_frame(&resume) {
            Ok(encoded) => {
                if websocket_writer
                    .send(Message::Text(encoded.into()))
                    .await
                    .is_err()
                {
                    let _ = disconnected_tx.send(true);
                    return;
                }
            }
            Err(err) => {
                warn!("failed to serialize relay resume frame: {err}");
                let _ = disconnected_tx.send(true);
                return;
            }
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
            match serialize_relay_message_frame(&frame) {
                Ok(encoded) => {
                    if websocket_writer
                        .send(Message::Text(encoded.into()))
                        .await
                        .is_err()
                    {
                        let _ = disconnected_tx.send(true);
                        break;
                    }
                }
                Err(err) => {
                    warn!("failed to serialize relay data message frame: {err}");
                    let _ = disconnected_tx.send(true);
                    break;
                }
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
        mpsc::channel::<String>(CHANNEL_CAPACITY);
    let writer_task = tokio::spawn(async move {
        while let Some(encoded) = physical_outgoing_rx.recv().await {
            if websocket_writer
                .send(Message::Text(encoded.into()))
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
            Some(Ok(Message::Text(text))) => {
                match serde_json::from_str::<RelayMessageFrame>(text.as_ref()) {
                    Ok(frame) => frame,
                    Err(err) => {
                        warn!("dropping malformed relay message frame from harness: {err}");
                        continue;
                    }
                }
            }
            Some(Ok(Message::Close(_))) | None => break,
            Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_))) => continue,
            Some(Ok(Message::Binary(_))) => {
                warn!("dropping non-text relay message frame from harness");
                continue;
            }
            Some(Err(err)) => {
                debug!("multiplexed executor websocket read failed: {err}");
                break;
            }
        };

        if let Err(err) = frame.validate() {
            warn!("dropping invalid relay message frame: {err}");
            continue;
        }

        match frame.kind {
            RelayMessageFrameKind::Data => {
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
            RelayMessageFrameKind::Reset => {
                if let Some(stream) = streams.remove(&frame.stream_id) {
                    stream.disconnect(frame.reason).await;
                }
            }
            RelayMessageFrameKind::Ack
            | RelayMessageFrameKind::Resume
            | RelayMessageFrameKind::Heartbeat => {}
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
    physical_outgoing_tx: mpsc::Sender<String>,
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
            match serialize_relay_message_frame(&frame) {
                Ok(encoded) => {
                    if physical_outgoing_tx.send(encoded).await.is_err() {
                        break;
                    }
                }
                Err(err) => {
                    warn!("failed to serialize virtual stream relay message frame: {err}");
                    break;
                }
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
