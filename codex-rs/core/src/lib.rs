//! Root of the `codex-core` library.

// Prevent accidental direct writes to stdout/stderr in library code. All
// user-visible output must go through the appropriate abstraction (e.g.,
// the TUI or the tracing stack).
#![deny(clippy::print_stdout, clippy::print_stderr)]

mod apply_patch;
pub mod auth;
pub mod bash;
mod chat_completions;
mod client;
mod client_common;
pub mod codex;
mod codex_conversation;
pub use codex_conversation::CodexConversation;
mod codex_delegate;
mod command_safety;
pub mod config;
pub mod config_loader;
mod context_manager;
pub mod custom_prompts;
mod environment_context;
pub mod error;
pub mod exec;
pub mod exec_env;
pub mod features;
mod flags;
pub mod git_info;
pub mod landlock;
pub mod mcp;
mod mcp_connection_manager;
mod mcp_tool_call;
mod message_history;
mod model_provider_info;
pub mod parse_command;
mod response_processing;
pub mod sandboxing;
pub mod token_data;
mod truncate;
mod unified_exec;
mod user_instructions;
pub use model_provider_info::{
    BUILT_IN_OSS_MODEL_PROVIDER_ID, ModelProviderInfo, WireApi, built_in_model_providers,
    create_oss_provider_with_base_url,
};
mod conversation_manager;
mod event_mapping;
pub mod review_format;
// Re-export common auth types for workspace consumers
pub use auth::AuthManager;
pub use auth::CodexAuth;
pub use codex_protocol::protocol::InitialHistory;
pub use conversation_manager::{ConversationManager, NewConversation};
pub mod default_client;
pub mod model_family;
mod openai_model_info;
pub mod project_doc;
mod rollout;
pub(crate) mod safety;
pub mod seatbelt;
pub mod shell;
pub mod spawn;
pub mod terminal;
mod tools;
pub mod turn_diff_tracker;
pub use rollout::list::{
    ConversationItem, ConversationsPage, Cursor, parse_cursor, read_head_for_summary,
};
pub use rollout::{
    ARCHIVED_SESSIONS_SUBDIR, INTERACTIVE_SESSION_SOURCES, RolloutRecorder, SESSIONS_SUBDIR,
    SessionMeta, find_conversation_path_by_id_str,
};
mod function_tool;
mod state;
mod tasks;
mod user_notification;
pub mod util;

pub use apply_patch::CODEX_APPLY_PATCH_ARG1;
pub use client::ModelClient;
pub use client_common::{Prompt, REVIEW_PROMPT, ResponseEvent, ResponseStream};
pub use codex::compact::content_items_to_text;
// Re-export protocol config enums to ensure call sites can use the same types
// as those in the protocol crate when constructing protocol messages.
pub use codex_protocol::config_types as protocol_config_types;
pub use codex_protocol::models::{
    ContentItem, LocalShellAction, LocalShellExecAction, LocalShellStatus, ResponseItem,
};
// Re-export the protocol types from the standalone `codex-protocol` crate so existing
// `codex_core::protocol::...` references continue to work across the workspace.
pub use codex_protocol::protocol;
pub use command_safety::is_safe_command;
pub use event_mapping::parse_turn_item;
pub use safety::{get_platform_sandbox, set_windows_sandbox_enabled};
pub mod otel_init;
