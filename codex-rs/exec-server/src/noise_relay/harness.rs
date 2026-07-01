//! Harness side of the Noise relay.
//!
//! The rendezvous service routes frames by `stream_id`, but does not authenticate
//! the executor or see JSON-RPC plaintext. We claim a stream, complete hybrid IK
//! against the registry-provided executor key, and then expose the result as a
//! normal `JsonRpcConnection`. Outbound JSON-RPC is framed and split into Noise
//! records; inbound records are reordered before decryption and reassembly.

use futures::FutureExt;
use futures::Sink;
use futures::SinkExt;
use futures::Stream;
use futures::StreamExt;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio_tungstenite::tungstenite::Message;
use tracing::Instrument;
use tracing::debug;
use tracing::info;
use tracing::warn;
use uuid::Uuid;

use crate::ExecServerError;
use crate::connection::CHANNEL_CAPACITY;
use crate::connection::JsonRpcConnection;
use crate::connection::JsonRpcConnectionEvent;
use crate::connection::JsonRpcTransport;
use crate::connection::WEBSOCKET_KEEPALIVE_INTERVAL;
use crate::noise_channel::InitiatorHandshake;
use crate::noise_channel::NoiseChannelIdentity;
use crate::noise_channel::NoiseChannelPublicKey;
use crate::noise_channel::NoiseTransport;
use crate::noise_channel::noise_channel_prologue;
use crate::noise_relay::message_framing::JsonRpcMessageDecoder;
use crate::noise_relay::message_framing::NOISE_RECORD_PLAINTEXT_LEN;
use crate::noise_relay::message_framing::frame_jsonrpc_message;
use crate::noise_relay::ordered_ciphertext::OrderedCiphertextFrames;
use crate::noise_relay::take_next_sequence;
use crate::relay::RelayFrameBodyKind;
use crate::relay::decode_relay_message_frame;
use crate::relay::encode_relay_message_frame;
use crate::relay_proto::RelayData;
use crate::relay_proto::RelayMessageFrame;
use crate::websocket_pong_watchdog::WEBSOCKET_PONG_TIMEOUT;
use crate::websocket_pong_watchdog::WEBSOCKET_PONG_TIMEOUT_REASON;
use crate::websocket_pong_watchdog::WebSocketPongWatchdog;

/// Values that bind one harness websocket to the intended executor registration.
///
/// These fields all come from the same registry response. Keeping them together
/// makes that relationship visible at the call site and avoids mixing up the
/// several string and key arguments used to start the handshake.
pub(crate) struct NoiseHarnessConnectionArgs {
    pub(crate) connection_label: String,
    pub(crate) environment_id: String,
    pub(crate) executor_registration_id: String,
    pub(crate) identity: NoiseChannelIdentity,
    pub(crate) responder_public_key: NoiseChannelPublicKey,
    pub(crate) harness_key_authorization: String,
}

// Reset frames are cleartext relay control and are not authenticated by Noise.
// Preserve the availability signal while replacing attacker-controlled reason
// text before it reaches disconnect diagnostics.
const NOISE_RELAY_RESET_DISCONNECT_REASON: &str = "Noise relay stream reset";
// Give a Pong already queued behind data a bounded chance to reach the reader.
const MAX_FRAMES_DRAINED_AFTER_PONG_DEADLINE: usize = 32;

/// Adapt one harness rendezvous websocket into an authenticated JSON-RPC connection.
///
/// The returned connection is not usable until the background task completes
/// hybrid IK against the registry-pinned exec-server key. Rendezvous can see
/// stream metadata and ciphertext, but never JSON-RPC plaintext or either
/// endpoint's private key. Failures close the connection rather than falling
/// back to plaintext.
pub(crate) fn noise_harness_connection_from_websocket<T, E>(
    stream: T,
    args: NoiseHarnessConnectionArgs,
) -> JsonRpcConnection
where
    T: Sink<Message, Error = E> + Stream<Item = Result<Message, E>> + Unpin + Send + 'static,
    E: std::fmt::Display + Send + 'static,
{
    let NoiseHarnessConnectionArgs {
        connection_label,
        environment_id,
        executor_registration_id,
        identity,
        responder_public_key,
        harness_key_authorization,
    } = args;
    let stream_id = Uuid::new_v4().to_string();
    let (outgoing_tx, mut outgoing_rx) = mpsc::channel(CHANNEL_CAPACITY);
    let (incoming_tx, incoming_rx) = mpsc::channel(CHANNEL_CAPACITY);
    let (disconnected_tx, disconnected_rx) = watch::channel(false);
    let stream_span = tracing::debug_span!("noise_relay.stream", noise_side = "harness",);
    debug!(
        environment_id,
        executor_registration_id, stream_id, "Noise harness relay details"
    );

    let websocket_task = tokio::spawn(async move {
        let mut websocket = stream;

        // Bind the Noise transcript to the exact environment registration and
        // virtual relay stream before emitting any handshake bytes. A captured
        // handshake cannot be spliced onto a different routed connection.
        let prologue =
            noise_channel_prologue(&environment_id, &executor_registration_id, &stream_id);
        let (initiator_handshake, request) = match InitiatorHandshake::start(
            &identity,
            &responder_public_key,
            &prologue,
            harness_key_authorization.as_bytes(),
        ) {
            Ok(handshake) => handshake,
            Err(error) => {
                send_disconnected(
                    &incoming_tx,
                    &disconnected_tx,
                    format!("failed to start Noise relay handshake: {error}"),
                );
                return;
            }
        };

        // Resume claims the stream ID at rendezvous; Handshake carries the
        // opaque first IK message. No JSON-RPC data is sent before the
        // responder proves possession of the pinned static key.
        let resume = RelayMessageFrame::resume(stream_id.clone());
        let handshake = RelayMessageFrame::handshake(stream_id.clone(), request);
        if websocket
            .send(Message::Binary(encode_relay_message_frame(&resume).into()))
            .await
            .is_err()
            || websocket
                .send(Message::Binary(
                    encode_relay_message_frame(&handshake).into(),
                ))
                .await
                .is_err()
        {
            let _ = disconnected_tx.send(true);
            return;
        }

        // During the handshake, ignore unrelated routed streams and control
        // frames, but reject data on our stream. Accepting early data would
        // create a plaintext or unauthenticated application path.
        let mut transport = loop {
            let Some(incoming_message) = websocket.next().await else {
                send_disconnected(
                    &incoming_tx,
                    &disconnected_tx,
                    "Noise relay websocket ended during handshake".to_string(),
                );
                return;
            };
            let message = match incoming_message {
                Ok(Message::Binary(payload)) => payload,
                Ok(Message::Close(_)) => {
                    send_disconnected(
                        &incoming_tx,
                        &disconnected_tx,
                        "Noise relay websocket received close frame during handshake".to_string(),
                    );
                    return;
                }
                Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_)) => continue,
                Ok(Message::Text(_)) => {
                    send_disconnected(
                        &incoming_tx,
                        &disconnected_tx,
                        "Noise relay transport expects binary protobuf frames".to_string(),
                    );
                    return;
                }
                Err(error) => {
                    send_disconnected(
                        &incoming_tx,
                        &disconnected_tx,
                        format!(
                            "failed to read Noise relay websocket from {connection_label}: {error}"
                        ),
                    );
                    return;
                }
            };
            let frame = match decode_relay_message_frame(message.as_ref()) {
                Ok(frame) => frame,
                Err(error) => {
                    send_disconnected(
                        &incoming_tx,
                        &disconnected_tx,
                        format!("failed to parse Noise relay frame: {error}"),
                    );
                    return;
                }
            };
            if frame.stream_id != stream_id {
                debug!("Noise relay ignored frame for unrelated stream during handshake");
                continue;
            }
            match frame.validate() {
                Ok(RelayFrameBodyKind::Handshake) => {
                    let response = match frame.into_handshake_payload() {
                        Ok(response) => response,
                        Err(error) => {
                            send_disconnected(
                                &incoming_tx,
                                &disconnected_tx,
                                format!("invalid Noise relay handshake response: {error}"),
                            );
                            return;
                        }
                    };
                    match initiator_handshake.finish(&response) {
                        Ok(transport) => {
                            info!(
                                noise_event = "handshake",
                                noise_outcome = "ok",
                                "Noise harness handshake completed"
                            );
                            break transport;
                        }
                        Err(error) => {
                            send_disconnected(
                                &incoming_tx,
                                &disconnected_tx,
                                format!("Noise relay handshake failed: {error}"),
                            );
                            return;
                        }
                    }
                }
                Ok(RelayFrameBodyKind::Reset) => {
                    send_disconnected(
                        &incoming_tx,
                        &disconnected_tx,
                        NOISE_RELAY_RESET_DISCONNECT_REASON.to_string(),
                    );
                    return;
                }
                Ok(
                    RelayFrameBodyKind::Ack
                    | RelayFrameBodyKind::Resume
                    | RelayFrameBodyKind::Heartbeat,
                ) => {}
                Ok(RelayFrameBodyKind::Data) | Err(_) => {
                    send_disconnected(
                        &incoming_tx,
                        &disconnected_tx,
                        "Noise relay received data before handshake completion".to_string(),
                    );
                    return;
                }
            }
        };

        // After the handshake, each relay sequence maps to exactly one Noise
        // transport record. Outbound records are encrypted once; inbound
        // records are reordered and deduplicated before decryption.
        let mut websocket = websocket.peekable();
        let mut next_outbound_seq = 0u32;
        let mut inbound_ciphertexts = OrderedCiphertextFrames::default();
        let mut inbound_decoder = JsonRpcMessageDecoder::default();
        let mut keepalive = tokio::time::interval_at(
            tokio::time::Instant::now() + WEBSOCKET_KEEPALIVE_INTERVAL,
            WEBSOCKET_KEEPALIVE_INTERVAL,
        );
        keepalive.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut pong_watchdog = WebSocketPongWatchdog::new(WEBSOCKET_PONG_TIMEOUT);
        let pong_deadline = tokio::time::sleep(WEBSOCKET_PONG_TIMEOUT);
        tokio::pin!(pong_deadline);
        // Keep one framed message as a cursor. Sending one Noise record per loop
        // creates a scheduling point for keepalive and inbound control frames
        // without splitting the WebSocket reader and writer.
        let mut pending_outbound: Option<(Vec<u8>, usize)> = None;
        let mut force_incoming = false;
        let mut frames_drained_after_pong_deadline = 0usize;
        'relay: loop {
            // Consume a due tick before the always-ready record arm below can win
            // another select iteration and postpone the keepalive.
            if pong_watchdog.deadline().is_none()
                && keepalive.tick().now_or_never().is_some()
            {
                if let Err(error) = send_keepalive_ping(
                    &mut websocket,
                    &mut pong_watchdog,
                    pong_deadline.as_mut(),
                )
                .await
                {
                    warn!("failed to write Noise relay keepalive ping: {error}");
                    break;
                }
                frames_drained_after_pong_deadline = 0;
                continue;
            }

            let pong_deadline_expired = pong_watchdog
                .deadline()
                .is_some_and(|deadline| tokio::time::Instant::now() >= deadline);
            // After expiry, inspect only frames already queued. Forcing the peeked
            // item through next() makes the 32-frame grace deterministic.
            if pong_deadline_expired && !force_incoming {
                if frames_drained_after_pong_deadline
                    < MAX_FRAMES_DRAINED_AFTER_PONG_DEADLINE
                    && std::pin::Pin::new(&mut websocket)
                        .peek()
                        .now_or_never()
                        .is_some()
                {
                    force_incoming = true;
                } else {
                    warn!(
                        noise_reason = WEBSOCKET_PONG_TIMEOUT_REASON,
                        "Noise harness rendezvous websocket disconnected"
                    );
                    send_disconnected(
                        &incoming_tx,
                        &disconnected_tx,
                        WEBSOCKET_PONG_TIMEOUT_REASON.to_string(),
                    );
                    return;
                }
            }

            // While a Pong is outstanding, drain already-queued inbound traffic
            // before the next fragment so a queued Pong cannot sit behind writes.
            if !force_incoming
                && pong_watchdog.deadline().is_some()
                && pending_outbound.is_some()
                && std::pin::Pin::new(&mut websocket)
                    .peek()
                    .now_or_never()
                    .is_some()
            {
                force_incoming = true;
            }

            tokio::select! {
                maybe_message = outgoing_rx.recv(), if pending_outbound.is_none() && !force_incoming && !pong_deadline_expired => {
                    let Some(message) = maybe_message else {
                        break;
                    };
                    pending_outbound = Some(match frame_jsonrpc_message(&message) {
                        Ok(framed) => (framed, 0),
                        Err(error) => {
                            warn!("failed to frame JSON-RPC payload for Noise relay: {error}");
                            break;
                        }
                    });
                }
                _ = std::future::ready(()), if pending_outbound.is_some() && !force_incoming && !pong_deadline_expired => {
                    let seq = match take_next_sequence(&mut next_outbound_seq) {
                        Ok(seq) => seq,
                        Err(error) => {
                            warn!("Noise relay sequence exhausted: {error}");
                            break 'relay;
                        }
                    };
                    let (ciphertext, next_offset, message_complete) = {
                        let Some((framed, offset)) = pending_outbound.as_ref() else {
                            continue;
                        };
                        let next_offset = (*offset + NOISE_RECORD_PLAINTEXT_LEN).min(framed.len());
                        let ciphertext = match transport.encrypt(&framed[*offset..next_offset]) {
                            Ok(ciphertext) => ciphertext,
                            Err(error) => {
                                warn!("failed to encrypt JSON-RPC payload for Noise relay: {error}");
                                break 'relay;
                            }
                        };
                        (ciphertext, next_offset, next_offset == framed.len())
                    };
                    let frame = RelayMessageFrame::data(stream_id.clone(), seq, ciphertext);
                    // A Pong can arrive after the readiness check while this write owns the
                    // combined sink and stream. A single bounded record can therefore hit the
                    // deadline and disconnect with that Pong queued. Treat that as write
                    // backpressure; this loop yields only between records.
                    if let Err(error) = send_websocket_message(
                        &mut websocket,
                        Message::Binary(encode_relay_message_frame(&frame).into()),
                        pong_watchdog.write_deadline(tokio::time::Instant::now()),
                    )
                    .await
                    {
                        warn!("failed to write Noise relay websocket: {error}");
                        break 'relay;
                    }
                    if message_complete {
                        pending_outbound = None;
                    } else if let Some((_framed, offset)) = pending_outbound.as_mut() {
                        *offset = next_offset;
                    }
                }
                _ = &mut pong_deadline, if pong_watchdog.deadline().is_some() && !force_incoming => {
                    continue;
                }
                _ = keepalive.tick(), if pong_watchdog.deadline().is_none() => {
                    if let Err(error) = send_keepalive_ping(
                        &mut websocket,
                        &mut pong_watchdog,
                        pong_deadline.as_mut(),
                    )
                    .await
                    {
                        warn!("failed to write Noise relay keepalive ping: {error}");
                        break;
                    }
                    frames_drained_after_pong_deadline = 0;
                }
                incoming_message = websocket.next() => {
                    force_incoming = false;
                    let Some(incoming_message) = incoming_message else {
                        break;
                    };
                    // Count each completed read after expiry. If only the deadline arm
                    // advanced this counter, reads won by a simultaneously ready incoming
                    // arm would not count toward the 32-frame cap.
                    if pong_watchdog
                        .deadline()
                        .is_some_and(|deadline| tokio::time::Instant::now() >= deadline)
                    {
                        frames_drained_after_pong_deadline += 1;
                    }
                    match incoming_message {
                        Ok(Message::Binary(payload)) => {
                            let frame = match decode_relay_message_frame(payload.as_ref()) {
                                Ok(frame) => frame,
                                Err(error) => {
                                    send_malformed(&incoming_tx, error.to_string());
                                    break;
                                }
                            };
                            if frame.stream_id != stream_id {
                                continue;
                            }
                            match frame.validate() {
                                Ok(RelayFrameBodyKind::Data) => {
                                    let data = match frame.into_data() {
                                        Ok(data) => data,
                                        Err(error) => {
                                            send_malformed(&incoming_tx, error.to_string());
                                            break;
                                        }
                                    };
                                    if let Err(error) = receive_data(
                                        &mut inbound_ciphertexts,
                                        &mut transport,
                                        &mut inbound_decoder,
                                        data,
                                        pong_watchdog.write_deadline(tokio::time::Instant::now()),
                                        &incoming_tx,
                                    )
                                    .await
                                    {
                                        if matches!(error, ExecServerError::Closed) {
                                            break;
                                        }
                                        send_malformed(&incoming_tx, error.to_string());
                                        break;
                                    }
                                }
                                Ok(RelayFrameBodyKind::Reset) => {
                                    let _ = incoming_tx.try_send(
                                        JsonRpcConnectionEvent::Disconnected {
                                            reason: Some(
                                                NOISE_RELAY_RESET_DISCONNECT_REASON.to_string(),
                                            ),
                                        },
                                    );
                                    break;
                                }
                                Ok(
                                    RelayFrameBodyKind::Ack
                                    | RelayFrameBodyKind::Resume
                                    | RelayFrameBodyKind::Heartbeat,
                                ) => {}
                                Ok(RelayFrameBodyKind::Handshake) | Err(_) => {
                                    send_malformed(
                                        &incoming_tx,
                                        "Noise relay received invalid post-handshake frame".to_string(),
                                    );
                                    break;
                                }
                            }
                        }
                        Ok(Message::Close(_)) => break,
                        Ok(Message::Pong(_)) => {
                            pong_watchdog.received_pong();
                            frames_drained_after_pong_deadline = 0;
                        }
                        Ok(Message::Ping(_) | Message::Frame(_)) => {}
                        Ok(Message::Text(_)) => {
                            send_malformed(
                                &incoming_tx,
                                "Noise relay transport expects binary protobuf frames".to_string(),
                            );
                            break;
                        }
                        Err(error) => {
                            debug!("Noise relay websocket read failed: {error}");
                            break;
                        }
                    }
                }
            }
        }
        let _ = disconnected_tx.send(true);
    }
    .instrument(stream_span));

    JsonRpcConnection {
        outgoing_tx,
        incoming_rx,
        disconnected_rx,
        task_handles: vec![websocket_task],
        transport: JsonRpcTransport::Plain,
    }
}

async fn send_websocket_message<T, E>(
    websocket: &mut T,
    message: Message,
    deadline: tokio::time::Instant,
) -> Result<(), String>
where
    T: Sink<Message, Error = E> + Unpin,
    E: std::fmt::Display,
{
    match tokio::time::timeout_at(deadline, websocket.send(message)).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(error.to_string()),
        Err(_) => Err("websocket write timed out".to_string()),
    }
}

async fn send_keepalive_ping<T, E>(
    websocket: &mut T,
    pong_watchdog: &mut WebSocketPongWatchdog,
    pong_deadline: std::pin::Pin<&mut tokio::time::Sleep>,
) -> Result<(), String>
where
    T: Sink<Message, Error = E> + Unpin,
    E: std::fmt::Display,
{
    send_websocket_message(
        websocket,
        Message::Ping(Vec::new().into()),
        pong_watchdog.write_deadline(tokio::time::Instant::now()),
    )
    .await?;
    // Start the response clock after the Ping flushes; waiting for sink capacity
    // is governed by the write deadline above.
    pong_watchdog.ping_sent(tokio::time::Instant::now());
    if let Some(deadline) = pong_watchdog.deadline() {
        pong_deadline.reset(deadline);
    }
    Ok(())
}

/// Order and decrypt one relay frame, then emit any complete JSON-RPC messages.
/// Relay records and JSON-RPC messages do not share boundaries, so reassembly
/// happens after decryption.
async fn receive_data(
    inbound_ciphertexts: &mut OrderedCiphertextFrames,
    transport: &mut NoiseTransport,
    decoder: &mut JsonRpcMessageDecoder,
    data: RelayData,
    delivery_deadline: tokio::time::Instant,
    incoming_tx: &mpsc::Sender<JsonRpcConnectionEvent>,
) -> Result<(), ExecServerError> {
    // Ordering must happen before decryption because Noise transport nonces are
    // implicit. A future or duplicate ciphertext passed directly to Clatter
    // would desynchronize the channel.
    for ciphertext in inbound_ciphertexts.push(data.seq, data.payload)? {
        let plaintext = transport.decrypt(&ciphertext).map_err(|error| {
            ExecServerError::Protocol(format!("Noise relay decryption failed: {error}"))
        })?;

        // The authenticated byte stream can carry partial or multiple JSON-RPC
        // messages; emit only complete, successfully parsed messages.
        for message in decoder.push(&plaintext)? {
            send_incoming_event(
                incoming_tx,
                JsonRpcConnectionEvent::Message(message),
                delivery_deadline,
            )
            .await?;
        }
    }
    Ok(())
}

async fn send_incoming_event(
    incoming_tx: &mpsc::Sender<JsonRpcConnectionEvent>,
    event: JsonRpcConnectionEvent,
    deadline: tokio::time::Instant,
) -> Result<(), ExecServerError> {
    match tokio::time::timeout_at(deadline, incoming_tx.send(event)).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(_)) => Err(ExecServerError::Closed),
        Err(_) => {
            warn!(
                noise_reason = "application_backpressure",
                "Noise harness application event delivery timed out"
            );
            Err(ExecServerError::Closed)
        }
    }
}

fn send_malformed(incoming_tx: &mpsc::Sender<JsonRpcConnectionEvent>, reason: String) {
    let _ = incoming_tx.try_send(JsonRpcConnectionEvent::MalformedMessage { reason });
}

fn send_disconnected(
    incoming_tx: &mpsc::Sender<JsonRpcConnectionEvent>,
    disconnected_tx: &watch::Sender<bool>,
    reason: String,
) {
    let _ = disconnected_tx.send(true);
    let _ = incoming_tx.try_send(JsonRpcConnectionEvent::Disconnected {
        reason: Some(reason),
    });
}

#[cfg(test)]
#[path = "harness_tests.rs"]
mod tests;
