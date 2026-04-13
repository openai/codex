//! Reusable local bridge primitives for typed Rust-to-Python calls.
//!
//! The bridge intentionally separates a small MsgPack-encoded control message from large opaque
//! byte frames. Rust callers can keep strongly typed DTOs while Python services can persist or
//! forward opaque fields without deserializing them.

mod client;
mod error;
mod frame;
mod opaque;
mod schema;
mod transport;

pub use client::BridgeClient;
pub use client::BridgeResponse;
pub use error::BridgeError;
pub use error::BridgeResult;
pub use frame::BridgeEnvelope;
pub use frame::BridgeFrame;
pub use frame::BridgeRequest;
pub use frame::OpaqueFrame;
pub use opaque::decode_opaque_msgpack;
pub use opaque::encode_opaque_msgpack;
pub use schema::BridgeField;
pub use schema::BridgeMethod;
pub use schema::BridgeSchema;
pub use schema::BridgeType;
pub use schema::OpaqueField;
pub use transport::BridgeTransport;

#[cfg(unix)]
pub use transport::UnixSocketBridgeTransport;
