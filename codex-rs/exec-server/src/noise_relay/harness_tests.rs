use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::task::Context as TaskContext;
use std::task::Poll;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_exec_server_protocol::JSONRPCMessage;
use codex_exec_server_protocol::JSONRPCRequest;
use codex_exec_server_protocol::RequestId;
use futures::Sink;
use futures::SinkExt;
use futures::StreamExt;
use futures::channel::mpsc as futures_mpsc;
use pretty_assertions::assert_eq;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio::time::timeout;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use super::*;
use crate::connection::JsonRpcConnectionEvent;
use crate::noise_channel::PendingResponderHandshake;

const ENVIRONMENT_ID: &str = "environment-1";
const EXECUTOR_REGISTRATION_ID: &str = "registration-1";

#[tokio::test]
async fn fragmented_writes_yield_to_keepalive_and_queued_pong() -> Result<()> {
    let (connection, mut control, mut outbound_rx) = connected_controlled_harness().await?;

    connection
        .outgoing_tx
        .send(JSONRPCMessage::Request(JSONRPCRequest {
            id: RequestId::Integer(1),
            method: "large".to_string(),
            params: Some(serde_json::json!({
                "payload": "x".repeat(NOISE_RECORD_PLAINTEXT_LEN * 3),
            })),
            trace: None,
        }))
        .await?;

    control.wait_for_blocked_write(/*expected*/ 1).await?;
    tokio::time::sleep(WEBSOCKET_KEEPALIVE_INTERVAL + Duration::from_millis(10)).await;
    control.grant_writes(/*count*/ 1);
    let first_data = read_outbound_data(&mut outbound_rx).await?;
    assert_eq!(first_data.seq, 0);

    control.wait_for_blocked_write(/*expected*/ 2).await?;
    control.grant_writes(/*count*/ 1);
    let Message::Ping(ping_payload) = timeout(Duration::from_secs(1), outbound_rx.next())
        .await?
        .context("harness closed before sending keepalive")?
    else {
        anyhow::bail!("expected keepalive between fragmented writes");
    };

    control.wait_for_blocked_write(/*expected*/ 3).await?;
    control.send_inbound(Message::Pong(ping_payload))?;
    tokio::time::sleep(WEBSOCKET_KEEPALIVE_INTERVAL + Duration::from_millis(10)).await;
    control.grant_writes(/*count*/ 1);
    let second_data = read_outbound_data(&mut outbound_rx).await?;
    assert_eq!(second_data.seq, 1);

    control.wait_for_blocked_write(/*expected*/ 4).await?;
    control.grant_writes(/*count*/ 1);
    let next_message = timeout(Duration::from_secs(1), outbound_rx.next())
        .await?
        .context("harness closed after receiving queued Pong")?;
    assert!(matches!(next_message, Message::Ping(_)));

    for task in &connection.task_handles {
        task.abort();
    }
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn post_deadline_drain_stops_before_frame_33() -> Result<()> {
    let (mut connection, mut control, mut outbound_rx) = connected_controlled_harness().await?;

    control.wait_for_blocked_write(/*expected*/ 1).await?;
    control.grant_writes(/*count*/ 1);
    let Message::Ping(ping_payload) = timeout(Duration::from_secs(1), outbound_rx.next())
        .await?
        .context("harness closed before sending keepalive")?
    else {
        anyhow::bail!("expected keepalive ping");
    };
    let reads_before_deadline = control.inbound_reads();

    let unrelated_frame =
        encode_relay_message_frame(&RelayMessageFrame::resume("unrelated-stream".to_string()));
    for _ in 0..MAX_FRAMES_DRAINED_AFTER_PONG_DEADLINE {
        control.send_inbound(Message::Binary(unrelated_frame.clone().into()))?;
    }
    control.send_inbound(Message::Pong(ping_payload))?;

    // Keep the current-thread runtime from consuming the queued frames until the
    // Pong deadline and every frame are ready together.
    std::thread::sleep(WEBSOCKET_PONG_TIMEOUT + Duration::from_millis(10));

    let event = timeout(Duration::from_secs(1), connection.incoming_rx.recv()).await?;
    let Some(JsonRpcConnectionEvent::Disconnected { reason }) = event else {
        anyhow::bail!("expected Pong timeout, got {event:?}");
    };
    assert_eq!(reason.as_deref(), Some(WEBSOCKET_PONG_TIMEOUT_REASON));
    assert_eq!(
        control.inbound_reads() - reads_before_deadline,
        MAX_FRAMES_DRAINED_AFTER_PONG_DEADLINE
    );
    Ok(())
}

#[tokio::test]
async fn pong_keeps_harness_alive_until_peer_stops_responding() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let websocket_url = format!("ws://{}", listener.local_addr()?);
    let harness_connection = tokio::spawn(connect_async(websocket_url));
    let (socket, _peer_addr) = listener.accept().await?;
    let mut executor_websocket = accept_async(socket).await?;
    let (harness_websocket, _response) = harness_connection.await??;

    let executor_identity = NoiseChannelIdentity::generate()?;
    let mut connection = noise_harness_connection_from_websocket(
        harness_websocket,
        NoiseHarnessConnectionArgs {
            connection_label: "test rendezvous".to_string(),
            environment_id: ENVIRONMENT_ID.to_string(),
            executor_registration_id: EXECUTOR_REGISTRATION_ID.to_string(),
            identity: NoiseChannelIdentity::generate()?,
            responder_public_key: executor_identity.public_key(),
            harness_key_authorization: "authorization".to_string(),
        },
    );

    let resume_message = timeout(Duration::from_secs(1), executor_websocket.next())
        .await?
        .context("harness closed before sending resume")??;
    let Message::Binary(resume_payload) = resume_message else {
        anyhow::bail!("expected resume frame, got {resume_message:?}");
    };
    let resume = decode_relay_message_frame(resume_payload.as_ref())?;
    assert_eq!(resume.validate()?, RelayFrameBodyKind::Resume);

    let handshake_message = timeout(Duration::from_secs(1), executor_websocket.next())
        .await?
        .context("harness closed before sending handshake")??;
    let Message::Binary(handshake_payload) = handshake_message else {
        anyhow::bail!("expected handshake frame, got {handshake_message:?}");
    };
    let handshake = decode_relay_message_frame(handshake_payload.as_ref())?;
    assert_eq!(handshake.stream_id, resume.stream_id);
    let stream_id = handshake.stream_id.clone();
    let prologue =
        noise_channel_prologue(ENVIRONMENT_ID, EXECUTOR_REGISTRATION_ID, stream_id.as_str());
    let pending = PendingResponderHandshake::read_request(
        &executor_identity,
        &prologue,
        &handshake.into_handshake_payload()?,
    )?;
    let (_transport, response) = pending.complete()?;
    let response = RelayMessageFrame::handshake(stream_id, response);
    executor_websocket
        .send(Message::Binary(
            encode_relay_message_frame(&response).into(),
        ))
        .await?;

    let mut pings = 0;
    while pings < 6 {
        let message = timeout(Duration::from_secs(1), executor_websocket.next())
            .await?
            .context("harness disconnected before six keepalive pings")??;
        match message {
            Message::Ping(payload) => {
                executor_websocket.send(Message::Pong(payload)).await?;
                pings += 1;
            }
            Message::Pong(_) | Message::Frame(_) => {}
            message => anyhow::bail!("expected keepalive ping, got {message:?}"),
        }
    }

    // Keep non-Pong traffic flowing after responses stop. It must not defeat
    // the bounded grace for a Pong already queued behind data.
    let unrelated_frame =
        encode_relay_message_frame(&RelayMessageFrame::resume("unrelated-stream".to_string()));
    let traffic_task = tokio::spawn(async move {
        loop {
            if executor_websocket
                .send(Message::Binary(unrelated_frame.clone().into()))
                .await
                .is_err()
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    });
    let event = timeout(Duration::from_secs(1), connection.incoming_rx.recv()).await?;
    traffic_task.abort();
    let _ = traffic_task.await;
    let Some(JsonRpcConnectionEvent::Disconnected { reason }) = event else {
        anyhow::bail!("expected pong timeout, got {event:?}");
    };
    assert_eq!(reason.as_deref(), Some(WEBSOCKET_PONG_TIMEOUT_REASON));
    Ok(())
}

#[tokio::test]
async fn application_event_delivery_is_bounded() -> Result<()> {
    let (incoming_tx, _incoming_rx) = mpsc::channel(1);
    incoming_tx
        .send(JsonRpcConnectionEvent::MalformedMessage {
            reason: "fill queue".to_string(),
        })
        .await?;

    let result = send_incoming_event(
        &incoming_tx,
        JsonRpcConnectionEvent::MalformedMessage {
            reason: "blocked event".to_string(),
        },
        Instant::now() + Duration::from_millis(10),
    )
    .await;

    assert!(matches!(result, Err(ExecServerError::Closed)));
    Ok(())
}

async fn read_outbound_data(
    outbound_rx: &mut futures_mpsc::UnboundedReceiver<Message>,
) -> Result<RelayData> {
    let Message::Binary(payload) = timeout(Duration::from_secs(1), outbound_rx.next())
        .await?
        .context("harness closed before sending data")?
    else {
        anyhow::bail!("expected relay data frame");
    };
    let frame = decode_relay_message_frame(payload.as_ref())?;
    assert_eq!(frame.validate()?, RelayFrameBodyKind::Data);
    frame.into_data().map_err(anyhow::Error::from)
}

async fn connected_controlled_harness() -> Result<(
    JsonRpcConnection,
    ControlledWebSocketHandle,
    futures_mpsc::UnboundedReceiver<Message>,
)> {
    let (websocket, control, mut outbound_rx) = ControlledWebSocket::new(/*write_permits*/ 2);
    let executor_identity = NoiseChannelIdentity::generate()?;
    let connection = noise_harness_connection_from_websocket(
        websocket,
        NoiseHarnessConnectionArgs {
            connection_label: "test rendezvous".to_string(),
            environment_id: ENVIRONMENT_ID.to_string(),
            executor_registration_id: EXECUTOR_REGISTRATION_ID.to_string(),
            identity: NoiseChannelIdentity::generate()?,
            responder_public_key: executor_identity.public_key(),
            harness_key_authorization: "authorization".to_string(),
        },
    );

    let Message::Binary(resume_payload) = timeout(Duration::from_secs(1), outbound_rx.next())
        .await?
        .context("harness closed before sending resume")?
    else {
        anyhow::bail!("expected resume frame");
    };
    let resume = decode_relay_message_frame(resume_payload.as_ref())?;
    let Message::Binary(handshake_payload) = timeout(Duration::from_secs(1), outbound_rx.next())
        .await?
        .context("harness closed before sending handshake")?
    else {
        anyhow::bail!("expected handshake frame");
    };
    let handshake = decode_relay_message_frame(handshake_payload.as_ref())?;
    let stream_id = handshake.stream_id.clone();
    assert_eq!(stream_id, resume.stream_id);
    let prologue =
        noise_channel_prologue(ENVIRONMENT_ID, EXECUTOR_REGISTRATION_ID, stream_id.as_str());
    let pending = PendingResponderHandshake::read_request(
        &executor_identity,
        &prologue,
        &handshake.into_handshake_payload()?,
    )?;
    let (_transport, response) = pending.complete()?;
    control.send_inbound(Message::Binary(
        encode_relay_message_frame(&RelayMessageFrame::handshake(stream_id, response)).into(),
    ))?;
    Ok((connection, control, outbound_rx))
}

struct ControlledWebSocket {
    inbound_rx: futures_mpsc::UnboundedReceiver<Result<Message, std::convert::Infallible>>,
    outbound_tx: futures_mpsc::UnboundedSender<Message>,
    write_permit_rx: futures_mpsc::UnboundedReceiver<()>,
    blocked_write_tx: futures_mpsc::UnboundedSender<usize>,
    write_waiting: bool,
    blocked_writes: usize,
    inbound_reads: Arc<AtomicUsize>,
}

struct ControlledWebSocketHandle {
    inbound_tx: futures_mpsc::UnboundedSender<Result<Message, std::convert::Infallible>>,
    write_permit_tx: futures_mpsc::UnboundedSender<()>,
    blocked_write_rx: futures_mpsc::UnboundedReceiver<usize>,
    inbound_reads: Arc<AtomicUsize>,
}

impl ControlledWebSocket {
    fn new(
        write_permits: usize,
    ) -> (
        Self,
        ControlledWebSocketHandle,
        futures_mpsc::UnboundedReceiver<Message>,
    ) {
        let (inbound_tx, inbound_rx) = futures_mpsc::unbounded();
        let (outbound_tx, outbound_rx) = futures_mpsc::unbounded();
        let (write_permit_tx, write_permit_rx) = futures_mpsc::unbounded();
        let (blocked_write_tx, blocked_write_rx) = futures_mpsc::unbounded();
        for _ in 0..write_permits {
            write_permit_tx
                .unbounded_send(())
                .expect("test write permit receiver should stay open");
        }
        let inbound_reads = Arc::new(AtomicUsize::new(0));
        (
            Self {
                inbound_rx,
                outbound_tx,
                write_permit_rx,
                blocked_write_tx,
                write_waiting: false,
                blocked_writes: 0,
                inbound_reads: Arc::clone(&inbound_reads),
            },
            ControlledWebSocketHandle {
                inbound_tx,
                write_permit_tx,
                blocked_write_rx,
                inbound_reads,
            },
            outbound_rx,
        )
    }
}

impl ControlledWebSocketHandle {
    fn send_inbound(&self, message: Message) -> Result<()> {
        self.inbound_tx
            .unbounded_send(Ok(message))
            .map_err(anyhow::Error::from)
    }

    fn grant_writes(&self, count: usize) {
        for _ in 0..count {
            self.write_permit_tx
                .unbounded_send(())
                .expect("test write permit receiver should stay open");
        }
    }

    fn inbound_reads(&self) -> usize {
        self.inbound_reads.load(Ordering::Acquire)
    }

    async fn wait_for_blocked_write(&mut self, expected: usize) -> Result<()> {
        let actual = timeout(Duration::from_secs(1), self.blocked_write_rx.next())
            .await?
            .context("websocket closed before blocking the expected write")?;
        assert_eq!(actual, expected);
        Ok(())
    }
}

impl Sink<Message> for ControlledWebSocket {
    type Error = std::convert::Infallible;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.get_mut();
        match Pin::new(&mut this.write_permit_rx).poll_next(cx) {
            Poll::Ready(Some(())) => {
                this.write_waiting = false;
                Poll::Ready(Ok(()))
            }
            Poll::Ready(None) | Poll::Pending => {
                if !this.write_waiting {
                    this.write_waiting = true;
                    this.blocked_writes += 1;
                    this.blocked_write_tx
                        .unbounded_send(this.blocked_writes)
                        .expect("test blocked-write receiver should stay open");
                }
                Poll::Pending
            }
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
        _cx: &mut TaskContext<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(
        self: Pin<&mut Self>,
        _cx: &mut TaskContext<'_>,
    ) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
}

impl futures::Stream for ControlledWebSocket {
    type Item = Result<Message, std::convert::Infallible>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Option<Self::Item>> {
        let result = Pin::new(&mut self.inbound_rx).poll_next(cx);
        if matches!(result, Poll::Ready(Some(_))) {
            self.inbound_reads.fetch_add(1, Ordering::Release);
        }
        result
    }
}
