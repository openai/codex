use super::*;
use crate::chatwidget::realtime::tests::activate_voice_for_thread;
use pretty_assertions::assert_eq;

const ERROR_MESSAGE: &str = "Responses API returned misalignment_policy_violation";

#[tokio::test]
async fn misalignment_precaution_retires_voice_and_keeps_late_turn_blocked() {
    let (mut chat, _events, mut ops) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    handle_turn_started(&mut chat, "unsafe-turn");
    activate_voice_for_thread(&mut chat, thread_id);

    handle_error(
        &mut chat,
        ERROR_MESSAGE,
        Some(CodexErrorInfo::MisalignmentPolicyViolation),
    );

    assert!(!chat.realtime_conversation_is_running());
    assert_matches!(
        ops.try_recv(),
        Ok(Op::RealtimeConversationStop { thread_id: stopped }) if stopped == thread_id
    );
    assert!(ops.try_recv().is_err());
    assert!(chat.has_misalignment_policy_violation());
    assert!(!chat.bottom_pane.composer_input_enabled());
    assert!(chat.submit_op(Op::RealtimeConversationStop { thread_id }));
    assert_matches!(ops.try_recv(), Ok(Op::RealtimeConversationStop { .. }));

    // A handoff already in flight may still produce a TurnStarted notification.
    // It must not dismiss the precaution or restart local capture.
    handle_turn_started(&mut chat, "late-voice-turn");
    assert!(chat.has_misalignment_policy_violation());
    assert!(!chat.bottom_pane.composer_input_enabled());
    chat.toggle_realtime_conversation();
    assert!(!chat.realtime_conversation_is_running());
    assert!(ops.try_recv().is_err());

    chat.clear_misalignment_for_new_turn(
        "acknowledged-turn",
        MisalignmentTurnSource::AcknowledgedContinuation,
    );
    assert!(!chat.has_misalignment_policy_violation());
    assert!(chat.bottom_pane.composer_input_enabled());
}

#[tokio::test]
async fn misalignment_policy_failure_stops_the_thread_and_renders_once() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    handle_turn_started(&mut chat, "turn-1");
    chat.handle_server_notification(
        ServerNotification::ModelSafetyBufferingUpdated(ModelSafetyBufferingUpdatedNotification {
            thread_id: thread_id.to_string(),
            turn_id: "turn-1".to_string(),
            model: "current-model".to_string(),
            use_cases: Vec::new(),
            reasons: Vec::new(),
            show_buffering_ui: true,
            faster_model: None,
        }),
        /*replay_kind*/ None,
    );
    assert!(
        render_bottom_popup(&chat, /*width*/ 80)
            .contains("Giving this request a little extra thought")
    );
    chat.queue_user_message(UserMessage::from("queued follow-up"));
    chat.bottom_pane
        .set_composer_text("stale draft".to_string(), Vec::new(), Vec::new());
    drain_insert_history(&mut rx);

    handle_error(
        &mut chat,
        ERROR_MESSAGE,
        Some(CodexErrorInfo::MisalignmentPolicyViolation),
    );

    assert!(!chat.bottom_pane.is_task_running());
    assert!(!chat.bottom_pane.composer_input_enabled());
    assert!(chat.queued_user_message_texts().is_empty());
    assert_eq!(chat.bottom_pane.composer_text_with_pending(), "");

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert_chatwidget_snapshot!("misalignment_policy_violation", popup);
    assert_chatwidget_snapshot!(
        "misalignment_policy_violation_narrow",
        render_bottom_popup(&chat, /*width*/ 38)
    );

    chat.submit_user_message(UserMessage::from("direct follow-up"));
    chat.queue_user_message(UserMessage::from("queued follow-up"));
    assert!(!chat.submit_op(Op::Compact));
    assert!(!chat.submit_op(Op::run_user_shell_command("echo unsafe".to_string())));
    assert!(!chat.maybe_send_next_queued_input());
    assert!(chat.queued_user_message_texts().is_empty());
    assert_no_submit_op(&mut op_rx);

    chat.handle_server_request(
        ServerRequest::ToolRequestUserInput {
            request_id: AppServerRequestId::Integer(1),
            params: ToolRequestUserInputParams {
                thread_id: thread_id.to_string(),
                turn_id: "turn-1".to_string(),
                item_id: "input-1".to_string(),
                questions: Vec::new(),
                is_blocking: true,
                auto_resolution_ms: None,
            },
        },
        /*replay_kind*/ None,
    );

    chat.handle_server_notification(
        ServerNotification::TurnCompleted(TurnCompletedNotification {
            thread_id: thread_id.to_string(),
            turn: app_server_turn(
                "turn-1",
                AppServerTurnStatus::Failed,
                /*duration_ms*/ None,
                Some(AppServerTurnError {
                    misalignment: None,
                    message: ERROR_MESSAGE.to_string(),
                    codex_error_info: Some(CodexErrorInfo::MisalignmentPolicyViolation),
                    additional_details: None,
                }),
            ),
        }),
        /*replay_kind*/ None,
    );

    assert_eq!(render_bottom_popup(&chat, /*width*/ 80), popup);
    assert!(drain_insert_history(&mut rx).is_empty());

    chat.handle_key_event(KeyEvent::from(KeyCode::Esc));
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    assert_matches!(rx.try_recv(), Ok(AppEvent::NewSession { name: None }));
    assert!(rx.try_recv().is_err());

    chat.show_misalignment_policy_precaution();
    assert_eq!(render_bottom_popup(&chat, /*width*/ 80), popup);
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    assert_matches!(rx.try_recv(), Ok(AppEvent::NewSession { name: None }));

    chat.show_misalignment_policy_precaution();
    chat.handle_key_event(KeyEvent::from(KeyCode::Down));
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    assert_matches!(rx.try_recv(), Ok(AppEvent::OpenResumePicker));
    assert!(chat.bottom_pane.has_active_view());

    chat.remote_connection = Some(crate::status::remote_connection::RemoteConnectionStatus {
        address: "wss://remote.example.com".to_string(),
        version: "v1.0.0".to_string(),
        is_local_daemon: false,
    });
    chat.show_misalignment_policy_precaution();
    assert_chatwidget_snapshot!(
        "misalignment_policy_violation_remote",
        render_bottom_popup(&chat, /*width*/ 80)
    );
    chat.handle_key_event(KeyEvent::from(KeyCode::Down));
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    assert_matches!(rx.try_recv(), Ok(AppEvent::OpenAgentsOverview));
    assert!(chat.bottom_pane.has_active_view());
}

fn review_details() -> codex_app_server_protocol::MisalignmentErrorDetails {
    codex_app_server_protocol::MisalignmentErrorDetails {
        error_type: Some("new_category".to_string()),
        detailed_explanation: Some(
            "The agent proposed a change outside your instructions.".to_string(),
        ),
        steer: Some(codex_app_server_protocol::MisalignmentSteer {
            message: "Continue with the requested change only.".to_string(),
        }),
    }
}

fn select_review(
    chat: &mut ChatWidget,
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
) -> Arc<MisalignmentReview> {
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    std::iter::from_fn(|| rx.try_recv().ok())
        .find_map(|event| match event {
            AppEvent::ReviewMisalignment(review) => Some(review),
            _ => None,
        })
        .expect("review action")
}

#[tokio::test]
async fn misalignment_review_requires_current_findings() {
    let (mut chat, mut rx, _) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    handle_turn_started(&mut chat, "turn-1");
    chat.on_misalignment_error(Some("turn-1".to_string()), Some(review_details()));
    let review = select_review(&mut chat, &mut rx);
    chat.show_misalignment_review_confirmation(Arc::clone(&review));
    assert_chatwidget_snapshot!(
        "misalignment_review_narrow",
        render_bottom_popup(&chat, /*width*/ 38)
    );
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    assert!(
        matches!(rx.try_recv(), Ok(AppEvent::ContinueMisalignment(current)) if Arc::ptr_eq(&current, &review))
    );

    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    assert!(rx.try_recv().is_err());

    // A duplicate terminal event must not reset an in-progress review.
    chat.on_misalignment_error(Some("turn-1".to_string()), Some(review_details()));
    chat.on_misalignment_error(Some("turn-1".to_string()), /*details*/ None);
    assert!(chat.is_current_misalignment_review(&review));
    let mut changed = review_details();
    changed.detailed_explanation = Some("Updated findings".to_string());
    chat.on_misalignment_error(Some("turn-1".to_string()), Some(changed));
    assert!(!chat.is_current_misalignment_review(&review));

    // Present-but-invalid findings withdraw the old authorization.
    for explanation in [None, Some(" ".to_string()), Some("a".repeat(65537))] {
        chat.on_misalignment_error(Some("turn-1".to_string()), Some(review_details()));
        let replacement = select_review(&mut chat, &mut rx);
        let mut details = review_details();
        details.detailed_explanation = explanation;
        chat.on_misalignment_error(Some("turn-1".to_string()), Some(details));
        assert!(!chat.is_current_misalignment_review(&replacement));
    }

    for steer in [None, Some(" ".to_string()), Some("a".repeat(1025))] {
        let mut details = review_details();
        details.steer =
            steer.map(|message| codex_app_server_protocol::MisalignmentSteer { message });
        chat.on_misalignment_error(Some("turn-1".to_string()), Some(details));
        let review = select_review(&mut chat, &mut rx);
        chat.show_misalignment_review_confirmation(Arc::clone(&review));
        assert!(review.continuation_message().is_none());
        chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
        assert!(
            !std::iter::from_fn(|| rx.try_recv().ok())
                .any(|event| matches!(event, AppEvent::ContinueMisalignment(_)))
        );
        assert!(!chat.bottom_pane.composer_input_enabled());
    }
    handle_turn_started(&mut chat, "turn-2");
    chat.on_misalignment_error(Some("turn-1".to_string()), Some(review_details()));
    assert!(!chat.has_misalignment_policy_violation());
    assert!(chat.bottom_pane.composer_input_enabled());
    // A queued Back action cannot reopen the precaution after a new turn starts.
    chat.show_misalignment_policy_precaution();
    assert!(!chat.bottom_pane.has_active_view());
}
