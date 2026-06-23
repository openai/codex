use std::error::Error;
use std::fmt;
use std::marker::PhantomData;

use codex_code_mode_protocol::host::ClientToHost;
use codex_code_mode_protocol::host::HostToClient;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

/// The client side of a deterministic in-memory host connection.
pub type ClientEndpoint = Endpoint<ClientToHost, HostToClient>;

/// The host side of a deterministic in-memory host connection.
pub type HostEndpoint = Endpoint<HostToClient, ClientToHost>;

/// A typed endpoint whose outbound and inbound frames are controlled separately.
pub struct Endpoint<Outbound, Inbound> {
    outbound: mpsc::UnboundedSender<Vec<u8>>,
    inbound: mpsc::UnboundedReceiver<Delivery>,
    received: mpsc::UnboundedSender<ReceivedFrameData>,
    message_types: PhantomData<fn(Outbound) -> Inbound>,
}

impl<Outbound, Inbound> Endpoint<Outbound, Inbound>
where
    Outbound: Serialize,
    Inbound: DeserializeOwned,
{
    /// Queues one encoded frame for the controller to inspect and deliver.
    pub fn send(&self, message: &Outbound) -> Result<(), TransportError> {
        let frame = serde_json::to_vec(message).map_err(TransportError::Encode)?;
        self.outbound
            .send(frame)
            .map_err(|_| TransportError::WriteClosed)
    }

    /// Receives one controller-released frame, or `None` after orderly closure.
    pub async fn receive(&mut self) -> Result<Option<Inbound>, TransportError> {
        let Some(delivery) = self.inbound.recv().await else {
            return Ok(None);
        };
        let frame = match delivery {
            Delivery::Frame(frame) => frame,
            Delivery::ReadFailure(message) => {
                return Err(TransportError::ReadFailed(message));
            }
        };

        let (release, released) = oneshot::channel();
        self.received
            .send(ReceivedFrameData {
                bytes: frame.clone(),
                release,
            })
            .map_err(|_| TransportError::ControllerClosed)?;
        released
            .await
            .map_err(|_| TransportError::ControllerClosed)?;

        serde_json::from_slice(&frame)
            .map(Some)
            .map_err(TransportError::Decode)
    }
}

/// Controls delivery and fault injection for both directions of a connection.
pub struct ConnectionController {
    pub client_to_host: DirectionController,
    pub host_to_client: DirectionController,
}

/// Controls one ordered direction of an in-memory connection.
pub struct DirectionController {
    pending: mpsc::UnboundedReceiver<Vec<u8>>,
    delivery: Option<mpsc::UnboundedSender<Delivery>>,
    received: mpsc::UnboundedReceiver<ReceivedFrameData>,
}

impl DirectionController {
    /// Waits until the sender has queued its next frame.
    pub async fn next_pending_frame(&mut self) -> Option<PendingFrame> {
        self.pending.recv().await.map(PendingFrame)
    }

    /// Delivers a previously observed frame to the receiving endpoint.
    pub fn deliver(&self, frame: PendingFrame) -> Result<(), ControlError> {
        self.deliver_bytes(frame.0)
    }

    /// Injects arbitrary bytes as one complete in-memory frame.
    pub fn deliver_bytes(&self, bytes: Vec<u8>) -> Result<(), ControlError> {
        self.delivery
            .as_ref()
            .ok_or(ControlError::DirectionClosed)?
            .send(Delivery::Frame(bytes))
            .map_err(|_| ControlError::DirectionClosed)
    }

    /// Waits until the receiver has dequeued a frame, then holds its decode step.
    pub async fn next_received_frame(&mut self) -> Option<ReceivedFrame> {
        self.received.recv().await.map(|frame| ReceivedFrame {
            bytes: frame.bytes,
            release: frame.release,
        })
    }

    /// Makes the receiving endpoint's next read fail with the supplied message.
    pub fn inject_read_failure(&self, message: impl Into<String>) -> Result<(), ControlError> {
        self.delivery
            .as_ref()
            .ok_or(ControlError::DirectionClosed)?
            .send(Delivery::ReadFailure(message.into()))
            .map_err(|_| ControlError::DirectionClosed)
    }

    /// Rejects new sends while preserving frames that were already queued.
    pub fn fail_writes(&mut self) {
        self.pending.close();
    }

    /// Closes this direction so new sends fail and the receiver observes EOF.
    pub fn close(&mut self) {
        self.pending.close();
        self.delivery = None;
    }
}

/// A frame queued by an endpoint but not yet delivered by the controller.
pub struct PendingFrame(Vec<u8>);

impl PendingFrame {
    /// Returns the encoded bytes without consuming the pending frame.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Consumes the pending frame and returns its encoded bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }
}

/// A delivered frame paused after receiver dequeue and before JSON decoding.
pub struct ReceivedFrame {
    bytes: Vec<u8>,
    release: oneshot::Sender<()>,
}

impl ReceivedFrame {
    /// Returns the delivered bytes without releasing the receiver.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Allows the receiving endpoint to decode and return the frame.
    pub fn release(self) -> Result<(), ControlError> {
        self.release
            .send(())
            .map_err(|_| ControlError::ReceiverClosed)
    }
}

struct ReceivedFrameData {
    bytes: Vec<u8>,
    release: oneshot::Sender<()>,
}

enum Delivery {
    Frame(Vec<u8>),
    ReadFailure(String),
}

/// Endpoint failures surfaced by the deterministic transport.
#[derive(Debug)]
pub enum TransportError {
    Encode(serde_json::Error),
    Decode(serde_json::Error),
    WriteClosed,
    ReadFailed(String),
    ControllerClosed,
}

impl fmt::Display for TransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encode(error) => write!(formatter, "failed to encode frame: {error}"),
            Self::Decode(error) => write!(formatter, "failed to decode frame: {error}"),
            Self::WriteClosed => formatter.write_str("transport write direction is closed"),
            Self::ReadFailed(message) => write!(formatter, "transport read failed: {message}"),
            Self::ControllerClosed => formatter.write_str("transport controller is closed"),
        }
    }
}

impl Error for TransportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Encode(error) | Self::Decode(error) => Some(error),
            Self::WriteClosed | Self::ReadFailed(_) | Self::ControllerClosed => None,
        }
    }
}

/// Controller failures caused by a closed direction or receiver.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlError {
    DirectionClosed,
    ReceiverClosed,
}

impl fmt::Display for ControlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DirectionClosed => formatter.write_str("transport direction is closed"),
            Self::ReceiverClosed => formatter.write_str("transport receiver is closed"),
        }
    }
}

impl Error for ControlError {}

/// Creates a typed connection and a controller that gates every frame.
pub fn in_memory_connection() -> (ClientEndpoint, HostEndpoint, ConnectionController) {
    let (client_outbound, client_pending) = mpsc::unbounded_channel();
    let (host_delivery, host_inbound) = mpsc::unbounded_channel();
    let (host_received, client_to_host_received) = mpsc::unbounded_channel();

    let (host_outbound, host_pending) = mpsc::unbounded_channel();
    let (client_delivery, client_inbound) = mpsc::unbounded_channel();
    let (client_received, host_to_client_received) = mpsc::unbounded_channel();

    let client = Endpoint {
        outbound: client_outbound,
        inbound: client_inbound,
        received: client_received,
        message_types: PhantomData,
    };
    let host = Endpoint {
        outbound: host_outbound,
        inbound: host_inbound,
        received: host_received,
        message_types: PhantomData,
    };
    let controller = ConnectionController {
        client_to_host: DirectionController {
            pending: client_pending,
            delivery: Some(host_delivery),
            received: client_to_host_received,
        },
        host_to_client: DirectionController {
            pending: host_pending,
            delivery: Some(client_delivery),
            received: host_to_client_received,
        },
    };

    (client, host, controller)
}

#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;
