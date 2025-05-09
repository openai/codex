//! Root of the `codex-core` library.

// Prevent accidental direct writes to stdout/stderr in library code. All
// user‑visible output must go through the appropriate abstraction (e.g.,
// the TUI or the tracing stack).
#![deny(clippy::print_stdout, clippy::print_stderr)]

mod chat_completions;

mod client;
mod client_common;
pub mod codex;
pub use codex::Codex;
pub mod codex_wrapper;
pub mod config;
mod conversation_history;
pub mod error;
pub mod exec;
mod flags;
mod is_safe_command;
#[cfg(target_os = "linux")]
pub mod linux;
mod mcp_connection_manager;
pub mod mcp_server_config;
mod mcp_tool_call;
mod model_provider_info;
pub use model_provider_info::ModelProviderInfo;
pub use model_provider_info::WireApi;
mod models;
pub mod protocol;
mod rollout;
mod safety;
mod user_notification;
pub mod util;
