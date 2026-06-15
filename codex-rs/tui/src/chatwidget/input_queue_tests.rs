use codex_app_server_protocol::TurnSubmission;
use pretty_assertions::assert_eq;

use super::*;

fn queued_item(text: &str, status: QueuedItemStatus) -> QueuedItem {
    QueuedItem {
        id: text.to_string(),
        submission: TurnSubmission {
            input: vec![codex_app_server_protocol::UserInput::Text {
                text: text.to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        },
        provenance: QueuedItemProvenance::User,
        status,
    }
}

#[test]
fn preview_keeps_queue_categories_separate() {
    let mut state = InputQueueState::default();
    state
        .user_message_queue
        .items
        .push(queued_item("server queued", QueuedItemStatus::Pending));
    state
        .queued_user_messages
        .push_back(UserMessage::from("local queued").into());
    state
        .rejected_steers_queue
        .push_back(UserMessage::from("rejected"));
    state.pending_steers.push_back(PendingSteer {
        user_message: UserMessage::from("pending"),
        history_record: UserMessageHistoryRecord::UserMessageText,
        compare_key: crate::chatwidget::user_messages::PendingSteerCompareKey {
            message: "pending".to_string(),
            image_count: 0,
        },
    });

    assert_eq!(
        state.preview(),
        PendingInputPreview {
            queued_messages: vec!["server queued".to_string(), "local queued".to_string()],
            pending_steers: vec!["pending".to_string()],
            rejected_steers: vec!["rejected".to_string()],
            has_editable_queued_message: true,
        }
    );
}

#[test]
fn snapshot_replaces_the_displayed_server_queue() {
    let mut state = InputQueueState::default();
    state
        .user_message_queue
        .items
        .push(queued_item("old", QueuedItemStatus::Pending));

    state
        .user_message_queue
        .set_snapshot(ThreadQueueListResponse {
            data: vec![queued_item("current", QueuedItemStatus::Pending)],
            next_cursor: None,
        });

    assert_eq!(state.preview().queued_messages, vec!["current"]);
}

#[test]
fn preview_includes_external_provenance_and_failure() {
    let mut item = queued_item(
        "run the release check",
        QueuedItemStatus::Failed {
            error: "thread unavailable".to_string(),
        },
    );
    item.provenance = QueuedItemProvenance::ExternalEvent {
        source: "deploy-hook".to_string(),
        metadata: Default::default(),
    };
    let mut state = InputQueueState::default();
    state.user_message_queue.items.push(item);

    assert_eq!(
        state.preview().queued_messages,
        vec!["[failed: thread unavailable] [deploy-hook] run the release check"]
    );
    assert!(!state.blocks_local_queue_autosend());
}

#[test]
fn server_queue_state_distinguishes_follow_ups_from_refreshes() {
    let mut state = InputQueueState::default();
    state
        .user_message_queue
        .items
        .push(queued_item("pending", QueuedItemStatus::Pending));
    assert!(state.blocks_local_queue_autosend());
    assert!(state.has_queued_follow_up_messages());

    state.user_message_queue.items.clear();
    state.user_message_queue.has_more = true;
    assert!(state.blocks_local_queue_autosend());
    assert!(state.has_queued_follow_up_messages());

    state.user_message_queue.has_more = false;
    state.user_message_queue.refresh_in_flight = true;
    assert!(state.blocks_local_queue_autosend());
    assert!(!state.has_queued_follow_up_messages());
}

#[test]
fn clear_resets_local_and_server_queue_state() {
    let mut state = InputQueueState::default();
    state
        .user_message_queue
        .items
        .push(queued_item("server queued", QueuedItemStatus::Pending));
    state.user_message_queue.has_more = true;
    state.user_message_queue.refresh_in_flight = true;
    state
        .queued_user_messages
        .push_back(UserMessage::from("local queued").into());
    state.user_turn_pending_start = true;

    state.clear();

    assert_eq!(state.preview(), PendingInputPreview::default());
    assert!(!state.user_turn_pending_start);
    assert!(!state.blocks_local_queue_autosend());
}
