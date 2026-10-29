use std::future::Future;
use std::sync::Arc;

use bytes::Bytes;
use codex_exec_server_protocol::JSONRPCMessage;
use codex_protocol::protocol::W3cTraceContext;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::ExecServerError;
use crate::connection::JsonRpcConnection;
use crate::connection::JsonRpcConnectionEvent;
use crate::connection::JsonRpcTransport;
use crate::noise_relay::message_framing::frame_jsonrpc_message;
use crate::server::ConnectionProcessor;
use crate::telemetry::ExecutorRegistration;

pub(crate) struct NoiseStreamConnection<I, O> {
    pub(crate) outgoing_tx: mpsc::Sender<O>,
    pub(crate) incoming_rx: mpsc::Receiver<I>,
    pub(crate) disconnected_rx: watch::Receiver<bool>,
    pub(crate) writer_task: JoinHandle<()>,
    pub(crate) executor_registration: Option<Arc<ExecutorRegistration>>,
}

pub(crate) struct NoiseOutboundMessage {
    /// Payload with the authenticated length prefix supplied by message_framing.
    pub(crate) framed: Vec<u8>,
    pub(crate) trace: Option<W3cTraceContext>,
}

/// Adapts complete Noise payloads to one owner without adding an inbound queue.
/// Decoding is synchronous so execution spans begin before queue admission.
pub(crate) trait NoiseStreamHandler: Clone + Send + 'static {
    type Incoming: Send + 'static;
    type Outgoing: Send + 'static;

    fn decode(payload: Bytes) -> Result<Self::Incoming, ExecServerError>;
    fn encode(message: Self::Outgoing) -> Result<NoiseOutboundMessage, ExecServerError>;
    fn run_connection(
        self,
        connection: NoiseStreamConnection<Self::Incoming, Self::Outgoing>,
    ) -> impl Future<Output = ()> + Send;
}

impl NoiseStreamHandler for ConnectionProcessor {
    type Incoming = JsonRpcConnectionEvent;
    type Outgoing = JSONRPCMessage;

    fn decode(payload: Bytes) -> Result<Self::Incoming, ExecServerError> {
        let event = JsonRpcConnectionEvent::message(serde_json::from_slice(&payload)?);
        if let JsonRpcConnectionEvent::QueuedRequest { request_span, .. } = &event {
            // The request span can remain open while queued or executing. Finish a
            // payload-free receipt marker now so a stalled executor is distinguishable
            // from a request that never arrived, without waiting for that span to close.
            drop(tracing::info_span!(
                target: "codex_otel.trace_safe",
                parent: request_span,
                "codex.exec_server.noise_request_received",
            ));
        }
        Ok(event)
    }

    fn encode(message: Self::Outgoing) -> Result<NoiseOutboundMessage, ExecServerError> {
        let framed = frame_jsonrpc_message(&message)?;
        let trace = match message {
            JSONRPCMessage::Request(request) => request.trace,
            JSONRPCMessage::Notification(_)
            | JSONRPCMessage::Response(_)
            | JSONRPCMessage::Error(_) => None,
        };
        Ok(NoiseOutboundMessage { framed, trace })
    }

    async fn run_connection(
        self,
        connection: NoiseStreamConnection<Self::Incoming, Self::Outgoing>,
    ) {
        ConnectionProcessor::run_registered_connection(
            &self,
            JsonRpcConnection {
                outgoing_tx: connection.outgoing_tx,
                incoming_rx: connection.incoming_rx,
                disconnected_rx: connection.disconnected_rx,
                task_handles: vec![connection.writer_task],
                transport: JsonRpcTransport::Plain,
            },
            connection.executor_registration,
        )
        .await;
    }
}

#[cfg(test)]
#[path = "stream_handler_tests.rs"]
mod tests;
