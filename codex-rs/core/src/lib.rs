//! Root of the `codex-core` library.

// Prevent accidental direct writes to stdout/stderr in library code. All
// user-visible output must go through the appropriate abstraction (e.g.,
// the TUI or the tracing stack).
#![deny(clippy::print_stdout, clippy::print_stderr)]

mod apply_patch;
mod bash;
mod chat_completions;
mod client;
mod client_common;
pub mod codex;
mod codex_conversation;
pub use codex_conversation::CodexConversation;
pub mod config;
pub mod config_profile;
pub mod config_types;
mod conversation_history;
mod environment_context;
pub mod error;
pub mod exec;
mod exec_command;
pub mod exec_env;
mod flags;
pub mod git_info;
mod is_safe_command;
pub mod landlock;
mod mcp_connection_manager;
mod mcp_tool_call;
mod message_history;
mod model_provider_info;
pub mod parse_command;
pub use model_provider_info::BUILT_IN_OSS_MODEL_PROVIDER_ID;
pub use model_provider_info::ModelProviderInfo;
pub use model_provider_info::WireApi;
pub use model_provider_info::built_in_model_providers;
pub use model_provider_info::create_oss_provider_with_base_url;
mod conversation_manager;
pub use conversation_manager::ConversationManager;
pub use conversation_manager::NewConversation;
pub mod model_family;
mod openai_model_info;
mod openai_tools;
pub mod plan_tool;
pub mod project_doc;
mod rollout;
pub(crate) mod safety;
pub mod seatbelt;
pub mod shell;
pub mod spawn;
pub mod terminal;
mod tool_apply_patch;
pub mod turn_diff_tracker;
pub mod user_agent;
mod user_notification;
pub mod util;
pub use apply_patch::CODEX_APPLY_PATCH_ARG1;
pub use safety::get_platform_sandbox;
// Re-export the protocol types from the standalone `codex-protocol` crate so existing
// `codex_core::protocol::...` references continue to work across the workspace.
pub use codex_protocol::protocol;
// Re-export protocol config enums to ensure call sites can use the same types
// as those in the protocol crate when constructing protocol messages.
pub use codex_protocol::config_types as protocol_config_types;
