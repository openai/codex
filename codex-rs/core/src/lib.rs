//! Root of the `codex-core` library.

// Prevent accidental direct writes to stdout/stderr in library code. All
// user‑visible output must go through the appropriate abstraction (e.g.,
// the TUI or the tracing stack).
#![deny(clippy::print_stdout, clippy::print_stderr)]

mod client;
pub mod codex;
pub mod codex_wrapper;
pub mod config;
pub mod error;
pub mod exec;
mod flags;
mod is_safe_command;
#[cfg(target_os = "linux")]
pub mod linux;
mod mcp_connection_manager;
pub mod mcp_server_config;
mod mcp_tool_call;
mod models;
pub mod protocol;
mod rollout;
mod safety;
mod user_notification;
pub mod util;
mod zdr_transcript;

pub use codex::Codex;
