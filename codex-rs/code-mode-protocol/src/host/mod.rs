//! Transport-neutral messages for the callback-only code-mode host boundary.
//!
//! Protocol version 1 relies on ordered framing and connection-scoped
//! fail-stop behavior rather than message sequence numbers. It defines no
//! optional capabilities yet; capability names provide an extension point for
//! later versions without weakening the v1 decoder.

mod error;
mod message;
mod types;

pub use error::HandshakeRejectReason;
pub use message::ClientHello;
pub use message::ClientHelloError;
pub use message::ClientToHost;
pub use message::HostHello;
pub use message::HostToClient;
pub use types::Capability;
pub use types::CapabilitySet;
pub use types::DuplicateCapability;
pub use types::InvalidIdentifier;
pub use types::InvalidSupportedProtocolVersions;
pub use types::ProtocolVersion;
pub use types::SessionId;
pub use types::SupportedProtocolVersions;

#[cfg(test)]
#[path = "host_tests.rs"]
mod tests;
