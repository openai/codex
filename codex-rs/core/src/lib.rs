//! Root of the `codex-core` library.

// Prevent accidental direct writes to stdout/stderr in library code. All
// user-visible output must go through the appropriate abstraction (e.g.,
// the TUI or the tracing stack).
#![deny(clippy::print_stdout, clippy::print_stderr)]

mod bash;
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
mod flags;
pub mod git_info;
mod is_safe_command;
mod mcp_connection_manager;
mod mcp_tool_call;
mod message_history;
mod model_provider_info;
pub use model_provider_info::ModelProviderInfo;
pub use model_provider_info::WireApi;
mod models;
pub mod openai_api_key;
mod openai_model_info;
mod openai_tools;
mod project_doc;
pub mod protocol;
mod rollout;
mod safety;
pub mod session_manager;
pub use session_manager::SessionsMode;
pub use session_manager::SessionsPage;
pub use session_manager::get_sessions;
pub mod shell;
mod user_notification;
pub mod util;

pub use client_common::model_supports_reasoning_summaries;
