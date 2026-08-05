//! Messages and framing for the code-mode host boundary.
//!
//! Protocol version 1 multiplexes session operations and delegate callbacks by
//! request ID over one ordered connection. WebSocket peers can negotiate a
//! separate bulk connection without changing the existing inner messages.

mod codec;
mod error;
mod message;
mod payload;
mod types;

/// Maximum number of unresolved delegate callbacks allowed per host connection.
pub const MAX_PENDING_DELEGATE_CALLS: usize = 1_024;

/// Optional second WebSocket carrying delegate callbacks and their responses.
pub const DUAL_WEBSOCKET_CAPABILITY: &str = "dual-websocket-v1";

/// Negotiated support for cell execution resource limits on `session/open`.
pub const SESSION_RESOURCE_LIMITS_CAPABILITY: &str = "session-cell-execution-resource-limits";

/// Selects one socket of a negotiated dual-WebSocket connection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransportLane {
    Control,
    Bulk,
}

pub use codec::EncodedFrame;
pub use codec::FramedReader;
pub use codec::FramedWriter;
pub use codec::MAX_FRAME_BYTES;
pub use error::HandshakeRejectReason;
pub use message::ClientHello;
pub use message::ClientHelloError;
pub use message::ClientToHost;
pub use message::DelegateRequest;
pub use message::DelegateResponse;
pub use message::HostHello;
pub use message::HostRequest;
pub use message::HostResponse;
pub use message::HostToClient;
pub use message::WireResult;
pub use payload::WireCellId;
pub use payload::WireContentItem;
pub use payload::WireExecuteRequest;
pub use payload::WireImageDetail;
pub use payload::WireNestedToolCall;
pub use payload::WireRuntimeResponse;
pub use payload::WireSessionCellExecutionLimits;
pub use payload::WireToolDefinition;
pub use payload::WireToolKind;
pub use payload::WireToolName;
pub use payload::WireWaitOutcome;
pub use payload::WireWaitRequest;
pub use types::Capability;
pub use types::CapabilitySet;
pub use types::DelegateRequestId;
pub use types::DuplicateCapability;
pub use types::InvalidIdentifier;
pub use types::InvalidSupportedProtocolVersions;
pub use types::ProtocolVersion;
pub use types::RequestId;
pub use types::SessionId;
pub use types::SupportedProtocolVersions;

#[cfg(test)]
#[path = "host_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "codec_tests.rs"]
mod codec_tests;
