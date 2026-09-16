use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::Result;
use codex_exec_server_protocol::JSONRPCMessage;
use codex_exec_server_protocol::JSONRPCResponse;
use codex_exec_server_protocol::RequestId;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::net::TcpListener;
use tokio::sync::Notify;
use tokio::time::timeout;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_util::task::AbortOnDropHandle;

use super::HarnessKeyValidator;
use super::MAX_FAILED_NOISE_HANDSHAKES;
use super::MAX_HARNESS_KEY_AUTHORIZATION_BYTES;
use super::NOISE_HANDSHAKE_FAILURE_COOLDOWN;
use super::RendezvousDisconnectReason;
use super::run_multiplexed_environment;
use crate::ExecServerError;
use crate::ExecServerRuntimePaths;
use crate::connection::JsonRpcConnectionEvent;
use crate::noise_channel::InitiatorHandshake;
use crate::noise_channel::NoiseChannelIdentity;
use crate::noise_channel::NoiseChannelPublicKey;
use crate::noise_channel::noise_channel_prologue;
use crate::noise_relay::NoiseHarnessConnectionArgs;
use crate::noise_relay::message_framing::JsonRpcMessageDecoder;
use crate::noise_relay::message_framing::frame_jsonrpc_message;
use crate::noise_relay::noise_harness_connection_from_websocket_with_readiness;
use crate::noise_relay::stream_handler::NoiseOutboundMessage;
use crate::noise_relay::stream_handler::NoiseStreamConnection;
use crate::noise_relay::stream_handler::NoiseStreamHandler;
use crate::relay::RelayFrameBodyKind;
use crate::relay::decode_relay_message_frame;
use crate::relay::encode_relay_message_frame;
use crate::relay_proto::RelayMessageFrame;
use crate::server::ConnectionProcessor;

const ENVIRONMENT_ID: &str = "environment-1";
const EXECUTOR_REGISTRATION_ID: &str = "registration-1";

async fn next_relay_frame<S>(websocket: &mut WebSocketStream<S>) -> Result<RelayMessageFrame>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    loop {
        match websocket.next().await {
            Some(Ok(Message::Binary(payload))) => {
                return Ok(decode_relay_message_frame(payload.as_ref())?);
            }
            Some(Ok(Message::Ping(payload))) => websocket.send(Message::Pong(payload)).await?,
            Some(Ok(Message::Pong(_) | Message::Frame(_))) => {}
            Some(Ok(message)) => anyhow::bail!("expected relay frame, got {message:?}"),
            Some(Err(error)) => return Err(error.into()),
            None => anyhow::bail!("environment closed before sending relay frame"),
        }
    }
}

#[derive(Clone)]
struct EchoMessages;

impl NoiseStreamHandler for EchoMessages {
    type Incoming = JSONRPCMessage;
    type Outgoing = JSONRPCMessage;

    fn decode(payload: bytes::Bytes) -> Result<Self::Incoming, ExecServerError> {
        Ok(serde_json::from_slice(&payload)?)
    }

    fn encode(message: Self::Outgoing) -> Result<NoiseOutboundMessage, ExecServerError> {
        ConnectionProcessor::encode(message)
    }

    async fn run_connection(
        self,
        mut connection: NoiseStreamConnection<Self::Incoming, Self::Outgoing>,
    ) {
        while let Some(message) = connection.incoming_rx.recv().await {
            if connection.outgoing_tx.send(message).await.is_err() {
                break;
            }
        }
    }
}

#[derive(Clone)]
struct ObservedRegistration(
    ConnectionProcessor,
    tokio::sync::mpsc::Sender<Option<(String, String)>>,
);

impl NoiseStreamHandler for ObservedRegistration {
    type Incoming = JsonRpcConnectionEvent;
    type Outgoing = JSONRPCMessage;

    fn decode(payload: bytes::Bytes) -> Result<Self::Incoming, ExecServerError> {
        ConnectionProcessor::decode(payload)
    }
    fn encode(message: Self::Outgoing) -> Result<NoiseOutboundMessage, ExecServerError> {
        ConnectionProcessor::encode(message)
    }
    async fn run_connection(
        self,
        connection: NoiseStreamConnection<Self::Incoming, Self::Outgoing>,
    ) {
        let registration = connection
            .executor_registration
            .as_ref()
            .map(|registration| {
                (
                    registration.environment_id.clone(),
                    registration.executor_registration_id.clone(),
                )
            });
        let _ = self.1.send(registration).await;
        NoiseStreamHandler::run_connection(self.0, connection).await;
    }
}

#[tokio::test]
async fn missing_pong_disconnects_physical_relay() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let websocket_url = format!("ws://{}", listener.local_addr()?);
    let harness_connection = tokio::spawn(connect_async(websocket_url));
    let (socket, _peer_addr) = listener.accept().await?;
    let environment_websocket = accept_async(socket).await?;
    let (_harness_websocket, _response) = harness_connection.await??;

    let environment_task = tokio::spawn(run_multiplexed_environment(
        environment_websocket,
        ConnectionProcessor::new(ExecServerRuntimePaths::new(
            std::env::current_exe()?,
            /*codex_linux_sandbox_exe*/ None,
        )?),
        ENVIRONMENT_ID.to_string(),
        EXECUTOR_REGISTRATION_ID.to_string(),
        NoiseChannelIdentity::generate()?,
        BlockingValidator {
            calls: Arc::new(AtomicUsize::new(0)),
            release: Arc::new(Notify::new()),
        },
    ));

    assert_eq!(
        timeout(Duration::from_secs(1), environment_task).await??,
        RendezvousDisconnectReason::PongTimeout
    );
    Ok(())
}

#[tokio::test]
async fn pong_keeps_physical_relay_connected() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let websocket_url = format!("ws://{}", listener.local_addr()?);
    let harness_connection = tokio::spawn(connect_async(websocket_url));
    let (socket, _peer_addr) = listener.accept().await?;
    let environment_websocket = accept_async(socket).await?;
    let (mut harness_websocket, _response) = harness_connection.await??;

    let environment_task = tokio::spawn(run_multiplexed_environment(
        environment_websocket,
        ConnectionProcessor::new(ExecServerRuntimePaths::new(
            std::env::current_exe()?,
            /*codex_linux_sandbox_exe*/ None,
        )?),
        ENVIRONMENT_ID.to_string(),
        EXECUTOR_REGISTRATION_ID.to_string(),
        NoiseChannelIdentity::generate()?,
        BlockingValidator {
            calls: Arc::new(AtomicUsize::new(0)),
            release: Arc::new(Notify::new()),
        },
    ));

    timeout(Duration::from_secs(1), async {
        let mut pings = 0;
        while pings < 6 {
            match harness_websocket.next().await {
                Some(Ok(Message::Ping(payload))) => {
                    harness_websocket.send(Message::Pong(payload)).await?;
                    pings += 1;
                }
                Some(Ok(Message::Pong(_) | Message::Frame(_))) => {}
                Some(Ok(message)) => anyhow::bail!("expected keepalive ping, got {message:?}"),
                Some(Err(error)) => return Err(error.into()),
                None => anyhow::bail!("environment disconnected before six keepalive pings"),
            }
        }
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    harness_websocket.close(None).await?;
    assert_eq!(
        timeout(Duration::from_secs(1), environment_task).await??,
        RendezvousDisconnectReason::PeerClose
    );
    Ok(())
}

#[derive(Clone)]
struct BlockingValidator {
    calls: Arc<AtomicUsize>,
    release: Arc<Notify>,
}

impl HarnessKeyValidator for BlockingValidator {
    fn validate_harness_key(
        &self,
        _harness_public_key: &NoiseChannelPublicKey,
        _authorization: &str,
    ) -> impl std::future::Future<Output = Result<(), ExecServerError>> + Send {
        let calls = Arc::clone(&self.calls);
        let release = Arc::clone(&self.release);
        async move {
            calls.fetch_add(1, Ordering::SeqCst);
            release.notified().await;
            Ok(())
        }
    }
}

#[tokio::test]
async fn processor_exit_resets_noise_harness_stream() -> Result<()> {
    let (registration_tx, mut registration_rx) = tokio::sync::mpsc::channel(1);
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let connecting = tokio::spawn(connect_async(format!("ws://{}", listener.local_addr()?)));
    let (socket, _) = listener.accept().await?;
    let environment_websocket = accept_async(socket).await?;
    let (harness_websocket, _) = connecting.await??;
    let identity = NoiseChannelIdentity::generate()?;
    let release = Arc::new(Notify::new());
    release.notify_one();
    let environment_task = AbortOnDropHandle::new(tokio::spawn(run_multiplexed_environment(
        environment_websocket,
        ObservedRegistration(
            ConnectionProcessor::new(ExecServerRuntimePaths::new(
                std::env::current_exe()?,
                /*codex_linux_sandbox_exe*/ None,
            )?),
            registration_tx,
        ),
        ENVIRONMENT_ID.to_string(),
        EXECUTOR_REGISTRATION_ID.to_string(),
        identity.clone(),
        BlockingValidator {
            calls: Arc::new(AtomicUsize::new(0)),
            release,
        },
    )));
    let mut connection = noise_harness_connection_from_websocket_with_readiness(
        harness_websocket,
        NoiseHarnessConnectionArgs {
            connection_label: "processor exit test".to_string(),
            environment_id: ENVIRONMENT_ID.to_string(),
            executor_registration_id: EXECUTOR_REGISTRATION_ID.to_string(),
            identity: NoiseChannelIdentity::generate()?,
            responder_public_key: identity.public_key(),
            harness_key_authorization: "authorization".to_string(),
        },
    )
    .connection;
    // Valid JSON reaches the processor; an unsolicited response closes it and
    // aborts its writer. The physical relay must still deliver the reset.
    connection
        .outgoing_tx
        .send(JSONRPCMessage::Response(JSONRPCResponse {
            id: RequestId::Integer(1),
            result: serde_json::Value::Null,
        }))
        .await?;
    assert_eq!(
        timeout(Duration::from_secs(1), registration_rx.recv()).await?,
        Some(Some((
            ENVIRONMENT_ID.to_string(),
            EXECUTOR_REGISTRATION_ID.to_string()
        ))),
    );
    assert!(matches!(
        timeout(Duration::from_secs(1), connection.incoming_rx.recv()).await?,
        Some(JsonRpcConnectionEvent::Disconnected { reason: Some(reason) })
            if reason == "Noise relay stream reset"
    ));
    for task in connection.task_handles {
        task.abort();
        let _ = task.await;
    }
    environment_task.abort();
    let _ = environment_task.await;
    Ok(())
}

#[tokio::test]
async fn pending_harness_key_validation_does_not_block_new_handshakes() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let websocket_url = format!("ws://{}", listener.local_addr()?);
    let harness_connection = tokio::spawn(connect_async(websocket_url));
    let (socket, _peer_addr) = listener.accept().await?;
    let environment_websocket = accept_async(socket).await?;
    let (mut harness_websocket, _response) = harness_connection.await??;

    let environment_identity = NoiseChannelIdentity::generate()?;
    let harness_identity = NoiseChannelIdentity::generate()?;
    let calls = Arc::new(AtomicUsize::new(0));
    let environment_task = tokio::spawn(run_multiplexed_environment(
        environment_websocket,
        ConnectionProcessor::new(ExecServerRuntimePaths::new(
            std::env::current_exe()?,
            /*codex_linux_sandbox_exe*/ None,
        )?),
        ENVIRONMENT_ID.to_string(),
        EXECUTOR_REGISTRATION_ID.to_string(),
        environment_identity.clone(),
        BlockingValidator {
            calls: Arc::clone(&calls),
            release: Arc::new(Notify::new()),
        },
    ));

    for stream_id in ["stream-1", "stream-2"] {
        let prologue = noise_channel_prologue(ENVIRONMENT_ID, EXECUTOR_REGISTRATION_ID, stream_id);
        let (_handshake, request) = InitiatorHandshake::start(
            &harness_identity,
            &environment_identity.public_key(),
            &prologue,
            b"authorization",
        )?;
        let frame = RelayMessageFrame::handshake(stream_id.to_string(), request);
        harness_websocket
            .send(Message::Binary(encode_relay_message_frame(&frame).into()))
            .await?;
    }

    timeout(Duration::from_secs(1), async {
        while calls.load(Ordering::SeqCst) != 2 {
            tokio::task::yield_now().await;
        }
    })
    .await?;

    harness_websocket.close(None).await?;
    timeout(Duration::from_secs(1), environment_task).await??;
    Ok(())
}

#[tokio::test]
async fn duplicate_handshakes_pause_admission_without_disconnecting() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let websocket_url = format!("ws://{}", listener.local_addr()?);
    let harness_connection = tokio::spawn(connect_async(websocket_url));
    let (socket, _peer_addr) = listener.accept().await?;
    let environment_websocket = accept_async(socket).await?;
    let (mut harness_websocket, _response) = harness_connection.await??;

    let environment_identity = NoiseChannelIdentity::generate()?;
    let harness_identity = NoiseChannelIdentity::generate()?;
    let calls = Arc::new(AtomicUsize::new(0));
    let release = Arc::new(Notify::new());
    let environment_task = tokio::spawn(run_multiplexed_environment(
        environment_websocket,
        ConnectionProcessor::new(ExecServerRuntimePaths::new(
            std::env::current_exe()?,
            /*codex_linux_sandbox_exe*/ None,
        )?),
        ENVIRONMENT_ID.to_string(),
        EXECUTOR_REGISTRATION_ID.to_string(),
        environment_identity.clone(),
        BlockingValidator {
            calls: Arc::clone(&calls),
            release: Arc::clone(&release),
        },
    ));

    let stream_id = "stream-1";
    let prologue = noise_channel_prologue(ENVIRONMENT_ID, EXECUTOR_REGISTRATION_ID, stream_id);
    let (_handshake, request) = InitiatorHandshake::start(
        &harness_identity,
        &environment_identity.public_key(),
        &prologue,
        b"authorization",
    )?;
    let frame = RelayMessageFrame::handshake(stream_id.to_string(), request);
    let encoded = encode_relay_message_frame(&frame);
    harness_websocket
        .send(Message::Binary(encoded.clone().into()))
        .await?;
    timeout(Duration::from_secs(1), async {
        while calls.load(Ordering::SeqCst) != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await?;

    for attempt in 1..MAX_FAILED_NOISE_HANDSHAKES {
        if attempt > 1 {
            harness_websocket
                .send(Message::Binary(encoded.clone().into()))
                .await?;
            timeout(Duration::from_secs(1), async {
                while calls.load(Ordering::SeqCst) != attempt {
                    tokio::task::yield_now().await;
                }
            })
            .await?;
        }
        harness_websocket
            .send(Message::Binary(encoded.clone().into()))
            .await?;
        let reset = timeout(
            Duration::from_secs(1),
            next_relay_frame(&mut harness_websocket),
        )
        .await??;
        assert_eq!(reset.stream_id, stream_id);
        assert_eq!(reset.validate()?, RelayFrameBodyKind::Reset);
    }

    harness_websocket
        .send(Message::Binary(encoded.clone().into()))
        .await?;
    timeout(Duration::from_secs(1), async {
        while calls.load(Ordering::SeqCst) != MAX_FAILED_NOISE_HANDSHAKES {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    harness_websocket
        .send(Message::Binary(encoded.clone().into()))
        .await?;
    let reset = timeout(
        Duration::from_secs(1),
        next_relay_frame(&mut harness_websocket),
    )
    .await??;
    assert_eq!(reset.validate()?, RelayFrameBodyKind::Reset);
    // The next valid request is rejected before starting another registry check.
    harness_websocket
        .send(Message::Binary(encoded.into()))
        .await?;
    let reset = timeout(
        Duration::from_secs(1),
        next_relay_frame(&mut harness_websocket),
    )
    .await??;
    assert_eq!(reset.validate()?, RelayFrameBodyKind::Reset);
    assert_eq!(calls.load(Ordering::SeqCst), MAX_FAILED_NOISE_HANDSHAKES);
    harness_websocket.close(None).await?;
    assert_eq!(
        timeout(Duration::from_secs(1), environment_task).await??,
        RendezvousDisconnectReason::PeerClose,
    );
    release.notify_waiters();
    Ok(())
}

#[tokio::test]
async fn oversized_harness_authorization_is_rejected_before_validation() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let websocket_url = format!("ws://{}", listener.local_addr()?);
    let harness_connection = tokio::spawn(connect_async(websocket_url));
    let (socket, _peer_addr) = listener.accept().await?;
    let environment_websocket = accept_async(socket).await?;
    let (mut harness_websocket, _response) = harness_connection.await??;

    let environment_identity = NoiseChannelIdentity::generate()?;
    let harness_identity = NoiseChannelIdentity::generate()?;
    let calls = Arc::new(AtomicUsize::new(0));
    let environment_task = tokio::spawn(run_multiplexed_environment(
        environment_websocket,
        ConnectionProcessor::new(ExecServerRuntimePaths::new(
            std::env::current_exe()?,
            /*codex_linux_sandbox_exe*/ None,
        )?),
        ENVIRONMENT_ID.to_string(),
        EXECUTOR_REGISTRATION_ID.to_string(),
        environment_identity.clone(),
        BlockingValidator {
            calls: Arc::clone(&calls),
            release: Arc::new(Notify::new()),
        },
    ));

    let stream_id = "stream-1";
    let prologue = noise_channel_prologue(ENVIRONMENT_ID, EXECUTOR_REGISTRATION_ID, stream_id);
    let oversized_authorization = vec![b'a'; MAX_HARNESS_KEY_AUTHORIZATION_BYTES + 1];
    let (_handshake, request) = InitiatorHandshake::start(
        &harness_identity,
        &environment_identity.public_key(),
        &prologue,
        &oversized_authorization,
    )?;
    let frame = RelayMessageFrame::handshake(stream_id.to_string(), request);
    harness_websocket
        .send(Message::Binary(encode_relay_message_frame(&frame).into()))
        .await?;

    let Message::Binary(payload) = timeout(Duration::from_secs(1), harness_websocket.next())
        .await?
        .ok_or_else(|| anyhow::anyhow!("environment closed before sending reset"))??
    else {
        anyhow::bail!("expected binary reset frame");
    };
    let reset = decode_relay_message_frame(payload.as_ref())?;
    assert_eq!(reset.validate()?, RelayFrameBodyKind::Reset);
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    harness_websocket.close(None).await?;
    timeout(Duration::from_secs(1), environment_task).await??;
    Ok(())
}

#[tokio::test]
async fn malformed_handshakes_preserve_active_streams_and_admission_recovers() -> Result<()> {
    tokio::time::pause();
    // Socket I/O must not auto-advance the paused cooldown or keepalive clocks.
    let (_clock_guard, clock_released) = std::sync::mpsc::channel::<()>();
    tokio::task::spawn_blocking(move || {
        let _ = clock_released.recv();
    });
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let websocket_url = format!("ws://{}", listener.local_addr()?);
    let harness_connection = tokio::spawn(connect_async(websocket_url));
    let (socket, _peer_addr) = listener.accept().await?;
    let environment_websocket = accept_async(socket).await?;
    let (mut harness_websocket, _response) = harness_connection.await??;

    let environment_identity = NoiseChannelIdentity::generate()?;
    let harness_identity = NoiseChannelIdentity::generate()?;
    let calls = Arc::new(AtomicUsize::new(0));
    let release = Arc::new(Notify::new());
    let environment_task = AbortOnDropHandle::new(tokio::spawn(run_multiplexed_environment(
        environment_websocket,
        EchoMessages,
        ENVIRONMENT_ID.to_string(),
        EXECUTOR_REGISTRATION_ID.to_string(),
        environment_identity.clone(),
        BlockingValidator {
            calls: Arc::clone(&calls),
            release: Arc::clone(&release),
        },
    )));

    let healthy_stream_id = "healthy";
    let prologue =
        noise_channel_prologue(ENVIRONMENT_ID, EXECUTOR_REGISTRATION_ID, healthy_stream_id);
    let (handshake, request) = InitiatorHandshake::start(
        &harness_identity,
        &environment_identity.public_key(),
        &prologue,
        b"authorization",
    )?;
    release.notify_one();
    harness_websocket
        .send(Message::Binary(
            encode_relay_message_frame(&RelayMessageFrame::handshake(
                healthy_stream_id.to_string(),
                request,
            ))
            .into(),
        ))
        .await?;
    let response = timeout(
        Duration::from_secs(1),
        next_relay_frame(&mut harness_websocket),
    )
    .await??;
    assert_eq!(response.stream_id, healthy_stream_id);
    let mut healthy_transport = handshake.finish(&response.into_handshake_payload()?)?;

    for attempt in 0..MAX_FAILED_NOISE_HANDSHAKES {
        let stream_id = format!("malformed-{attempt}");
        let prologue = noise_channel_prologue(ENVIRONMENT_ID, EXECUTOR_REGISTRATION_ID, &stream_id);
        let (_handshake, mut request) = InitiatorHandshake::start(
            &harness_identity,
            &environment_identity.public_key(),
            &prologue,
            b"authorization",
        )?;
        let last_byte = request.last_mut().expect("handshake request is not empty");
        *last_byte ^= 1;
        let frame = RelayMessageFrame::handshake(stream_id, request);
        harness_websocket
            .send(Message::Binary(encode_relay_message_frame(&frame).into()))
            .await?;
        let reset = timeout(
            Duration::from_secs(1),
            next_relay_frame(&mut harness_websocket),
        )
        .await??;
        assert_eq!(
            reset,
            RelayMessageFrame::reset(
                frame.stream_id,
                crate::noise_relay::NOISE_RELAY_RESET_REASON.to_string(),
            )
        );
    }

    let new_stream_id = "after-cooldown";
    let prologue = noise_channel_prologue(ENVIRONMENT_ID, EXECUTOR_REGISTRATION_ID, new_stream_id);
    let (new_handshake, request) = InitiatorHandshake::start(
        &harness_identity,
        &environment_identity.public_key(),
        &prologue,
        b"authorization",
    )?;
    let new_frame = RelayMessageFrame::handshake(new_stream_id.to_string(), request);
    // Check both immediate rejection and a later attempt that must not extend
    // the cooldown. Encrypted traffic on the healthy stream still round-trips.
    for seq in 0..2 {
        harness_websocket
            .send(Message::Binary(
                encode_relay_message_frame(&new_frame).into(),
            ))
            .await?;
        let reset = timeout(
            Duration::from_secs(1),
            next_relay_frame(&mut harness_websocket),
        )
        .await??;
        assert_eq!(reset.stream_id, new_stream_id);
        assert_eq!(reset.validate()?, RelayFrameBodyKind::Reset);
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        let message = JSONRPCMessage::Response(JSONRPCResponse {
            id: RequestId::Integer(i64::from(seq)),
            result: serde_json::json!({"healthy": true}),
        });
        let frame = RelayMessageFrame::data(
            healthy_stream_id.to_string(),
            seq,
            healthy_transport.encrypt(&frame_jsonrpc_message(&message)?)?,
            /*trace*/ None,
        );
        harness_websocket
            .send(Message::Binary(encode_relay_message_frame(&frame).into()))
            .await?;
        let response = timeout(
            Duration::from_secs(1),
            next_relay_frame(&mut harness_websocket),
        )
        .await??;
        assert_eq!(response.stream_id, healthy_stream_id);
        let data = response.into_data()?;
        assert_eq!(data.seq, seq);
        let messages =
            JsonRpcMessageDecoder::default().push(&healthy_transport.decrypt(&data.payload)?)?;
        assert_eq!(messages, vec![message]);

        tokio::time::advance(NOISE_HANDSHAKE_FAILURE_COOLDOWN / 2).await;
    }

    release.notify_one();
    harness_websocket
        .send(Message::Binary(
            encode_relay_message_frame(&new_frame).into(),
        ))
        .await?;
    let response = timeout(
        Duration::from_secs(1),
        next_relay_frame(&mut harness_websocket),
    )
    .await??;
    assert_eq!(response.stream_id, new_stream_id);
    new_handshake.finish(&response.into_handshake_payload()?)?;
    assert_eq!(calls.load(Ordering::SeqCst), 2);

    harness_websocket.close(None).await?;
    assert_eq!(
        timeout(Duration::from_secs(1), environment_task).await??,
        RendezvousDisconnectReason::PeerClose,
    );
    Ok(())
}

#[tokio::test]
async fn early_data_during_validation_pauses_admission_without_disconnecting() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let websocket_url = format!("ws://{}", listener.local_addr()?);
    let harness_connection = tokio::spawn(connect_async(websocket_url));
    let (socket, _peer_addr) = listener.accept().await?;
    let environment_websocket = accept_async(socket).await?;
    let (mut harness_websocket, _response) = harness_connection.await??;

    let environment_identity = NoiseChannelIdentity::generate()?;
    let harness_identity = NoiseChannelIdentity::generate()?;
    let environment_task = tokio::spawn(run_multiplexed_environment(
        environment_websocket,
        ConnectionProcessor::new(ExecServerRuntimePaths::new(
            std::env::current_exe()?,
            /*codex_linux_sandbox_exe*/ None,
        )?),
        ENVIRONMENT_ID.to_string(),
        EXECUTOR_REGISTRATION_ID.to_string(),
        environment_identity.clone(),
        BlockingValidator {
            calls: Arc::new(AtomicUsize::new(0)),
            release: Arc::new(Notify::new()),
        },
    ));

    for attempt in 0..MAX_FAILED_NOISE_HANDSHAKES {
        let stream_id = format!("early-data-{attempt}");
        let prologue = noise_channel_prologue(ENVIRONMENT_ID, EXECUTOR_REGISTRATION_ID, &stream_id);
        let (_handshake, request) = InitiatorHandshake::start(
            &harness_identity,
            &environment_identity.public_key(),
            &prologue,
            b"authorization",
        )?;
        for frame in [
            RelayMessageFrame::handshake(stream_id.clone(), request),
            RelayMessageFrame::data(stream_id, /*seq*/ 0, vec![0], /*trace*/ None),
        ] {
            harness_websocket
                .send(Message::Binary(encode_relay_message_frame(&frame).into()))
                .await?;
        }
        let reset = timeout(
            Duration::from_secs(1),
            next_relay_frame(&mut harness_websocket),
        )
        .await??;
        assert_eq!(reset.validate()?, RelayFrameBodyKind::Reset);
    }

    harness_websocket.close(None).await?;
    assert_eq!(
        timeout(Duration::from_secs(1), environment_task).await??,
        RendezvousDisconnectReason::PeerClose,
    );
    Ok(())
}
