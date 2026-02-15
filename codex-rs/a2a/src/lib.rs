//! # codex-a2a
//!
//! A2A (Agent-to-Agent) RC v1 protocol implementation built from the
//! official specification (`a2a.proto`).
//!
//! ## Architecture (mirrors `a2a-js`)
//!
//! - [`types`] — Wire types matching the A2A proto spec
//! - [`error`] — [`A2AError`] with JSON-RPC error codes
//! - [`store`] — [`TaskStore`] trait + [`InMemoryTaskStore`]
//! - [`event`] — [`ExecutionEvent`] enum + [`EventBus`] for streaming
//! - [`executor`] — [`AgentExecutor`] trait (user implements this)
//! - [`server`] — Axum HTTP server with RC v1 routes

pub mod error;
pub mod event;
pub mod executor;
pub mod server;
pub mod store;
pub mod types;

// Re-export commonly used items at crate root.
pub use error::A2AError;
pub use event::{EventBus, ExecutionEvent};
pub use executor::{AgentExecutor, RequestContext};
pub use server::{A2AServer, A2AServerState};
pub use store::{InMemoryTaskStore, TaskStore};
pub use types::*;
