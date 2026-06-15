use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::Result;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use tokio::net::TcpListener;
use tokio::sync::Notify;
use tokio::time::timeout;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use super::HarnessKeyValidator;
use super::MAX_FAILED_NOISE_HANDSHAKES;
use super::MAX_HARNESS_KEY_AUTHORIZATION_BYTES;
use super::run_multiplexed_environment;
use crate::ExecServerError;
use crate::ExecServerRuntimePaths;
use crate::noise_channel::InitiatorHandshake;
use crate::noise_channel::NoiseChannelIdentity;
use crate::noise_channel::NoiseChannelPublicKey;
use crate::noise_channel::noise_channel_prologue;
use crate::relay::RelayFrameBodyKind;
use crate::relay::decode_relay_message_frame;
use crate::relay::encode_relay_message_frame;
use crate::relay_proto::RelayMessageFrame;
use crate::server::ConnectionProcessor;

const ENVIRONMENT_ID: &str = "environment-1";
const EXECUTOR_REGISTRATION_ID: &str = "registration-1";

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
async fn duplicate_handshakes_exhaust_failure_budget() -> Result<()> {
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
        let payload = timeout(Duration::from_secs(1), async {
            loop {
                match harness_websocket.next().await {
                    Some(Ok(Message::Binary(payload))) => break Ok(payload),
                    Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_))) => {}
                    Some(Ok(message)) => anyhow::bail!("expected reset frame, got {message:?}"),
                    Some(Err(error)) => break Err(error.into()),
                    None => anyhow::bail!("environment closed before sending reset"),
                }
            }
        })
        .await??;
        let reset = decode_relay_message_frame(payload.as_ref())?;
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
        .send(Message::Binary(encoded.into()))
        .await?;
    timeout(Duration::from_secs(1), environment_task).await??;
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
async fn repeated_malformed_handshakes_close_the_physical_relay() -> Result<()> {
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
    }

    timeout(Duration::from_secs(1), environment_task).await??;
    Ok(())
}

#[tokio::test]
async fn repeated_early_data_during_validation_closes_the_physical_relay() -> Result<()> {
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
            RelayMessageFrame::data(stream_id, /*seq*/ 0, vec![0]),
        ] {
            harness_websocket
                .send(Message::Binary(encode_relay_message_frame(&frame).into()))
                .await?;
        }
    }

    timeout(Duration::from_secs(1), environment_task).await??;
    Ok(())
}
