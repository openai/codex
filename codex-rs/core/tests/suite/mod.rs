// Aggregates all former standalone integration tests as modules.
use codex_apply_patch::CODEX_CORE_APPLY_PATCH_ARG1;
use codex_sandboxing::landlock::CODEX_LINUX_SANDBOX_ARG0;
use codex_test_binary_support::TestBinaryDispatchGuard;
use codex_test_binary_support::TestBinaryDispatchMode;
use codex_test_binary_support::configure_test_binary_dispatch;
use ctor::ctor;

// This code runs before any other tests are run.
// It allows the test binary to behave like codex and dispatch to apply_patch and codex-linux-sandbox
// based on the arg0.
// NOTE: this doesn't work on ARM
#[ctor]
pub static CODEX_ALIASES_TEMP_DIR: Option<TestBinaryDispatchGuard> = {
    configure_test_binary_dispatch("codex-core-tests", |exe_name, argv1| {
        if argv1 == Some(CODEX_CORE_APPLY_PATCH_ARG1) {
            return TestBinaryDispatchMode::DispatchArg0Only;
        }
        if exe_name == CODEX_LINUX_SANDBOX_ARG0 {
            return TestBinaryDispatchMode::DispatchArg0Only;
        }
        TestBinaryDispatchMode::InstallAliases
    })
};

#[cfg(not(target_os = "windows"))]
mod abort_tasks;
mod agent_jobs;
mod agent_websocket;
mod agents_md;
mod apply_patch_cli;
#[cfg(not(target_os = "windows"))]
mod approvals;
mod cli_stream;
mod client;
mod client_websockets;
mod code_mode;
mod codex_delegate;
mod collaboration_instructions;
mod compact;
mod compact_remote;
mod compact_resume_fork;
mod deprecation_notice;
mod exec;
mod exec_policy;
mod fork_thread;
mod hierarchical_agents;
#[cfg(not(target_os = "windows"))]
mod hooks;
mod image_rollout;
mod items;
mod js_repl;
mod json_result;
mod live_cli;
mod live_reload;
mod memories;
mod model_overrides;
mod model_switching;
mod model_visible_layout;
mod models_cache_ttl;
mod models_etag_responses;
mod openai_file_mcp;
mod otel;
mod pending_input;
mod permissions_messages;
mod personality;
mod personality_migration;
mod plugins;
mod prompt_caching;
mod prompt_debug_tests;
mod quota_exceeded;
mod realtime_conversation;
mod remote_env;
mod remote_models;
mod request_compression;
#[cfg(not(target_os = "windows"))]
mod request_permissions;
#[cfg(not(target_os = "windows"))]
mod request_permissions_tool;
mod request_user_input;
mod responses_api_proxy_headers;
mod resume;
mod resume_warning;
mod review;
mod rmcp_client;
mod rollout_list_find;
mod safety_check_downgrade;
mod search_tool;
mod shell_command;
mod shell_serialization;
mod shell_snapshot;
mod skill_approval;
mod skills;
mod spawn_agent_description;
mod sqlite_state;
mod stream_error_allows_next_turn;
mod stream_no_completed;
mod subagent_notifications;
mod tool_harness;
mod tool_parallelism;
mod tool_suggest;
mod tools;
mod truncation;
mod turn_state;
mod undo;
mod unified_exec;
mod unstable_features_warning;
mod user_notification;
mod user_shell_cmd;
mod view_image;
mod web_search;
mod websocket_fallback;
mod window_headers;
