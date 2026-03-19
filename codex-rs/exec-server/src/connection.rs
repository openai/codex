use codex_app_server_protocol::JSONRPCMessage;
use futures::SinkExt;
use futures::StreamExt;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::io::BufWriter;
use tokio::sync::mpsc;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;


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

        let reader_label = connection_label.clone();
        let incoming_tx_for_reader = incoming_tx.clone();
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
                        send_disconnected(&incoming_tx_for_reader, /*reason*/ None).await;
                        break;
                    }
                    Err(err) => {
                        send_disconnected(
                            &incoming_tx_for_reader,
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
            task_handles: vec![reader_task, writer_task],
        }
    }

    pub(crate) fn from_websocket<S>(stream: WebSocketStream<S>, connection_label: String) -> Self
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let (outgoing_tx, mut outgoing_rx) = mpsc::channel(CHANNEL_CAPACITY);
        let (incoming_tx, incoming_rx) = mpsc::channel(CHANNEL_CAPACITY);
        let (mut websocket_writer, mut websocket_reader) = stream.split();

        let reader_label = connection_label.clone();
        let incoming_tx_for_reader = incoming_tx.clone();
        let reader_task = tokio::spawn(async move {
            loop {
                match websocket_reader.next().await {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<JSONRPCMessage>(text.as_ref()) {
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
                                        "failed to parse websocket JSON-RPC message from {reader_label}: {err}"
                                    )),
                                )
                                .await;
                            }
                        }
                    }
                    Some(Ok(Message::Binary(bytes))) => {
                        match serde_json::from_slice::<JSONRPCMessage>(bytes.as_ref()) {
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
                                        "failed to parse websocket JSON-RPC message from {reader_label}: {err}"
                                    )),
                                )
                                .await;
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) => {
                        send_disconnected(&incoming_tx_for_reader, /*reason*/ None).await;
                        break;
                    }
                    Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => {}
                    Some(Ok(_)) => {}
                    Some(Err(err)) => {
                        send_disconnected(
                            &incoming_tx_for_reader,
                            Some(format!(
                                "failed to read websocket JSON-RPC message from {reader_label}: {err}"
                            )),
                        )
                        .await;
                        break;
                    }
                    None => {
                        send_disconnected(&incoming_tx_for_reader, /*reason*/ None).await;
                        break;
                    }
                }
            }
        });

        let writer_task = tokio::spawn(async move {
            while let Some(message) = outgoing_rx.recv().await {
                match serialize_jsonrpc_message(&message) {
                    Ok(encoded) => {
                        if let Err(err) = websocket_writer.send(Message::Text(encoded.into())).await
                        {
                            send_disconnected(
                                &incoming_tx,
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
            task_handles: vec![reader_task, writer_task],
        }
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        mpsc::Sender<JSONRPCMessage>,
        mpsc::Receiver<JsonRpcConnectionEvent>,
        Vec<tokio::task::JoinHandle<()>>,
    ) {
        (self.outgoing_tx, self.incoming_rx, self.task_handles)
    }
}

async fn send_disconnected(
    incoming_tx: &mpsc::Sender<JsonRpcConnectionEvent>,
    reason: Option<String>,
) {
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
