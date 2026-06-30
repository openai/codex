//! One executor-side virtual stream after the Noise handshake.
//!
//! The environment loop owns reads and a per-stream task owns writes. They share
//! `NoiseTransport` because its send and receive nonces live in the same value;
//! the mutex is never held across `.await`.

use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

use codex_exec_server_protocol::JSONRPCMessage;
use codex_protocol::protocol::W3cTraceContext;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tracing::Instrument;
use tracing::Span;
use tracing::field;
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
use crate::noise_relay::trace_context::NoiseTraceContext;
use crate::relay::encode_relay_message_frame;
use crate::relay_proto::RelayData;
use crate::relay_proto::RelayMessageFrame;
use crate::server::ConnectionProcessor;
use crate::telemetry::ConnectionTransport;

/// Identifies one completed virtual-stream instance.
///
/// Stream IDs are supplied by the untrusted relay peer and may be reused. The
/// instance ID prevents a delayed writer notification from removing a newer
/// stream that happens to use the same routing ID.
pub(crate) struct ClosedNoiseVirtualStream {
    pub(crate) stream_id: String,
    pub(crate) instance_id: u64,
}

/// One frame queued for the executor's shared physical rendezvous websocket.
///
/// Data frames carry the current trace context across the channel so the
/// physical websocket send remains attributable to the originating RPC or
/// process notification. Relay control frames intentionally carry no context.
pub(crate) struct NoisePhysicalFrame {
    pub(crate) encoded: Vec<u8>,
    pub(crate) queued_at: Instant,
    pub(crate) trace: Option<Arc<W3cTraceContext>>,
}

impl NoisePhysicalFrame {
    pub(crate) fn data(encoded: Vec<u8>, trace: Option<Arc<W3cTraceContext>>) -> Self {
        Self {
            encoded,
            queued_at: Instant::now(),
            trace,
        }
    }

    pub(crate) fn control(encoded: Vec<u8>) -> Self {
        Self {
            encoded,
            queued_at: Instant::now(),
            trace: None,
        }
    }
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
    inbound_ciphertexts: OrderedCiphertextFrames,
    inbound_decoder: JsonRpcMessageDecoder,
    trace_context: Arc<Mutex<NoiseTraceContext>>,
    pub(crate) instance_id: u64,
}

struct ExecutorInboundTimings {
    websocket_ingress_to_decode: Duration,
    relay_dispatch: Duration,
    reorder: Duration,
    decrypt: Duration,
    decode: Duration,
    trace_context: Duration,
}

impl NoiseVirtualStream {
    pub(crate) fn disconnect(self, reason: Option<String>) {
        let _ = self.disconnected_tx.send(true);
        let _ = self
            .incoming_tx
            .try_send(JsonRpcConnectionEvent::Disconnected { reason });
    }

    /// Reorder and decrypt one inbound record, then queue complete JSON-RPC messages.
    /// This must stay nonblocking because all virtual streams share the read loop.
    pub(crate) fn receive_data(
        &mut self,
        data: RelayData,
        physical_received_at: Instant,
    ) -> Result<(), ExecServerError> {
        let relay_seq = data.seq;
        let ciphertext_bytes = data.payload.len();
        let relay_dispatch_elapsed = physical_received_at.elapsed();
        let reorder_started_at = Instant::now();
        let ciphertexts = self.inbound_ciphertexts.push(data.seq, data.payload)?;
        let reorder_elapsed = reorder_started_at.elapsed();
        for ciphertext in ciphertexts {
            let decrypt_started_at = Instant::now();
            let plaintext = {
                let mut transport = self
                    .transport
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                transport.decrypt(&ciphertext).map_err(|error| {
                    ExecServerError::Protocol(format!("Noise relay decryption failed: {error}"))
                })?
            };
            let decrypt_elapsed = decrypt_started_at.elapsed();
            let decode_started_at = Instant::now();
            let messages = self.inbound_decoder.push(&plaintext)?;
            let decode_elapsed = decode_started_at.elapsed();
            for message in messages {
                let trace_context_started_at = Instant::now();
                self.trace_context
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .observe_request(&message);
                let trace_context_elapsed = trace_context_started_at.elapsed();
                let decoded_span = decoded_message_span(
                    &message,
                    relay_seq,
                    ciphertext_bytes,
                    ExecutorInboundTimings {
                        websocket_ingress_to_decode: physical_received_at.elapsed(),
                        relay_dispatch: relay_dispatch_elapsed,
                        reorder: reorder_elapsed,
                        decrypt: decrypt_elapsed,
                        decode: decode_elapsed,
                        trace_context: trace_context_elapsed,
                    },
                );
                let enqueued_span = enqueued_message_span(&message, relay_seq);
                let enqueue_started_at = Instant::now();
                let enqueue_result = decoded_span.in_scope(|| {
                    self.incoming_tx
                        .try_send(JsonRpcConnectionEvent::Message(message))
                        .map_err(|_| {
                            ExecServerError::Protocol(
                                "Noise virtual stream inbound queue is full or closed".to_string(),
                            )
                        })
                });
                enqueued_span.record(
                    "enqueue_ms",
                    enqueue_started_at.elapsed().as_secs_f64() * 1_000.0,
                );
                enqueue_result?;
                enqueued_span.in_scope(|| {});
            }
        }
        Ok(())
    }
}

fn decoded_message_span(
    message: &JSONRPCMessage,
    relay_seq: u32,
    ciphertext_bytes: usize,
    timings: ExecutorInboundTimings,
) -> Span {
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
        "exec_server.noise.executor_message_decoded",
        message_kind,
        method,
        relay_seq,
        ciphertext_bytes,
        executor_websocket_ingress_to_decode_ms =
            timings.websocket_ingress_to_decode.as_secs_f64() * 1_000.0,
        relay_dispatch_ms = timings.relay_dispatch.as_secs_f64() * 1_000.0,
        reorder_ms = timings.reorder.as_secs_f64() * 1_000.0,
        decrypt_ms = timings.decrypt.as_secs_f64() * 1_000.0,
        decode_ms = timings.decode.as_secs_f64() * 1_000.0,
        trace_context_ms = timings.trace_context.as_secs_f64() * 1_000.0,
    );
    if let Some(trace) = trace {
        let _ = codex_otel::set_parent_from_w3c_trace_context(&span, trace);
    }
    span
}

fn enqueued_message_span(message: &JSONRPCMessage, relay_seq: u32) -> Span {
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
        "exec_server.noise.executor_message_enqueued",
        message_kind,
        method,
        relay_seq,
        enqueue_ms = field::Empty,
    );
    if let Some(trace) = trace {
        let _ = codex_otel::set_parent_from_w3c_trace_context(&span, trace);
    }
    span
}

/// Start JSON-RPC processing for a completed handshake.
///
/// The returned value is the read half; the spawned task owns outbound framing
/// and reports its instance ID on exit so stream-ID reuse is safe.
pub(crate) fn spawn_noise_virtual_stream(
    stream_id: String,
    instance_id: u64,
    processor: ConnectionProcessor,
    physical_outgoing_tx: mpsc::Sender<NoisePhysicalFrame>,
    closed_stream_tx: mpsc::Sender<ClosedNoiseVirtualStream>,
    transport: NoiseTransport,
) -> NoiseVirtualStream {
    let (json_outgoing_tx, mut json_outgoing_rx) = mpsc::channel(CHANNEL_CAPACITY);
    let (incoming_tx, incoming_rx) = mpsc::channel(CHANNEL_CAPACITY);
    let (disconnected_tx, disconnected_rx) = watch::channel(false);
    let transport = Arc::new(Mutex::new(transport));
    let writer_transport = Arc::clone(&transport);
    let trace_context = Arc::new(Mutex::new(NoiseTraceContext::default()));
    let writer_trace_context = Arc::clone(&trace_context);
    let processor_stream_id = stream_id.clone();
    let processor_closed_stream_tx = closed_stream_tx.clone();
    let writer_stream_id = stream_id;
    let writer_task = tokio::spawn(async move {
        let mut next_seq = 0u32;
        'writer: while let Some(message) = json_outgoing_rx.recv().await {
            let outbound_span = outbound_message_span(&message, &writer_trace_context);
            if let Err(error) = send_outbound_message(
                &physical_outgoing_tx,
                &writer_transport,
                &writer_stream_id,
                &mut next_seq,
                &message,
            )
            .instrument(outbound_span)
            .await
            {
                warn!("failed to send Noise virtual stream JSON-RPC payload: {error}");
                break 'writer;
            }
        }

        // The reset is best effort; the local close notification is not.
        let closed_stream = ClosedNoiseVirtualStream {
            stream_id: writer_stream_id.clone(),
            instance_id,
        };
        let reset =
            RelayMessageFrame::reset(writer_stream_id, NOISE_RELAY_RESET_REASON.to_string());
        let _ = physical_outgoing_tx.try_send(NoisePhysicalFrame::control(
            encode_relay_message_frame(&reset),
        ));
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
        inbound_ciphertexts: OrderedCiphertextFrames::default(),
        inbound_decoder: JsonRpcMessageDecoder::default(),
        trace_context,
        instance_id,
    }
}

fn outbound_message_span(
    message: &JSONRPCMessage,
    trace_context: &Mutex<NoiseTraceContext>,
) -> Span {
    let (message_kind, method) = match message {
        JSONRPCMessage::Request(request) => ("request", request.method.as_str()),
        JSONRPCMessage::Notification(notification) => {
            ("notification", notification.method.as_str())
        }
        JSONRPCMessage::Response(_) => ("response", ""),
        JSONRPCMessage::Error(_) => ("error", ""),
    };
    let trace = trace_context
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .return_trace(message);
    let span = tracing::info_span!(
        "exec_server.noise.executor_outbound",
        message_kind,
        method,
        framed_bytes = field::Empty,
        records = field::Empty,
        frame_complete_ms = field::Empty,
        encrypt_complete_ms = field::Empty,
        physical_queue_complete_ms = field::Empty,
    );
    if let Some(trace) = trace.as_ref() {
        let _ = codex_otel::set_parent_from_w3c_trace_context(&span, trace);
    }
    span
}

async fn send_outbound_message(
    physical_outgoing_tx: &mpsc::Sender<NoisePhysicalFrame>,
    transport: &Mutex<NoiseTransport>,
    stream_id: &str,
    next_seq: &mut u32,
    message: &JSONRPCMessage,
) -> Result<(), ExecServerError> {
    let started_at = Instant::now();
    let framed = tracing::info_span!("exec_server.noise.executor_frame_message")
        .in_scope(|| frame_jsonrpc_message(message))?;
    let record_count = framed.len().div_ceil(NOISE_RECORD_PLAINTEXT_LEN);
    let span = Span::current();
    span.record("framed_bytes", framed.len());
    span.record("records", record_count);
    span.record(
        "frame_complete_ms",
        started_at.elapsed().as_secs_f64() * 1_000.0,
    );
    // Extracting W3C context allocates its string carrier. Do that once for the
    // logical JSON-RPC message, then share it across all physical Noise records.
    let trace = codex_otel::current_span_w3c_trace_context().map(Arc::new);

    for (record_index, plaintext_record) in framed.chunks(NOISE_RECORD_PLAINTEXT_LEN).enumerate() {
        let seq = take_next_sequence(next_seq)?;
        let ciphertext =
            tracing::info_span!("exec_server.noise.executor_encrypt_record", record_index,)
                .in_scope(|| {
                    transport
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .encrypt(plaintext_record)
                })
                .map_err(|error| {
                    ExecServerError::Protocol(format!(
                        "failed to encrypt Noise virtual stream payload: {error}"
                    ))
                })?;
        span.record(
            "encrypt_complete_ms",
            started_at.elapsed().as_secs_f64() * 1_000.0,
        );
        let frame = RelayMessageFrame::data(stream_id.to_string(), seq, ciphertext);
        let encoded = encode_relay_message_frame(&frame);
        let permit = physical_outgoing_tx
            .reserve()
            .instrument(tracing::info_span!(
                "exec_server.noise.executor_physical_queue",
                record_index,
            ))
            .await
            .map_err(|_| ExecServerError::Closed)?;
        permit.send(NoisePhysicalFrame::data(encoded, trace.clone()));
        span.record(
            "physical_queue_complete_ms",
            started_at.elapsed().as_secs_f64() * 1_000.0,
        );
    }
    Ok(())
}

#[cfg(test)]
#[path = "executor_stream_tests.rs"]
mod tests;
