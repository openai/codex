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
use codex_protocol::protocol::W3cTraceContext;
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

fn noise_harness_connection_from_websocket<T>(
    stream: T,
    args: NoiseHarnessConnectionArgs,
) -> JsonRpcConnection
where
    T: Sink<Message, Error = WebSocketError>
        + Stream<Item = Result<Message, WebSocketError>>
        + Unpin
        + Send
        + 'static,
{
    noise_harness_connection_from_websocket_with_readiness(stream, args).connection
}

const ENVIRONMENT_ID: &str = "environment-1";
const EXECUTOR_REGISTRATION_ID: &str = "registration-1";

#[test]
fn websocket_read_diagnostics_use_typed_errors_only() {
    let reset = std::io::Error::new(std::io::ErrorKind::ConnectionReset, "private data");
    for (error, expected) in [
        (WebSocketError::Io(reset), "websocket_read_connection_reset"),
        (
            WebSocketError::Protocol(ProtocolError::ResetWithoutClosingHandshake),
            "websocket_read_reset_without_close",
        ),
        (
            WebSocketError::Protocol(ProtocolError::InvalidOpcode(255)),
            "websocket_read_protocol",
        ),
    ] {
        assert_eq!(websocket_read_error_reason(&error), expected);
    }
}

#[tokio::test(start_paused = true)]
async fn first_encrypted_request_frame_exposes_only_its_trace_context() -> Result<()> {
    use opentelemetry::trace::TracerProvider as _;
    use opentelemetry_sdk::trace::InMemorySpanExporter;
    use opentelemetry_sdk::trace::SdkTracerProvider;
    use tracing_subscriber::prelude::*;
    let exporter = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_simple_exporter(exporter.clone())
        .build();
    let events = SafeEvents::default();
    let _subscriber = tracing::subscriber::set_default(
        tracing_subscriber::registry()
            .with(
                events
                    .clone()
                    .with_filter(tracing_subscriber::filter::LevelFilter::INFO),
            )
            .with(
                tracing_opentelemetry::layer()
                    .with_tracer(provider.tracer("harness-test"))
                    .with_filter(tracing_subscriber::filter::LevelFilter::INFO),
            ),
    );
    let (connection, mut control, mut outbound_rx) = connected_controlled_harness().await?;
    let traceparent = "00-00000000000000000000000000000001-0000000000000002-01";
    let tracestate = "dd=s:1";

    connection
        .outgoing_tx
        .send(JSONRPCMessage::Request(JSONRPCRequest {
            id: RequestId::Integer(1),
            method: "fs/getMetadata".to_string(),
            params: Some(serde_json::json!({
                "payload": "x".repeat(NOISE_RECORD_PLAINTEXT_LEN * 2),
            })),
            trace: Some(W3cTraceContext {
                traceparent: Some(traceparent.to_string()),
                tracestate: Some(tracestate.to_string()),
            }),
        }))
        .await?;

    control.wait_for_blocked_write(/*expected*/ 1).await?;
    {
        let captured = events.0.lock().unwrap();
        let dequeue = captured
            .iter()
            .find(|event| {
                event
                    .get("event.name")
                    .is_some_and(|name| name.contains("request_dequeued"))
            })
            .context("live dequeue")?;
        assert_eq!(
            dequeue.get("trace_id").map(String::as_str),
            Some("00000000000000000000000000000001")
        );
        assert!(
            exporter
                .get_finished_spans()?
                .iter()
                .all(|span| span.name != "codex.exec_server.noise_request_write")
        );
    }
    control.grant_writes(/*count*/ 1);
    let first = read_outbound_frame(&mut outbound_rx).await?;
    assert_eq!(
        (first.traceparent.as_deref(), first.tracestate.as_deref()),
        (Some(traceparent), Some(tracestate))
    );
    let stream_id = first.stream_id.clone();
    let first_payload = first.into_data()?.payload;
    assert!(
        !first_payload
            .windows("fs/getMetadata".len())
            .any(|window| window == b"fs/getMetadata")
    );

    control.wait_for_blocked_write(/*expected*/ 2).await?;
    control.grant_writes(/*count*/ 1);
    let second = read_outbound_frame(&mut outbound_rx).await?;
    assert_eq!((second.traceparent, second.tracestate), (None, None));
    assert!(!format!("{:?}", events.0.lock().unwrap()).contains("request_written"));
    control.wait_for_blocked_write(/*expected*/ 3).await?;
    control.grant_writes(/*count*/ 1);
    read_outbound_frame(&mut outbound_rx).await?;
    let spans = exporter.get_finished_spans()?;
    let write_span = spans
        .iter()
        .find(|span| span.name == "codex.exec_server.noise_request_write")
        .context("expected finished request write span")?;
    assert_eq!(
        write_span.span_context.trace_id().to_string(),
        "00000000000000000000000000000001"
    );
    assert_eq!(write_span.parent_span_id.to_string(), "0000000000000002");
    let captured = events.0.lock().unwrap();
    let progress: Vec<_> = captured
        .iter()
        .filter(|e| {
            e.get("event.name").is_some_and(|name| {
                name.contains("request_dequeued") || name.contains("request_written")
            })
        })
        .collect();
    assert_eq!(progress.len(), 2);
    for event in &progress {
        assert_eq!(
            event.get("trace_id").map(String::as_str),
            Some("00000000000000000000000000000001")
        );
    }
    assert_eq!(progress[0]["stream_id"], format!("{stream_id:?}"));
    assert_eq!(progress[0]["stream_id"], progress[1]["stream_id"]);
    assert_eq!(
        progress[0]["message_sequence"],
        progress[1]["message_sequence"]
    );
    drop(captured);

    for task in &connection.task_handles {
        task.abort();
    }
    Ok(())
}

#[tokio::test(start_paused = true)]
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
    tokio::time::advance(WEBSOCKET_KEEPALIVE_INTERVAL + Duration::from_millis(10)).await;
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
    tokio::time::advance(WEBSOCKET_KEEPALIVE_INTERVAL + Duration::from_millis(10)).await;
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

/// A full consumer queue times out with occupancy and wait duration, without logging its data.
#[tokio::test(start_paused = true)]
async fn application_event_delivery_is_bounded() -> Result<()> {
    use tracing_subscriber::prelude::*;
    let events = SafeEvents::default();
    let _subscriber =
        tracing::subscriber::set_default(tracing_subscriber::registry().with(events.clone()));
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
    let events = events.0.lock().unwrap();
    assert_eq!(events.len(), 1);
    let event = &events[0];
    assert_eq!(event["noise_reason"], "\"application_backpressure\"");
    assert_eq!(event["incoming_queue_depth"], "1");
    assert_eq!(event["incoming_queue_capacity"], "1");
    assert!(event["delivery_wait_ms"].parse::<u64>()? >= 10);
    assert!(!format!("{events:?}").contains("blocked event"));
    Ok(())
}

async fn read_outbound_data(
    outbound_rx: &mut futures_mpsc::UnboundedReceiver<Message>,
) -> Result<RelayData> {
    read_outbound_frame(outbound_rx)
        .await?
        .into_data()
        .map_err(anyhow::Error::from)
}

async fn read_outbound_frame(
    outbound_rx: &mut futures_mpsc::UnboundedReceiver<Message>,
) -> Result<RelayMessageFrame> {
    let Message::Binary(payload) = timeout(Duration::from_secs(1), outbound_rx.next())
        .await?
        .context("harness closed before sending data")?
    else {
        anyhow::bail!("expected relay data frame");
    };
    let frame = decode_relay_message_frame(payload.as_ref())?;
    assert_eq!(frame.validate()?, RelayFrameBodyKind::Data);
    Ok(frame)
}

async fn connected_controlled_harness() -> Result<(
    JsonRpcConnection,
    ControlledWebSocketHandle,
    futures_mpsc::UnboundedReceiver<Message>,
)> {
    let (connection, control, outbound_rx, _transport, _stream_id) =
        connected_controlled_harness_with_transport().await?;
    Ok((connection, control, outbound_rx))
}

async fn connected_controlled_harness_with_transport() -> Result<(
    JsonRpcConnection,
    ControlledWebSocketHandle,
    futures_mpsc::UnboundedReceiver<Message>,
    NoiseTransport,
    String,
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
    let (transport, response) = pending.complete()?;
    control.send_inbound(Message::Binary(
        encode_relay_message_frame(&RelayMessageFrame::handshake(stream_id.clone(), response))
            .into(),
    ))?;
    Ok((connection, control, outbound_rx, transport, stream_id))
}

struct ControlledWebSocket {
    inbound_rx: futures_mpsc::UnboundedReceiver<Result<Message, WebSocketError>>,
    outbound_tx: futures_mpsc::UnboundedSender<Message>,
    write_permit_rx: futures_mpsc::UnboundedReceiver<()>,
    blocked_write_tx: futures_mpsc::UnboundedSender<usize>,
    write_waiting: bool,
    blocked_writes: usize,
    inbound_reads: Arc<AtomicUsize>,
}

struct ControlledWebSocketHandle {
    inbound_tx: futures_mpsc::UnboundedSender<Result<Message, WebSocketError>>,
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
    type Error = WebSocketError;

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
    type Item = Result<Message, WebSocketError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Option<Self::Item>> {
        let result = Pin::new(&mut self.inbound_rx).poll_next(cx);
        if matches!(result, Poll::Ready(Some(_))) {
            self.inbound_reads.fetch_add(1, Ordering::Release);
        }
        result
    }
}

#[derive(Clone, Default)]
struct SafeEvents(Arc<std::sync::Mutex<Vec<std::collections::BTreeMap<String, String>>>>);

impl<S> tracing_subscriber::Layer<S> for SafeEvents
where
    S: tracing::Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        context: tracing_subscriber::layer::Context<'_, S>,
    ) {
        if event.metadata().target() != "codex_otel.trace_safe" {
            return;
        }
        #[derive(Default)]
        struct Fields(std::collections::BTreeMap<String, String>);
        impl tracing::field::Visit for Fields {
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                self.0.insert(field.name().to_owned(), format!("{value:?}"));
            }
        }
        let mut fields = Fields::default();
        event.record(&mut fields);
        // Match CCA's live JSON lookup: inspecting OtelData must not materialize
        // the context and accidentally hide a missing producer initialization.
        if let Some(trace_id) = context.event_scope(event).and_then(|mut scope| {
            scope.find_map(|span| {
                span.extensions()
                    .get::<tracing_opentelemetry::OtelData>()?
                    .trace_id()
            })
        }) {
            fields.0.insert("trace_id".to_owned(), trace_id.to_string());
        }
        self.0.lock().unwrap().push(fields.0);
    }
}

/// A blocked write can time out with a Pong queued; its diagnostic must distinguish this
/// local write stall from the read loop's missing-Pong watchdog.
#[tokio::test(start_paused = true)]
async fn blocked_write_reports_timeout_with_pong_still_queued() -> Result<()> {
    use tracing_subscriber::prelude::*;
    let events = SafeEvents::default();
    let _subscriber =
        tracing::subscriber::set_default(tracing_subscriber::registry().with(events.clone()));
    let (mut connection, mut control, mut outbound_rx) = connected_controlled_harness().await?;
    control.wait_for_blocked_write(/*expected*/ 1).await?;
    control.grant_writes(/*count*/ 1);
    let Message::Ping(payload) = outbound_rx.next().await.context("expected Ping")? else {
        anyhow::bail!("expected Ping before blocked write");
    };
    connection
        .outgoing_tx
        .send(JSONRPCMessage::Request(JSONRPCRequest {
            id: RequestId::Integer(1),
            method: "SYNTHETIC_PRIVATE_METHOD".to_owned(),
            params: Some(serde_json::json!({"secret": "SYNTHETIC_PRIVATE_PAYLOAD"})),
            trace: None,
        }))
        .await?;
    control.wait_for_blocked_write(/*expected*/ 2).await?;
    connection
        .outgoing_tx
        .send(JSONRPCMessage::Request(JSONRPCRequest {
            id: RequestId::Integer(2),
            method: "SYNTHETIC_PRIVATE_QUEUED".to_owned(),
            params: None,
            trace: None,
        }))
        .await?;
    let reads = control.inbound_reads();
    control.send_inbound(Message::Pong(payload))?;
    tokio::time::advance(WEBSOCKET_PONG_TIMEOUT + Duration::from_millis(1)).await;
    for task in connection.task_handles.drain(..) {
        task.await?;
    }
    assert_eq!(control.inbound_reads(), reads);
    assert!(*connection.disconnected_rx.borrow());
    let events = events.0.lock().unwrap();
    let terminal: Vec<_> = events
        .iter()
        .filter(|e| {
            e.get("event.name")
                .is_some_and(|name| name == "\"codex.exec_server.noise_transport_closed\"")
        })
        .collect();
    assert_eq!(terminal.len(), 1);
    let event = terminal[0];
    assert_eq!(event["noise_reason"], "\"websocket_write_timeout\"");
    assert_eq!(event["write_pending"], "true");
    assert_eq!(event["write_kind"], "\"data\"");
    assert_eq!(event["pong_pending"], "true");
    assert_eq!(event["records_written"], "0");
    assert_eq!(event["messages_dequeued"], "1");
    assert_eq!(event["messages_written"], "0");
    assert_eq!(event["outgoing_queue_depth"], "1");
    assert_eq!(event["pings_written"], "1");
    assert_eq!(event["pongs_received"], "0");
    assert!(!format!("{events:?}").contains("request_written"));
    assert!(event["write_wait_ms"].parse::<u64>()? >= WEBSOCKET_PONG_TIMEOUT.as_millis() as u64);
    assert!(!format!("{events:?}").contains("SYNTHETIC_PRIVATE"));
    Ok(())
}

/// Peer-controlled read errors must produce a fixed terminal category without retaining
/// the raw error, even though ordinary debug logs still carry it.
#[tokio::test(start_paused = true)]
async fn read_error_reports_safe_terminal_category() -> Result<()> {
    use tracing_subscriber::prelude::*;
    let events = SafeEvents::default();
    let _subscriber =
        tracing::subscriber::set_default(tracing_subscriber::registry().with(events.clone()));
    let (mut connection, control, _outbound_rx) = connected_controlled_harness().await?;
    control
        .inbound_tx
        .unbounded_send(Err(WebSocketError::Io(std::io::Error::other(
            "SYNTHETIC_PRIVATE_ERROR",
        ))))?;
    for task in connection.task_handles.drain(..) {
        task.await?;
    }
    let events = events.0.lock().unwrap();
    let event = events.last().context("expected terminal event")?;
    assert_eq!(
        event["event.name"],
        "\"codex.exec_server.noise_transport_closed\""
    );
    assert_eq!(event["noise_reason"], "\"websocket_read\"");
    assert_eq!(event["read_reason"], "\"websocket_read_io_other\"");
    assert!(!format!("{events:?}").contains("SYNTHETIC_PRIVATE_ERROR"));
    Ok(())
}

/// A keepalive write stall has no outstanding Pong yet; retain that distinction
/// when a disconnect aligns with the periodic keepalive schedule.
#[tokio::test(start_paused = true)]
async fn blocked_keepalive_reports_write_phase_without_pong_deadline() -> Result<()> {
    use tracing_subscriber::prelude::*;
    let events = SafeEvents::default();
    let _subscriber =
        tracing::subscriber::set_default(tracing_subscriber::registry().with(events.clone()));
    let (mut connection, mut control, _outbound_rx) = connected_controlled_harness().await?;
    control.wait_for_blocked_write(/*expected*/ 1).await?;
    tokio::time::advance(WEBSOCKET_PONG_TIMEOUT + Duration::from_millis(1)).await;
    for task in connection.task_handles.drain(..) {
        task.await?;
    }
    let events = events.0.lock().unwrap();
    let event = events.last().context("expected terminal event")?;
    assert_eq!(event["noise_reason"], "\"websocket_write_timeout\"");
    assert_eq!(event["write_kind"], "\"keepalive\"");
    assert_eq!(event["pong_pending"], "false");
    assert_eq!(event["write_pending"], "true");
    Ok(())
}

/// Successful local writes with no inbound traffic reproduce a blackholed path,
/// which must report a Pong timeout rather than a blocked write.
#[tokio::test(start_paused = true)]
async fn blackholed_peer_reports_pong_timeout_after_successful_writes() -> Result<()> {
    use tracing_subscriber::prelude::*;
    let events = SafeEvents::default();
    let _subscriber =
        tracing::subscriber::set_default(tracing_subscriber::registry().with(events.clone()));
    let (mut connection, mut control, mut outbound_rx) = connected_controlled_harness().await?;
    control.wait_for_blocked_write(/*expected*/ 1).await?;
    control.grant_writes(/*count*/ 1);
    assert!(matches!(outbound_rx.next().await, Some(Message::Ping(_))));
    connection
        .outgoing_tx
        .send(JSONRPCMessage::Request(JSONRPCRequest {
            id: RequestId::Integer(1),
            method: "test".to_owned(),
            params: None,
            trace: None,
        }))
        .await?;
    control.wait_for_blocked_write(/*expected*/ 2).await?;
    control.grant_writes(/*count*/ 1);
    read_outbound_data(&mut outbound_rx).await?;
    tokio::time::advance(WEBSOCKET_PONG_TIMEOUT / 2).await;
    assert!(!*connection.disconnected_rx.borrow());
    tokio::time::advance(WEBSOCKET_PONG_TIMEOUT / 2 + Duration::from_millis(1)).await;
    for task in connection.task_handles.drain(..) {
        task.await?;
    }
    let events = events.0.lock().unwrap();
    let event = events.last().context("expected terminal event")?;
    assert_eq!(event["noise_reason"], "\"pong_timeout\"");
    assert_eq!(event["write_pending"], "false");
    assert_eq!(event["records_written"], "1");
    assert_eq!(event["messages_dequeued"], "1");
    assert_eq!(event["messages_written"], "1");
    assert_eq!(event["pings_written"], "1");
    assert_eq!(event["pongs_received"], "0");
    assert!(format!("{events:?}").contains("request_written"));
    assert_eq!(event["pong_pending"], "true");
    Ok(())
}

/// A stalled application consumer can prevent the shared loop from reading an
/// already queued Pong; retain queue saturation separately from network timeout.
#[tokio::test(start_paused = true)]
async fn full_application_queue_reports_backpressure_with_pong_queued() -> Result<()> {
    use tracing_subscriber::prelude::*;
    let events = SafeEvents::default();
    let _subscriber =
        tracing::subscriber::set_default(tracing_subscriber::registry().with(events.clone()));
    let (mut connection, mut control, mut outbound_rx, mut transport, stream_id) =
        connected_controlled_harness_with_transport().await?;
    control.wait_for_blocked_write(/*expected*/ 1).await?;
    control.grant_writes(/*count*/ 1);
    let Some(Message::Ping(payload)) = outbound_rx.next().await else {
        anyhow::bail!("expected Ping");
    };
    let framed = frame_jsonrpc_message(&JSONRPCMessage::Response(
        codex_exec_server_protocol::JSONRPCResponse {
            id: RequestId::Integer(1),
            result: serde_json::json!({"private": "SYNTHETIC_PRIVATE_RESPONSE"}),
        },
    ))?;
    let reads = control.inbound_reads();
    for seq in 0..=CHANNEL_CAPACITY {
        let frame = RelayMessageFrame::data(
            stream_id.clone(),
            seq as u32,
            transport.encrypt(&framed)?,
            /*trace*/ None,
        );
        control.send_inbound(Message::Binary(encode_relay_message_frame(&frame).into()))?;
    }
    control.send_inbound(Message::Pong(payload))?;
    // Awaiting the task lets the paused clock advance only once frame processing
    // has filled the application queue and is waiting on its delivery deadline.
    for task in connection.task_handles.drain(..) {
        task.await?;
    }
    assert_eq!(control.inbound_reads() - reads, CHANNEL_CAPACITY + 1);
    assert_eq!(connection.incoming_rx.len(), CHANNEL_CAPACITY);
    let events = events.0.lock().unwrap();
    let backpressure = events
        .iter()
        .find(|event| {
            event
                .get("noise_reason")
                .is_some_and(|value| value == "\"application_backpressure\"")
        })
        .context("expected application backpressure")?;
    assert_eq!(
        backpressure["incoming_queue_depth"],
        CHANNEL_CAPACITY.to_string()
    );
    let terminal = events.last().context("expected terminal event")?;
    assert_eq!(terminal["noise_reason"], "\"application_delivery\"");
    assert_eq!(terminal["pong_pending"], "true");
    assert!(!format!("{events:?}").contains("SYNTHETIC_PRIVATE_RESPONSE"));
    Ok(())
}

/// A responsive relay can coexist with a missing executor response. Ping/Pong
/// progress must remain distinct from successful application delivery.
#[tokio::test(start_paused = true)]
async fn responsive_relay_without_application_response_reports_progress() -> Result<()> {
    use tracing_subscriber::prelude::*;
    let events = SafeEvents::default();
    let _subscriber =
        tracing::subscriber::set_default(tracing_subscriber::registry().with(events.clone()));
    let (mut connection, mut control, mut outbound_rx) = connected_controlled_harness().await?;
    connection
        .outgoing_tx
        .send(JSONRPCMessage::Request(JSONRPCRequest {
            id: RequestId::Integer(1),
            method: "SYNTHETIC_PRIVATE_METHOD".to_owned(),
            params: None,
            trace: None,
        }))
        .await?;
    control.wait_for_blocked_write(/*expected*/ 1).await?;
    control.grant_writes(/*count*/ 1);
    read_outbound_data(&mut outbound_rx).await?;
    for expected in 2..=3 {
        control.wait_for_blocked_write(expected).await?;
        control.grant_writes(/*count*/ 1);
        let Some(Message::Ping(payload)) = outbound_rx.next().await else {
            anyhow::bail!("expected Ping")
        };
        for _ in 0..3 {
            control.send_inbound(Message::Pong(payload.clone()))?;
        }
    }
    // A subsequent Close is ordered behind the last Pong and completes the summary.
    control.send_inbound(Message::Close(None))?;
    for task in connection.task_handles.drain(..) {
        task.await?;
    }
    assert!(connection.incoming_rx.is_empty());
    let captured = events.0.lock().unwrap();
    let terminal = captured.last().context("expected terminal")?;
    assert_eq!(terminal["noise_reason"], "\"websocket_close\"");
    assert_eq!(terminal["messages_written"], "1");
    assert_eq!(terminal["pings_written"], "2");
    assert_eq!(terminal["pongs_received"], "2");
    let heartbeats: Vec<_> = captured
        .iter()
        .filter_map(|event| event.get("phase"))
        .collect();
    assert_eq!(
        heartbeats,
        vec![
            "\"ping_written\"",
            "\"pong_received\"",
            "\"ping_written\"",
            "\"pong_received\""
        ]
    );
    assert!(!format!("{captured:?}").contains("SYNTHETIC_PRIVATE"));
    Ok(())
}
