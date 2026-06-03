use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use futures::SinkExt;
use futures::StreamExt;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tokio::time::timeout;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;
use tracing::debug;
use tracing::warn;

use crate::ExecServerError;
use crate::connection::CHANNEL_CAPACITY;
use crate::connection::JsonRpcConnection;
use crate::connection::JsonRpcConnectionEvent;
use crate::connection::JsonRpcTransport;
use crate::relay::RelayFrameBodyKind;
use crate::relay::decode_relay_message_frame;
use crate::relay::encode_relay_message_frame;
use crate::relay_proto::RelayData;
use crate::relay_proto::RelayMessageFrame;
use crate::secure_channel::PendingResponderHandshake;
use crate::secure_channel::SecureChannelIdentity;
use crate::secure_channel::SecureChannelPublicKey;
use crate::secure_channel::SecureTransport;
use crate::secure_channel::secure_channel_prologue;
use crate::secure_relay::message_framing::JsonRpcMessageDecoder;
use crate::secure_relay::message_framing::SECURE_RECORD_PLAINTEXT_LEN;
use crate::secure_relay::message_framing::frame_jsonrpc_message;
use crate::secure_relay::ordered_ciphertext::OrderedCiphertextFrames;
use crate::secure_relay::take_next_sequence;
use crate::server::ConnectionProcessor;

const SECURE_RELAY_RESET_REASON: &str = "secure_relay_protocol_error";
const MAX_ACTIVE_SECURE_RELAY_STREAMS: usize = 128;
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
        harness_public_key: &SecureChannelPublicKey,
        authorization: &str,
    ) -> impl std::future::Future<Output = Result<(), ExecServerError>> + Send;
}

/// Serve many authenticated virtual JSON-RPC streams over one executor websocket.
///
/// Each stream has an independent Noise handshake and transport state. The
/// outer websocket and rendezvous route are treated as untrusted delivery:
/// malformed, unauthorized, or cryptographically invalid streams fail closed
/// without creating a `JsonRpcConnection`.
pub(crate) async fn run_secure_multiplexed_environment<S, V>(
    stream: WebSocketStream<S>,
    processor: ConnectionProcessor,
    environment_id: String,
    executor_registration_id: String,
    identity: SecureChannelIdentity,
    validator: V,
) where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    V: HarnessKeyValidator + Clone + 'static,
{
    let mut websocket = stream;
    let (physical_outgoing_tx, mut physical_outgoing_rx) =
        mpsc::channel::<Vec<u8>>(CHANNEL_CAPACITY);
    let mut streams: HashMap<String, SecureVirtualStream> = HashMap::new();
    let mut pending_handshakes: HashMap<String, PendingHandshake> = HashMap::new();
    let mut validation_tasks: JoinSet<HarnessKeyValidationResult> = JoinSet::new();
    let mut next_validation_id = 0u64;

    loop {
        // Keep registry validation out of the main relay loop. A slow or
        // malicious authorization request must not block existing streams or
        // prevent other handshakes from being received and bounded.
        let frame = tokio::select! {
            maybe_encoded = physical_outgoing_rx.recv() => {
                let Some(encoded) = maybe_encoded else {
                    break;
                };
                if websocket.send(Message::Binary(encoded.into())).await.is_err() {
                    break;
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
                        if let Err(error) = validation_result.result {
                            warn!("secure relay harness key validation failed: {error}");
                            send_reset(&physical_outgoing_tx, validation_result.stream_id).await;
                            continue;
                        }
                        if streams.len() >= MAX_ACTIVE_SECURE_RELAY_STREAMS {
                            warn!("secure relay has too many active streams");
                            send_reset(&physical_outgoing_tx, validation_result.stream_id).await;
                            continue;
                        }

                        // This is the only point where the responder completes
                        // IK and exposes a JSON-RPC stream: Noise authenticated
                        // the harness key and the registry authorized it.
                        let (transport, response) = match pending.handshake.complete() {
                            Ok(completed) => completed,
                            Err(error) => {
                                warn!("failed to complete secure relay handshake: {error}");
                                send_reset(&physical_outgoing_tx, validation_result.stream_id).await;
                                continue;
                            }
                        };
                        let response = RelayMessageFrame::handshake(
                            validation_result.stream_id.clone(),
                            response,
                        );
                        if physical_outgoing_tx
                            .send(encode_relay_message_frame(&response))
                            .await
                            .is_err()
                        {
                            break;
                        }
                        streams.insert(
                            validation_result.stream_id.clone(),
                            spawn_secure_virtual_stream(
                                validation_result.stream_id,
                                processor.clone(),
                                physical_outgoing_tx.clone(),
                                transport,
                            ),
                        );
                    }
                    Some(Err(error)) => {
                        warn!("secure relay harness key validation task failed: {error}");
                        let stream_ids = pending_handshakes.keys().cloned().collect::<Vec<_>>();
                        pending_handshakes.clear();
                        for stream_id in stream_ids {
                            send_reset(&physical_outgoing_tx, stream_id).await;
                        }
                    }
                    None => {}
                }
                continue;
            }
            incoming_message = websocket.next() => match incoming_message {
                Some(Ok(Message::Binary(payload))) => match decode_relay_message_frame(payload.as_ref()) {
                    Ok(frame) => frame,
                    Err(error) => {
                        warn!("dropping malformed secure relay frame from harness: {error}");
                        continue;
                    }
                },
                Some(Ok(Message::Close(_))) | None => break,
                Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_))) => continue,
                Some(Ok(Message::Text(_))) => {
                    warn!("dropping non-binary secure relay frame from harness");
                    continue;
                }
                Some(Err(error)) => {
                    debug!("secure multiplexed environment websocket read failed: {error}");
                    break;
                }
            }
        };

        let kind = match frame.validate() {
            Ok(kind) => kind,
            Err(error) => {
                warn!("dropping invalid secure relay frame: {error}");
                continue;
            }
        };
        let stream_id = frame.stream_id.clone();
        match kind {
            RelayFrameBodyKind::Handshake => {
                // Bound all pre-authentication state before doing expensive
                // hybrid cryptography or starting an external validation.
                if streams.contains_key(&stream_id) || pending_handshakes.contains_key(&stream_id) {
                    send_reset(&physical_outgoing_tx, stream_id).await;
                    continue;
                }
                if streams.len() >= MAX_ACTIVE_SECURE_RELAY_STREAMS {
                    warn!("secure relay has too many active streams");
                    send_reset(&physical_outgoing_tx, stream_id).await;
                    continue;
                }
                if validation_tasks.len() >= MAX_PENDING_HANDSHAKE_VALIDATIONS {
                    warn!("secure relay has too many pending harness key validations");
                    send_reset(&physical_outgoing_tx, stream_id).await;
                    continue;
                }
                let prologue = match secure_channel_prologue(
                    &environment_id,
                    &executor_registration_id,
                    &stream_id,
                ) {
                    Ok(prologue) => prologue,
                    Err(error) => {
                        warn!("failed to build secure relay prologue: {error}");
                        send_reset(&physical_outgoing_tx, stream_id).await;
                        continue;
                    }
                };
                let request = match frame.into_handshake_payload() {
                    Ok(request) => request,
                    Err(error) => {
                        warn!("failed to read secure relay handshake frame: {error}");
                        send_reset(&physical_outgoing_tx, stream_id).await;
                        continue;
                    }
                };
                let pending =
                    match PendingResponderHandshake::read_request(&identity, &prologue, &request) {
                        Ok(pending) => pending,
                        Err(error) => {
                            warn!("failed to read secure relay handshake request: {error}");
                            send_reset(&physical_outgoing_tx, stream_id).await;
                            continue;
                        }
                    };

                // The authorization is encrypted inside the first IK message.
                // It is meaningful only alongside the initiator static key
                // that Clatter authenticated from that same message.
                let authorization = match std::str::from_utf8(pending.payload()) {
                    Ok(authorization) => authorization.to_string(),
                    Err(_) => {
                        warn!("secure relay handshake authorization is not UTF-8");
                        send_reset(&physical_outgoing_tx, stream_id).await;
                        continue;
                    }
                };
                let harness_public_key = pending.initiator_public_key().clone();
                let validation_id = next_validation_id;
                let Some(next_id) = next_validation_id.checked_add(1) else {
                    warn!("secure relay harness key validation id exhausted");
                    send_reset(&physical_outgoing_tx, stream_id).await;
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
                            "timed out validating secure relay harness key".to_string(),
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
                // a pending handshake also ensures a peer cannot keep its
                // authorization task alive while sending application records.
                let Some(stream) = streams.get_mut(&stream_id) else {
                    pending_handshakes.remove(&stream_id);
                    send_reset(&physical_outgoing_tx, stream_id).await;
                    continue;
                };
                let data = match frame.into_data() {
                    Ok(data) => data,
                    Err(error) => {
                        warn!("dropping malformed secure relay data frame: {error}");
                        streams.remove(&stream_id);
                        send_reset(&physical_outgoing_tx, stream_id).await;
                        continue;
                    }
                };
                if let Err(error) = stream.receive_data(data).await {
                    warn!("failed to process secure relay payload: {error}");
                    streams.remove(&stream_id);
                    send_reset(&physical_outgoing_tx, stream_id).await;
                }
            }
            RelayFrameBodyKind::Reset => {
                pending_handshakes.remove(&stream_id);
                if let Some(stream) = streams.remove(&stream_id) {
                    stream.disconnect(frame.into_reset_reason()).await;
                }
            }
            RelayFrameBodyKind::Ack
            | RelayFrameBodyKind::Resume
            | RelayFrameBodyKind::Heartbeat => {}
        }
    }

    for (_stream_id, stream) in streams {
        stream.disconnect(/*reason*/ None).await;
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

struct SecureVirtualStream {
    incoming_tx: mpsc::Sender<JsonRpcConnectionEvent>,
    disconnected_tx: watch::Sender<bool>,
    transport: Arc<Mutex<SecureTransport>>,
    inbound_ciphertexts: OrderedCiphertextFrames,
    inbound_decoder: JsonRpcMessageDecoder,
}

impl SecureVirtualStream {
    async fn disconnect(self, reason: Option<String>) {
        let _ = self.disconnected_tx.send(true);
        let _ = self
            .incoming_tx
            .send(JsonRpcConnectionEvent::Disconnected { reason })
            .await;
    }

    async fn receive_data(&mut self, data: RelayData) -> Result<(), ExecServerError> {
        // Relay sequence ordering is enforced before taking the transport lock
        // and decrypting. Each virtual stream owns one ordered Noise nonce
        // space shared by its reader and writer transport halves.
        for ciphertext in self.inbound_ciphertexts.push(data.seq, data.payload)? {
            let plaintext = {
                let mut transport = self
                    .transport
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                transport.decrypt(&ciphertext)?
            };
            for message in self.inbound_decoder.push(&plaintext)? {
                self.incoming_tx
                    .send(JsonRpcConnectionEvent::Message(message))
                    .await
                    .map_err(|_| ExecServerError::Closed)?;
            }
        }
        Ok(())
    }
}

fn spawn_secure_virtual_stream(
    stream_id: String,
    processor: ConnectionProcessor,
    physical_outgoing_tx: mpsc::Sender<Vec<u8>>,
    transport: SecureTransport,
) -> SecureVirtualStream {
    let (json_outgoing_tx, mut json_outgoing_rx) = mpsc::channel(CHANNEL_CAPACITY);
    let (incoming_tx, incoming_rx) = mpsc::channel(CHANNEL_CAPACITY);
    let (disconnected_tx, disconnected_rx) = watch::channel(false);
    let transport = Arc::new(Mutex::new(transport));
    let writer_transport = Arc::clone(&transport);
    let writer_stream_id = stream_id;
    let writer_task = tokio::spawn(async move {
        let mut next_seq = 0u32;
        'writer: while let Some(message) = json_outgoing_rx.recv().await {
            // Frame first, then split into bounded Noise records. Each record
            // receives one checked relay sequence and is encrypted exactly
            // once, preserving the implicit Noise sending nonce.
            let framed = match frame_jsonrpc_message(&message) {
                Ok(framed) => framed,
                Err(error) => {
                    warn!("failed to frame secure virtual stream JSON-RPC payload: {error}");
                    break;
                }
            };
            for plaintext_record in framed.chunks(SECURE_RECORD_PLAINTEXT_LEN) {
                let seq = match take_next_sequence(&mut next_seq) {
                    Ok(seq) => seq,
                    Err(error) => {
                        warn!("secure virtual stream sequence exhausted: {error}");
                        break 'writer;
                    }
                };
                let ciphertext = {
                    let mut transport = writer_transport
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    transport.encrypt(plaintext_record)
                };
                let ciphertext = match ciphertext {
                    Ok(ciphertext) => ciphertext,
                    Err(error) => {
                        warn!("failed to encrypt secure virtual stream payload: {error}");
                        break 'writer;
                    }
                };
                let frame = RelayMessageFrame::data(writer_stream_id.clone(), seq, ciphertext);
                if physical_outgoing_tx
                    .send(encode_relay_message_frame(&frame))
                    .await
                    .is_err()
                {
                    break 'writer;
                }
            }
        }

        // Tell the harness to discard this virtual stream whenever its writer
        // exits, including processor shutdown or a cryptographic/send failure.
        // Otherwise the peer could wait indefinitely on a dead stream.
        let reset =
            RelayMessageFrame::reset(writer_stream_id, SECURE_RELAY_RESET_REASON.to_string());
        let _ = physical_outgoing_tx
            .send(encode_relay_message_frame(&reset))
            .await;
    });

    let connection = JsonRpcConnection {
        outgoing_tx: json_outgoing_tx,
        incoming_rx,
        disconnected_rx,
        task_handles: vec![writer_task],
        transport: JsonRpcTransport::External,
    };
    tokio::spawn(async move {
        processor.run_connection(connection).await;
    });

    SecureVirtualStream {
        incoming_tx,
        disconnected_tx,
        transport,
        inbound_ciphertexts: OrderedCiphertextFrames::default(),
        inbound_decoder: JsonRpcMessageDecoder::default(),
    }
}

async fn send_reset(physical_outgoing_tx: &mpsc::Sender<Vec<u8>>, stream_id: String) {
    let reset = RelayMessageFrame::reset(stream_id, SECURE_RELAY_RESET_REASON.to_string());
    let _ = physical_outgoing_tx
        .send(encode_relay_message_frame(&reset))
        .await;
}

#[cfg(test)]
#[path = "environment_tests.rs"]
mod tests;
