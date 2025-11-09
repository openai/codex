//! Root of the `codex-core` library.

// Prevent accidental direct writes to stdout/stderr in library code. All
// user-visible output must go through the appropriate abstraction (e.g.,
// the TUI or the tracing stack).
#![deny(clippy::print_stdout, clippy::print_stderr)]
// Allow unwrap in core library for backwards compatibility and error handling patterns
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::type_complexity)]
#![allow(clippy::collapsible_match)]
#![allow(clippy::empty_line_after_doc_comments)]

pub mod agent_interpreter;
pub mod agents;
mod apply_patch;
pub mod async_subagent_integration;
pub mod audit_log;
pub mod auth;
pub mod bash;
mod chat_completions;
mod client;
mod client_common;
pub mod codex;
mod codex_conversation;
pub mod execution;
pub mod plan;
pub mod telemetry;
pub mod webhooks;
pub use codex_conversation::CodexConversation;
mod codex_delegate;
mod command_safety;
pub use codex_utils_string::take_bytes_at_char_boundary;
pub use codex_utils_string::take_last_bytes_at_char_boundary;
pub mod config;
pub mod config_loader;
mod conversation_history;
pub mod custom_commands;
pub mod custom_prompts;
mod environment_context;
pub mod error;
pub mod exec;
pub mod exec_env;
pub mod features;
mod flags;
pub mod git;
pub mod git_info;
pub mod hooks;
pub mod integrations;
pub mod landlock;
pub mod lock;
pub mod mcp;
mod mcp_connection_manager;
mod mcp_tool_call;
mod message_history;
mod model_provider_info;
pub mod natural_language_parser;
pub mod orchestration;
pub mod parse_command;
mod response_processing;
pub mod sandboxing;
pub mod token_data;
mod truncate;
mod unified_exec;
mod user_instructions;
pub mod context_manager;
pub use model_provider_info::BUILT_IN_OSS_MODEL_PROVIDER_ID;
pub use model_provider_info::ModelProviderInfo;
pub use model_provider_info::WireApi;
pub use model_provider_info::built_in_model_providers;
pub use model_provider_info::create_oss_provider_with_base_url;
mod conversation_manager;
mod event_mapping;
pub mod review_format;
pub use codex_protocol::protocol::InitialHistory;
pub use conversation_manager::ConversationManager;
pub use conversation_manager::NewConversation;
// Re-export common auth types for workspace consumers
pub use auth::AuthManager;
pub use auth::CodexAuth;
pub mod default_client;
pub mod hybrid_acceleration;
pub mod model_family;
mod openai_model_info;
pub mod project_doc;
mod rollout;
pub(crate) mod safety;
pub mod seatbelt;
pub mod shell;
pub mod spawn;
pub mod terminal;
pub mod token_budget;
mod tools;
pub mod turn_diff_tracker;

#[cfg(feature = "windows-ai")]
pub mod windows_ai_integration;

pub use rollout::ARCHIVED_SESSIONS_SUBDIR;
pub use rollout::INTERACTIVE_SESSION_SOURCES;
pub use rollout::RolloutRecorder;
pub use rollout::SESSIONS_SUBDIR;
pub use rollout::SessionMeta;
pub use rollout::find_conversation_path_by_id_str;
pub use rollout::list::ConversationItem;
pub use rollout::list::ConversationsPage;
pub use rollout::list::Cursor;
pub use rollout::list::parse_cursor;
pub use rollout::list::read_head_for_summary;
mod function_tool;
mod state;
mod tasks;
mod user_notification;
pub mod util;

pub use apply_patch::CODEX_APPLY_PATCH_ARG1;
pub use command_safety::is_safe_command;
pub use safety::get_platform_sandbox;
pub use safety::set_windows_sandbox_enabled;
// Re-export the protocol types from the standalone `codex-protocol` crate so existing
// `codex_core::protocol::...` references continue to work across the workspace.
pub use codex_protocol::protocol;
// Re-export protocol config enums to ensure call sites can use the same types
// as those in the protocol crate when constructing protocol messages.
pub use codex_protocol::config_types as protocol_config_types;

pub use client::ModelClient;
pub use client_common::Prompt;
pub use client_common::REVIEW_PROMPT;
pub use client_common::ResponseEvent;
pub use client_common::ResponseStream;
pub use codex::compact::content_items_to_text;
pub use codex_protocol::models::ContentItem;
pub use codex_protocol::models::LocalShellAction;
pub use codex_protocol::models::LocalShellExecAction;
pub use codex_protocol::models::LocalShellStatus;
pub use codex_protocol::models::ResponseItem;
pub use event_mapping::parse_turn_item;
pub mod otel_init;
