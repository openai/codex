//! MCP access to Codex memories.
//!
//! This crate only exposes tools for discovering and reading memory files. The
//! policy that tells a model when to use those tools is injected elsewhere.

pub mod backend;
pub mod local;

mod schema;
mod server;

pub use local::LocalMemoriesBackend;
pub use server::MemoriesMcpServer;
pub use server::run_stdio_server;
