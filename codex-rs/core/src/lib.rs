//! Root of the `codex-core` library.

// Prevent accidental direct writes to stdout/stderr in library code. All
// user-visible output must go through the appropriate abstraction (e.g.,
// the TUI or the tracing stack).
#![deny(clippy::print_stdout, clippy::print_stderr)]

mod chat_completions;
mod client;
mod client_common;
pub mod codex;
pub use codex::Codex;
pub mod codex_wrapper;
pub mod config;
pub mod config_profile;
pub mod config_types;
mod conversation_history;
pub mod error;
pub mod exec;
pub mod exec_env;
pub mod exec_linux;
mod flags;
mod is_safe_command;
#[cfg(target_os = "linux")]
pub mod landlock;
mod mcp_connection_manager;
mod mcp_tool_call;
mod message_history;
mod model_provider_info;
pub use model_provider_info::ModelProviderInfo;
pub use model_provider_info::WireApi;
mod models;
mod project_doc;
pub mod protocol;
mod rollout;
mod safety;
mod user_notification;
pub mod util;
