use std::collections::HashMap;
use std::time::Duration;

use futures::SinkExt;
use futures::StreamExt;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tokio::time::timeout;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;
use tracing::debug;
use tracing::warn;

use crate::ExecServerError;
use crate::connection::CHANNEL_CAPACITY;
use crate::noise_channel::NoiseChannelIdentity;
use crate::noise_channel::NoiseChannelPublicKey;
use crate::noise_channel::PendingResponderHandshake;
use crate::noise_channel::noise_channel_prologue;
use crate::noise_relay::NOISE_RELAY_RESET_REASON;
use crate::noise_relay::executor_stream::ClosedNoiseVirtualStream;
use crate::noise_relay::executor_stream::NoiseVirtualStream;
use crate::noise_relay::executor_stream::spawn_noise_virtual_stream;
use crate::relay::RelayFrameBodyKind;
use crate::relay::decode_relay_message_frame;
use crate::relay::encode_relay_message_frame;
use crate::relay_proto::RelayMessageFrame;
use crate::server::ConnectionProcessor;

const MAX_ACTIVE_NOISE_RELAY_STREAMS: usize = 128;
const MAX_HARNESS_KEY_AUTHORIZATION_BYTES: usize = 4096;
const MAX_PENDING_HANDSHAKE_VALIDATIONS: usize = 32;
const HARNESS_KEY_VALIDATION_TIMEOUT: Duration = Duration::from_secs(10);

/// Validates that a Noise-authenticated harness public key is authorized.
///
/// Implementations must consult an authority independent of rendezvous. The
/// exec-server invokes this after parsing the first IK message and before
/// completing the responder handshake.
pub(crate) trait HarnessKeyValidator: Send + Sync {
    fn validate_harness_key(
        &self,
        harness_public_key: &NoiseChannelPublicKey,
        authorization: &str,
    ) -> impl std::future::Future<Output = Result<(), ExecServerError>> + Send;
}

/// Serve many authenticated virtual JSON-RPC streams over one executor websocket.
///
/// Each stream has an independent Noise handshake and transport state. The
/// outer websocket and rendezvous route are treated as untrusted delivery:
/// malformed, unauthorized, or cryptographically invalid streams fail closed
/// without creating a `JsonRpcConnection`.
pub(crate) async fn run_noise_multiplexed_environment<S, V>(
    stream: WebSocketStream<S>,
    processor: ConnectionProcessor,
    environment_id: String,
    executor_registration_id: String,
    identity: NoiseChannelIdentity,
    validator: V,
) where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    V: HarnessKeyValidator + Clone + 'static,
{
    let (mut websocket_sink, mut websocket_stream) = stream.split();
    let (physical_outgoing_tx, mut physical_outgoing_rx) =
        mpsc::channel::<Vec<u8>>(CHANNEL_CAPACITY);
    let (closed_stream_tx, mut closed_stream_rx) =
        mpsc::channel::<ClosedNoiseVirtualStream>(MAX_ACTIVE_NOISE_RELAY_STREAMS);
    // A separate writer task is required because this state machine also
    // produces resets and handshake responses. If the same task both sent into
    // and drained the bounded outgoing channel, backpressure could make it wait
    // on itself and stop servicing the physical websocket.
    let mut physical_writer_task = tokio::spawn(async move {
        while let Some(encoded) = physical_outgoing_rx.recv().await {
            if let Err(error) = websocket_sink.send(Message::Binary(encoded.into())).await {
                debug!("Noise multiplexed environment websocket write failed: {error}");
                break;
            }
        }
    });
    let mut streams: HashMap<String, NoiseVirtualStream> = HashMap::new();
    let mut pending_handshakes: HashMap<String, PendingHandshake> = HashMap::new();
    let mut validation_tasks: JoinSet<HarnessKeyValidationResult> = JoinSet::new();
    let mut next_validation_id = 0u64;

    loop {
        // Keep registry validation out of the main relay loop. A slow or
        // malicious authorization request must not block existing streams or
        // prevent other handshakes from being received and bounded.
        let frame = tokio::select! {
            writer_result = &mut physical_writer_task => {
                if let Err(error) = writer_result {
                    warn!("Noise multiplexed environment websocket writer failed: {error}");
                }
                break;
            }
            Some(closed_stream) = closed_stream_rx.recv() => {
                // A writer can finish after its peer resets and reuses the same
                // routing ID. Remove only the exact authenticated stream
                // instance that produced this close notification.
                let is_current = streams
                    .get(&closed_stream.stream_id)
                    .is_some_and(|stream| stream.is_instance(closed_stream.instance_id));
                if is_current {
                    streams.remove(&closed_stream.stream_id);
                }
                continue;
            }
            validation_result = validation_tasks.join_next(), if !validation_tasks.is_empty() => {
                match validation_result {
                    Some(Ok(validation_result)) => {
                        // Stream IDs may be reset and reused while validation
                        // is in flight. The monotonic validation ID ensures a
                        // stale task can never complete a newer handshake.
                        let is_current = pending_handshakes
                            .get(&validation_result.stream_id)
                            .is_some_and(|pending| {
                                pending.validation_id == validation_result.validation_id
                            });
                        if !is_current {
                            continue;
                        }
                        let Some(pending) =
                            pending_handshakes.remove(&validation_result.stream_id)
                        else {
                            continue;
                        };
                        if validation_result.result.is_err() {
                            // Validators receive the short-lived authorization.
                            // Keep their error text out of logs even though the
                            // registry implementation below also sanitizes
                            // response bodies.
                            warn!("Noise relay harness key validation failed");
                            send_reset(&physical_outgoing_tx, validation_result.stream_id);
                            continue;
                        }
                        if streams.len() >= MAX_ACTIVE_NOISE_RELAY_STREAMS {
                            warn!("Noise relay has too many active streams");
                            send_reset(&physical_outgoing_tx, validation_result.stream_id);
                            continue;
                        }

                        // This is the only point where the responder completes
                        // IK and exposes a JSON-RPC stream: Noise authenticated
                        // the harness key and the registry authorized it.
                        let (transport, response) = match pending.handshake.complete() {
                            Ok(completed) => completed,
                            Err(error) => {
                                warn!("failed to complete Noise relay handshake: {error}");
                                send_reset(&physical_outgoing_tx, validation_result.stream_id);
                                continue;
                            }
                        };
                        let response = RelayMessageFrame::handshake(
                            validation_result.stream_id.clone(),
                            response,
                        );
                        // The shared state machine must never wait behind an
                        // overloaded writer queue. If a successful handshake
                        // response cannot be queued immediately, close this
                        // physical connection rather than expose a half-open
                        // virtual stream.
                        if physical_outgoing_tx
                            .try_send(encode_relay_message_frame(&response))
                            .is_err()
                        {
                            break;
                        }
                        streams.insert(
                            validation_result.stream_id.clone(),
                            spawn_noise_virtual_stream(
                                validation_result.stream_id,
                                validation_result.validation_id,
                                processor.clone(),
                                physical_outgoing_tx.clone(),
                                closed_stream_tx.clone(),
                                transport,
                            ),
                        );
                    }
                    Some(Err(error)) => {
                        warn!("Noise relay harness key validation task failed: {error}");
                        let stream_ids = pending_handshakes.keys().cloned().collect::<Vec<_>>();
                        pending_handshakes.clear();
                        for stream_id in stream_ids {
                            send_reset(&physical_outgoing_tx, stream_id);
                        }
                    }
                    None => {}
                }
                continue;
            }
            incoming_message = websocket_stream.next() => match incoming_message {
                Some(Ok(Message::Binary(payload))) => match decode_relay_message_frame(payload.as_ref()) {
                    Ok(frame) => frame,
                    Err(error) => {
                        warn!("dropping malformed Noise relay frame from harness: {error}");
                        continue;
                    }
                },
                Some(Ok(Message::Close(_))) | None => break,
                Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_))) => continue,
                Some(Ok(Message::Text(_))) => {
                    warn!("dropping non-binary Noise relay frame from harness");
                    continue;
                }
                Some(Err(error)) => {
                    debug!("Noise multiplexed environment websocket read failed: {error}");
                    break;
                }
            }
        };

        let kind = match frame.validate() {
            Ok(kind) => kind,
            Err(error) => {
                warn!("dropping invalid Noise relay frame: {error}");
                continue;
            }
        };
        let stream_id = frame.stream_id.clone();
        match kind {
            RelayFrameBodyKind::Handshake => {
                // Bound all pre-authentication state before doing expensive
                // hybrid cryptography or starting an external validation.
                if streams.contains_key(&stream_id) || pending_handshakes.contains_key(&stream_id) {
                    send_reset(&physical_outgoing_tx, stream_id);
                    continue;
                }
                if streams.len() >= MAX_ACTIVE_NOISE_RELAY_STREAMS {
                    warn!("Noise relay has too many active streams");
                    send_reset(&physical_outgoing_tx, stream_id);
                    continue;
                }
                if validation_tasks.len() >= MAX_PENDING_HANDSHAKE_VALIDATIONS {
                    warn!("Noise relay has too many pending harness key validations");
                    send_reset(&physical_outgoing_tx, stream_id);
                    continue;
                }
                let prologue = match noise_channel_prologue(
                    &environment_id,
                    &executor_registration_id,
                    &stream_id,
                ) {
                    Ok(prologue) => prologue,
                    Err(error) => {
                        warn!("failed to build Noise relay prologue: {error}");
                        send_reset(&physical_outgoing_tx, stream_id);
                        continue;
                    }
                };
                let request = match frame.into_handshake_payload() {
                    Ok(request) => request,
                    Err(error) => {
                        warn!("failed to read Noise relay handshake frame: {error}");
                        send_reset(&physical_outgoing_tx, stream_id);
                        continue;
                    }
                };
                let mut pending =
                    match PendingResponderHandshake::read_request(&identity, &prologue, &request) {
                        Ok(pending) => pending,
                        Err(error) => {
                            warn!("failed to read Noise relay handshake request: {error}");
                            send_reset(&physical_outgoing_tx, stream_id);
                            continue;
                        }
                    };

                // The authorization is encrypted inside the first IK message.
                // It is meaningful only alongside the initiator static key
                // that Clatter authenticated from that same message.
                let authorization = match String::from_utf8(pending.take_payload()) {
                    Ok(authorization)
                        if authorization.len() <= MAX_HARNESS_KEY_AUTHORIZATION_BYTES =>
                    {
                        authorization
                    }
                    Ok(_) => {
                        warn!("Noise relay handshake authorization is too long");
                        send_reset(&physical_outgoing_tx, stream_id);
                        continue;
                    }
                    Err(_) => {
                        warn!("Noise relay handshake authorization is not UTF-8");
                        send_reset(&physical_outgoing_tx, stream_id);
                        continue;
                    }
                };
                let harness_public_key = pending.initiator_public_key().clone();
                let validation_id = next_validation_id;
                let Some(next_id) = next_validation_id.checked_add(1) else {
                    warn!("Noise relay harness key validation id exhausted");
                    send_reset(&physical_outgoing_tx, stream_id);
                    continue;
                };
                next_validation_id = next_id;
                pending_handshakes.insert(
                    stream_id.clone(),
                    PendingHandshake {
                        validation_id,
                        handshake: pending,
                    },
                );
                let validator = validator.clone();

                // Validation is time-bounded and concurrency-bounded above.
                // Failure leaves no transport state and returns a generic
                // protocol reset to avoid exposing authorization details.
                validation_tasks.spawn(async move {
                    let result = match timeout(
                        HARNESS_KEY_VALIDATION_TIMEOUT,
                        validator.validate_harness_key(&harness_public_key, &authorization),
                    )
                    .await
                    {
                        Ok(result) => result,
                        Err(_) => Err(ExecServerError::Protocol(
                            "timed out validating Noise relay harness key".to_string(),
                        )),
                    };
                    HarnessKeyValidationResult {
                        stream_id,
                        validation_id,
                        result,
                    }
                });
            }
            RelayFrameBodyKind::Data => {
                // Data before handshake completion is always invalid. Removing
                // pending state makes the time-bounded validation result stale,
                // so it can never complete a stream after this protocol error.
                let Some(stream) = streams.get_mut(&stream_id) else {
                    pending_handshakes.remove(&stream_id);
                    send_reset(&physical_outgoing_tx, stream_id);
                    continue;
                };
                let data = match frame.into_data() {
                    Ok(data) => data,
                    Err(error) => {
                        warn!("dropping malformed Noise relay data frame: {error}");
                        streams.remove(&stream_id);
                        send_reset(&physical_outgoing_tx, stream_id);
                        continue;
                    }
                };
                if let Err(error) = stream.receive_data(data) {
                    warn!("failed to process Noise relay payload: {error}");
                    streams.remove(&stream_id);
                    send_reset(&physical_outgoing_tx, stream_id);
                }
            }
            RelayFrameBodyKind::Reset => {
                pending_handshakes.remove(&stream_id);
                if let Some(stream) = streams.remove(&stream_id) {
                    // Reset is cleartext relay control and is not authenticated
                    // by Noise. Honor its availability effect, but never
                    // forward attacker-controlled reason text into logs.
                    stream.disconnect(/*reason*/ None);
                }
            }
            RelayFrameBodyKind::Ack
            | RelayFrameBodyKind::Resume
            | RelayFrameBodyKind::Heartbeat => {}
        }
    }

    for (_stream_id, stream) in streams {
        stream.disconnect(/*reason*/ None);
    }
    // Dropping the JoinSet below aborts any still-running registry validations.
    // Await an abort only when the select loop did not already consume the
    // writer result.
    if !physical_writer_task.is_finished() {
        physical_writer_task.abort();
        let _ = physical_writer_task.await;
    }
}

struct PendingHandshake {
    validation_id: u64,
    handshake: PendingResponderHandshake,
}

struct HarnessKeyValidationResult {
    stream_id: String,
    validation_id: u64,
    result: Result<(), ExecServerError>,
}

fn send_reset(physical_outgoing_tx: &mpsc::Sender<Vec<u8>>, stream_id: String) {
    let reset = RelayMessageFrame::reset(stream_id, NOISE_RELAY_RESET_REASON.to_string());
    // Resets are best effort. Untrusted relay input must never block the shared
    // state machine behind an overloaded physical writer queue.
    let _ = physical_outgoing_tx.try_send(encode_relay_message_frame(&reset));
}

#[cfg(test)]
#[path = "environment_tests.rs"]
mod tests;
