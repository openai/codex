//! Executor side of the multiplexed Noise relay.
//!
//! A stream is pending after its first IK message is parsed, and becomes active
//! only after the registry authorizes the authenticated harness key. Registry
//! checks run outside the websocket loop. `validation_id` distinguishes reused
//! stream IDs so a late validation cannot activate a newer stream.

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
use tracing::info;
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
const MAX_FAILED_NOISE_HANDSHAKES: usize = 8;
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

/// Serve authenticated virtual JSON-RPC streams over one executor websocket.
///
/// Parsing the first Noise message authenticates the harness key. Only a
/// successful registry check turns that pending handshake into a virtual stream.
#[tracing::instrument(
    level = "debug",
    skip_all,
    fields(
        noise_side = "executor",
        environment_id = %environment_id,
        executor_registration_id = %executor_registration_id,
    )
)]
pub(crate) async fn run_multiplexed_environment<S, V>(
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
    // Use a separate writer so this loop never waits on the channel it drains.
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
    let mut failed_handshakes = 0usize;
    let mut next_validation_id = 0u64;

    loop {
        // Registry calls run separately so a slow check does not block the relay.
        let frame = tokio::select! {
            writer_result = &mut physical_writer_task => {
                if let Err(error) = writer_result {
                    warn!("Noise multiplexed environment websocket writer failed: {error}");
                }
                break;
            }
            Some(closed_stream) = closed_stream_rx.recv() => {
                // A stream ID may have been reused before this writer exits.
                // Remove only the instance that sent the notification.
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
                        // The stream ID may have been reused while validation ran.
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
                            // Validator errors may contain authorization details.
                            warn!(
                                noise_event = "authorization",
                                noise_outcome = "error",
                                noise_reason = "authorization_failed",
                                "Noise harness authorization failed"
                            );
                            debug!(
                                stream_id = validation_result.stream_id,
                                "Noise harness authorization failure details"
                            );
                            send_reset(&physical_outgoing_tx, validation_result.stream_id);
                            if failed_handshake_budget_exhausted(&mut failed_handshakes) {
                                warn!("closing Noise relay after repeated handshake failures");
                                break;
                            }
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
                                if failed_handshake_budget_exhausted(&mut failed_handshakes) {
                                    warn!("closing Noise relay after repeated handshake failures");
                                    break;
                                }
                                continue;
                            }
                        };
                        let response = RelayMessageFrame::handshake(
                            validation_result.stream_id.clone(),
                            response,
                        );
                        // Do not leave a half-open stream if the handshake reply
                        // cannot be queued immediately.
                        if physical_outgoing_tx
                            .try_send(encode_relay_message_frame(&response))
                            .is_err()
                        {
                            break;
                        }
                        info!(
                            noise_event = "handshake",
                            noise_outcome = "ok",
                            "Noise executor handshake completed"
                        );
                        debug!(
                            stream_id = validation_result.stream_id,
                            active_streams = streams.len() + 1,
                            "Noise executor stream activated"
                        );
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
                // Reject duplicate or busy streams before paying for a hybrid
                // handshake. Malformed attempts that reach cryptography are
                // covered by the connection-wide failure budget below.
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
                            if failed_handshake_budget_exhausted(&mut failed_handshakes) {
                                warn!("closing Noise relay after repeated handshake failures");
                                break;
                            }
                            continue;
                        }
                    };

                // The authorization and authenticated harness key come from the
                // same encrypted IK message and are validated together.
                let authorization = match String::from_utf8(pending.take_payload()) {
                    Ok(authorization)
                        if authorization.len() <= MAX_HARNESS_KEY_AUTHORIZATION_BYTES =>
                    {
                        Some(authorization)
                    }
                    Ok(_) => {
                        warn!("Noise relay handshake authorization is too long");
                        None
                    }
                    Err(_) => {
                        warn!("Noise relay handshake authorization is not UTF-8");
                        None
                    }
                };
                let Some(authorization) = authorization else {
                    send_reset(&physical_outgoing_tx, stream_id);
                    if failed_handshake_budget_exhausted(&mut failed_handshakes) {
                        warn!("closing Noise relay after repeated handshake failures");
                        break;
                    }
                    continue;
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

                // Failed validation leaves no transport state and sends only a
                // generic reset.
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
                // Removing pending state also makes any in-flight validation stale.
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
                    // The reset reason is unauthenticated, so do not log it.
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
    // Dropping the JoinSet aborts any registry checks still running.
    if !physical_writer_task.is_finished() {
        physical_writer_task.abort();
        let _ = physical_writer_task.await;
    }
}

/// Charge one failed authenticated-channel attempt to this physical relay.
///
/// Closing after a small fixed budget prevents a peer that has not been
/// authorized from triggering unbounded hybrid handshakes or registry checks.
fn failed_handshake_budget_exhausted(failed_handshakes: &mut usize) -> bool {
    *failed_handshakes += 1;
    *failed_handshakes >= MAX_FAILED_NOISE_HANDSHAKES
}

/// Responder state held while registry authorization is pending.
struct PendingHandshake {
    validation_id: u64,
    handshake: PendingResponderHandshake,
}

/// `validation_id` prevents an old check from completing a reused `stream_id`.
struct HarnessKeyValidationResult {
    stream_id: String,
    validation_id: u64,
    result: Result<(), ExecServerError>,
}

/// Queue a best-effort reset without blocking the shared websocket loop.
/// Reset reasons are relay control data and are not treated as trusted text.
fn send_reset(physical_outgoing_tx: &mpsc::Sender<Vec<u8>>, stream_id: String) {
    let reset = RelayMessageFrame::reset(stream_id, NOISE_RELAY_RESET_REASON.to_string());
    let _ = physical_outgoing_tx.try_send(encode_relay_message_frame(&reset));
}

#[cfg(test)]
#[path = "environment_tests.rs"]
mod tests;
