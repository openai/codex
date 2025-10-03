// Aggregates all former standalone integration tests as modules.

#[cfg(not(target_os = "windows"))]
mod abort_tasks;
mod cli_stream;
mod client;
mod compact;
mod compact_resume_fork;
mod exec;
mod exec_stream_events;
mod fork_conversation;
mod json_result;
mod live_cli;
mod model_overrides;
mod model_tools;
mod otel;
mod prompt_caching;
mod read_file;
mod review;
mod rmcp_client;
mod rollout_list_find;
mod seatbelt;
mod stream_error_allows_next_turn;
mod stream_no_completed;
mod tool_harness;
mod tools;
mod unified_exec;
mod user_notification;
mod view_image;
