//! Root of the `codex-core` library.

// Prevent accidental direct writes to stdout/stderr in library code. All
// user-visible output must go through the appropriate abstraction (e.g.,
// the TUI or the tracing stack).
#![deny(clippy::print_stdout, clippy::print_stderr)]

mod analytics_client;
pub mod api_bridge;
mod apply_patch;
mod apps;
mod arc_monitor;
pub use codex_login as auth;
mod auth_env_telemetry;
mod client;
mod client_common;
pub mod codex;
mod realtime_context;
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
pub mod config_loader;
pub mod connectors;
mod context_manager;
mod contextual_user_message;
pub mod custom_prompts;
pub mod env;
mod environment_context;
pub mod error;
pub mod exec;
pub mod exec_env;
mod exec_policy;
pub mod external_agent_config;
mod file_watcher;
mod flags;
pub mod git_info;
mod guardian;
mod history_truncation;
mod hook_runtime;
pub mod instructions;
pub mod landlock;
pub mod mcp;
mod mcp_connection_manager;
mod mcp_tool_approval_templates;
pub mod models_manager;
mod network_policy_decision;
pub mod network_proxy_loader;
mod original_image_detail;
mod packages;
pub use mcp_connection_manager::MCP_SANDBOX_STATE_CAPABILITY;
pub use mcp_connection_manager::MCP_SANDBOX_STATE_METHOD;
pub use mcp_connection_manager::SandboxState;
pub use text_encoding::bytes_to_string_smart;
mod mcp_tool_call;
mod memories;
pub mod mention_syntax;
mod mentions;
pub mod message_history;
mod model_provider_info;
pub mod path_utils;
pub mod personality_migration;
pub mod plugins;
mod rollout_config_builder;
mod sandbox_tags;
pub mod sandboxing;
mod session_prefix;
mod session_startup_prewarm;
mod shell_detect;
mod stream_events_utils;
pub mod test_support;
mod text_encoding;
pub use codex_login::token_data;
mod truncate;
mod unified_exec;
pub mod windows_sandbox;
pub use client::X_RESPONSESAPI_INCLUDE_TIMING_METRICS_HEADER;
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
mod seatbelt_permissions;
mod thread_manager;
pub mod web_search;
pub mod windows_sandbox_read_grants;
pub use thread_manager::NewThread;
pub use thread_manager::ThreadManager;
#[deprecated(note = "use ThreadManager")]
pub type ConversationManager = ThreadManager;
#[deprecated(note = "use NewThread")]
pub type NewConversation = NewThread;
#[deprecated(note = "use CodexThread")]
pub type CodexConversation = CodexThread;
// Re-export common auth types for workspace consumers
pub use analytics_client::AnalyticsEventsClient;
pub use auth::AuthManager;
pub use auth::CodexAuth;
mod default_client_forwarding;

/// Default Codex HTTP client headers and reqwest construction.
///
/// Implemented in [`codex_login::default_client`]; this module re-exports that API for crates
/// that import `codex_core::default_client`.
pub mod default_client {
    pub use super::default_client_forwarding::*;
}
mod function_tool;
pub mod project_doc;
pub(crate) mod safety;
pub mod seatbelt;
pub mod shell;
pub mod shell_snapshot;
pub mod skills;
pub mod spawn;
mod state;
pub mod state_runtime;
mod tasks;
mod tools;
pub mod turn_diff_tracker;
mod turn_metadata;
mod turn_timing;
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
pub use compact::content_items_to_text;
pub use event_mapping::parse_turn_item;
pub use exec_policy::ExecPolicyError;
pub use exec_policy::check_execpolicy_for_warnings;
pub use exec_policy::format_exec_policy_error_with_source;
pub use exec_policy::load_exec_policy;
pub use file_watcher::FileWatcherEvent;
pub use rollout_config_builder::rollout_config;
pub use rollout_config_builder::rollout_config_with_cwd;
pub use safety::get_platform_sandbox;
pub use tools::spec::parse_tool_input_schema;
pub use turn_metadata::build_turn_metadata_header;
pub mod compact;
pub mod memory_trace;
pub mod otel_init;
