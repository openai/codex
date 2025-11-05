/// Orchestrator server for multi-instance coordination
///
/// Provides repository-level locking, single-writer queue, and RPC API
/// for coordinating multiple CLI/GUI/agent instances.
pub mod auth;
pub mod rpc;
pub mod server;
pub mod transport;

pub use auth::{AuthHeader, AuthManager, HmacAuthenticator};
pub use rpc::*;
pub use server::{OrchestratorConfig, OrchestratorServer};
pub use transport::{
    Connection, Transport, TransportConfig, TransportInfo, TransportPreference, create_transport,
};
