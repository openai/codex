//! Root of the `codex-core` library.

// Prevent accidental direct writes to stdout/stderr in library code. All
// user-visible output must go through the appropriate abstraction (e.g.,
// the TUI or the tracing stack).
#![deny(clippy::print_stdout, clippy::print_stderr)]

pub mod accounts;
mod analytics_client;
pub mod api_bridge;
mod apply_patch;
pub mod auth;
pub mod bash;
mod client;
mod client_common;
pub mod codex;
mod codex_thread;
mod compact_remote;
pub use codex_thread::CodexThread;
pub use codex_thread::ThreadConfigSnapshot;
mod agent;
mod codex_delegate;
mod command_safety;
pub mod config;
pub mod config_loader;
pub mod connectors;
mod context_manager;
pub mod custom_prompts;
pub mod env;
mod environment_context;
pub mod error;
pub mod exec;
pub mod exec_env;
mod exec_policy;
pub mod features;
mod flags;
pub mod git_info;
pub mod instructions;
pub mod landlock;
pub mod mcp;
mod mcp_connection_manager;
pub mod models_manager;
mod transport_manager;
pub use mcp_connection_manager::MCP_SANDBOX_STATE_CAPABILITY;
pub use mcp_connection_manager::MCP_SANDBOX_STATE_METHOD;
pub use mcp_connection_manager::SandboxState;
mod mcp_tool_call;
mod mentions;
mod message_history;
mod model_provider_info;
pub mod parse_command;
pub mod path_utils;
pub mod personality_migration;
pub mod powershell;
mod proposed_plan_parser;
pub mod sandboxing;
mod session_prefix;
mod stream_events_utils;
mod tagged_block_parser;
mod text_encoding;
pub mod token_data;
mod truncate;
mod unified_exec;
pub mod windows_sandbox;
pub use model_provider_info::CHAT_WIRE_API_DEPRECATION_SUMMARY;
pub use model_provider_info::DEFAULT_LMSTUDIO_PORT;
pub use model_provider_info::DEFAULT_OLLAMA_PORT;
pub use model_provider_info::LMSTUDIO_OSS_PROVIDER_ID;
pub use model_provider_info::ModelProviderInfo;
pub use model_provider_info::OLLAMA_CHAT_PROVIDER_ID;
pub use model_provider_info::OLLAMA_OSS_PROVIDER_ID;
pub use model_provider_info::WireApi;
pub use model_provider_info::built_in_model_providers;
pub use model_provider_info::create_oss_provider_with_base_url;
mod event_mapping;
pub mod review_format;
pub mod review_prompts;
mod thread_manager;
pub mod web_search;
pub use codex_protocol::protocol::InitialHistory;
pub use thread_manager::NewThread;
pub use thread_manager::ThreadManager;
#[deprecated(note = "use ThreadManager")]
pub type ConversationManager = ThreadManager;
#[deprecated(note = "use NewThread")]
pub type NewConversation = NewThread;
#[deprecated(note = "use CodexThread")]
pub type CodexConversation = CodexThread;
// Re-export common auth types for workspace consumers
pub use auth::AuthManager;
pub use auth::CodexAuth;
pub mod default_client;
pub mod project_doc;
mod rollout;
pub(crate) mod safety;
pub mod seatbelt;
pub mod shell;
pub mod shell_snapshot;
pub mod skills;
pub mod spawn;
pub mod state_db;
pub mod terminal;
mod tools;
pub mod turn_diff_tracker;
pub use rollout::ARCHIVED_SESSIONS_SUBDIR;
pub use rollout::INTERACTIVE_SESSION_SOURCES;
pub use rollout::RolloutRecorder;
pub use rollout::RolloutRecorderParams;
pub use rollout::SESSIONS_SUBDIR;
pub use rollout::SessionMeta;
pub use rollout::find_archived_thread_path_by_id_str;
#[deprecated(note = "use find_thread_path_by_id_str")]
pub use rollout::find_conversation_path_by_id_str;
pub use rollout::find_thread_path_by_id_str;
pub use rollout::find_thread_path_by_name_str;
pub use rollout::list::Cursor;
pub use rollout::list::ThreadItem;
pub use rollout::list::ThreadSortKey;
pub use rollout::list::ThreadsPage;
pub use rollout::list::parse_cursor;
pub use rollout::list::read_head_for_summary;
pub use rollout::list::read_session_meta_line;
pub use rollout::rollout_date_parts;
pub use transport_manager::TransportManager;
mod function_tool;
mod state;
mod tasks;
mod user_notification;
mod user_shell_command;
pub mod util;

pub use apply_patch::CODEX_APPLY_PATCH_ARG1;
pub use client::WEB_SEARCH_ELIGIBLE_HEADER;
pub use command_safety::is_dangerous_command;
pub use command_safety::is_safe_command;
pub use exec_policy::ExecPolicyError;
pub use exec_policy::check_execpolicy_for_warnings;
pub use exec_policy::load_exec_policy;
pub use safety::get_platform_sandbox;
pub use tools::spec::parse_tool_input_schema;
// Re-export the protocol types from the standalone `codex-protocol` crate so existing
// `codex_core::protocol::...` references continue to work across the workspace.
pub use codex_protocol::protocol;
// Re-export protocol config enums to ensure call sites can use the same types
// as those in the protocol crate when constructing protocol messages.
pub use codex_protocol::config_types as protocol_config_types;

pub use client::ModelClient;
pub use client::ModelClientSession;
pub use client_common::Prompt;
pub use client_common::REVIEW_PROMPT;
pub use client_common::ResponseEvent;
pub use client_common::ResponseStream;
pub use codex_protocol::models::ContentItem;
pub use codex_protocol::models::LocalShellAction;
pub use codex_protocol::models::LocalShellExecAction;
pub use codex_protocol::models::LocalShellStatus;
pub use codex_protocol::models::ResponseItem;
pub use compact::content_items_to_text;
pub use event_mapping::parse_turn_item;
pub mod compact;
pub mod otel_init;
