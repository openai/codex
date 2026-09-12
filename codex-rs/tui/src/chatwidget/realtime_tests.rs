//! Regression coverage for thread-owned voice sessions and transcript/handoff safety.
//! Synthetic events preserve typed turns and reject stale session generations.

#[path = "realtime/recording_controls_tests.rs"]
mod recording_controls_tests;
#[path = "realtime/session_metrics_tests.rs"]
mod session_metrics_tests;

use super::RealtimeConversationPhase;
use crate::app_command::AppCommand;
use crate::app_event::AppEvent;
use crate::chatwidget::ChatWidget;
use crate::chatwidget::ReplayKind;
use crate::chatwidget::tests::make_chatwidget_manual_with_sender;
use crate::chatwidget::tests::render_bottom_popup;
use codex_app_server_protocol::AgentMessageDeltaNotification;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::ReasoningSummaryPartAddedNotification;
use codex_app_server_protocol::ReasoningSummaryTextDeltaNotification;
use codex_app_server_protocol::ReasoningTextDeltaNotification;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnItemsView;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use codex_protocol::ThreadId;
use codex_protocol::items::AsyncUserInputQuestion;
use codex_protocol::models::MessagePhase;
use futures::future::AbortHandle;
use std::collections::VecDeque;

// Model the app's atomic handoff before inspecting its ordinary history events.
pub(crate) fn commit_realtime_history_events(
    chat: &mut ChatWidget,
    events: &mut tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
) {
    let mut forwarded = Vec::new();
    while let Ok(event) = events.try_recv() {
        match event {
            AppEvent::CommitRealtimeTranscriptHistory => forwarded.extend(
                chat.take_realtime_transcript_history()
                    .into_iter()
                    .map(AppEvent::InsertHistoryCell),
            ),
            event => forwarded.push(event),
        }
    }
    for event in forwarded {
        chat.app_event_tx.send(event);
    }
}

fn activate_voice(chat: &mut ChatWidget) -> ThreadId {
    let thread_id = ThreadId::new();
    activate_voice_for_thread(chat, thread_id);
    thread_id
}

pub(crate) fn activate_voice_for_thread(chat: &mut ChatWidget, thread_id: ThreadId) {
    chat.thread_id = Some(thread_id);
    chat.realtime_conversation.phase = RealtimeConversationPhase::Active;
    chat.realtime_conversation.thread_id = Some(thread_id);
    chat.realtime_conversation.backend_started = true;
    chat.realtime_conversation.latest_input_was_voice = true;
}

fn user_item(text: &str) -> ThreadItem {
    ThreadItem::UserMessage {
        id: format!("user-{text}"),
        client_id: None,
        content: vec![UserInput::Text {
            text: text.to_string(),
            text_elements: Vec::new(),
        }],
    }
}

fn agent_item(item_id: &str, text: &str, phase: Option<MessagePhase>) -> ThreadItem {
    ThreadItem::AgentMessage {
        id: item_id.to_string(),
        text: text.to_string(),
        phase,
        questions: None,
        memory_citation: None,
        delivery: None,
    }
}

fn start_item(chat: &mut ChatWidget, thread_id: ThreadId, turn_id: &str, item: ThreadItem) {
    chat.handle_server_notification(
        ServerNotification::ItemStarted(ItemStartedNotification {
            thread_id: thread_id.to_string(),
            turn_id: turn_id.to_string(),
            item,
            started_at_ms: 0,
        }),
        /*replay_kind*/ None,
    );
}

fn complete_item(chat: &mut ChatWidget, thread_id: ThreadId, turn_id: &str, item: ThreadItem) {
    chat.handle_server_notification(
        ServerNotification::ItemCompleted(ItemCompletedNotification {
            thread_id: thread_id.to_string(),
            turn_id: turn_id.to_string(),
            item,
            completed_at_ms: 0,
        }),
        /*replay_kind*/ None,
    );
}

fn finish_turn(
    chat: &mut ChatWidget,
    thread_id: ThreadId,
    turn_id: &str,
    items: Vec<ThreadItem>,
    status: TurnStatus,
) {
    chat.handle_server_notification(
        ServerNotification::TurnCompleted(TurnCompletedNotification {
            thread_id: thread_id.to_string(),
            turn: Turn {
                id: turn_id.to_string(),
                items,
                items_view: TurnItemsView::Summary,
                status,
                error: None,
                started_at: None,
                completed_at: None,
                duration_ms: None,
            },
        }),
        /*replay_kind*/ None,
    );
}

fn without_completion_metadata(rendered: &str) -> String {
    rendered
        .lines()
        .filter_map(|line| {
            let stable = line
                .split_once("  done ")
                .map_or(line, |(before, _)| before);
            (!stable.is_empty()).then_some(stable)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[path = "realtime_tests/caption_replay.rs"]
mod caption_replay;
#[path = "realtime_tests/handoff_privacy.rs"]
mod handoff_privacy;
#[path = "realtime_tests/handoffs.rs"]
mod handoffs;
#[path = "realtime_tests/lifecycle.rs"]
mod lifecycle;
#[path = "realtime_tests/speech_recovery.rs"]
mod speech_recovery;
#[path = "realtime_tests/transcripts.rs"]
mod transcripts;
