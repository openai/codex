//! Storage-neutral parent/child topology for thread-spawned agents.

mod error;
mod local;
mod store;
mod types;

pub use error::AgentGraphStoreError;
pub use error::AgentGraphStoreResult;
pub use local::LocalAgentGraphStore;
pub use store::AgentGraphStore;
pub use types::ThreadSpawnEdgeStatus;
