//! Root of the `codex-core` library.

// Prevent accidental direct writes to stdout/stderr in library code. All
// user-visible output must go through the appropriate abstraction (e.g.,
// the TUI or the tracing stack).
#![deny(clippy::print_stdout, clippy::print_stderr)]

pub mod api_bridge;
mod apply_patch;
mod apps;
mod arc_monitor;
mod async_fs;
mod async_runtime;
pub use codex_login as auth;
mod auth_env_telemetry;
mod client;
mod client_common;
pub mod codex;
#[cfg_attr(target_arch = "wasm32", path = "realtime_context_wasm.rs")]
mod realtime_context;
#[cfg_attr(target_arch = "wasm32", path = "realtime_conversation_wasm.rs")]
mod realtime_conversation;
pub use codex::SteerInputError;
mod codex_thread;
mod compact_remote;
pub use codex_thread::CodexThread;
pub use codex_thread::ThreadConfigSnapshot;
mod agent;
mod codex_delegate;
mod command_canonicalization;
mod commit_attribution;
pub mod config;
#[cfg_attr(target_arch = "wasm32", path = "config_loader_wasm.rs")]
pub mod config_loader;
pub mod connectors;
mod context_manager;
mod contextual_user_message;
pub use codex_utils_path::env;
mod environment_context;
pub mod error;
#[cfg_attr(target_arch = "wasm32", path = "exec_wasm.rs")]
pub mod exec;
pub mod exec_env;
#[cfg_attr(target_arch = "wasm32", path = "exec_policy_wasm.rs")]
mod exec_policy;
pub mod external_agent_config;
pub mod file_watcher;
mod flags;
#[cfg(test)]
mod git_info_tests;
mod guardian;
mod hook_runtime;
pub mod instructions;
#[cfg_attr(target_arch = "wasm32", path = "landlock_wasm.rs")]
pub mod landlock;
pub mod mcp;
#[cfg_attr(target_arch = "wasm32", path = "mcp_connection_manager_wasm.rs")]
mod mcp_connection_manager;
mod mcp_tool_approval_templates;
mod mcp_types;
pub mod models_manager;
mod network_policy_decision;
pub mod network_proxy_loader;
mod original_image_detail;
pub use mcp_connection_manager::MCP_SANDBOX_STATE_CAPABILITY;
pub use mcp_connection_manager::MCP_SANDBOX_STATE_METHOD;
pub use mcp_connection_manager::SandboxState;
pub use text_encoding::bytes_to_string_smart;
mod mcp_tool_call;
#[cfg_attr(target_arch = "wasm32", path = "memories_wasm.rs")]
mod memories;
pub mod mention_syntax;
#[cfg_attr(target_arch = "wasm32", path = "message_history_wasm.rs")]
pub mod message_history;
mod model_provider_info;
mod monotonic_time;
pub mod utils;
pub use utils::path_utils;
#[cfg_attr(target_arch = "wasm32", path = "personality_migration_wasm.rs")]
pub mod personality_migration;
pub mod plugins;
mod provider_auth;
pub(crate) mod mentions {
    pub(crate) use crate::plugins::build_connector_slug_counts;
    pub(crate) use crate::plugins::build_skill_name_counts;
    pub(crate) use crate::plugins::collect_explicit_app_ids;
    pub(crate) use crate::plugins::collect_explicit_plugin_mentions;
    pub(crate) use crate::plugins::collect_tool_mentions_from_messages;
}
mod sandbox_tags;
pub mod sandboxing;
mod session_prefix;
mod session_startup_prewarm;
mod shell_detect;
pub mod skills;
pub(crate) use skills::SkillError;
pub(crate) use skills::SkillInjections;
pub(crate) use skills::SkillLoadOutcome;
pub(crate) use skills::SkillMetadata;
pub(crate) use skills::SkillsLoadInput;
pub(crate) use skills::SkillsManager;
pub(crate) use skills::build_skill_injections;
pub(crate) use skills::build_skill_name_counts;
pub(crate) use skills::collect_env_var_dependencies;
pub(crate) use skills::collect_explicit_skill_mentions;
pub(crate) use skills::config_rules;
pub(crate) use skills::injection;
pub(crate) use skills::loader;
pub(crate) use skills::manager;
pub(crate) use skills::maybe_emit_implicit_skill_invocation;
pub(crate) use skills::model;
pub(crate) use skills::render_skills_section;
pub(crate) use skills::resolve_skill_dependencies_for_turn;
pub(crate) use skills::skills_load_input_from_config;
mod skills_watcher;
mod stream_events_utils;
pub mod test_support;
mod text_encoding;
mod unified_exec;
pub mod windows_sandbox;
pub use client::X_RESPONSESAPI_INCLUDE_TIMING_METRICS_HEADER;
pub use codex_protocol::config_types::ModelProviderAuthInfo;
pub use model_provider_info::DEFAULT_LMSTUDIO_PORT;
pub use model_provider_info::DEFAULT_OLLAMA_PORT;
pub use model_provider_info::LMSTUDIO_OSS_PROVIDER_ID;
pub use model_provider_info::ModelProviderInfo;
pub use model_provider_info::OLLAMA_OSS_PROVIDER_ID;
pub use model_provider_info::OPENAI_PROVIDER_ID;
pub use model_provider_info::WireApi;
pub use model_provider_info::built_in_model_providers;
pub use model_provider_info::create_oss_provider_with_base_url;
mod event_mapping;
mod response_debug_context;
pub mod review_format;
pub mod review_prompts;
mod thread_manager;
pub mod web_search;
pub mod windows_sandbox_read_grants;
pub use thread_manager::ForkSnapshot;
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
pub use codex_analytics::AnalyticsEventsClient;
mod default_client_forwarding;

/// Default Codex HTTP client headers and reqwest construction.
///
/// Implemented in [`codex_login::default_client`]; this module re-exports that API for crates
/// that import `codex_core::default_client`.
pub mod default_client {
    pub use super::default_client_forwarding::*;
}
#[cfg_attr(target_arch = "wasm32", path = "project_doc_wasm.rs")]
pub mod project_doc;
mod rollout;
pub(crate) mod safety;
pub mod seatbelt;
mod session_rollout_init_error;
pub mod shell;
#[cfg_attr(target_arch = "wasm32", path = "shell_snapshot_wasm.rs")]
pub mod shell_snapshot;
#[cfg_attr(target_arch = "wasm32", path = "spawn_wasm.rs")]
pub mod spawn;
pub mod state_db_bridge;
pub mod state_db {
    pub use crate::state_db_bridge::*;
}
mod thread_rollout_truncation;
mod tools;
pub mod turn_diff_tracker;
mod turn_metadata;
mod turn_timing;
pub use rollout::ARCHIVED_SESSIONS_SUBDIR;
pub use rollout::INTERACTIVE_SESSION_SOURCES;
pub use rollout::RolloutRecorder;
pub use rollout::RolloutRecorderParams;
pub use rollout::SESSIONS_SUBDIR;
pub use rollout::SessionMeta;
pub use rollout::append_thread_name;
pub use rollout::find_archived_thread_path_by_id_str;
#[deprecated(note = "use find_thread_path_by_id_str")]
pub use rollout::find_conversation_path_by_id_str;
pub use rollout::find_thread_name_by_id;
pub use rollout::find_thread_path_by_id_str;
pub use rollout::find_thread_path_by_name_str;
pub use rollout::list::Cursor;
pub use rollout::list::ThreadItem;
pub use rollout::list::ThreadSortKey;
pub use rollout::list::ThreadsPage;
pub use rollout::list::parse_cursor;
pub use rollout::list::read_head_for_summary;
pub use rollout::list::read_session_meta_line;
pub use rollout::policy::EventPersistenceMode;
pub use rollout::rollout_date_parts;
pub use rollout::session_index::find_thread_names_by_ids;
mod function_tool;
mod state;
mod tasks;
mod user_shell_command;
pub mod util;
pub(crate) use codex_protocol::protocol;
pub(crate) use codex_shell_command::bash;
pub(crate) use codex_shell_command::is_dangerous_command;
pub(crate) use codex_shell_command::is_safe_command;
pub(crate) use codex_shell_command::parse_command;
pub(crate) use codex_shell_command::powershell;

pub use client::ModelClient;
pub use client::ModelClientSession;
pub use client::X_CODEX_TURN_METADATA_HEADER;
pub use client_common::Prompt;
pub use client_common::REVIEW_PROMPT;
pub use client_common::ResponseEvent;
pub use client_common::ResponseStream;
pub use codex_code_mode::CodeModeRuntime;
pub use codex_sandboxing::get_platform_sandbox;
pub use codex_tools::parse_tool_input_schema;
pub use compact::content_items_to_text;
pub use event_mapping::parse_turn_item;
pub use exec_policy::ExecPolicyError;
pub use exec_policy::check_execpolicy_for_warnings;
pub use exec_policy::format_exec_policy_error_with_source;
pub use exec_policy::load_exec_policy;
pub use file_watcher::FileWatcherEvent;
pub use turn_metadata::build_turn_metadata_header;
pub mod compact;
pub mod memory_trace;
#[cfg_attr(target_arch = "wasm32", path = "otel_init_wasm.rs")]
pub mod otel_init;
