use codex_app_server_protocol::JSONRPCMessage;
use futures::Sink;
use futures::SinkExt;
use futures::Stream;
use futures::StreamExt;
use prost::Message as ProstMessage;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio_tungstenite::tungstenite::Message;
use tracing::warn;
use uuid::Uuid;

use crate::ExecServerError;
use crate::connection::CHANNEL_CAPACITY;
use crate::connection::JsonRpcConnection;
use crate::connection::JsonRpcConnectionEvent;
use crate::connection::JsonRpcTransport;
use crate::relay_proto::RelayData;
use crate::relay_proto::RelayHandshake;
use crate::relay_proto::RelayMessageFrame;
use crate::relay_proto::RelayReset;
use crate::relay_proto::RelayResume;
use crate::relay_proto::relay_message_frame;

const RELAY_MESSAGE_FRAME_VERSION: u32 = 1;
const MAX_RELAY_RESET_REASON_BYTES: usize = 256;
const MAX_RELAY_STREAM_ID_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum RelayFrameBodyKind {
    Data,
    Ack,
    Resume,
    Reset,
    Heartbeat,
    Handshake,
}

impl RelayMessageFrame {
    pub(crate) fn data(stream_id: String, seq: u32, payload: Vec<u8>) -> Self {
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

    pub(crate) fn resume(stream_id: String) -> Self {
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

    pub(crate) fn handshake(stream_id: String, payload: Vec<u8>) -> Self {
        Self {
            version: RELAY_MESSAGE_FRAME_VERSION,
            stream_id,
            ack: 0,
            ack_bits: 0,
            body: Some(relay_message_frame::Body::Handshake(RelayHandshake {
                payload,
            })),
        }
    }

    pub(crate) fn reset(stream_id: String, reason: String) -> Self {
        Self {
            version: RELAY_MESSAGE_FRAME_VERSION,
            stream_id,
            ack: 0,
            ack_bits: 0,
            body: Some(relay_message_frame::Body::Reset(RelayReset { reason })),
        }
    }

    /// Validate cleartext routing metadata before a frame reaches either the
    /// direct JSON-RPC parser or the Noise relay state machine.
    ///
    /// The encrypted path intentionally leaves this metadata visible to
    /// rendezvous, so it must be canonical and tightly bounded everywhere.
    pub(crate) fn validate(&self) -> Result<RelayFrameBodyKind, ExecServerError> {
        if self.version != RELAY_MESSAGE_FRAME_VERSION {
            return Err(ExecServerError::Protocol(format!(
                "unsupported relay message frame version {}",
                self.version
            )));
        }
        if self.stream_id.is_empty() {
            return Err(ExecServerError::Protocol(
                "relay message frame is missing stream_id".to_string(),
            ));
        }
        if self.stream_id.trim() != self.stream_id {
            return Err(ExecServerError::Protocol(
                "relay message frame stream_id is not canonical".to_string(),
            ));
        }
        if self.stream_id.len() > MAX_RELAY_STREAM_ID_BYTES {
            return Err(ExecServerError::Protocol(
                "relay message frame stream_id is too long".to_string(),
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
                if reset.reason.is_empty() || reset.reason.len() > MAX_RELAY_RESET_REASON_BYTES {
                    return Err(ExecServerError::Protocol(
                        "relay reset message frame has invalid reason".to_string(),
                    ));
                }
                Ok(RelayFrameBodyKind::Reset)
            }
            Some(relay_message_frame::Body::Heartbeat(_)) => Ok(RelayFrameBodyKind::Heartbeat),
            Some(relay_message_frame::Body::Handshake(handshake)) => {
                if handshake.payload.is_empty() {
                    return Err(ExecServerError::Protocol(
                        "relay handshake message frame is missing payload".to_string(),
                    ));
                }
                Ok(RelayFrameBodyKind::Handshake)
            }
            None => Err(ExecServerError::Protocol(
                "relay message frame is missing body".to_string(),
            )),
        }
    }

    pub(crate) fn into_data(self) -> Result<RelayData, ExecServerError> {
        let kind = self.validate()?;
        if kind != RelayFrameBodyKind::Data {
            return Err(ExecServerError::Protocol(
                "expected relay data message frame".to_string(),
            ));
        }
        match self.body {
            Some(relay_message_frame::Body::Data(data)) => Ok(data),
            _ => Err(ExecServerError::Protocol(
                "expected relay data message frame".to_string(),
            )),
        }
    }

    fn into_jsonrpc_message(self) -> Result<JSONRPCMessage, ExecServerError> {
        let payload = self.into_data()?.payload;
        serde_json::from_slice(&payload).map_err(ExecServerError::Json)
    }

    pub(crate) fn into_handshake_payload(self) -> Result<Vec<u8>, ExecServerError> {
        let kind = self.validate()?;
        if kind != RelayFrameBodyKind::Handshake {
            return Err(ExecServerError::Protocol(
                "expected relay handshake message frame".to_string(),
            ));
        }
        match self.body {
            Some(relay_message_frame::Body::Handshake(handshake)) => Ok(handshake.payload),
            _ => Err(ExecServerError::Protocol(
                "expected relay handshake message frame".to_string(),
            )),
        }
    }

    pub(crate) fn into_reset_reason(self) -> Option<String> {
        match self.body {
            Some(relay_message_frame::Body::Reset(reset)) if !reset.reason.is_empty() => {
                Some(reset.reason)
            }
            _ => None,
        }
    }
}

pub(crate) fn encode_relay_message_frame(frame: &RelayMessageFrame) -> Vec<u8> {
    frame.encode_to_vec()
}

pub(crate) fn decode_relay_message_frame(
    payload: &[u8],
) -> Result<RelayMessageFrame, ExecServerError> {
    RelayMessageFrame::decode(payload)
        .map_err(|err| ExecServerError::Protocol(format!("invalid relay message frame: {err}")))
}

pub(crate) fn jsonrpc_payload(message: &JSONRPCMessage) -> Result<Vec<u8>, ExecServerError> {
    serde_json::to_vec(message).map_err(ExecServerError::Json)
}

pub(crate) fn harness_connection_from_websocket<T, E>(
    stream: T,
    connection_label: String,
) -> JsonRpcConnection
where
    T: Sink<Message, Error = E> + Stream<Item = Result<Message, E>> + Unpin + Send + 'static,
    E: std::fmt::Display + Send + 'static,
{
    let stream_id = Uuid::new_v4().to_string();
    let (outgoing_tx, mut outgoing_rx) = mpsc::channel(CHANNEL_CAPACITY);
    let (incoming_tx, incoming_rx) = mpsc::channel(CHANNEL_CAPACITY);
    let (disconnected_tx, disconnected_rx) = watch::channel(false);

    let websocket_task = tokio::spawn(async move {
        let mut websocket = stream;
        let reader_label = connection_label;
        let reader_stream_id = stream_id.clone();
        let resume = RelayMessageFrame::resume(stream_id.clone());
        if websocket
            .send(Message::Binary(encode_relay_message_frame(&resume).into()))
            .await
            .is_err()
        {
            let _ = disconnected_tx.send(true);
            return;
        }

        let mut next_seq = 0u32;
        loop {
            tokio::select! {
                maybe_message = outgoing_rx.recv() => {
                    let Some(message) = maybe_message else {
                        break;
                    };
                    let payload = match jsonrpc_payload(&message) {
                        Ok(payload) => payload,
                        Err(err) => {
                            warn!("failed to serialize JSON-RPC payload for relay transport: {err}");
                            break;
                        }
                    };
                    let frame = RelayMessageFrame::data(stream_id.clone(), next_seq, payload);
                    next_seq = next_seq.wrapping_add(1);
                    if websocket
                        .send(Message::Binary(encode_relay_message_frame(&frame).into()))
                        .await
                        .is_err()
                    {
                        let _ = disconnected_tx.send(true);
                        break;
                    }
                }
                incoming_message = websocket.next() => {
                    match incoming_message {
                        Some(Ok(Message::Binary(payload))) => {
                            let frame = match decode_relay_message_frame(payload.as_ref()) {
                                Ok(frame) => frame,
                                Err(err) => {
                                    let _ = incoming_tx
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
                                    let _ = incoming_tx
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
                                        if incoming_tx
                                            .send(JsonRpcConnectionEvent::Message(message))
                                            .await
                                            .is_err()
                                        {
                                            break;
                                        }
                                    }
                                    Err(err) => {
                                        let _ = incoming_tx
                                            .send(JsonRpcConnectionEvent::MalformedMessage {
                                                reason: err.to_string(),
                                            })
                                            .await;
                                    }
                                },
                                RelayFrameBodyKind::Reset => {
                                    let _ = disconnected_tx.send(true);
                                    let _ = incoming_tx
                                        .send(JsonRpcConnectionEvent::Disconnected {
                                            reason: frame.into_reset_reason(),
                                        })
                                        .await;
                                    break;
                                }
                                RelayFrameBodyKind::Ack
                                | RelayFrameBodyKind::Resume
                                | RelayFrameBodyKind::Heartbeat
                                | RelayFrameBodyKind::Handshake => {}
                            }
                        }
                        Some(Ok(Message::Close(_))) | None => {
                            let _ = disconnected_tx.send(true);
                            let _ = incoming_tx
                                .send(JsonRpcConnectionEvent::Disconnected { reason: None })
                                .await;
                            break;
                        }
                        Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_))) => {}
                        Some(Ok(Message::Text(_))) => {
                            let _ = incoming_tx
                                .send(JsonRpcConnectionEvent::MalformedMessage {
                                    reason: "relay exec-server transport expects binary protobuf frames"
                                        .to_string(),
                                })
                                .await;
                        }
                        Some(Err(err)) => {
                            let _ = disconnected_tx.send(true);
                            let _ = incoming_tx
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
            }
        }
    });

    JsonRpcConnection {
        outgoing_tx,
        incoming_rx,
        disconnected_rx,
        task_handles: vec![websocket_task],
        transport: JsonRpcTransport::Plain,
    }
}

pub(crate) use crate::noise_relay::environment::HarnessKeyValidator;
/// Runs the executor-side multiplexed relay with Noise-protected payloads.
pub(crate) use crate::noise_relay::environment::run_multiplexed_environment;

#[cfg(test)]
mod tests {
    use std::pin::Pin;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;
    use std::task::Context;
    use std::task::Poll;
    use std::time::Duration;

    use codex_app_server_protocol::JSONRPCRequest;
    use codex_app_server_protocol::RequestId;
    use futures::Sink;
    use futures::Stream;
    use futures::channel::mpsc as futures_mpsc;
    use futures::task::AtomicWaker;
    use tokio::net::TcpListener;
    use tokio::time::timeout;
    use tokio_tungstenite::WebSocketStream;
    use tokio_tungstenite::accept_async;
    use tokio_tungstenite::connect_async;
    use tokio_tungstenite::tungstenite::Message;

    use super::*;

    #[tokio::test]
    async fn harness_connection_receives_relay_data() -> anyhow::Result<()> {
        let (client_websocket, mut server_websocket) = websocket_pair().await?;
        let mut connection =
            harness_connection_from_websocket(client_websocket, "test".to_string());
        let stream_id = read_resume_stream_id(&mut server_websocket).await?;
        let message = test_jsonrpc_message();

        server_websocket
            .send(Message::Binary(
                encode_relay_message_frame(&RelayMessageFrame::data(
                    stream_id,
                    /*seq*/ 0,
                    jsonrpc_payload(&message)?,
                ))
                .into(),
            ))
            .await?;
        assert!(matches!(
            timeout(Duration::from_secs(1), connection.incoming_rx.recv()).await?,
            Some(JsonRpcConnectionEvent::Message(actual)) if actual == message
        ));

        drop(connection);
        Ok(())
    }

    #[tokio::test]
    async fn harness_connection_reports_text_frames_as_malformed() -> anyhow::Result<()> {
        let (client_websocket, mut server_websocket) = websocket_pair().await?;
        let mut connection =
            harness_connection_from_websocket(client_websocket, "test".to_string());

        read_resume_stream_id(&mut server_websocket).await?;
        server_websocket.send(Message::Text("nope".into())).await?;
        assert!(matches!(
            timeout(Duration::from_secs(1), connection.incoming_rx.recv()).await?,
            Some(JsonRpcConnectionEvent::MalformedMessage { reason })
                if reason == "relay exec-server transport expects binary protobuf frames"
        ));

        drop(connection);
        Ok(())
    }

    #[tokio::test]
    async fn harness_connection_reports_server_close() -> anyhow::Result<()> {
        let (client_websocket, mut server_websocket) = websocket_pair().await?;
        let mut connection =
            harness_connection_from_websocket(client_websocket, "test".to_string());

        read_resume_stream_id(&mut server_websocket).await?;
        server_websocket.close(None).await?;
        assert!(matches!(
            timeout(Duration::from_secs(1), connection.incoming_rx.recv()).await?,
            Some(JsonRpcConnectionEvent::Disconnected { reason: None })
        ));

        drop(connection);
        Ok(())
    }

    #[tokio::test]
    async fn harness_connection_keeps_outbound_frame_while_send_is_backpressured()
    -> anyhow::Result<()> {
        let (websocket, control, mut outbound_rx) =
            ControlledWebSocket::new(/*write_ready*/ true);
        let mut connection = harness_connection_from_websocket(websocket, "test".to_string());
        let Message::Binary(resume_payload) = timeout(Duration::from_secs(1), outbound_rx.next())
            .await?
            .expect("resume frame")
        else {
            anyhow::bail!("expected relay resume frame");
        };
        let stream_id = decode_relay_message_frame(resume_payload.as_ref())?.stream_id;
        let message = test_jsonrpc_message();

        control.set_write_blocked();
        connection.outgoing_tx.send(message.clone()).await?;
        control.wait_for_blocked_write().await?;
        control.send_inbound(Message::Pong(b"check".to_vec().into()))?;
        assert!(
            timeout(Duration::from_millis(50), connection.incoming_rx.recv())
                .await
                .is_err()
        );

        control.set_write_ready();
        let Message::Binary(data_payload) = timeout(Duration::from_secs(1), outbound_rx.next())
            .await?
            .expect("data frame")
        else {
            anyhow::bail!("expected relay data frame");
        };
        let frame = decode_relay_message_frame(data_payload.as_ref())?;
        assert_eq!(frame.stream_id, stream_id);
        assert_eq!(frame.into_jsonrpc_message()?, message);
        drop(connection);
        Ok(())
    }

    async fn websocket_pair() -> anyhow::Result<(
        WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
        WebSocketStream<tokio::net::TcpStream>,
    )> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let websocket_url = format!("ws://{}", listener.local_addr()?);
        let server_task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await?;
            accept_async(stream).await.map_err(anyhow::Error::from)
        });
        let (client_websocket, _) = connect_async(websocket_url).await?;
        let server_websocket = server_task.await??;
        Ok((client_websocket, server_websocket))
    }

    async fn read_resume_stream_id(
        websocket: &mut WebSocketStream<tokio::net::TcpStream>,
    ) -> anyhow::Result<String> {
        let message = timeout(Duration::from_secs(1), websocket.next())
            .await?
            .expect("websocket should stay open")?;
        let Message::Binary(payload) = message else {
            anyhow::bail!("expected relay resume frame, got {message:?}");
        };
        let frame = decode_relay_message_frame(payload.as_ref())?;
        assert_eq!(frame.validate()?, RelayFrameBodyKind::Resume);
        Ok(frame.stream_id)
    }

    fn test_jsonrpc_message() -> JSONRPCMessage {
        JSONRPCMessage::Request(JSONRPCRequest {
            id: RequestId::Integer(1),
            method: "test".to_string(),
            params: None,
            trace: None,
        })
    }

    struct ControlledWebSocket {
        inbound_rx: futures_mpsc::UnboundedReceiver<Result<Message, std::convert::Infallible>>,
        outbound_tx: futures_mpsc::UnboundedSender<Message>,
        write_ready: Arc<AtomicBool>,
        write_blocked: Arc<AtomicBool>,
        write_blocked_waker: Arc<AtomicWaker>,
        write_waker: Arc<AtomicWaker>,
    }

    struct ControlledWebSocketHandle {
        inbound_tx: futures_mpsc::UnboundedSender<Result<Message, std::convert::Infallible>>,
        write_ready: Arc<AtomicBool>,
        write_blocked: Arc<AtomicBool>,
        write_blocked_waker: Arc<AtomicWaker>,
        write_waker: Arc<AtomicWaker>,
    }

    impl ControlledWebSocket {
        fn new(
            write_ready: bool,
        ) -> (
            Self,
            ControlledWebSocketHandle,
            futures_mpsc::UnboundedReceiver<Message>,
        ) {
            let (inbound_tx, inbound_rx) = futures_mpsc::unbounded();
            let (outbound_tx, outbound_rx) = futures_mpsc::unbounded();
            let write_ready = Arc::new(AtomicBool::new(write_ready));
            let write_blocked = Arc::new(AtomicBool::new(false));
            let write_blocked_waker = Arc::new(AtomicWaker::new());
            let write_waker = Arc::new(AtomicWaker::new());
            (
                Self {
                    inbound_rx,
                    outbound_tx,
                    write_ready: Arc::clone(&write_ready),
                    write_blocked: Arc::clone(&write_blocked),
                    write_blocked_waker: Arc::clone(&write_blocked_waker),
                    write_waker: Arc::clone(&write_waker),
                },
                ControlledWebSocketHandle {
                    inbound_tx,
                    write_ready,
                    write_blocked,
                    write_blocked_waker,
                    write_waker,
                },
                outbound_rx,
            )
        }
    }

    impl ControlledWebSocketHandle {
        fn send_inbound(&self, message: Message) -> anyhow::Result<()> {
            self.inbound_tx
                .unbounded_send(Ok(message))
                .map_err(anyhow::Error::from)
        }

        fn set_write_blocked(&self) {
            self.write_ready.store(false, Ordering::Release);
        }

        fn set_write_ready(&self) {
            self.write_ready.store(true, Ordering::Release);
            self.write_waker.wake();
        }

        async fn wait_for_blocked_write(&self) -> anyhow::Result<()> {
            timeout(
                Duration::from_secs(1),
                futures::future::poll_fn(|cx| {
                    if self.write_blocked.load(Ordering::Acquire) {
                        Poll::Ready(())
                    } else {
                        self.write_blocked_waker.register(cx.waker());
                        Poll::Pending
                    }
                }),
            )
            .await?;
            Ok(())
        }
    }

    impl Sink<Message> for ControlledWebSocket {
        type Error = std::convert::Infallible;

        fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            if self.write_ready.load(Ordering::Acquire) {
                Poll::Ready(Ok(()))
            } else {
                self.write_blocked.store(true, Ordering::Release);
                self.write_blocked_waker.wake();
                self.write_waker.register(cx.waker());
                Poll::Pending
            }
        }

        fn start_send(self: Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
            self.outbound_tx
                .unbounded_send(item)
                .expect("test outbound receiver should stay open");
            Ok(())
        }

        fn poll_flush(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn poll_close(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
    }

    impl Stream for ControlledWebSocket {
        type Item = Result<Message, std::convert::Infallible>;

        fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            Pin::new(&mut self.inbound_rx).poll_next(cx)
        }
    }
}
