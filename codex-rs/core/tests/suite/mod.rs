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
mod prompt_caching;
mod review;
mod rmcp_client;
mod rollout_list_find;
mod seatbelt;
mod stream_error_allows_next_turn;
mod stream_no_completed;
mod user_notification;
