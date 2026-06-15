use super::*;
use crate::chatwidget::UserMessage;

#[tokio::test]
async fn lag_refresh_targets_the_widget_thread() {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let app_server = Box::pin(crate::start_embedded_app_server_for_picker(
        app.chat_widget.config_ref(),
    ))
    .await
    .expect("embedded app server");
    let thread_id = ThreadId::new();
    app.chat_widget
        .set_feature_enabled(Feature::UserMessageQueue, /*enabled*/ false);
    app.chat_widget
        .handle_thread_session(test_thread_session(thread_id, test_path_buf("/tmp/origin")));
    let _ = app.config.features.enable(Feature::UserMessageQueue);
    app.chat_widget
        .set_feature_enabled(Feature::UserMessageQueue, /*enabled*/ true);
    app.active_thread_id = Some(ThreadId::new());
    while app_event_rx.try_recv().is_ok() {}

    app.handle_app_server_event(
        &app_server,
        codex_app_server_client::AppServerEvent::Lagged { skipped: 1 },
    )
    .await;

    assert!(matches!(
        app_event_rx.try_recv(),
        Ok(AppEvent::SubmitThreadOp {
            thread_id: refresh_thread_id,
            op: AppCommand::RefreshUserMessageQueue,
        }) if refresh_thread_id == thread_id
    ));
}

#[tokio::test]
async fn queue_snapshot_updates_its_origin_thread() {
    let mut app = make_test_app().await;
    let thread_id = ThreadId::new();
    app.chat_widget
        .handle_thread_session(test_thread_session(thread_id, test_path_buf("/tmp/origin")));
    let mut input_state = app
        .chat_widget
        .capture_thread_input_state()
        .expect("chat widget input state");
    input_state.user_message_queue.refresh_in_flight = true;
    app.ensure_thread_channel(thread_id)
        .store
        .lock()
        .await
        .input_state = Some(input_state.clone());
    let (chat_widget, _app_event_tx, _rx, _op_rx) = make_chatwidget_manual_with_sender().await;
    app.chat_widget = chat_widget;
    let snapshot = codex_app_server_protocol::ThreadQueueListResponse {
        data: vec![codex_app_server_protocol::QueuedItem {
            id: "queued-1".to_string(),
            submission: codex_app_server_protocol::TurnSubmission::default(),
            provenance: codex_app_server_protocol::QueuedItemProvenance::User,
            status: codex_app_server_protocol::QueuedItemStatus::Pending,
        }],
        next_cursor: None,
    };
    let mut expected = input_state;
    expected.user_message_queue.set_snapshot(snapshot.clone());

    app.apply_server_queue_snapshot(thread_id, snapshot).await;

    let actual = app
        .thread_event_channels
        .get(&thread_id)
        .expect("origin thread channel")
        .store
        .lock()
        .await
        .input_state
        .clone();
    pretty_assertions::assert_eq!(actual, Some(expected));
}

#[tokio::test]
async fn unsupported_server_queue_requeues_the_message_locally() {
    Box::pin(async {
        let mut app = make_test_app().await;
        let mut app_server = Box::pin(crate::start_embedded_app_server_for_picker(
            app.chat_widget.config_ref(),
        ))
        .await
        .expect("embedded app server");
        let started = app_server
            .start_thread(app.chat_widget.config_ref())
            .await
            .expect("thread/start should succeed");
        let thread_id = started.session.thread_id;
        assert!(!app_server.thread_queue_supported(thread_id).await);
        app.enqueue_primary_thread_session(started.session, started.turns)
            .await
            .expect("primary thread should be registered");
        app.chat_widget.handle_server_notification(
            turn_started_notification(thread_id, "active-turn"),
            /*replay_kind*/ None,
        );
        let op = AppCommand::QueueUserMessage {
            submission: codex_app_server_protocol::TurnSubmissionParams::default(),
            fallback_user_message: UserMessage::from("use the local fallback"),
        };

        assert!(
            app.try_submit_active_thread_op_via_app_server(&mut app_server, thread_id, &op)
                .await
                .expect("queue fallback should not fail")
        );
        pretty_assertions::assert_eq!(
            app.chat_widget.queued_user_message_texts(),
            vec!["use the local fallback"]
        );
    })
    .await;
}
