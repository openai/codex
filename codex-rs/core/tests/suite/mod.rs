// Aggregates all former standalone integration tests as modules.

#[cfg(not(target_os = "windows"))]
mod abort_tasks;
#[cfg(not(target_os = "windows"))]
mod apply_patch_cli;
#[cfg(not(target_os = "windows"))]
mod approvals;
mod cli_stream;
mod client;
mod compact;
mod compact_resume_fork;
mod exec;
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
mod read_file;
mod resume;
mod review;
mod rmcp_client;
mod rollout_list_find;
mod seatbelt;
mod shell_serialization;
mod stream_error_allows_next_turn;
mod stream_no_completed;
mod tool_harness;
mod tool_parallelism;
mod tools;
mod truncation;
mod unified_exec;
mod user_notification;
mod user_shell_cmd;
mod view_image;
