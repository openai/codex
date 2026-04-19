use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn interrupted_turn_restores_queued_messages_with_images_and_elements() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    let first_placeholder = "[Image #1]";
    let first_text = format!("{first_placeholder} first");
    let first_elements = vec![TextElement::new(
        (0..first_placeholder.len()).into(),
        Some(first_placeholder.to_string()),
    )];
    let first_images = [PathBuf::from("/tmp/first.png")];

    let second_placeholder = "[Image #1]";
    let second_text = format!("{second_placeholder} second");
    let second_elements = vec![TextElement::new(
        (0..second_placeholder.len()).into(),
        Some(second_placeholder.to_string()),
    )];
    let second_images = [PathBuf::from("/tmp/second.png")];

    let existing_placeholder = "[Image #1]";
    let existing_text = format!("{existing_placeholder} existing");
    let existing_elements = vec![TextElement::new(
        (0..existing_placeholder.len()).into(),
        Some(existing_placeholder.to_string()),
    )];
    let existing_images = vec![PathBuf::from("/tmp/existing.png")];

    chat.queued_user_messages.push_back(UserMessage {
        text: first_text,
        local_images: vec![LocalImageAttachment {
            placeholder: first_placeholder.to_string(),
            path: first_images[0].clone(),
        }],
        remote_image_urls: Vec::new(),
        text_elements: first_elements,
        mention_bindings: Vec::new(),
    });
    chat.queued_user_messages.push_back(UserMessage {
        text: second_text,
        local_images: vec![LocalImageAttachment {
            placeholder: second_placeholder.to_string(),
            path: second_images[0].clone(),
        }],
        remote_image_urls: Vec::new(),
        text_elements: second_elements,
        mention_bindings: Vec::new(),
    });
    chat.refresh_pending_input_preview();

    chat.bottom_pane
        .set_composer_text(existing_text, existing_elements, existing_images.clone());

    // When interrupted, queued messages are merged into the composer; image placeholders
    // must be renumbered to match the combined local image list.
    chat.handle_codex_event(Event {
        id: "interrupt".into(),
        msg: EventMsg::TurnAborted(codex_protocol::protocol::TurnAbortedEvent {
            turn_id: Some("turn-1".to_string()),
            reason: TurnAbortReason::Interrupted,
            completed_at: None,
            duration_ms: None,
        }),
    });

    let first = "[Image #1] first".to_string();
    let second = "[Image #2] second".to_string();
    let third = "[Image #3] existing".to_string();
    let expected_text = format!("{first}\n{second}\n{third}");
    assert_eq!(chat.bottom_pane.composer_text(), expected_text);

    let first_start = 0;
    let second_start = first.len() + 1;
    let third_start = second_start + second.len() + 1;
    let expected_elements = vec![
        TextElement::new(
            (first_start..first_start + "[Image #1]".len()).into(),
            Some("[Image #1]".to_string()),
        ),
        TextElement::new(
            (second_start..second_start + "[Image #2]".len()).into(),
            Some("[Image #2]".to_string()),
        ),
        TextElement::new(
            (third_start..third_start + "[Image #3]".len()).into(),
            Some("[Image #3]".to_string()),
        ),
    ];
    assert_eq!(chat.bottom_pane.composer_text_elements(), expected_elements);
    assert_eq!(
        chat.bottom_pane.composer_local_image_paths(),
        vec![
            first_images[0].clone(),
            second_images[0].clone(),
            existing_images[0].clone(),
        ]
    );
}

/// Entering review mode uses the hint provided by the review request.
#[tokio::test]
async fn entered_review_mode_uses_request_hint() {
    let (mut chat, mut rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_codex_event(Event {
        id: "review-start".into(),
        msg: EventMsg::EnteredReviewMode(ReviewRequest {
            target: ReviewTarget::BaseBranch {
                branch: "feature".to_string(),
            },
            user_facing_hint: Some("feature branch".to_string()),
        }),
    });

    let cells = drain_insert_history(&mut rx);
    let banner = lines_to_single_string(cells.last().expect("review banner"));
    assert_eq!(banner, ">> Code review started: feature branch <<\n");
    assert!(chat.is_review_mode);
}

/// Entering review mode renders the current changes banner when requested.
#[tokio::test]
async fn entered_review_mode_defaults_to_current_changes_banner() {
    let (mut chat, mut rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_codex_event(Event {
        id: "review-start".into(),
        msg: EventMsg::EnteredReviewMode(ReviewRequest {
            target: ReviewTarget::UncommittedChanges,
            user_facing_hint: None,
        }),
    });

    let cells = drain_insert_history(&mut rx);
    let banner = lines_to_single_string(cells.last().expect("review banner"));
    assert_eq!(banner, ">> Code review started: current changes <<\n");
    assert!(chat.is_review_mode);
}

#[tokio::test]
async fn steer_rejection_queues_review_follow_up_before_existing_queued_messages() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.handle_codex_event(Event {
        id: "turn-start".into(),
        msg: EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: "turn-1".to_string(),
            started_at: None,
            model_context_window: None,
            collaboration_mode_kind: ModeKind::Default,
        }),
    });
    chat.handle_codex_event(Event {
        id: "review-start".into(),
        msg: EventMsg::EnteredReviewMode(ReviewRequest {
            target: ReviewTarget::BaseBranch {
                branch: "feature".to_string(),
            },
            user_facing_hint: Some("feature branch".to_string()),
        }),
    });
    let _ = drain_insert_history(&mut rx);
    chat.queued_user_messages
        .push_back(UserMessage::from("queued later"));

    chat.submit_user_message(UserMessage::from("review follow-up one"));
    chat.submit_user_message(UserMessage::from("review follow-up two"));

    assert_eq!(chat.pending_steers.len(), 2);
    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "review follow-up one".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected running-turn steer submit, got {other:?}"),
    }
    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "review follow-up two".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected second running-turn steer submit, got {other:?}"),
    }

    chat.handle_codex_event(Event {
        id: "steer-rejected-1".into(),
        msg: EventMsg::Error(ErrorEvent {
            message: "cannot steer a review turn".to_string(),
            codex_error_info: Some(CodexErrorInfo::ActiveTurnNotSteerable {
                turn_kind: NonSteerableTurnKind::Review,
            }),
        }),
    });
    chat.handle_codex_event(Event {
        id: "steer-rejected-2".into(),
        msg: EventMsg::Error(ErrorEvent {
            message: "cannot steer a review turn".to_string(),
            codex_error_info: Some(CodexErrorInfo::ActiveTurnNotSteerable {
                turn_kind: NonSteerableTurnKind::Review,
            }),
        }),
    });

    assert!(chat.pending_steers.is_empty());
    assert_eq!(
        chat.queued_user_message_texts(),
        vec![
            "review follow-up one",
            "review follow-up two",
            "queued later"
        ]
    );
    assert!(drain_insert_history(&mut rx).is_empty());

    chat.handle_codex_event(Event {
        id: "review-exit".into(),
        msg: EventMsg::ExitedReviewMode(ExitedReviewModeEvent {
            review_output: None,
        }),
    });
    chat.handle_codex_event(Event {
        id: "turn-complete".into(),
        msg: EventMsg::TurnComplete(TurnCompleteEvent {
            turn_id: "turn-1".to_string(),
            last_agent_message: None,
            completed_at: None,
            duration_ms: None,
        }),
    });

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "review follow-up one\nreview follow-up two".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected merged rejected-steer follow-up submit, got {other:?}"),
    }

    chat.handle_codex_event(Event {
        id: "turn-complete-2".into(),
        msg: EventMsg::TurnComplete(TurnCompleteEvent {
            turn_id: "turn-2".to_string(),
            last_agent_message: None,
            completed_at: None,
            duration_ms: None,
        }),
    });

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "queued later".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected queued draft submit after rejected steers, got {other:?}"),
    }
}

#[tokio::test]
async fn live_agent_message_renders_during_review_mode() {
    let (mut chat, mut rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_codex_event(Event {
        id: "review-start".into(),
        msg: EventMsg::EnteredReviewMode(ReviewRequest {
            target: ReviewTarget::UncommittedChanges,
            user_facing_hint: None,
        }),
    });
    let _ = drain_insert_history(&mut rx);

    chat.handle_codex_event(Event {
        id: "review-message".into(),
        msg: EventMsg::AgentMessage(AgentMessageEvent {
            message: "Review progress update".to_string(),
            phase: None,
            memory_citation: None,
        }),
    });

    let inserted = drain_insert_history(&mut rx);
    assert_eq!(inserted.len(), 1);
    assert!(lines_to_single_string(&inserted[0]).contains("Review progress update"));
}

/// Exiting review restores the pre-review context window indicator.
#[tokio::test]
async fn review_restores_context_window_indicator() {
    let (mut chat, mut rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;

    let context_window = 13_000;
    let pre_review_tokens = 12_700; // ~30% remaining after subtracting baseline.
    let review_tokens = 12_030; // ~97% remaining after subtracting baseline.

    chat.handle_codex_event(Event {
        id: "token-before".into(),
        msg: EventMsg::TokenCount(TokenCountEvent {
            info: Some(make_token_info(pre_review_tokens, context_window)),
            rate_limits: None,
        }),
    });
    assert_eq!(chat.bottom_pane.context_window_percent(), Some(30));

    chat.handle_codex_event(Event {
        id: "review-start".into(),
        msg: EventMsg::EnteredReviewMode(ReviewRequest {
            target: ReviewTarget::BaseBranch {
                branch: "feature".to_string(),
            },
            user_facing_hint: Some("feature branch".to_string()),
        }),
    });

    chat.handle_codex_event(Event {
        id: "token-review".into(),
        msg: EventMsg::TokenCount(TokenCountEvent {
            info: Some(make_token_info(review_tokens, context_window)),
            rate_limits: None,
        }),
    });
    assert_eq!(chat.bottom_pane.context_window_percent(), Some(97));

    chat.handle_codex_event(Event {
        id: "review-end".into(),
        msg: EventMsg::ExitedReviewMode(ExitedReviewModeEvent {
            review_output: None,
        }),
    });
    let _ = drain_insert_history(&mut rx);

    assert_eq!(chat.bottom_pane.context_window_percent(), Some(30));
    assert!(!chat.is_review_mode);
}

#[tokio::test]
async fn restore_thread_input_state_restores_pending_steers_without_downgrading_them() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let mut pending_steers = VecDeque::new();
    pending_steers.push_back(UserMessage::from("pending steer"));
    let mut rejected_steers_queue = VecDeque::new();
    rejected_steers_queue.push_back(UserMessage::from("already rejected"));
    let mut queued_user_messages = VecDeque::new();
    queued_user_messages.push_back(UserMessage::from("queued draft"));

    chat.restore_thread_input_state(Some(ThreadInputState {
        composer: None,
        pending_steers,
        pending_steer_history_records: VecDeque::new(),
        rejected_steers_queue,
        rejected_steer_history_records: VecDeque::new(),
        queued_user_messages,
        queued_user_message_history_records: VecDeque::new(),
        current_collaboration_mode: chat.current_collaboration_mode.clone(),
        active_collaboration_mask: chat.active_collaboration_mask.clone(),
        task_running: false,
        agent_turn_running: false,
    }));

    assert_eq!(
        chat.queued_user_message_texts(),
        vec!["already rejected", "queued draft"]
    );
    assert_eq!(chat.pending_steers.len(), 1);
    assert_eq!(
        chat.pending_steers.front().unwrap().user_message.text,
        "pending steer"
    );
}

#[tokio::test]
async fn steer_enter_queues_while_plan_stream_is_active() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
    let plan_mask = collaboration_modes::mask_for_kind(chat.model_catalog.as_ref(), ModeKind::Plan)
        .expect("expected plan collaboration mask");
    chat.set_collaboration_mask(plan_mask);
    chat.on_task_started();
    chat.on_plan_delta("- Step 1".to_string());
    let _ = drain_insert_history(&mut rx);

    chat.bottom_pane
        .set_composer_text("queued submission".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_eq!(chat.active_collaboration_mode_kind(), ModeKind::Plan);
    assert_eq!(chat.queued_user_messages.len(), 1);
    assert_eq!(
        chat.queued_user_messages.front().unwrap().text,
        "queued submission"
    );
    assert!(chat.pending_steers.is_empty());
    assert_no_submit_op(&mut op_rx);
    assert!(drain_insert_history(&mut rx).is_empty());
}

#[tokio::test]
async fn steer_enter_uses_pending_steers_while_turn_is_running_without_streaming() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.on_task_started();

    chat.bottom_pane
        .set_composer_text("queued while running".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(chat.queued_user_messages.is_empty());
    assert_eq!(chat.pending_steers.len(), 1);
    assert_eq!(
        chat.pending_steers.front().unwrap().user_message.text,
        "queued while running"
    );
    match next_submit_op(&mut op_rx) {
        Op::UserTurn { .. } => {}
        other => panic!("expected Op::UserTurn, got {other:?}"),
    }
    assert!(drain_insert_history(&mut rx).is_empty());

    complete_user_message(&mut chat, "user-1", "queued while running");

    assert!(chat.pending_steers.is_empty());
    let inserted = drain_insert_history(&mut rx);
    assert_eq!(inserted.len(), 1);
    assert!(lines_to_single_string(&inserted[0]).contains("queued while running"));
}

#[tokio::test]
async fn steer_enter_uses_pending_steers_while_final_answer_stream_is_active() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.on_task_started();
    // Keep the assistant stream open (no commit tick/finalize) to model the repro window:
    // user presses Enter while the final answer is still streaming.
    chat.on_agent_message_delta("Final answer line\n".to_string());

    chat.bottom_pane.set_composer_text(
        "queued while streaming".to_string(),
        Vec::new(),
        Vec::new(),
    );
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(chat.queued_user_messages.is_empty());
    assert_eq!(chat.pending_steers.len(), 1);
    assert_eq!(
        chat.pending_steers.front().unwrap().user_message.text,
        "queued while streaming"
    );
    match next_submit_op(&mut op_rx) {
        Op::UserTurn { .. } => {}
        other => panic!("expected Op::UserTurn, got {other:?}"),
    }
    assert!(drain_insert_history(&mut rx).is_empty());

    complete_user_message(&mut chat, "user-1", "queued while streaming");

    assert!(chat.pending_steers.is_empty());
    let inserted = drain_insert_history(&mut rx);
    assert_eq!(inserted.len(), 1);
    assert!(lines_to_single_string(&inserted[0]).contains("queued while streaming"));
}

#[tokio::test]
async fn failed_pending_steer_submit_does_not_add_pending_preview() {
    let (mut chat, mut rx, op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.on_task_started();
    drop(op_rx);

    chat.bottom_pane.set_composer_text(
        "queued while streaming".to_string(),
        Vec::new(),
        Vec::new(),
    );
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(chat.pending_steers.is_empty());
    assert!(chat.queued_user_messages.is_empty());
    assert!(drain_insert_history(&mut rx).is_empty());
}

#[tokio::test]
async fn item_completed_only_pops_front_pending_steer() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.pending_steers.push_back(pending_steer("first"));
    chat.pending_steers.push_back(pending_steer("second"));
    chat.refresh_pending_input_preview();

    complete_user_message(&mut chat, "user-other", "other");

    assert_eq!(chat.pending_steers.len(), 2);
    assert_eq!(
        chat.pending_steers.front().unwrap().user_message.text,
        "first"
    );
    let inserted = drain_insert_history(&mut rx);
    assert_eq!(inserted.len(), 1);
    assert!(lines_to_single_string(&inserted[0]).contains("other"));

    complete_user_message(&mut chat, "user-first", "first");

    assert_eq!(chat.pending_steers.len(), 1);
    assert_eq!(
        chat.pending_steers.front().unwrap().user_message.text,
        "second"
    );
    let inserted = drain_insert_history(&mut rx);
    assert_eq!(inserted.len(), 1);
    assert!(lines_to_single_string(&inserted[0]).contains("first"));
}

#[tokio::test(flavor = "multi_thread")]
async fn item_completed_pops_pending_steer_with_local_image_and_text_elements() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.on_task_started();

    let temp = tempdir().expect("tempdir");
    let image_path = temp.path().join("pending-steer.png");
    const TINY_PNG_BYTES: &[u8] = &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6,
        0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 11, 73, 68, 65, 84, 120, 156, 99, 96, 0, 2, 0, 0, 5, 0,
        1, 122, 94, 171, 63, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];
    std::fs::write(&image_path, TINY_PNG_BYTES).expect("write image");

    let text = "note".to_string();
    let text_elements = vec![TextElement::new((0..4).into(), Some("note".to_string()))];
    chat.submit_user_message(UserMessage {
        text: text.clone(),
        local_images: vec![LocalImageAttachment {
            placeholder: "[Image #1]".to_string(),
            path: image_path,
        }],
        remote_image_urls: Vec::new(),
        text_elements,
        mention_bindings: Vec::new(),
    });

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { .. } => {}
        other => panic!("expected Op::UserTurn, got {other:?}"),
    }

    assert_eq!(chat.pending_steers.len(), 1);
    let pending = chat.pending_steers.front().unwrap();
    assert_eq!(pending.user_message.local_images.len(), 1);
    assert_eq!(pending.user_message.text_elements.len(), 1);
    assert_eq!(pending.compare_key.message, text);
    assert_eq!(pending.compare_key.image_count, 1);

    complete_user_message_for_inputs(
        &mut chat,
        "user-1",
        vec![
            UserInput::Image {
                image_url: "data:image/png;base64,placeholder".to_string(),
            },
            UserInput::Text {
                text,
                text_elements: Vec::new(),
            },
        ],
    );

    assert!(chat.pending_steers.is_empty());

    let mut user_cell = None;
    while let Ok(ev) = rx.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = ev
            && let Some(cell) = cell.as_any().downcast_ref::<UserHistoryCell>()
        {
            user_cell = Some((
                cell.message.clone(),
                cell.text_elements.clone(),
                cell.local_image_paths.clone(),
                cell.remote_image_urls.clone(),
            ));
            break;
        }
    }

    let (stored_message, stored_elements, stored_images, stored_remote_image_urls) =
        user_cell.expect("expected pending steer user history cell");
    assert_eq!(stored_message, "note");
    assert_eq!(
        stored_elements,
        vec![TextElement::new((0..4).into(), Some("note".to_string()))]
    );
    assert_eq!(stored_images.len(), 1);
    assert!(stored_images[0].ends_with("pending-steer.png"));
    assert!(stored_remote_image_urls.is_empty());
}

#[tokio::test]
async fn steer_enter_during_final_stream_preserves_follow_up_prompts_in_order() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.on_task_started();
    // Simulate "dead mode" repro timing by keeping a final-answer stream active while the
    // user submits multiple follow-up prompts.
    chat.on_agent_message_delta("Final answer line\n".to_string());

    chat.bottom_pane
        .set_composer_text("first follow-up".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    chat.bottom_pane
        .set_composer_text("second follow-up".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(chat.queued_user_messages.is_empty());
    assert_eq!(chat.pending_steers.len(), 2);
    assert_eq!(
        chat.pending_steers.front().unwrap().user_message.text,
        "first follow-up"
    );
    assert_eq!(
        chat.pending_steers.back().unwrap().user_message.text,
        "second follow-up"
    );

    let first_items = match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => items,
        other => panic!("expected Op::UserTurn, got {other:?}"),
    };
    assert_eq!(
        first_items,
        vec![UserInput::Text {
            text: "first follow-up".to_string(),
            text_elements: Vec::new(),
        }]
    );
    let second_items = match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => items,
        other => panic!("expected Op::UserTurn, got {other:?}"),
    };
    assert_eq!(
        second_items,
        vec![UserInput::Text {
            text: "second follow-up".to_string(),
            text_elements: Vec::new(),
        }]
    );
    assert!(drain_insert_history(&mut rx).is_empty());

    complete_user_message(&mut chat, "user-1", "first follow-up");

    assert_eq!(chat.pending_steers.len(), 1);
    assert_eq!(
        chat.pending_steers.front().unwrap().user_message.text,
        "second follow-up"
    );
    let first_insert = drain_insert_history(&mut rx);
    assert_eq!(first_insert.len(), 1);
    assert!(lines_to_single_string(&first_insert[0]).contains("first follow-up"));

    complete_user_message(&mut chat, "user-2", "second follow-up");

    assert!(chat.pending_steers.is_empty());
    let second_insert = drain_insert_history(&mut rx);
    assert_eq!(second_insert.len(), 1);
    assert!(lines_to_single_string(&second_insert[0]).contains("second follow-up"));
}

#[tokio::test]
async fn manual_interrupt_restores_pending_steers_to_composer() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.on_task_started();
    chat.on_agent_message_delta(
        "Final answer line
"
        .to_string(),
    );

    chat.bottom_pane.set_composer_text(
        "queued while streaming".to_string(),
        Vec::new(),
        Vec::new(),
    );
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_eq!(chat.pending_steers.len(), 1);
    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "queued while streaming".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected Op::UserTurn, got {other:?}"),
    }
    assert!(drain_insert_history(&mut rx).is_empty());

    chat.on_interrupted_turn(TurnAbortReason::Interrupted);

    assert!(chat.pending_steers.is_empty());
    assert_eq!(chat.bottom_pane.composer_text(), "queued while streaming");
    assert_no_submit_op(&mut op_rx);

    let inserted = drain_insert_history(&mut rx);
    assert!(
        inserted
            .iter()
            .all(|cell| !lines_to_single_string(cell).contains("queued while streaming"))
    );
}

#[tokio::test]
async fn esc_interrupt_sends_all_pending_steers_immediately_and_keeps_existing_draft() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.on_task_started();
    chat.on_agent_message_delta("Final answer line\n".to_string());

    chat.bottom_pane
        .set_composer_text("first pending steer".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "first pending steer".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected Op::UserTurn, got {other:?}"),
    }

    chat.bottom_pane
        .set_composer_text("second pending steer".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "second pending steer".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected Op::UserTurn, got {other:?}"),
    }

    chat.queued_user_messages
        .push_back(UserMessage::from("queued draft".to_string()));
    chat.refresh_pending_input_preview();
    chat.bottom_pane
        .set_composer_text("still editing".to_string(), Vec::new(), Vec::new());

    chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    next_interrupt_op(&mut op_rx);

    chat.on_interrupted_turn(TurnAbortReason::Interrupted);

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "first pending steer\nsecond pending steer".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected merged pending steers to submit, got {other:?}"),
    }

    assert!(chat.pending_steers.is_empty());
    assert_eq!(chat.bottom_pane.composer_text(), "still editing");
    assert_eq!(chat.queued_user_messages.len(), 1);
    assert_eq!(
        chat.queued_user_messages.front().unwrap().text,
        "queued draft"
    );

    let inserted = drain_insert_history(&mut rx);
    assert!(
        inserted
            .iter()
            .any(|cell| lines_to_single_string(cell).contains("first pending steer"))
    );
    assert!(
        inserted
            .iter()
            .any(|cell| lines_to_single_string(cell).contains("second pending steer"))
    );
}

#[tokio::test]
async fn esc_with_pending_steers_overrides_agent_command_interrupt_behavior() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.on_task_started();

    chat.bottom_pane
        .set_composer_text("pending steer".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    match next_submit_op(&mut op_rx) {
        Op::UserTurn { .. } => {}
        other => panic!("expected Op::UserTurn, got {other:?}"),
    }

    chat.bottom_pane
        .set_composer_text("/agent ".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

    next_interrupt_op(&mut op_rx);
    assert_eq!(chat.bottom_pane.composer_text(), "/agent ");
}

#[tokio::test]
async fn manual_interrupt_restores_pending_steer_mention_bindings_to_composer() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.on_task_started();
    chat.on_agent_message_delta("Final answer line\n".to_string());

    let mention_bindings = vec![MentionBinding {
        mention: "figma".to_string(),
        path: "/tmp/skills/figma/SKILL.md".to_string(),
    }];
    chat.bottom_pane.set_composer_text_with_mention_bindings(
        "please use $figma".to_string(),
        vec![TextElement::new(
            (11..17).into(),
            Some("$figma".to_string()),
        )],
        Vec::new(),
        mention_bindings.clone(),
    );
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "please use $figma".to_string(),
                text_elements: vec![TextElement::new(
                    (11..17).into(),
                    Some("$figma".to_string()),
                )],
            }]
        ),
        other => panic!("expected Op::UserTurn, got {other:?}"),
    }

    chat.on_interrupted_turn(TurnAbortReason::Interrupted);

    assert_eq!(chat.bottom_pane.composer_text(), "please use $figma");
    assert_eq!(chat.bottom_pane.take_mention_bindings(), mention_bindings);
    assert_no_submit_op(&mut op_rx);
}

#[tokio::test]
async fn manual_interrupt_restores_pending_steers_before_queued_messages() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.on_task_started();
    chat.on_agent_message_delta(
        "Final answer line
"
        .to_string(),
    );

    chat.bottom_pane
        .set_composer_text("pending steer".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    chat.queued_user_messages
        .push_back(UserMessage::from("queued draft".to_string()));
    chat.refresh_pending_input_preview();

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "pending steer".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected Op::UserTurn, got {other:?}"),
    }
    assert!(drain_insert_history(&mut rx).is_empty());

    chat.on_interrupted_turn(TurnAbortReason::Interrupted);

    assert!(chat.pending_steers.is_empty());
    assert!(chat.queued_user_messages.is_empty());
    assert_eq!(
        chat.bottom_pane.composer_text(),
        "pending steer
queued draft"
    );
    assert_no_submit_op(&mut op_rx);
}

#[tokio::test]
async fn replaced_turn_clears_pending_steers_but_keeps_queued_drafts() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.on_task_started();
    chat.on_agent_message_delta(
        "Final answer line
"
        .to_string(),
    );

    chat.bottom_pane
        .set_composer_text("pending steer".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    chat.queued_user_messages
        .push_back(UserMessage::from("queued draft".to_string()));
    chat.refresh_pending_input_preview();

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "pending steer".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected Op::UserTurn, got {other:?}"),
    }
    assert!(drain_insert_history(&mut rx).is_empty());

    chat.handle_codex_event(Event {
        id: "replaced".into(),
        msg: EventMsg::TurnAborted(codex_protocol::protocol::TurnAbortedEvent {
            turn_id: Some("turn-1".to_string()),
            reason: TurnAbortReason::Replaced,
            completed_at: None,
            duration_ms: None,
        }),
    });

    assert!(chat.pending_steers.is_empty());
    assert!(chat.queued_user_messages.is_empty());
    assert_eq!(chat.bottom_pane.composer_text(), "");
    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "queued draft".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected queued draft Op::UserTurn, got {other:?}"),
    }
}

#[tokio::test]
async fn ctrl_c_shutdown_works_with_caps_lock() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_key_event(KeyEvent::new(KeyCode::Char('C'), KeyModifiers::CONTROL));

    assert_matches!(rx.try_recv(), Ok(AppEvent::Exit(ExitMode::ShutdownFirst)));
}

#[tokio::test]
async fn ctrl_c_interrupts_without_arming_quit_when_double_press_disabled() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.bottom_pane.set_task_running(true);

    chat.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));

    next_interrupt_op(&mut op_rx);
    assert_matches!(rx.try_recv(), Err(TryRecvError::Empty));
    assert!(!chat.bottom_pane.quit_shortcut_hint_visible());

    chat.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));

    next_interrupt_op(&mut op_rx);
    assert_matches!(rx.try_recv(), Err(TryRecvError::Empty));
    assert!(!chat.bottom_pane.quit_shortcut_hint_visible());
}

#[tokio::test]
async fn ctrl_c_closes_realtime_conversation_before_interrupt_or_quit() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.realtime_conversation.phase = RealtimeConversationPhase::Active;
    chat.bottom_pane
        .set_composer_text("recording meter".to_string(), Vec::new(), Vec::new());

    chat.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));

    next_realtime_close_op(&mut op_rx);
    assert_eq!(
        chat.realtime_conversation.phase,
        RealtimeConversationPhase::Stopping
    );
    assert_eq!(chat.bottom_pane.composer_text(), "recording meter");
    assert!(!chat.bottom_pane.quit_shortcut_hint_visible());
    assert_matches!(rx.try_recv(), Err(TryRecvError::Empty));
}

#[tokio::test]
async fn ctrl_c_cleared_prompt_is_recoverable_via_history() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.bottom_pane.insert_str("draft message ");
    chat.bottom_pane
        .attach_image(PathBuf::from("/tmp/preview.png"));
    let placeholder = "[Image #1]";
    assert!(
        chat.bottom_pane.composer_text().ends_with(placeholder),
        "expected placeholder {placeholder:?} in composer text"
    );

    chat.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(chat.bottom_pane.composer_text().is_empty());
    assert_matches!(op_rx.try_recv(), Err(TryRecvError::Empty));
    assert!(!chat.bottom_pane.quit_shortcut_hint_visible());

    chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    let restored_text = chat.bottom_pane.composer_text();
    assert!(
        restored_text.ends_with(placeholder),
        "expected placeholder {placeholder:?} after history recall"
    );
    assert!(restored_text.starts_with("draft message "));
    assert!(!chat.bottom_pane.quit_shortcut_hint_visible());

    let images = chat.bottom_pane.take_recent_submission_images();
    assert_eq!(vec![PathBuf::from("/tmp/preview.png")], images);
}

/// Selecting the custom prompt option from the review popup sends
/// OpenReviewCustomPrompt to the app event channel.
#[tokio::test]
async fn review_popup_custom_prompt_action_sends_event() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    // Open the preset selection popup
    chat.open_review_popup();

    // Move selection down to the fourth item: "Custom review instructions"
    chat.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    // Activate
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    // Drain events and ensure we saw the OpenReviewCustomPrompt request
    let mut found = false;
    while let Ok(ev) = rx.try_recv() {
        if let AppEvent::OpenReviewCustomPrompt = ev {
            found = true;
            break;
        }
    }
    assert!(found, "expected OpenReviewCustomPrompt event to be sent");
}

/// The commit picker shows only commit subjects (no timestamps).
#[tokio::test]
async fn review_commit_picker_shows_subjects_without_timestamps() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    // Open the Review presets parent popup.
    chat.open_review_popup();

    // Show commit picker with synthetic entries.
    let entries = vec![
        CommitLogEntry {
            sha: "1111111deadbeef".to_string(),
            timestamp: 0,
            subject: "Add new feature X".to_string(),
        },
        CommitLogEntry {
            sha: "2222222cafebabe".to_string(),
            timestamp: 0,
            subject: "Fix bug Y".to_string(),
        },
    ];
    super::show_review_commit_picker_with_entries(&mut chat, entries);

    // Render the bottom pane and inspect the lines for subjects and absence of time words.
    let width = 72;
    let height = chat.desired_height(width);
    let area = ratatui::layout::Rect::new(0, 0, width, height);
    let mut buf = ratatui::buffer::Buffer::empty(area);
    chat.render(area, &mut buf);

    let mut blob = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            let s = buf[(x, y)].symbol();
            if s.is_empty() {
                blob.push(' ');
            } else {
                blob.push_str(s);
            }
        }
        blob.push('\n');
    }

    assert!(
        blob.contains("Add new feature X"),
        "expected subject in output"
    );
    assert!(blob.contains("Fix bug Y"), "expected subject in output");

    // Ensure no relative-time phrasing is present.
    let lowered = blob.to_lowercase();
    assert!(
        !lowered.contains("ago")
            && !lowered.contains(" second")
            && !lowered.contains(" minute")
            && !lowered.contains(" hour")
            && !lowered.contains(" day"),
        "expected no relative time in commit picker output: {blob:?}"
    );
}

/// Submitting the custom prompt view sends Op::Review with the typed prompt
/// and uses the same text for the user-facing hint.
#[tokio::test]
async fn custom_prompt_submit_sends_review_op() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.show_review_custom_prompt();
    // Paste prompt text via ChatWidget handler, then submit
    chat.handle_paste("  please audit dependencies  ".to_string());
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    // Expect AppEvent::CodexOp(Op::Review { .. }) with trimmed prompt
    let evt = rx.try_recv().expect("expected one app event");
    match evt {
        AppEvent::CodexOp(Op::Review { review_request }) => {
            assert_eq!(
                review_request,
                ReviewRequest {
                    target: ReviewTarget::Custom {
                        instructions: "please audit dependencies".to_string(),
                    },
                    user_facing_hint: None,
                }
            );
        }
        other => panic!("unexpected app event: {other:?}"),
    }
}

/// Hitting Enter on an empty custom prompt view does not submit.
#[tokio::test]
async fn custom_prompt_enter_empty_does_not_send() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.show_review_custom_prompt();
    // Enter without any text
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    // No AppEvent::CodexOp should be sent
    assert!(rx.try_recv().is_err(), "no app event should be sent");
}

// Snapshot test: interrupting a running exec finalizes the active cell with a red ✗
// marker (replacing the spinner) and flushes it into history.
#[tokio::test]
async fn interrupt_exec_marks_failed_snapshot() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    // Begin a long-running command so we have an active exec cell with a spinner.
    begin_exec(&mut chat, "call-int", "sleep 1");

    // Simulate the task being aborted (as if ESC was pressed), which should
    // cause the active exec cell to be finalized as failed and flushed.
    chat.handle_codex_event(Event {
        id: "call-int".into(),
        msg: EventMsg::TurnAborted(codex_protocol::protocol::TurnAbortedEvent {
            turn_id: Some("turn-1".to_string()),
            reason: TurnAbortReason::Interrupted,
            completed_at: None,
            duration_ms: None,
        }),
    });

    let cells = drain_insert_history(&mut rx);
    assert!(
        !cells.is_empty(),
        "expected finalized exec cell to be inserted into history"
    );

    // The first inserted cell should be the finalized exec; snapshot its text.
    let exec_blob = lines_to_single_string(&cells[0]);
    assert_chatwidget_snapshot!("interrupt_exec_marks_failed", exec_blob);
}

// Snapshot test: after an interrupted turn, a gentle error message is inserted
// suggesting the user to tell the model what to do differently and to use /feedback.
#[tokio::test]
async fn interrupted_turn_error_message_snapshot() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    // Simulate an in-progress task so the widget is in a running state.
    chat.handle_codex_event(Event {
        id: "task-1".into(),
        msg: EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: "turn-1".to_string(),
            started_at: None,
            model_context_window: None,
            collaboration_mode_kind: ModeKind::Default,
        }),
    });

    // Abort the turn (like pressing Esc) and drain inserted history.
    chat.handle_codex_event(Event {
        id: "task-1".into(),
        msg: EventMsg::TurnAborted(codex_protocol::protocol::TurnAbortedEvent {
            turn_id: Some("turn-1".to_string()),
            reason: TurnAbortReason::Interrupted,
            completed_at: None,
            duration_ms: None,
        }),
    });

    let cells = drain_insert_history(&mut rx);
    assert!(
        !cells.is_empty(),
        "expected error message to be inserted after interruption"
    );
    let last = lines_to_single_string(cells.last().unwrap());
    assert_chatwidget_snapshot!("interrupted_turn_error_message", last);
}

#[tokio::test]
async fn interrupted_turn_after_goal_budget_limited_uses_budget_message_snapshot() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::GoalMode, /*enabled*/ true);

    chat.handle_server_notification(
        codex_app_server_protocol::ServerNotification::TurnStarted(
            codex_app_server_protocol::TurnStartedNotification {
                thread_id: "thread-1".to_string(),
                turn: codex_app_server_protocol::Turn {
                    id: "turn-1".to_string(),
                    items: Vec::new(),
                    status: codex_app_server_protocol::TurnStatus::InProgress,
                    error: None,
                    started_at: None,
                    completed_at: None,
                    duration_ms: None,
                },
            },
        ),
        /*replay_kind*/ None,
    );
    chat.handle_server_notification(
        codex_app_server_protocol::ServerNotification::ThreadGoalUpdated(
            codex_app_server_protocol::ThreadGoalUpdatedNotification {
                thread_id: "thread-1".to_string(),
                turn_id: Some("turn-1".to_string()),
                goal: codex_app_server_protocol::ThreadGoal {
                    thread_id: "thread-1".to_string(),
                    objective: "Run until the token budget is limited".to_string(),
                    status: codex_app_server_protocol::ThreadGoalStatus::BudgetLimited,
                    token_budget: Some(10_000),
                    tokens_used: 10_500,
                    time_used_seconds: 0,
                    created_at: 0,
                    updated_at: 1,
                },
            },
        ),
        /*replay_kind*/ None,
    );
    chat.handle_server_notification(
        codex_app_server_protocol::ServerNotification::TurnCompleted(
            codex_app_server_protocol::TurnCompletedNotification {
                thread_id: "thread-1".to_string(),
                turn: codex_app_server_protocol::Turn {
                    id: "turn-1".to_string(),
                    items: Vec::new(),
                    status: codex_app_server_protocol::TurnStatus::Interrupted,
                    error: None,
                    started_at: None,
                    completed_at: None,
                    duration_ms: None,
                },
            },
        ),
        /*replay_kind*/ None,
    );

    let cells = drain_insert_history(&mut rx);
    let last = lines_to_single_string(cells.last().unwrap());
    assert_chatwidget_snapshot!("interrupted_turn_goal_budget_limited_message", last);
}

#[tokio::test]
async fn direct_budget_limited_turn_uses_budget_message_snapshot() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_codex_event(Event {
        id: "task-1".into(),
        msg: EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: "turn-1".to_string(),
            started_at: None,
            model_context_window: None,
            collaboration_mode_kind: ModeKind::Default,
        }),
    });
    chat.handle_codex_event(Event {
        id: "task-1".into(),
        msg: EventMsg::TurnAborted(codex_protocol::protocol::TurnAbortedEvent {
            turn_id: Some("turn-1".to_string()),
            reason: TurnAbortReason::BudgetLimited,
            completed_at: None,
            duration_ms: None,
        }),
    });

    let cells = drain_insert_history(&mut rx);
    let last = lines_to_single_string(cells.last().unwrap());
    assert_chatwidget_snapshot!("direct_budget_limited_turn_message", last);
}

#[tokio::test]
async fn budget_limited_turn_restores_queued_input_without_submitting() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.queued_user_messages
        .push_back(UserMessage::from("follow-up after budget stop"));
    chat.refresh_pending_input_preview();

    chat.handle_codex_event(Event {
        id: "task-1".into(),
        msg: EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: "turn-1".to_string(),
            started_at: None,
            model_context_window: None,
            collaboration_mode_kind: ModeKind::Default,
        }),
    });
    chat.handle_codex_event(Event {
        id: "task-1".into(),
        msg: EventMsg::TurnAborted(codex_protocol::protocol::TurnAbortedEvent {
            turn_id: Some("turn-1".to_string()),
            reason: TurnAbortReason::BudgetLimited,
            completed_at: None,
            duration_ms: None,
        }),
    });

    assert!(chat.queued_user_messages.is_empty());
    assert_eq!(
        chat.bottom_pane.composer_text(),
        "follow-up after budget stop"
    );
    assert_no_submit_op(&mut op_rx);
}

// Snapshot test: interrupting specifically to submit pending steers shows an
// informational banner instead of the generic "tell the model what to do
// differently" error prompt.
#[tokio::test]
async fn interrupted_turn_pending_steers_message_snapshot() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.pending_steers.push_back(pending_steer("steer 1"));
    chat.submit_pending_steers_after_interrupt = true;

    chat.handle_codex_event(Event {
        id: "task-1".into(),
        msg: EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: "turn-1".to_string(),
            started_at: None,
            model_context_window: None,
            collaboration_mode_kind: ModeKind::Default,
        }),
    });

    chat.handle_codex_event(Event {
        id: "task-1".into(),
        msg: EventMsg::TurnAborted(codex_protocol::protocol::TurnAbortedEvent {
            turn_id: Some("turn-1".to_string()),
            reason: TurnAbortReason::Interrupted,
            completed_at: None,
            duration_ms: None,
        }),
    });

    let cells = drain_insert_history(&mut rx);
    let info = cells
        .iter()
        .map(|cell| lines_to_single_string(cell))
        .find(|line| line.contains("Model interrupted to submit steer instructions."))
        .expect("expected steer interrupt info message to be inserted");
    assert_chatwidget_snapshot!("interrupted_turn_pending_steers_message", info);
}

/// Opening custom prompt from the review popup, pressing Esc returns to the
/// parent popup, pressing Esc again dismisses all panels (back to normal mode).
#[tokio::test]
async fn review_custom_prompt_escape_navigates_back_then_dismisses() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    // Open the Review presets parent popup.
    chat.open_review_popup();

    // Open the custom prompt submenu (child view) directly.
    chat.show_review_custom_prompt();

    // Verify child view is on top.
    let header = render_bottom_first_row(&chat, /*width*/ 60);
    assert!(
        header.contains("Custom review instructions"),
        "expected custom prompt view header: {header:?}"
    );

    // Esc once: child view closes, parent (review presets) remains.
    chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    let header = render_bottom_first_row(&chat, /*width*/ 60);
    assert!(
        header.contains("Select a review preset"),
        "expected to return to parent review popup: {header:?}"
    );

    // Esc again: parent closes; back to normal composer state.
    chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(
        chat.is_normal_backtrack_mode(),
        "expected to be back in normal composer mode"
    );
}

/// Opening base-branch picker from the review popup, pressing Esc returns to the
/// parent popup, pressing Esc again dismisses all panels (back to normal mode).
#[tokio::test]
async fn review_branch_picker_escape_navigates_back_then_dismisses() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    // Open the Review presets parent popup.
    chat.open_review_popup();

    // Open the branch picker submenu (child view). Using a temp cwd with no git repo is fine.
    let cwd = std::env::temp_dir();
    chat.show_review_branch_picker(&cwd).await;

    // Verify child view header.
    let header = render_bottom_first_row(&chat, /*width*/ 60);
    assert!(
        header.contains("Select a base branch"),
        "expected branch picker header: {header:?}"
    );

    // Esc once: child view closes, parent remains.
    chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    let header = render_bottom_first_row(&chat, /*width*/ 60);
    assert!(
        header.contains("Select a review preset"),
        "expected to return to parent review popup: {header:?}"
    );

    // Esc again: parent closes; back to normal composer state.
    chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(
        chat.is_normal_backtrack_mode(),
        "expected to be back in normal composer mode"
    );
}

#[tokio::test]
async fn review_ended_keeps_unified_exec_processes() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    begin_unified_exec_startup(&mut chat, "call-1", "process-1", "sleep 5");
    begin_unified_exec_startup(&mut chat, "call-2", "process-2", "sleep 6");
    assert_eq!(chat.unified_exec_processes.len(), 2);

    chat.handle_codex_event(Event {
        id: "turn-1".into(),
        msg: EventMsg::TurnAborted(codex_protocol::protocol::TurnAbortedEvent {
            turn_id: Some("turn-1".to_string()),
            reason: TurnAbortReason::ReviewEnded,
            completed_at: None,
            duration_ms: None,
        }),
    });

    assert_eq!(chat.unified_exec_processes.len(), 2);

    chat.add_ps_output();
    let cells = drain_insert_history(&mut rx);
    let combined = cells
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("Background terminals"),
        "expected /ps to remain available after review-ended abort; got {combined:?}"
    );
    assert!(
        combined.contains("sleep 5") && combined.contains("sleep 6"),
        "expected /ps to list running unified exec processes; got {combined:?}"
    );

    let _ = drain_insert_history(&mut rx);
}

#[tokio::test]
async fn enter_submits_steer_while_review_is_running() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.handle_codex_event(Event {
        id: "turn-start".into(),
        msg: EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: "turn-1".to_string(),
            started_at: None,
            model_context_window: None,
            collaboration_mode_kind: ModeKind::Default,
        }),
    });

    chat.handle_codex_event(Event {
        id: "review-1".into(),
        msg: EventMsg::EnteredReviewMode(ReviewRequest {
            target: ReviewTarget::UncommittedChanges,
            user_facing_hint: Some("current changes".to_string()),
        }),
    });
    let _ = drain_insert_history(&mut rx);

    chat.bottom_pane.set_composer_text(
        "Steer submitted while /review was running.".to_string(),
        Vec::new(),
        Vec::new(),
    );
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(chat.queued_user_messages.is_empty());
    assert_eq!(chat.pending_steers.len(), 1);
    assert_eq!(
        chat.pending_steers.front().unwrap().user_message.text,
        "Steer submitted while /review was running."
    );
    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "Steer submitted while /review was running.".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected running-turn steer submit, got {other:?}"),
    }
    assert!(drain_insert_history(&mut rx).is_empty());
}

#[tokio::test]
async fn review_queues_user_messages_snapshot() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.handle_codex_event(Event {
        id: "turn-start".into(),
        msg: EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: "turn-1".to_string(),
            started_at: None,
            model_context_window: None,
            collaboration_mode_kind: ModeKind::Default,
        }),
    });

    chat.handle_codex_event(Event {
        id: "review-1".into(),
        msg: EventMsg::EnteredReviewMode(ReviewRequest {
            target: ReviewTarget::UncommittedChanges,
            user_facing_hint: Some("current changes".to_string()),
        }),
    });
    let _ = drain_insert_history(&mut rx);

    chat.submit_user_message(UserMessage::from(
        "Steer submitted while /review was running.".to_string(),
    ));
    chat.handle_codex_event(Event {
        id: "steer-rejected".into(),
        msg: EventMsg::Error(ErrorEvent {
            message: "cannot steer a review turn".to_string(),
            codex_error_info: Some(CodexErrorInfo::ActiveTurnNotSteerable {
                turn_kind: NonSteerableTurnKind::Review,
            }),
        }),
    });

    let width: u16 = 80;
    let height: u16 = 18;
    let backend = VT100Backend::new(width, height);
    let mut term = crate::custom_terminal::Terminal::with_options(backend).expect("terminal");
    let desired_height = chat.desired_height(width).min(height);
    term.set_viewport_area(Rect::new(0, height - desired_height, width, desired_height));
    term.draw(|f| {
        chat.render(f.area(), f.buffer_mut());
    })
    .unwrap();
    assert_chatwidget_snapshot!(
        "review_queues_user_messages_snapshot",
        normalize_snapshot_paths(term.backend().vt100().screen().contents())
    );
}
