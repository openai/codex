use super::BufferedRawResponseRouting;
use super::BufferedThreadEvent;
use super::BusyHistoryReadDisposition;
use super::ListenerCommandTransition;
use super::RESUME_EXEC_DELTA_REPLAY_MAX_EVENTS;
use super::RESUME_EXEC_DELTA_REPLAY_TRUNCATION_MARKER;
use super::ResumeEventCoverage;
use super::ResumeExecDeltaReplay;
use super::ResumeInFlightEvent;
use super::ResumePayloadItemCoverage;
use super::ResumePayloadMode;
use super::apply_listener_command_transition;
use super::buffered_event_delivery_recipients;
use super::buffered_event_is_represented_in_resume_payload;
use super::buffered_event_recipients;
use super::buffered_raw_response_recipients;
use super::classify_busy_history_read;
use super::dispatch_replayed_exec_deltas_to_connection;
use super::project_buffered_request_liveness;
use super::project_thread_status_after_buffered_events;
use super::read_pending_thread_resume_history;
use super::route_resume_in_flight_event;
use super::run_cancelable_resume_worker;
use super::should_defer_listener_command;
use super::should_replay_reconciled_token_usage;
use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::OutgoingEnvelope;
use crate::outgoing_message::OutgoingMessage;
use crate::outgoing_message::OutgoingMessageSender;
use crate::request_processors::build_api_turns_from_rollout_items;
use crate::thread_state::ThreadListenerCommand;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadActiveFlag;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadStatus;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnItemsView;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::TurnsPage;
use codex_app_server_protocol::build_command_execution_end_item;
use codex_protocol::ThreadId;
use codex_protocol::items::AgentMessageContent;
use codex_protocol::items::AgentMessageItem;
use codex_protocol::items::HookPromptFragment;
use codex_protocol::items::ImageGenerationItem;
use codex_protocol::items::McpToolCallItem;
use codex_protocol::items::TurnItem;
use codex_protocol::items::build_hook_prompt_message;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AgentMessageContentDeltaEvent;
use codex_protocol::protocol::AgentMessageEvent;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecCommandBeginEvent;
use codex_protocol::protocol::ExecCommandEndEvent;
use codex_protocol::protocol::ExecCommandOutputDeltaEvent;
use codex_protocol::protocol::ExecCommandSource;
use codex_protocol::protocol::ExecCommandStatus;
use codex_protocol::protocol::ExecOutputStream;
use codex_protocol::protocol::ItemCompletedEvent;
use codex_protocol::protocol::ItemStartedEvent;
use codex_protocol::protocol::RateLimitSnapshot;
use codex_protocol::protocol::RawResponseItemEvent;
use codex_protocol::protocol::RequestUserInputEvent;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::ThreadRolledBackEvent;
use codex_protocol::protocol::TokenCountEvent;
use codex_protocol::protocol::TokenUsage;
use codex_protocol::protocol::TokenUsageInfo;
use codex_protocol::protocol::TurnAbortReason;
use codex_protocol::protocol::TurnAbortedEvent;
use codex_protocol::protocol::TurnCompleteEvent;
use codex_protocol::protocol::TurnStartedEvent;
use codex_thread_store::ThreadMetadataPatch;
use codex_utils_pty::DEFAULT_OUTPUT_BYTES_CAP;
use core_test_support::test_codex::TestCodexHarness;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

fn event_is_represented(
    buffered: &BufferedThreadEvent,
    thread_turns: &[Turn],
    initial_turns_page: Option<&TurnsPage>,
    resume_payload_mode: ResumePayloadMode,
) -> bool {
    let mut item_coverage = ResumePayloadItemCoverage::new(thread_turns, initial_turns_page);
    buffered_event_is_represented_in_resume_payload(
        buffered,
        thread_turns,
        initial_turns_page,
        &mut item_coverage,
        resume_payload_mode,
    )
}

fn turn_with_view(id: &str, items_view: TurnItemsView, status: TurnStatus) -> Turn {
    Turn {
        id: id.to_string(),
        items: Vec::new(),
        items_view,
        status,
        error: None,
        started_at: Some(1),
        completed_at: None,
        duration_ms: None,
    }
}

#[path = "thread_lifecycle/resume_event_coverage_tests.rs"]
mod resume_event_coverage_tests;
#[path = "thread_lifecycle/resume_projected_item_tests.rs"]
mod resume_projected_item_tests;
#[path = "thread_lifecycle/resume_token_replay_tests.rs"]
mod resume_token_replay_tests;
#[path = "thread_lifecycle/resume_worker_tests.rs"]
mod resume_worker_tests;
#[path = "thread_lifecycle/thread_history_tests.rs"]
mod thread_history_tests;
