//! Harness side of the Noise relay.
//!
//! The rendezvous service routes frames by `stream_id`, but does not authenticate
//! the executor or see JSON-RPC plaintext. We claim a stream, complete hybrid IK
//! against the registry-provided executor key, and then expose the result as a
//! normal `JsonRpcConnection`. Outbound JSON-RPC is framed and split into Noise
//! records; inbound records are reordered before decryption and reassembly.

use std::time::Instant;

use codex_exec_server_protocol::JSONRPCMessage;
use futures::Sink;
use futures::SinkExt;
use futures::Stream;
use futures::StreamExt;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio_tungstenite::tungstenite::Message;
use tracing::Instrument;
use tracing::Span;
use tracing::debug;
use tracing::field;
use tracing::info;
use tracing::warn;
use uuid::Uuid;

use crate::ExecServerError;
use crate::connection::CHANNEL_CAPACITY;
use crate::connection::JsonRpcConnection;
use crate::connection::JsonRpcConnectionEvent;
use crate::connection::JsonRpcTransport;
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
use crate::noise_relay::trace_context::NoiseTraceContext;
use crate::relay::RelayFrameBodyKind;
use crate::relay::decode_relay_message_frame;
use crate::relay::encode_relay_message_frame;
use crate::relay_proto::RelayData;
use crate::relay_proto::RelayMessageFrame;

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
                )
                .await;
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
                )
                .await;
                return;
            };
            let message = match incoming_message {
                Ok(Message::Binary(payload)) => payload,
                Ok(Message::Close(_)) => {
                    send_disconnected(
                        &incoming_tx,
                        &disconnected_tx,
                        "Noise relay websocket received close frame during handshake".to_string(),
                    )
                    .await;
                    return;
                }
                Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_)) => continue,
                Ok(Message::Text(_)) => {
                    send_disconnected(
                        &incoming_tx,
                        &disconnected_tx,
                        "Noise relay transport expects binary protobuf frames".to_string(),
                    )
                    .await;
                    return;
                }
                Err(error) => {
                    send_disconnected(
                        &incoming_tx,
                        &disconnected_tx,
                        format!(
                            "failed to read Noise relay websocket from {connection_label}: {error}"
                        ),
                    )
                    .await;
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
                    )
                    .await;
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
                            )
                            .await;
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
                            )
                            .await;
                            return;
                        }
                    }
                }
                Ok(RelayFrameBodyKind::Reset) => {
                    send_disconnected(
                        &incoming_tx,
                        &disconnected_tx,
                        NOISE_RELAY_RESET_DISCONNECT_REASON.to_string(),
                    )
                    .await;
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
                    )
                    .await;
                    return;
                }
            }
        };

        // After the handshake, each relay sequence maps to exactly one Noise
        // transport record. Outbound records are encrypted once; inbound
        // records are reordered and deduplicated before decryption.
        let mut next_outbound_seq = 0u32;
        let mut inbound_ciphertexts = OrderedCiphertextFrames::default();
        let mut inbound_decoder = JsonRpcMessageDecoder::default();
        let mut trace_context = NoiseTraceContext::default();
        'relay: loop {
            tokio::select! {
                maybe_message = outgoing_rx.recv() => {
                    let Some(message) = maybe_message else {
                        break;
                    };
                    trace_context.observe_request(&message);
                    record_outbound_message_dequeued(&message);
                    let outbound_span = outbound_message_span(&message);
                    if let Err(error) = send_outbound_message(
                        &mut websocket,
                        &mut transport,
                        &stream_id,
                        &mut next_outbound_seq,
                        &message,
                    )
                    .instrument(outbound_span)
                    .await
                    {
                        warn!("failed to send JSON-RPC payload over Noise relay: {error}");
                        break 'relay;
                    }
                }
                incoming_message = websocket.next() => {
                    let Some(incoming_message) = incoming_message else {
                        break;
                    };
                    match incoming_message {
                        Ok(Message::Binary(payload)) => {
                            let frame_received_at = Instant::now();
                            let frame = match decode_relay_message_frame(payload.as_ref()) {
                                Ok(frame) => frame,
                                Err(error) => {
                                    send_malformed(&incoming_tx, error.to_string()).await;
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
                                            send_malformed(&incoming_tx, error.to_string()).await;
                                            break;
                                        }
                                    };
                                    if let Err(error) = receive_data(
                                        &mut inbound_ciphertexts,
                                        &mut transport,
                                        &mut inbound_decoder,
                                        data,
                                        &incoming_tx,
                                        &mut trace_context,
                                        frame_received_at,
                                    )
                                    .await
                                    {
                                        send_malformed(&incoming_tx, error.to_string()).await;
                                        break;
                                    }
                                }
                                Ok(RelayFrameBodyKind::Reset) => {
                                    let _ = incoming_tx
                                        .send(JsonRpcConnectionEvent::Disconnected {
                                            reason: Some(
                                                NOISE_RELAY_RESET_DISCONNECT_REASON.to_string(),
                                            ),
                                        })
                                        .await;
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
                                    )
                                    .await;
                                    break;
                                }
                            }
                        }
                        Ok(Message::Close(_)) => break,
                        Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_)) => {}
                        Ok(Message::Text(_)) => {
                            send_malformed(
                                &incoming_tx,
                                "Noise relay transport expects binary protobuf frames".to_string(),
                            )
                            .await;
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

fn record_outbound_message_dequeued(message: &JSONRPCMessage) {
    let (message_kind, method, trace) = match message {
        JSONRPCMessage::Request(request) => {
            ("request", request.method.as_str(), request.trace.as_ref())
        }
        JSONRPCMessage::Notification(notification) => {
            ("notification", notification.method.as_str(), None)
        }
        JSONRPCMessage::Response(_) => ("response", "", None),
        JSONRPCMessage::Error(_) => ("error", "", None),
    };
    let span = tracing::info_span!(
        "exec_server.noise.harness_outbound_dequeued",
        message_kind,
        method,
    );
    if let Some(trace) = trace {
        let _ = codex_otel::set_parent_from_w3c_trace_context(&span, trace);
    }
    span.in_scope(|| {});
}

fn outbound_message_span(message: &JSONRPCMessage) -> Span {
    let (message_kind, method, trace) = match message {
        JSONRPCMessage::Request(request) => {
            ("request", request.method.as_str(), request.trace.as_ref())
        }
        JSONRPCMessage::Notification(notification) => {
            ("notification", notification.method.as_str(), None)
        }
        JSONRPCMessage::Response(_) => ("response", "", None),
        JSONRPCMessage::Error(_) => ("error", "", None),
    };
    let span = tracing::info_span!(
        "exec_server.noise.harness_outbound",
        message_kind,
        method,
        framed_bytes = field::Empty,
        records = field::Empty,
        frame_complete_ms = field::Empty,
        encrypt_complete_ms = field::Empty,
        send_complete_ms = field::Empty,
    );
    if let Some(trace) = trace {
        let _ = codex_otel::set_parent_from_w3c_trace_context(&span, trace);
    }
    span
}

async fn send_outbound_message<T, E>(
    websocket: &mut T,
    transport: &mut NoiseTransport,
    stream_id: &str,
    next_outbound_seq: &mut u32,
    message: &JSONRPCMessage,
) -> Result<(), ExecServerError>
where
    T: Sink<Message, Error = E> + Unpin,
    E: std::fmt::Display,
{
    let started_at = Instant::now();
    let framed = tracing::info_span!("exec_server.noise.frame_request")
        .in_scope(|| frame_jsonrpc_message(message))?;
    let record_count = framed.len().div_ceil(NOISE_RECORD_PLAINTEXT_LEN);
    let span = Span::current();
    span.record("framed_bytes", framed.len());
    span.record("records", record_count);
    span.record(
        "frame_complete_ms",
        started_at.elapsed().as_secs_f64() * 1_000.0,
    );

    for plaintext_record in framed.chunks(NOISE_RECORD_PLAINTEXT_LEN) {
        let seq = take_next_sequence(next_outbound_seq)?;
        let ciphertext = tracing::info_span!("exec_server.noise.encrypt_record")
            .in_scope(|| transport.encrypt(plaintext_record))
            .map_err(|error| {
                ExecServerError::Protocol(format!(
                    "failed to encrypt JSON-RPC payload for Noise relay: {error}"
                ))
            })?;
        span.record(
            "encrypt_complete_ms",
            started_at.elapsed().as_secs_f64() * 1_000.0,
        );
        let frame = RelayMessageFrame::data(stream_id.to_string(), seq, ciphertext);
        websocket
            .send(Message::Binary(encode_relay_message_frame(&frame).into()))
            .instrument(tracing::info_span!("exec_server.noise.websocket_send", seq,))
            .await
            .map_err(|error| {
                ExecServerError::Protocol(format!("failed to write Noise relay websocket: {error}"))
            })?;
    }
    span.record(
        "send_complete_ms",
        started_at.elapsed().as_secs_f64() * 1_000.0,
    );
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
    incoming_tx: &mpsc::Sender<JsonRpcConnectionEvent>,
    trace_context: &mut NoiseTraceContext,
    frame_received_at: Instant,
) -> Result<(), ExecServerError> {
    let frame_decode_elapsed = frame_received_at.elapsed();
    let ciphertext_bytes = data.payload.len();
    // Ordering must happen before decryption because Noise transport nonces are
    // implicit. A future or duplicate ciphertext passed directly to Clatter
    // would desynchronize the channel.
    let reorder_started_at = Instant::now();
    let ciphertexts = inbound_ciphertexts.push(data.seq, data.payload)?;
    let reorder_elapsed = reorder_started_at.elapsed();
    for ciphertext in ciphertexts {
        let decrypt_started_at = Instant::now();
        let plaintext = transport.decrypt(&ciphertext).map_err(|error| {
            ExecServerError::Protocol(format!("Noise relay decryption failed: {error}"))
        })?;
        let decrypt_elapsed = decrypt_started_at.elapsed();

        // The authenticated byte stream can carry partial or multiple JSON-RPC
        // messages; emit only complete, successfully parsed messages.
        let decode_started_at = Instant::now();
        let messages = decoder.push(&plaintext)?;
        let decode_elapsed = decode_started_at.elapsed();
        for message in messages {
            let trace = trace_context.return_trace(&message);
            let inbound_span = inbound_message_span(
                &message,
                trace.as_ref(),
                ciphertext_bytes,
                frame_decode_elapsed,
                reorder_elapsed,
                decrypt_elapsed,
                decode_elapsed,
            );
            let delivery_started_at = Instant::now();
            let send_result = async {
                match incoming_tx
                    .reserve()
                    .instrument(tracing::info_span!(
                        "exec_server.noise.harness_inbound_delivery"
                    ))
                    .await
                {
                    Ok(permit) => {
                        permit.send(JsonRpcConnectionEvent::TracedMessage {
                            message,
                            trace,
                            queued_at: Instant::now(),
                        });
                        Ok(())
                    }
                    Err(err) => Err(err),
                }
            }
            .instrument(inbound_span.clone())
            .await;
            inbound_span.record(
                "delivery_ms",
                delivery_started_at.elapsed().as_secs_f64() * 1_000.0,
            );
            inbound_span.record(
                "receive_complete_ms",
                frame_received_at.elapsed().as_secs_f64() * 1_000.0,
            );
            send_result.map_err(|_| ExecServerError::Closed)?;
        }
    }
    Ok(())
}

fn inbound_message_span(
    message: &JSONRPCMessage,
    trace: Option<&codex_protocol::protocol::W3cTraceContext>,
    ciphertext_bytes: usize,
    frame_decode_elapsed: std::time::Duration,
    reorder_elapsed: std::time::Duration,
    decrypt_elapsed: std::time::Duration,
    decode_elapsed: std::time::Duration,
) -> Span {
    let (message_kind, method) = match message {
        JSONRPCMessage::Request(request) => ("request", request.method.as_str()),
        JSONRPCMessage::Notification(notification) => {
            ("notification", notification.method.as_str())
        }
        JSONRPCMessage::Response(_) => ("response", ""),
        JSONRPCMessage::Error(_) => ("error", ""),
    };
    let span = tracing::info_span!(
        "exec_server.noise.harness_inbound",
        message_kind,
        method,
        ciphertext_bytes,
        frame_decode_ms = frame_decode_elapsed.as_secs_f64() * 1_000.0,
        reorder_ms = reorder_elapsed.as_secs_f64() * 1_000.0,
        decrypt_ms = decrypt_elapsed.as_secs_f64() * 1_000.0,
        decode_ms = decode_elapsed.as_secs_f64() * 1_000.0,
        delivery_ms = field::Empty,
        receive_complete_ms = field::Empty,
    );
    if let Some(trace) = trace {
        let _ = codex_otel::set_parent_from_w3c_trace_context(&span, trace);
    }
    span
}

async fn send_malformed(incoming_tx: &mpsc::Sender<JsonRpcConnectionEvent>, reason: String) {
    let _ = incoming_tx
        .send(JsonRpcConnectionEvent::MalformedMessage { reason })
        .await;
}

async fn send_disconnected(
    incoming_tx: &mpsc::Sender<JsonRpcConnectionEvent>,
    disconnected_tx: &watch::Sender<bool>,
    reason: String,
) {
    let _ = disconnected_tx.send(true);
    let _ = incoming_tx
        .send(JsonRpcConnectionEvent::Disconnected {
            reason: Some(reason),
        })
        .await;
}
