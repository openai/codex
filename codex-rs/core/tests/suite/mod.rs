// Aggregates all former standalone integration tests as modules.
use codex_arg0::arg0_dispatch;
use ctor::ctor;
use tempfile::TempDir;

// This code runs before any other tests are run.
// It allows the test binary to behave like codex and dispatch to apply_patch and codex-linux-sandbox
// based on the arg0.
// NOTE: this doesn't work on ARM
#[ctor]
pub static CODEX_ALIASES_TEMP_DIR: TempDir = unsafe {
    #[allow(clippy::unwrap_used)]
    arg0_dispatch().unwrap()
};

#[cfg(not(target_os = "windows"))]
mod abort_tasks;
#[cfg(not(target_os = "windows"))]
mod apply_patch_cli;
#[cfg(not(target_os = "windows"))]
mod approvals;
mod auth_refresh;
mod cli_stream;
mod client;
mod codex_delegate;
mod compact;
mod compact_remote;
mod compact_resume_fork;
mod deprecation_notice;
mod exec;
mod exec_policy;
mod fork_conversation;
mod grep_files;
mod items;
mod json_result;
mod list_dir;
mod live_cli;
mod model_overrides;
mod model_tools;
mod otel;
mod prompt_caching;
mod quota_exceeded;
mod read_file;
mod resume;
mod review;
mod rmcp_client;
mod rollout_list_find;
mod seatbelt;
mod shell_serialization;
mod stream_error_allows_next_turn;
mod stream_no_completed;
mod text_encoding_fix;
mod tool_harness;
mod tool_parallelism;
mod tools;
mod truncation;
mod undo;
mod unified_exec;
mod user_notification;
mod user_shell_cmd;
mod view_image;
