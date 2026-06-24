use codex_app_server_protocol::ErrorNotification;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnError;
use codex_app_server_protocol::TurnItemsView;
use codex_app_server_protocol::TurnStatus;
use codex_protocol::ThreadId;
use pretty_assertions::assert_eq;

use super::ActiveItemLifecycleTracker;

#[test]
fn replays_latest_content_until_terminal_notification() {
    let thread_id = ThreadId::new();
    let turn_id = "turn-1".to_string();
    let started = ItemStartedNotification {
        thread_id: thread_id.to_string(),
        turn_id: turn_id.clone(),
        started_at_ms: 1,
        item: ThreadItem::AgentMessage {
            id: "message-1".to_string(),
            text: String::new(),
            phase: None,
            memory_citation: None,
        },
    };
    let latest_item = ThreadItem::AgentMessage {
        id: "message-1".to_string(),
        text: "latest".to_string(),
        phase: None,
        memory_citation: None,
    };
    let active_turn = Turn {
        id: turn_id.clone(),
        items: vec![latest_item.clone()],
        items_view: TurnItemsView::Full,
        status: TurnStatus::InProgress,
        error: None,
        started_at: None,
        completed_at: None,
        duration_ms: None,
    };
    let terminal_notifications = [
        ServerNotification::ItemCompleted(ItemCompletedNotification {
            item: latest_item.clone(),
            thread_id: thread_id.to_string(),
            turn_id: turn_id.clone(),
            completed_at_ms: 2,
        }),
        ServerNotification::TurnCompleted(TurnCompletedNotification {
            thread_id: thread_id.to_string(),
            turn: active_turn.clone(),
        }),
        ServerNotification::Error(ErrorNotification {
            error: TurnError {
                message: "terminal failure".to_string(),
                codex_error_info: None,
                additional_details: None,
            },
            will_retry: false,
            thread_id: thread_id.to_string(),
            turn_id,
        }),
    ];

    for terminal_notification in terminal_notifications {
        let mut tracker = ActiveItemLifecycleTracker::default();
        tracker.note_notification(thread_id, &ServerNotification::ItemStarted(started.clone()));
        assert_eq!(
            tracker.active_starts_for_turn(thread_id, &active_turn),
            vec![ItemStartedNotification {
                item: latest_item.clone(),
                ..started.clone()
            }]
        );

        tracker.note_notification(thread_id, &terminal_notification);
        assert_eq!(
            tracker.active_starts_for_turn(thread_id, &active_turn),
            Vec::new()
        );
    }
}
