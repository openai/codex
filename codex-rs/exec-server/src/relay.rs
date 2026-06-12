use std::collections::HashMap;
use std::time::Duration;

use codex_app_server_protocol::JSONRPCMessage;
use futures::Sink;
use futures::SinkExt;
use futures::Stream;
use futures::StreamExt;
use prost::Message as ProstMessage;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tokio::time::timeout;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;
use tracing::debug;
use tracing::info;
use tracing::warn;
use uuid::Uuid;

use crate::ExecServerError;
use crate::connection::CHANNEL_CAPACITY;
use crate::connection::JsonRpcConnection;
use crate::connection::JsonRpcConnectionEvent;
use crate::connection::JsonRpcTransport;
use crate::noise_channel::NoiseChannelIdentity;
use crate::noise_channel::NoiseChannelPublicKey;
use crate::noise_channel::PendingResponderHandshake;
use crate::noise_channel::noise_channel_prologue;
use crate::noise_relay::NOISE_RELAY_RESET_REASON;
use crate::noise_relay::executor_stream::ClosedNoiseVirtualStream;
use crate::noise_relay::executor_stream::NoiseVirtualStream;
use crate::noise_relay::executor_stream::spawn_noise_virtual_stream;
use crate::relay_proto::RelayData;
use crate::relay_proto::RelayHandshake;
use crate::relay_proto::RelayMessageFrame;
use crate::relay_proto::RelayReset;
use crate::relay_proto::RelayResume;
use crate::relay_proto::relay_message_frame;
use crate::server::ConnectionProcessor;

const RELAY_MESSAGE_FRAME_VERSION: u32 = 1;
const MAX_ACTIVE_NOISE_RELAY_STREAMS: usize = 128;
const MAX_FAILED_NOISE_HANDSHAKES: usize = 8;
const MAX_HARNESS_KEY_AUTHORIZATION_BYTES: usize = 4096;
const MAX_PENDING_HANDSHAKE_VALIDATIONS: usize = 32;
const HARNESS_KEY_VALIDATION_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum RelayFrameBodyKind {
    Data,
    Ack,
    Resume,
    Reset,
    Heartbeat,
    Handshake,
}

impl RelayMessageFrame {
    pub(crate) fn data(stream_id: String, seq: u32, payload: Vec<u8>) -> Self {
        Self {
            version: RELAY_MESSAGE_FRAME_VERSION,
            stream_id,
            ack: 0,
            ack_bits: 0,
            body: Some(relay_message_frame::Body::Data(RelayData {
                seq,
                segment_index: 0,
                segment_count: 1,
                payload,
            })),
        }
    }

    pub(crate) fn resume(stream_id: String) -> Self {
        Self {
            version: RELAY_MESSAGE_FRAME_VERSION,
            stream_id,
            ack: 0,
            ack_bits: 0,
            body: Some(relay_message_frame::Body::Resume(RelayResume {
                next_seq: 0,
            })),
        }
    }

    pub(crate) fn handshake(stream_id: String, payload: Vec<u8>) -> Self {
        Self {
            version: RELAY_MESSAGE_FRAME_VERSION,
            stream_id,
            ack: 0,
            ack_bits: 0,
            body: Some(relay_message_frame::Body::Handshake(RelayHandshake {
                payload,
            })),
        }
    }

    pub(crate) fn reset(stream_id: String, reason: String) -> Self {
        Self {
            version: RELAY_MESSAGE_FRAME_VERSION,
            stream_id,
            ack: 0,
            ack_bits: 0,
            body: Some(relay_message_frame::Body::Reset(RelayReset { reason })),
        }
    }

    pub(crate) fn validate(&self) -> Result<RelayFrameBodyKind, ExecServerError> {
        if self.version != RELAY_MESSAGE_FRAME_VERSION {
            return Err(ExecServerError::Protocol(format!(
                "unsupported relay message frame version {}",
                self.version
            )));
        }
        if self.stream_id.trim().is_empty() {
            return Err(ExecServerError::Protocol(
                "relay message frame is missing stream_id".to_string(),
            ));
        }
        match self.body.as_ref() {
            Some(relay_message_frame::Body::Data(data)) => {
                if data.segment_index != 0 || data.segment_count != 1 || data.payload.is_empty() {
                    return Err(ExecServerError::Protocol(
                        "relay data message frame is missing required fields".to_string(),
                    ));
                }
                Ok(RelayFrameBodyKind::Data)
            }
            Some(relay_message_frame::Body::AckFrame(_)) => Ok(RelayFrameBodyKind::Ack),
            Some(relay_message_frame::Body::Resume(_)) => Ok(RelayFrameBodyKind::Resume),
            Some(relay_message_frame::Body::Reset(reset)) => {
                if reset.reason.is_empty() {
                    return Err(ExecServerError::Protocol(
                        "relay reset message frame is missing reason".to_string(),
                    ));
                }
                Ok(RelayFrameBodyKind::Reset)
            }
            Some(relay_message_frame::Body::Heartbeat(_)) => Ok(RelayFrameBodyKind::Heartbeat),
            Some(relay_message_frame::Body::Handshake(handshake)) => {
                if handshake.payload.is_empty() {
                    return Err(ExecServerError::Protocol(
                        "relay handshake message frame is missing payload".to_string(),
                    ));
                }
                Ok(RelayFrameBodyKind::Handshake)
            }
            None => Err(ExecServerError::Protocol(
                "relay message frame is missing body".to_string(),
            )),
        }
    }

    pub(crate) fn into_data(self) -> Result<RelayData, ExecServerError> {
        let kind = self.validate()?;
        if kind != RelayFrameBodyKind::Data {
            return Err(ExecServerError::Protocol(
                "expected relay data message frame".to_string(),
            ));
        }
        match self.body {
            Some(relay_message_frame::Body::Data(data)) => Ok(data),
            _ => Err(ExecServerError::Protocol(
                "expected relay data message frame".to_string(),
            )),
        }
    }

    fn into_jsonrpc_message(self) -> Result<JSONRPCMessage, ExecServerError> {
        let payload = self.into_data()?.payload;
        serde_json::from_slice(&payload).map_err(ExecServerError::Json)
    }

    pub(crate) fn into_handshake_payload(self) -> Result<Vec<u8>, ExecServerError> {
        let kind = self.validate()?;
        if kind != RelayFrameBodyKind::Handshake {
            return Err(ExecServerError::Protocol(
                "expected relay handshake message frame".to_string(),
            ));
        }
        match self.body {
            Some(relay_message_frame::Body::Handshake(handshake)) => Ok(handshake.payload),
            _ => Err(ExecServerError::Protocol(
                "expected relay handshake message frame".to_string(),
            )),
        }
    }

    pub(crate) fn into_reset_reason(self) -> Option<String> {
        match self.body {
            Some(relay_message_frame::Body::Reset(reset)) if !reset.reason.is_empty() => {
                Some(reset.reason)
            }
            _ => None,
        }
    }
}

pub(crate) fn encode_relay_message_frame(frame: &RelayMessageFrame) -> Vec<u8> {
    frame.encode_to_vec()
}

pub(crate) fn decode_relay_message_frame(
    payload: &[u8],
) -> Result<RelayMessageFrame, ExecServerError> {
    RelayMessageFrame::decode(payload)
        .map_err(|err| ExecServerError::Protocol(format!("invalid relay message frame: {err}")))
}

pub(crate) fn jsonrpc_payload(message: &JSONRPCMessage) -> Result<Vec<u8>, ExecServerError> {
    serde_json::to_vec(message).map_err(ExecServerError::Json)
}

pub(crate) fn harness_connection_from_websocket<T, E>(
    stream: T,
    connection_label: String,
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
        let reader_label = connection_label;
        let reader_stream_id = stream_id.clone();
        let resume = RelayMessageFrame::resume(stream_id.clone());
        if websocket
            .send(Message::Binary(encode_relay_message_frame(&resume).into()))
            .await
            .is_err()
        {
            let _ = disconnected_tx.send(true);
            return;
        }

        let mut next_seq = 0u32;
        loop {
            tokio::select! {
                maybe_message = outgoing_rx.recv() => {
                    let Some(message) = maybe_message else {
                        break;
                    };
                    let payload = match jsonrpc_payload(&message) {
                        Ok(payload) => payload,
                        Err(err) => {
                            warn!("failed to serialize JSON-RPC payload for relay transport: {err}");
                            break;
                        }
                    };
                    let frame = RelayMessageFrame::data(stream_id.clone(), next_seq, payload);
                    next_seq = next_seq.wrapping_add(1);
                    if websocket
                        .send(Message::Binary(encode_relay_message_frame(&frame).into()))
                        .await
                        .is_err()
                    {
                        let _ = disconnected_tx.send(true);
                        break;
                    }
                }
                incoming_message = websocket.next() => {
                    match incoming_message {
                        Some(Ok(Message::Binary(payload))) => {
                            let frame = match decode_relay_message_frame(payload.as_ref()) {
                                Ok(frame) => frame,
                                Err(err) => {
                                    let _ = incoming_tx
                                        .send(JsonRpcConnectionEvent::MalformedMessage {
                                            reason: format!(
                                                "failed to parse relay message frame from {reader_label}: {err}"
                                            ),
                                        })
                                        .await;
                                    continue;
                                }
                            };
                            if frame.stream_id != reader_stream_id {
                                continue;
                            }
                            let kind = match frame.validate() {
                                Ok(kind) => kind,
                                Err(err) => {
                                    let _ = incoming_tx
                                        .send(JsonRpcConnectionEvent::MalformedMessage {
                                            reason: err.to_string(),
                                        })
                                        .await;
                                    continue;
                                }
                            };
                            match kind {
                                RelayFrameBodyKind::Data => match frame.into_jsonrpc_message() {
                                    Ok(message) => {
                                        if incoming_tx
                                            .send(JsonRpcConnectionEvent::Message(message))
                                            .await
                                            .is_err()
                                        {
                                            break;
                                        }
                                    }
                                    Err(err) => {
                                        let _ = incoming_tx
                                            .send(JsonRpcConnectionEvent::MalformedMessage {
                                                reason: err.to_string(),
                                            })
                                            .await;
                                    }
                                },
                                RelayFrameBodyKind::Reset => {
                                    let _ = disconnected_tx.send(true);
                                    let _ = incoming_tx
                                        .send(JsonRpcConnectionEvent::Disconnected {
                                            reason: frame.into_reset_reason(),
                                        })
                                        .await;
                                    break;
                                }
                                RelayFrameBodyKind::Ack
                                | RelayFrameBodyKind::Resume
                                | RelayFrameBodyKind::Heartbeat
                                | RelayFrameBodyKind::Handshake => {}
                            }
                        }
                        Some(Ok(Message::Close(_))) | None => {
                            let _ = disconnected_tx.send(true);
                            let _ = incoming_tx
                                .send(JsonRpcConnectionEvent::Disconnected { reason: None })
                                .await;
                            break;
                        }
                        Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_))) => {}
                        Some(Ok(Message::Text(_))) => {
                            let _ = incoming_tx
                                .send(JsonRpcConnectionEvent::MalformedMessage {
                                    reason: "relay exec-server transport expects binary protobuf frames"
                                        .to_string(),
                                })
                                .await;
                        }
                        Some(Err(err)) => {
                            let _ = disconnected_tx.send(true);
                            let _ = incoming_tx
                                .send(JsonRpcConnectionEvent::Disconnected {
                                    reason: Some(format!(
                                        "failed to read relay websocket frame from {reader_label}: {err}"
                                    )),
                                })
                                .await;
                            break;
                        }
                    }
                }
            }
        }
    });

    JsonRpcConnection {
        outgoing_tx,
        incoming_rx,
        disconnected_rx,
        task_handles: vec![websocket_task],
        transport: JsonRpcTransport::Plain,
    }
}

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
                    .is_some_and(|stream| stream.instance_id == closed_stream.instance_id);
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
                let prologue =
                    noise_channel_prologue(&environment_id, &executor_registration_id, &stream_id);
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
                let authorization = match String::from_utf8(std::mem::take(&mut pending.payload)) {
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
                let harness_public_key = pending.initiator_public_key.clone();
                let validation_id = next_validation_id;
                next_validation_id += 1;
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
#[path = "relay_noise_tests.rs"]
mod noise_tests;

#[cfg(test)]
mod tests {
    use std::pin::Pin;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::Ordering;
    use std::task::Context;
    use std::task::Poll;
    use std::time::Duration;

    use codex_app_server_protocol::JSONRPCRequest;
    use codex_app_server_protocol::RequestId;
    use futures::Sink;
    use futures::Stream;
    use futures::channel::mpsc as futures_mpsc;
    use futures::task::AtomicWaker;
    use tokio::net::TcpListener;
    use tokio::time::timeout;
    use tokio_tungstenite::WebSocketStream;
    use tokio_tungstenite::accept_async;
    use tokio_tungstenite::connect_async;
    use tokio_tungstenite::tungstenite::Message;

    use super::*;

    #[tokio::test]
    async fn harness_connection_receives_relay_data() -> anyhow::Result<()> {
        let (client_websocket, mut server_websocket) = websocket_pair().await?;
        let mut connection =
            harness_connection_from_websocket(client_websocket, "test".to_string());
        let stream_id = read_resume_stream_id(&mut server_websocket).await?;
        let message = test_jsonrpc_message();

        server_websocket
            .send(Message::Binary(
                encode_relay_message_frame(&RelayMessageFrame::data(
                    stream_id,
                    /*seq*/ 0,
                    jsonrpc_payload(&message)?,
                ))
                .into(),
            ))
            .await?;
        assert!(matches!(
            timeout(Duration::from_secs(1), connection.incoming_rx.recv()).await?,
            Some(JsonRpcConnectionEvent::Message(actual)) if actual == message
        ));

        drop(connection);
        Ok(())
    }

    #[tokio::test]
    async fn harness_connection_reports_text_frames_as_malformed() -> anyhow::Result<()> {
        let (client_websocket, mut server_websocket) = websocket_pair().await?;
        let mut connection =
            harness_connection_from_websocket(client_websocket, "test".to_string());

        read_resume_stream_id(&mut server_websocket).await?;
        server_websocket.send(Message::Text("nope".into())).await?;
        assert!(matches!(
            timeout(Duration::from_secs(1), connection.incoming_rx.recv()).await?,
            Some(JsonRpcConnectionEvent::MalformedMessage { reason })
                if reason == "relay exec-server transport expects binary protobuf frames"
        ));

        drop(connection);
        Ok(())
    }

    #[tokio::test]
    async fn harness_connection_reports_server_close() -> anyhow::Result<()> {
        let (client_websocket, mut server_websocket) = websocket_pair().await?;
        let mut connection =
            harness_connection_from_websocket(client_websocket, "test".to_string());

        read_resume_stream_id(&mut server_websocket).await?;
        server_websocket.close(None).await?;
        assert!(matches!(
            timeout(Duration::from_secs(1), connection.incoming_rx.recv()).await?,
            Some(JsonRpcConnectionEvent::Disconnected { reason: None })
        ));

        drop(connection);
        Ok(())
    }

    #[tokio::test]
    async fn harness_connection_keeps_outbound_frame_while_send_is_backpressured()
    -> anyhow::Result<()> {
        let (websocket, control, mut outbound_rx) =
            ControlledWebSocket::new(/*write_ready*/ true);
        let mut connection = harness_connection_from_websocket(websocket, "test".to_string());
        let Message::Binary(resume_payload) = timeout(Duration::from_secs(1), outbound_rx.next())
            .await?
            .expect("resume frame")
        else {
            anyhow::bail!("expected relay resume frame");
        };
        let stream_id = decode_relay_message_frame(resume_payload.as_ref())?.stream_id;
        let message = test_jsonrpc_message();

        control.set_write_blocked();
        connection.outgoing_tx.send(message.clone()).await?;
        control.wait_for_blocked_write().await?;
        control.send_inbound(Message::Pong(b"check".to_vec().into()))?;
        assert!(
            timeout(Duration::from_millis(50), connection.incoming_rx.recv())
                .await
                .is_err()
        );

        control.set_write_ready();
        let Message::Binary(data_payload) = timeout(Duration::from_secs(1), outbound_rx.next())
            .await?
            .expect("data frame")
        else {
            anyhow::bail!("expected relay data frame");
        };
        let frame = decode_relay_message_frame(data_payload.as_ref())?;
        assert_eq!(frame.stream_id, stream_id);
        assert_eq!(frame.into_jsonrpc_message()?, message);
        drop(connection);
        Ok(())
    }

    async fn websocket_pair() -> anyhow::Result<(
        WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
        WebSocketStream<tokio::net::TcpStream>,
    )> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let websocket_url = format!("ws://{}", listener.local_addr()?);
        let server_task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await?;
            accept_async(stream).await.map_err(anyhow::Error::from)
        });
        let (client_websocket, _) = connect_async(websocket_url).await?;
        let server_websocket = server_task.await??;
        Ok((client_websocket, server_websocket))
    }

    async fn read_resume_stream_id(
        websocket: &mut WebSocketStream<tokio::net::TcpStream>,
    ) -> anyhow::Result<String> {
        let message = timeout(Duration::from_secs(1), websocket.next())
            .await?
            .expect("websocket should stay open")?;
        let Message::Binary(payload) = message else {
            anyhow::bail!("expected relay resume frame, got {message:?}");
        };
        let frame = decode_relay_message_frame(payload.as_ref())?;
        assert_eq!(frame.validate()?, RelayFrameBodyKind::Resume);
        Ok(frame.stream_id)
    }

    fn test_jsonrpc_message() -> JSONRPCMessage {
        JSONRPCMessage::Request(JSONRPCRequest {
            id: RequestId::Integer(1),
            method: "test".to_string(),
            params: None,
            trace: None,
        })
    }

    struct ControlledWebSocket {
        inbound_rx: futures_mpsc::UnboundedReceiver<Result<Message, std::convert::Infallible>>,
        outbound_tx: futures_mpsc::UnboundedSender<Message>,
        write_ready: Arc<AtomicBool>,
        write_blocked: Arc<AtomicBool>,
        write_blocked_waker: Arc<AtomicWaker>,
        write_waker: Arc<AtomicWaker>,
    }

    struct ControlledWebSocketHandle {
        inbound_tx: futures_mpsc::UnboundedSender<Result<Message, std::convert::Infallible>>,
        write_ready: Arc<AtomicBool>,
        write_blocked: Arc<AtomicBool>,
        write_blocked_waker: Arc<AtomicWaker>,
        write_waker: Arc<AtomicWaker>,
    }

    impl ControlledWebSocket {
        fn new(
            write_ready: bool,
        ) -> (
            Self,
            ControlledWebSocketHandle,
            futures_mpsc::UnboundedReceiver<Message>,
        ) {
            let (inbound_tx, inbound_rx) = futures_mpsc::unbounded();
            let (outbound_tx, outbound_rx) = futures_mpsc::unbounded();
            let write_ready = Arc::new(AtomicBool::new(write_ready));
            let write_blocked = Arc::new(AtomicBool::new(false));
            let write_blocked_waker = Arc::new(AtomicWaker::new());
            let write_waker = Arc::new(AtomicWaker::new());
            (
                Self {
                    inbound_rx,
                    outbound_tx,
                    write_ready: Arc::clone(&write_ready),
                    write_blocked: Arc::clone(&write_blocked),
                    write_blocked_waker: Arc::clone(&write_blocked_waker),
                    write_waker: Arc::clone(&write_waker),
                },
                ControlledWebSocketHandle {
                    inbound_tx,
                    write_ready,
                    write_blocked,
                    write_blocked_waker,
                    write_waker,
                },
                outbound_rx,
            )
        }
    }

    impl ControlledWebSocketHandle {
        fn send_inbound(&self, message: Message) -> anyhow::Result<()> {
            self.inbound_tx
                .unbounded_send(Ok(message))
                .map_err(anyhow::Error::from)
        }

        fn set_write_blocked(&self) {
            self.write_ready.store(false, Ordering::Release);
        }

        fn set_write_ready(&self) {
            self.write_ready.store(true, Ordering::Release);
            self.write_waker.wake();
        }

        async fn wait_for_blocked_write(&self) -> anyhow::Result<()> {
            timeout(
                Duration::from_secs(1),
                futures::future::poll_fn(|cx| {
                    if self.write_blocked.load(Ordering::Acquire) {
                        Poll::Ready(())
                    } else {
                        self.write_blocked_waker.register(cx.waker());
                        Poll::Pending
                    }
                }),
            )
            .await?;
            Ok(())
        }
    }

    impl Sink<Message> for ControlledWebSocket {
        type Error = std::convert::Infallible;

        fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            if self.write_ready.load(Ordering::Acquire) {
                Poll::Ready(Ok(()))
            } else {
                self.write_blocked.store(true, Ordering::Release);
                self.write_blocked_waker.wake();
                self.write_waker.register(cx.waker());
                Poll::Pending
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
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn poll_close(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
    }

    impl Stream for ControlledWebSocket {
        type Item = Result<Message, std::convert::Infallible>;

        fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            Pin::new(&mut self.inbound_rx).poll_next(cx)
        }
    }
}
