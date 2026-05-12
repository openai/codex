use axum::extract::ws::Message as AxumWebSocketMessage;
use axum::extract::ws::WebSocket as AxumWebSocket;
use codex_app_server_protocol::JSONRPCMessage;
use futures::Sink;
use futures::SinkExt;
use futures::Stream;
use futures::StreamExt;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;

use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::io::BufWriter;

pub(crate) const CHANNEL_CAPACITY: usize = 128;

#[derive(Debug)]
pub(crate) enum JsonRpcConnectionEvent {
    Message(JSONRPCMessage),
    MalformedMessage { reason: String },
    Disconnected { reason: Option<String> },
}

pub(crate) struct JsonRpcConnection {
    outgoing_tx: mpsc::Sender<JSONRPCMessage>,
    incoming_rx: mpsc::Receiver<JsonRpcConnectionEvent>,
    disconnected_rx: watch::Receiver<bool>,
    task_handles: Vec<tokio::task::JoinHandle<()>>,
}

impl JsonRpcConnection {
    pub(crate) fn from_stdio<R, W>(reader: R, writer: W, connection_label: String) -> Self
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let (outgoing_tx, mut outgoing_rx) = mpsc::channel(CHANNEL_CAPACITY);
        let (incoming_tx, incoming_rx) = mpsc::channel(CHANNEL_CAPACITY);
        let (disconnected_tx, disconnected_rx) = watch::channel(false);

        let reader_label = connection_label.clone();
        let incoming_tx_for_reader = incoming_tx.clone();
        let disconnected_tx_for_reader = disconnected_tx.clone();
        let reader_task = tokio::spawn(async move {
            let mut lines = BufReader::new(reader).lines();
            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        if line.trim().is_empty() {
                            continue;
                        }
                        match serde_json::from_str::<JSONRPCMessage>(&line) {
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
                                send_malformed_message(
                                    &incoming_tx_for_reader,
                                    Some(format!(
                                        "failed to parse JSON-RPC message from {reader_label}: {err}"
                                    )),
                                )
                                .await;
                            }
                        }
                    }
                    Ok(None) => {
                        send_disconnected(
                            &incoming_tx_for_reader,
                            &disconnected_tx_for_reader,
                            /*reason*/ None,
                        )
                        .await;
                        break;
                    }
                    Err(err) => {
                        send_disconnected(
                            &incoming_tx_for_reader,
                            &disconnected_tx_for_reader,
                            Some(format!(
                                "failed to read JSON-RPC message from {reader_label}: {err}"
                            )),
                        )
                        .await;
                        break;
                    }
                }
            }
        });

        let writer_task = tokio::spawn(async move {
            let mut writer = BufWriter::new(writer);
            while let Some(message) = outgoing_rx.recv().await {
                if let Err(err) = write_jsonrpc_line_message(&mut writer, &message).await {
                    send_disconnected(
                        &incoming_tx,
                        &disconnected_tx,
                        Some(format!(
                            "failed to write JSON-RPC message to {connection_label}: {err}"
                        )),
                    )
                    .await;
                    break;
                }
            }
        });

        Self {
            outgoing_tx,
            incoming_rx,
            disconnected_rx,
            task_handles: vec![reader_task, writer_task],
        }
    }

    pub(crate) fn from_websocket<S>(stream: WebSocketStream<S>, connection_label: String) -> Self
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let (websocket_writer, websocket_reader) = stream.split();
        Self::from_websocket_parts(websocket_writer, websocket_reader, connection_label)
    }

    pub(crate) fn from_axum_websocket(stream: AxumWebSocket, connection_label: String) -> Self {
        let (websocket_writer, websocket_reader) = stream.split();
        Self::from_websocket_parts(websocket_writer, websocket_reader, connection_label)
    }

    fn from_websocket_parts<W, R, M, E>(
        mut websocket_writer: W,
        mut websocket_reader: R,
        connection_label: String,
    ) -> Self
    where
        W: Sink<M, Error = E> + Unpin + Send + 'static,
        R: Stream<Item = Result<M, E>> + Unpin + Send + 'static,
        M: JsonRpcWebSocketMessage,
        E: std::fmt::Display + Send + 'static,
    {
        let (outgoing_tx, mut outgoing_rx) = mpsc::channel(CHANNEL_CAPACITY);
        let (incoming_tx, incoming_rx) = mpsc::channel(CHANNEL_CAPACITY);
        let (disconnected_tx, disconnected_rx) = watch::channel(false);

        let reader_label = connection_label.clone();
        let incoming_tx_for_reader = incoming_tx.clone();
        let disconnected_tx_for_reader = disconnected_tx.clone();
        let reader_task = tokio::spawn(async move {
            loop {
                match websocket_reader.next().await {
                    Some(Ok(message)) => match message.parse_jsonrpc_frame() {
                        Ok(JsonRpcWebSocketFrame::Message(message)) => {
                            if incoming_tx_for_reader
                                .send(JsonRpcConnectionEvent::Message(message))
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                        Err(err) => {
                            send_malformed_message(
                                &incoming_tx_for_reader,
                                Some(format!(
                                    "failed to parse websocket JSON-RPC message from {reader_label}: {err}"
                                )),
                            )
                            .await;
                        }
                        Ok(JsonRpcWebSocketFrame::Close) => {
                            send_disconnected(
                                &incoming_tx_for_reader,
                                &disconnected_tx_for_reader,
                                /*reason*/ None,
                            )
                            .await;
                            break;
                        }
                        Ok(JsonRpcWebSocketFrame::Ignore) => {}
                    },
                    Some(Err(err)) => {
                        send_disconnected(
                            &incoming_tx_for_reader,
                            &disconnected_tx_for_reader,
                            Some(format!(
                                "failed to read websocket JSON-RPC message from {reader_label}: {err}"
                            )),
                        )
                        .await;
                        break;
                    }
                    None => {
                        send_disconnected(
                            &incoming_tx_for_reader,
                            &disconnected_tx_for_reader,
                            /*reason*/ None,
                        )
                        .await;
                        break;
                    }
                }
            }
        });

        let writer_task = tokio::spawn(async move {
            while let Some(message) = outgoing_rx.recv().await {
                match serialize_jsonrpc_message(&message) {
                    Ok(encoded) => {
                        if let Err(err) = websocket_writer.send(M::from_text(encoded)).await {
                            send_disconnected(
                                &incoming_tx,
                                &disconnected_tx,
                                Some(format!(
                                    "failed to write websocket JSON-RPC message to {connection_label}: {err}"
                                )),
                            )
                            .await;
                            break;
                        }
                    }
                    Err(err) => {
                        send_disconnected(
                            &incoming_tx,
                            &disconnected_tx,
                            Some(format!(
                                "failed to serialize JSON-RPC message for {connection_label}: {err}"
                            )),
                        )
                        .await;
                        break;
                    }
                }
            }
        });

        Self {
            outgoing_tx,
            incoming_rx,
            disconnected_rx,
            task_handles: vec![reader_task, writer_task],
        }
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        mpsc::Sender<JSONRPCMessage>,
        mpsc::Receiver<JsonRpcConnectionEvent>,
        watch::Receiver<bool>,
        Vec<tokio::task::JoinHandle<()>>,
    ) {
        (
            self.outgoing_tx,
            self.incoming_rx,
            self.disconnected_rx,
            self.task_handles,
        )
    }
}

enum JsonRpcWebSocketFrame {
    Message(JSONRPCMessage),
    Close,
    Ignore,
}

trait JsonRpcWebSocketMessage: Send + 'static {
    fn parse_jsonrpc_frame(self) -> Result<JsonRpcWebSocketFrame, serde_json::Error>;
    fn from_text(text: String) -> Self;
}

impl JsonRpcWebSocketMessage for Message {
    fn parse_jsonrpc_frame(self) -> Result<JsonRpcWebSocketFrame, serde_json::Error> {
        match self {
            Message::Text(text) => {
                serde_json::from_str(text.as_ref()).map(JsonRpcWebSocketFrame::Message)
            }
            Message::Binary(bytes) => {
                serde_json::from_slice(bytes.as_ref()).map(JsonRpcWebSocketFrame::Message)
            }
            Message::Close(_) => Ok(JsonRpcWebSocketFrame::Close),
            Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => {
                Ok(JsonRpcWebSocketFrame::Ignore)
            }
        }
    }

    fn from_text(text: String) -> Self {
        Self::Text(text.into())
    }
}

impl JsonRpcWebSocketMessage for AxumWebSocketMessage {
    fn parse_jsonrpc_frame(self) -> Result<JsonRpcWebSocketFrame, serde_json::Error> {
        match self {
            AxumWebSocketMessage::Text(text) => {
                serde_json::from_str(text.as_ref()).map(JsonRpcWebSocketFrame::Message)
            }
            AxumWebSocketMessage::Binary(bytes) => {
                serde_json::from_slice(bytes.as_ref()).map(JsonRpcWebSocketFrame::Message)
            }
            AxumWebSocketMessage::Close(_) => Ok(JsonRpcWebSocketFrame::Close),
            AxumWebSocketMessage::Ping(_) | AxumWebSocketMessage::Pong(_) => {
                Ok(JsonRpcWebSocketFrame::Ignore)
            }
        }
    }

    fn from_text(text: String) -> Self {
        Self::Text(text.into())
    }
}

async fn send_disconnected(
    incoming_tx: &mpsc::Sender<JsonRpcConnectionEvent>,
    disconnected_tx: &watch::Sender<bool>,
    reason: Option<String>,
) {
    let _ = disconnected_tx.send(true);
    let _ = incoming_tx
        .send(JsonRpcConnectionEvent::Disconnected { reason })
        .await;
}

async fn send_malformed_message(
    incoming_tx: &mpsc::Sender<JsonRpcConnectionEvent>,
    reason: Option<String>,
) {
    let _ = incoming_tx
        .send(JsonRpcConnectionEvent::MalformedMessage {
            reason: reason.unwrap_or_else(|| "malformed JSON-RPC message".to_string()),
        })
        .await;
}

async fn write_jsonrpc_line_message<W>(
    writer: &mut BufWriter<W>,
    message: &JSONRPCMessage,
) -> std::io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    let encoded =
        serialize_jsonrpc_message(message).map_err(|err| std::io::Error::other(err.to_string()))?;
    writer.write_all(encoded.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await
}

fn serialize_jsonrpc_message(message: &JSONRPCMessage) -> Result<String, serde_json::Error> {
    serde_json::to_string(message)
}
