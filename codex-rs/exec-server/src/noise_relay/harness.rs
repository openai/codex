use futures::Sink;
use futures::SinkExt;
use futures::Stream;
use futures::StreamExt;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio_tungstenite::tungstenite::Message;
use tracing::debug;
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
use crate::relay::RelayFrameBodyKind;
use crate::relay::decode_relay_message_frame;
use crate::relay::encode_relay_message_frame;
use crate::relay_proto::RelayData;
use crate::relay_proto::RelayMessageFrame;

/// Adapt one harness rendezvous websocket into an authenticated JSON-RPC connection.
///
/// The returned connection is not usable until the background task completes
/// hybrid IK against the registry-pinned exec-server key. Rendezvous can see
/// stream metadata and ciphertext, but never JSON-RPC plaintext or either
/// endpoint's private key.
pub(crate) fn noise_harness_connection_from_websocket<T, E>(
    stream: T,
    connection_label: String,
    environment_id: String,
    executor_registration_id: String,
    identity: NoiseChannelIdentity,
    responder_public_key: NoiseChannelPublicKey,
    harness_key_authorization: String,
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

        // Bind the Noise transcript to the exact environment registration and
        // virtual relay stream before emitting any handshake bytes. A captured
        // handshake cannot be spliced onto a different routed connection.
        let prologue =
            match noise_channel_prologue(&environment_id, &executor_registration_id, &stream_id) {
                Ok(prologue) => prologue,
                Err(error) => {
                    send_disconnected(
                        &incoming_tx,
                        &disconnected_tx,
                        format!("failed to build Noise relay prologue: {error}"),
                    )
                    .await;
                    return;
                }
            };
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
                    "Noise relay websocket closed during handshake".to_string(),
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
                        "Noise relay websocket closed during handshake".to_string(),
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
                continue;
            }
            match frame.validate() {
                Ok(RelayFrameBodyKind::Handshake) => {
                    let response = match frame.into_handshake_payload() {
                        Ok(response) => response,
                        Err(error) => {
                            send_disconnected(&incoming_tx, &disconnected_tx, error.to_string())
                                .await;
                            return;
                        }
                    };
                    match initiator_handshake.finish(&response) {
                        Ok(transport) => break transport,
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
                        frame
                            .into_reset_reason()
                            .unwrap_or_else(|| "Noise relay reset during handshake".to_string()),
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
        'relay: loop {
            tokio::select! {
                maybe_message = outgoing_rx.recv() => {
                    let Some(message) = maybe_message else {
                        break;
                    };
                    let framed = match frame_jsonrpc_message(&message) {
                        Ok(framed) => framed,
                        Err(error) => {
                            warn!("failed to frame JSON-RPC payload for Noise relay: {error}");
                            break;
                        }
                    };
                    for plaintext_record in framed.chunks(NOISE_RECORD_PLAINTEXT_LEN) {
                        let seq = match take_next_sequence(&mut next_outbound_seq) {
                            Ok(seq) => seq,
                            Err(error) => {
                                warn!("Noise relay sequence exhausted: {error}");
                                break 'relay;
                            }
                        };
                        let ciphertext = match transport.encrypt(plaintext_record) {
                            Ok(ciphertext) => ciphertext,
                            Err(error) => {
                                warn!("failed to encrypt JSON-RPC payload for Noise relay: {error}");
                                break 'relay;
                            }
                        };
                        let frame = RelayMessageFrame::data(stream_id.clone(), seq, ciphertext);
                        if websocket
                            .send(Message::Binary(encode_relay_message_frame(&frame).into()))
                            .await
                            .is_err()
                        {
                            break 'relay;
                        }
                    }
                }
                incoming_message = websocket.next() => {
                    let Some(incoming_message) = incoming_message else {
                        break;
                    };
                    match incoming_message {
                        Ok(Message::Binary(payload)) => {
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
                                    )
                                    .await
                                    {
                                        send_malformed(&incoming_tx, error.to_string()).await;
                                        break;
                                    }
                                }
                                Ok(RelayFrameBodyKind::Reset) => {
                                    let reason = frame.into_reset_reason();
                                    let _ = incoming_tx
                                        .send(JsonRpcConnectionEvent::Disconnected { reason })
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
    });

    JsonRpcConnection {
        outgoing_tx,
        incoming_rx,
        disconnected_rx,
        task_handles: vec![websocket_task],
        transport: JsonRpcTransport::External,
    }
}

async fn receive_data(
    inbound_ciphertexts: &mut OrderedCiphertextFrames,
    transport: &mut NoiseTransport,
    decoder: &mut JsonRpcMessageDecoder,
    data: RelayData,
    incoming_tx: &mpsc::Sender<JsonRpcConnectionEvent>,
) -> Result<(), ExecServerError> {
    // Ordering must happen before decryption because Noise transport nonces are
    // implicit. A future or duplicate ciphertext passed directly to Clatter
    // would desynchronize the channel.
    for ciphertext in inbound_ciphertexts.push(data.seq, data.payload)? {
        let plaintext = transport.decrypt(&ciphertext)?;

        // The authenticated byte stream can carry partial or multiple JSON-RPC
        // messages; emit only complete, successfully parsed messages.
        for message in decoder.push(&plaintext)? {
            incoming_tx
                .send(JsonRpcConnectionEvent::Message(message))
                .await
                .map_err(|_| ExecServerError::Closed)?;
        }
    }
    Ok(())
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
