use super::*;
use codex_app_server_protocol::QueuedItem;

#[tokio::test]
async fn plain_follow_up_uses_the_server_queue() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.set_feature_enabled(Feature::UserMessageQueue, /*enabled*/ true);
    handle_turn_started(&mut chat, "active-turn");

    chat.queue_user_message(UserMessage::from("queued follow-up"));

    let Op::QueueUserMessage {
        submission,
        fallback_user_message,
    } = op_rx.try_recv().expect("server queue command")
    else {
        panic!("expected QueueUserMessage");
    };
    assert_eq!(
        submission.input,
        vec![UserInput::Text {
            text: "queued follow-up".to_string(),
            text_elements: Vec::new(),
        }]
    );
    assert_eq!(fallback_user_message.text, "queued follow-up");

    chat.set_server_queue_snapshot(codex_app_server_protocol::ThreadQueueListResponse {
        data: vec![QueuedItem {
            id: "queued-1".to_string(),
            submission: codex_app_server_protocol::TurnSubmission::default(),
            provenance: codex_app_server_protocol::QueuedItemProvenance::User,
            status: codex_app_server_protocol::QueuedItemStatus::Pending,
        }],
        next_cursor: None,
    });
    handle_turn_completed(&mut chat, "active-turn", /*duration_ms*/ None);
    chat.queue_user_message(UserMessage::from("another follow-up"));
    assert!(matches!(op_rx.try_recv(), Ok(Op::QueueUserMessage { .. })));
}

#[tokio::test]
async fn queue_invalidation_refreshes_only_the_active_thread() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let active_thread_id = ThreadId::new();
    chat.thread_id = Some(active_thread_id);
    chat.set_feature_enabled(Feature::UserMessageQueue, /*enabled*/ true);

    chat.handle_server_notification(
        ServerNotification::ThreadQueueChanged(
            codex_app_server_protocol::ThreadQueueChangedNotification {
                thread_id: ThreadId::new().to_string(),
            },
        ),
        /*replay_kind*/ None,
    );
    assert!(rx.try_recv().is_err());

    chat.handle_server_notification(
        ServerNotification::ThreadQueueChanged(
            codex_app_server_protocol::ThreadQueueChangedNotification {
                thread_id: active_thread_id.to_string(),
            },
        ),
        /*replay_kind*/ None,
    );

    assert!(op_rx.try_recv().is_err());
    assert!(matches!(
        rx.try_recv(),
        Ok(AppEvent::SubmitThreadOp {
            thread_id,
            op: Op::RefreshUserMessageQueue,
        }) if thread_id == active_thread_id
    ));
}

#[tokio::test]
async fn queue_refresh_sends_new_input_through_the_server_queue() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    chat.set_feature_enabled(Feature::UserMessageQueue, /*enabled*/ true);
    assert!(chat.start_server_queue_refresh(thread_id));

    chat.queue_user_message(UserMessage::from("wait for refresh"));

    let Op::QueueUserMessage { submission, .. } = op_rx.try_recv().expect("server queue command")
    else {
        panic!("expected QueueUserMessage");
    };
    assert_eq!(
        submission.input,
        vec![UserInput::Text {
            text: "wait for refresh".to_string(),
            text_elements: Vec::new(),
        }]
    );
}
