use std::sync::Arc;
use std::sync::Mutex;

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
use crate::noise_relay::take_next_sequence;
use crate::relay::encode_relay_message_frame;
use crate::relay_proto::RelayData;
use crate::relay_proto::RelayMessageFrame;
use crate::server::ConnectionProcessor;

/// Identifies one completed virtual-stream instance.
///
/// Stream IDs are supplied by the untrusted relay peer and may be reused. The
/// instance ID prevents a delayed writer notification from removing a newer
/// stream that happens to use the same routing ID.
pub(super) struct ClosedNoiseVirtualStream {
    pub(super) stream_id: String,
    pub(super) instance_id: u64,
}

/// One authenticated JSON-RPC stream carried by the executor's physical relay.
///
/// Inbound delivery is intentionally nonblocking. An overloaded or abandoned
/// stream fails independently instead of stalling every stream multiplexed over
/// the same physical websocket.
pub(super) struct NoiseVirtualStream {
    incoming_tx: mpsc::Sender<JsonRpcConnectionEvent>,
    disconnected_tx: watch::Sender<bool>,
    transport: Arc<Mutex<NoiseTransport>>,
    inbound_ciphertexts: OrderedCiphertextFrames,
    inbound_decoder: JsonRpcMessageDecoder,
    instance_id: u64,
}

impl NoiseVirtualStream {
    pub(super) fn disconnect(self, reason: Option<String>) {
        let _ = self.disconnected_tx.send(true);
        let _ = self
            .incoming_tx
            .try_send(JsonRpcConnectionEvent::Disconnected { reason });
    }

    pub(super) fn is_instance(&self, instance_id: u64) -> bool {
        self.instance_id == instance_id
    }

    pub(super) fn receive_data(&mut self, data: RelayData) -> Result<(), ExecServerError> {
        // Relay sequence ordering is enforced before taking the transport lock
        // and decrypting. Each virtual stream owns one ordered Noise nonce
        // space shared by its reader and writer transport halves.
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
        Ok(())
    }
}

pub(super) fn spawn_noise_virtual_stream(
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
                    warn!("failed to frame Noise virtual stream JSON-RPC payload: {error}");
                    break;
                }
            };
            for plaintext_record in framed.chunks(NOISE_RECORD_PLAINTEXT_LEN) {
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

        // Reset is best effort because an overloaded physical writer must not
        // keep this dead stream alive. The reliable local close notification
        // below lets the shared state machine reap the exact stream instance.
        let closed_stream = ClosedNoiseVirtualStream {
            stream_id: writer_stream_id.clone(),
            instance_id,
        };
        let reset =
            RelayMessageFrame::reset(writer_stream_id, NOISE_RELAY_RESET_REASON.to_string());
        let _ = physical_outgoing_tx
            .try_send(encode_relay_message_frame(&reset));
        let _ = closed_stream_tx.send(closed_stream).await;
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

    NoiseVirtualStream {
        incoming_tx,
        disconnected_tx,
        transport,
        inbound_ciphertexts: OrderedCiphertextFrames::default(),
        inbound_decoder: JsonRpcMessageDecoder::default(),
        instance_id,
    }
}
