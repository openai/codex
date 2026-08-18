//! One executor-side virtual stream after the Noise handshake.
//!
//! The environment loop owns reads and a per-stream task owns writes. They share
//! `NoiseTransport` because its send and receive nonces live in the same value;
//! the mutex is never held across `.await`.

use std::sync::Arc;
use std::sync::Mutex;

use tokio::sync::mpsc;
use tokio::sync::watch;
use tracing::warn;

use crate::ExecServerError;
use crate::connection::CHANNEL_CAPACITY;
use crate::noise_channel::NoiseTransport;
use crate::noise_relay::message_framing::MessageDecoder;
use crate::noise_relay::message_framing::NOISE_RECORD_PLAINTEXT_LEN;
use crate::noise_relay::ordered_ciphertext::OrderedCiphertextFrames;
use crate::noise_relay::stream_handler::NoiseStreamConnection;
use crate::noise_relay::stream_handler::NoiseStreamHandler;
use crate::noise_relay::take_next_sequence;
use crate::relay::encode_relay_message_frame;
use crate::relay_proto::RelayData;
use crate::relay_proto::RelayMessageFrame;

/// Identifies one completed virtual-stream instance.
///
/// Stream IDs are supplied by the untrusted relay peer and may be reused. The
/// instance ID prevents a delayed writer notification from removing a newer
/// stream that happens to use the same routing ID.
pub(crate) struct ClosedNoiseVirtualStream {
    pub(crate) stream_id: String,
    pub(crate) instance_id: u64,
}

/// One authenticated application stream carried by the executor's physical relay.
///
/// Inbound delivery is intentionally nonblocking. An overloaded or abandoned
/// stream fails independently instead of stalling every stream multiplexed over
/// the same physical websocket.
pub(crate) struct NoiseVirtualStream<H: NoiseStreamHandler> {
    incoming_tx: mpsc::Sender<H::Incoming>,
    disconnected_tx: watch::Sender<bool>,
    transport: Arc<Mutex<NoiseTransport>>,
    inbound_ciphertexts: OrderedCiphertextFrames,
    inbound_decoder: MessageDecoder,
    pub(crate) instance_id: u64,
}

impl<H: NoiseStreamHandler> NoiseVirtualStream<H> {
    pub(crate) fn disconnect(self) {
        let _ = self.disconnected_tx.send(true);
    }

    /// Reorder and decrypt one record, then deliver complete payloads to the handler.
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
                    .try_send(H::decode(message)?)
                    .map_err(|_| {
                        ExecServerError::Protocol(
                            "Noise virtual stream inbound queue is full or closed".to_string(),
                        )
                    })?;
            }
        }
        Ok(())
    }
}

/// Hand a completed handshake to its execution or forwarding owner.
///
/// The returned value is the read half; the spawned task owns outbound framing
/// and reports its instance ID on exit so stream-ID reuse is safe.
pub(crate) fn spawn_noise_virtual_stream<H: NoiseStreamHandler>(
    stream_id: String,
    instance_id: u64,
    handler: H,
    physical_outgoing_tx: mpsc::Sender<Vec<u8>>,
    closed_stream_tx: mpsc::Sender<ClosedNoiseVirtualStream>,
    transport: NoiseTransport,
) -> NoiseVirtualStream<H> {
    let (outgoing_tx, mut outgoing_rx) = mpsc::channel(CHANNEL_CAPACITY);
    let (incoming_tx, incoming_rx) = mpsc::channel(CHANNEL_CAPACITY);
    let (disconnected_tx, disconnected_rx) = watch::channel(false);
    let transport = Arc::new(Mutex::new(transport));
    let writer_transport = Arc::clone(&transport);
    let owner_stream_id = stream_id.clone();
    let owner_closed_stream_tx = closed_stream_tx.clone();
    let writer_stream_id = stream_id;
    let writer_task = tokio::spawn(async move {
        let mut next_seq = 0u32;
        'writer: while let Some(message) = outgoing_rx.recv().await {
            let message = match H::encode(message) {
                Ok(message) => message,
                Err(error) => {
                    warn!("failed to encode Noise virtual stream payload: {error}");
                    break;
                }
            };
            // Each chunk becomes one Noise record and consumes one nonce.
            let mut trace = message.trace;
            for plaintext_record in message.framed.chunks(NOISE_RECORD_PLAINTEXT_LEN) {
                let seq = match take_next_sequence(&mut next_seq) {
                    Ok(seq) => seq,
                    Err(error) => {
                        warn!("Noise virtual stream sequence exhausted: {error}");
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
                        warn!("failed to encrypt Noise virtual stream payload: {error}");
                        break 'writer;
                    }
                };
                let frame = RelayMessageFrame::data(
                    writer_stream_id.clone(),
                    seq,
                    ciphertext,
                    trace.take(),
                );
                if physical_outgoing_tx
                    .send(encode_relay_message_frame(&frame))
                    .await
                    .is_err()
                {
                    break 'writer;
                }
            }
        }

        // The physical relay owns reset delivery and rejects stale instance IDs.
        let closed_stream = ClosedNoiseVirtualStream {
            stream_id: writer_stream_id,
            instance_id,
        };
        let _ = closed_stream_tx.send(closed_stream).await;
    });

    let connection = NoiseStreamConnection {
        outgoing_tx,
        incoming_rx,
        disconnected_rx,
        writer_task,
    };
    tokio::spawn(async move {
        handler.run_connection(connection).await;
        let _ = owner_closed_stream_tx
            .send(ClosedNoiseVirtualStream {
                stream_id: owner_stream_id,
                instance_id,
            })
            .await;
    });

    NoiseVirtualStream {
        incoming_tx,
        disconnected_tx,
        transport,
        inbound_ciphertexts: OrderedCiphertextFrames::default(),
        inbound_decoder: MessageDecoder::default(),
        instance_id,
    }
}

#[cfg(test)]
#[path = "executor_stream_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "forward_stream_tests.rs"]
mod forward_tests;
