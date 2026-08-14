//! One executor-side virtual stream after the Noise handshake.
//!
//! The environment loop owns reads and a per-stream task owns writes. They share
//! `NoiseTransport` because its send and receive nonces live in the same value;
//! the mutex is never held across `.await`.

use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use tokio::sync::Notify;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tracing::warn;

use crate::ExecServerError;
use crate::connection::CHANNEL_CAPACITY;
use crate::connection::JsonRpcConnection;
use crate::connection::JsonRpcConnectionEvent;
use crate::connection::JsonRpcTransport;
use crate::noise_channel::NoiseTransport;
use crate::noise_relay::NOISE_RELAY_RESET_REASON;
use crate::noise_relay::message_framing::JsonRpcMessageDecoder;
use crate::noise_relay::message_framing::NOISE_RECORD_PLAINTEXT_LEN;
use crate::noise_relay::message_framing::frame_jsonrpc_message;
use crate::noise_relay::ordered_ciphertext::OrderedCiphertextFrames;
use crate::noise_relay::reliable_stream::MAX_UNACKED_BYTES;
use crate::noise_relay::reliable_stream::MAX_UNACKED_SEGMENTS;
use crate::noise_relay::reliable_stream::ReliableSender;
use crate::relay::RelayAckState;
use crate::relay::encode_relay_message_frame;
use crate::relay_proto::RelayData;
use crate::relay_proto::RelayMessageFrame;
use crate::server::ConnectionProcessor;
use crate::telemetry::ConnectionTransport;

const RELIABLE_RETRY_SCAN_INTERVAL: Duration = Duration::from_millis(50);
const MAX_RELIABLE_CIPHERTEXT_BYTES: usize = MAX_UNACKED_BYTES / MAX_UNACKED_SEGMENTS;

/// Identifies one completed virtual-stream instance.
///
/// Stream IDs are supplied by the untrusted relay peer. The instance ID lets
/// the environment verify that a delayed writer notification still belongs to
/// the active stream before retiring its routing ID.
pub(crate) struct ClosedNoiseVirtualStream {
    pub(crate) stream_id: String,
    pub(crate) instance_id: u64,
}

#[derive(Default)]
struct InboundAckState {
    latest: RelayAckState,
    pending: Option<RelayAckState>,
}

/// One authenticated JSON-RPC stream carried by the executor's physical relay.
///
/// Inbound delivery is intentionally nonblocking. An overloaded or abandoned
/// stream fails independently instead of stalling every stream multiplexed over
/// the same physical websocket.
pub(crate) struct NoiseVirtualStream {
    incoming_tx: mpsc::Sender<JsonRpcConnectionEvent>,
    disconnected_tx: watch::Sender<bool>,
    transport: Arc<Mutex<NoiseTransport>>,
    reliable_sender: Arc<Mutex<ReliableSender>>,
    inbound_ack_state: Arc<Mutex<InboundAckState>>,
    writer_wakeup: Arc<Notify>,
    inbound_ciphertexts: OrderedCiphertextFrames,
    inbound_decoder: JsonRpcMessageDecoder,
    pub(crate) instance_id: u64,
}

impl NoiseVirtualStream {
    pub(crate) fn disconnect(self, reason: Option<String>) {
        let _ = self.disconnected_tx.send(true);
        let _ = self
            .incoming_tx
            .try_send(JsonRpcConnectionEvent::Disconnected { reason });
    }

    /// Apply ack metadata from one post-handshake peer frame and wake the
    /// writer if it opened sequence or byte send capacity.
    pub(crate) fn process_peer_ack(&self, ack_state: RelayAckState) -> Result<(), ExecServerError> {
        let mut reliable_sender = self
            .reliable_sender
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        reliable_sender.process_peer_ack(ack_state)?;
        drop(reliable_sender);
        self.writer_wakeup.notify_one();
        Ok(())
    }

    /// Reorder and decrypt one inbound record, then queue complete JSON-RPC messages.
    /// This must stay nonblocking because all virtual streams share the read loop.
    pub(crate) fn receive_data(&mut self, data: RelayData) -> Result<(), ExecServerError> {
        for ciphertext in self.inbound_ciphertexts.push(data.seq, data.payload)? {
            let plaintext = {
                let mut transport = self
                    .transport
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                transport.decrypt(&ciphertext).map_err(|error| {
                    ExecServerError::Protocol(format!("Noise relay decryption failed: {error}"))
                })?
            };
            for message in self.inbound_decoder.push(&plaintext)? {
                self.incoming_tx
                    .try_send(JsonRpcConnectionEvent::Message(message))
                    .map_err(|_| {
                        ExecServerError::Protocol(
                            "Noise virtual stream inbound queue is full or closed".to_string(),
                        )
                    })?;
            }
        }
        let ack_state = self.inbound_ciphertexts.ack_state();
        let mut inbound_ack_state = self
            .inbound_ack_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        inbound_ack_state.latest = ack_state;
        inbound_ack_state.pending = Some(ack_state);
        drop(inbound_ack_state);
        self.writer_wakeup.notify_one();
        Ok(())
    }
}

/// Start JSON-RPC processing for a completed handshake.
///
/// The returned value is the read half; the spawned task owns outbound framing
/// and reports its instance ID on exit so stream-ID reuse is safe.
pub(crate) fn spawn_noise_virtual_stream(
    stream_id: String,
    instance_id: u64,
    processor: ConnectionProcessor,
    physical_outgoing_tx: mpsc::Sender<Vec<u8>>,
    closed_stream_tx: mpsc::Sender<ClosedNoiseVirtualStream>,
    transport: NoiseTransport,
) -> NoiseVirtualStream {
    let (json_outgoing_tx, mut json_outgoing_rx) = mpsc::channel(CHANNEL_CAPACITY);
    let (incoming_tx, incoming_rx) = mpsc::channel(CHANNEL_CAPACITY);
    let (disconnected_tx, disconnected_rx) = watch::channel(false);
    let transport = Arc::new(Mutex::new(transport));
    let reliable_sender = Arc::new(Mutex::new(ReliableSender::default()));
    let inbound_ack_state = Arc::new(Mutex::new(InboundAckState::default()));
    let writer_wakeup = Arc::new(Notify::new());
    let writer_transport = Arc::clone(&transport);
    let writer_reliable_sender = Arc::clone(&reliable_sender);
    let writer_inbound_ack_state = Arc::clone(&inbound_ack_state);
    let writer_wakeup_task = Arc::clone(&writer_wakeup);
    let writer_physical_outgoing_tx = physical_outgoing_tx;
    let processor_stream_id = stream_id.clone();
    let processor_closed_stream_tx = closed_stream_tx.clone();
    let writer_stream_id = stream_id;
    let writer_task = tokio::spawn(async move {
        let mut pending_outbound: Option<(Vec<u8>, usize)> = None;
        let mut retry_tick = tokio::time::interval_at(
            tokio::time::Instant::now() + RELIABLE_RETRY_SCAN_INTERVAL,
            RELIABLE_RETRY_SCAN_INTERVAL,
        );
        retry_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        'writer: loop {
            let can_send_pending = pending_outbound.is_some()
                && writer_reliable_sender
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .can_admit_ciphertext(MAX_RELIABLE_CIPHERTEXT_BYTES);
            let has_pending_ack = writer_inbound_ack_state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .pending
                .is_some();
            tokio::select! {
                maybe_message = json_outgoing_rx.recv(), if pending_outbound.is_none() => {
                    let Some(message) = maybe_message else {
                        break;
                    };
                    pending_outbound = Some(match frame_jsonrpc_message(&message) {
                        Ok(framed) => (framed, 0),
                        Err(error) => {
                            warn!("failed to frame Noise virtual stream JSON-RPC payload: {error}");
                            break;
                        }
                    });
                }
                _ = std::future::ready(()), if can_send_pending => {
                    let (ciphertext, next_offset, message_complete) = {
                        let Some((framed, offset)) = pending_outbound.as_ref() else {
                            continue;
                        };
                        let next_offset = (*offset + NOISE_RECORD_PLAINTEXT_LEN).min(framed.len());
                        let ciphertext = {
                            let mut transport = writer_transport
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner);
                            transport.encrypt(&framed[*offset..next_offset])
                        };
                        let ciphertext = match ciphertext {
                            Ok(ciphertext) => ciphertext,
                            Err(error) => {
                                warn!("failed to encrypt Noise virtual stream payload: {error}");
                                break 'writer;
                            }
                        };
                        if ciphertext.len() > MAX_RELIABLE_CIPHERTEXT_BYTES {
                            warn!("Noise virtual stream ciphertext exceeds reliable record budget");
                            break 'writer;
                        }
                        (ciphertext, next_offset, next_offset == framed.len())
                    };
                    let outbound = {
                        let mut reliable_sender = writer_reliable_sender
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        match reliable_sender
                            .admit_ciphertext(ciphertext, tokio::time::Instant::now())
                        {
                            Ok(outbound) => outbound,
                            Err(error) => {
                                warn!("failed to admit Noise reliable ciphertext: {error}");
                                break 'writer;
                            }
                        }
                    };
                    let ack_state = writer_inbound_ack_state
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .latest;
                    let frame = RelayMessageFrame::reliable_data(
                        writer_stream_id.clone(),
                        ack_state,
                        outbound.seq,
                        outbound.payload,
                    );
                    if writer_physical_outgoing_tx
                        .send(encode_relay_message_frame(&frame))
                        .await
                        .is_err()
                    {
                        break 'writer;
                    }
                    if message_complete {
                        pending_outbound = None;
                    } else if let Some((_framed, offset)) = pending_outbound.as_mut() {
                        *offset = next_offset;
                    }
                }
                _ = retry_tick.tick() => {
                    let retry = {
                        let mut reliable_sender = writer_reliable_sender
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        reliable_sender.next_retry_due(tokio::time::Instant::now())
                    };
                    if let Some(outbound) = retry {
                        let ack_state = writer_inbound_ack_state
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .latest;
                        let frame = RelayMessageFrame::reliable_data(
                            writer_stream_id.clone(),
                            ack_state,
                            outbound.seq,
                            outbound.payload,
                        );
                        if writer_physical_outgoing_tx
                            .send(encode_relay_message_frame(&frame))
                            .await
                            .is_err()
                        {
                            break 'writer;
                        }
                    }
                }
                _ = std::future::ready(()), if has_pending_ack => {
                    let ack_state = writer_inbound_ack_state
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .pending
                        .take();
                    let Some(ack_state) = ack_state else {
                        continue;
                    };
                    let frame = RelayMessageFrame::ack(writer_stream_id.clone(), ack_state);
                    if writer_physical_outgoing_tx
                        .send(encode_relay_message_frame(&frame))
                        .await
                        .is_err()
                    {
                        break 'writer;
                    }
                }
                _ = writer_wakeup_task.notified() => {}
            }
        }

        // The reset is best effort; the local close notification is not.
        let closed_stream = ClosedNoiseVirtualStream {
            stream_id: writer_stream_id.clone(),
            instance_id,
        };
        let reset =
            RelayMessageFrame::reset(writer_stream_id, NOISE_RELAY_RESET_REASON.to_string());
        let _ = writer_physical_outgoing_tx.try_send(encode_relay_message_frame(&reset));
        let _ = closed_stream_tx.send(closed_stream).await;
    });

    let connection = JsonRpcConnection {
        outgoing_tx: json_outgoing_tx,
        incoming_rx,
        disconnected_rx,
        task_handles: vec![writer_task],
        transport: JsonRpcTransport::Plain,
    };
    tokio::spawn(async move {
        processor
            .run_connection(connection, ConnectionTransport::Relay)
            .await;
        let _ = processor_closed_stream_tx
            .send(ClosedNoiseVirtualStream {
                stream_id: processor_stream_id,
                instance_id,
            })
            .await;
    });

    NoiseVirtualStream {
        incoming_tx,
        disconnected_tx,
        transport,
        reliable_sender,
        inbound_ack_state,
        writer_wakeup,
        inbound_ciphertexts: OrderedCiphertextFrames::default(),
        inbound_decoder: JsonRpcMessageDecoder::default(),
        instance_id,
    }
}

#[cfg(test)]
#[path = "executor_stream_tests.rs"]
mod tests;
