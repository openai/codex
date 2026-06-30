use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use futures::SinkExt;
use futures::StreamExt;
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
