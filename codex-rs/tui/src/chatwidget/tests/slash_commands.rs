use super::*;
use pretty_assertions::assert_eq;

fn turn_complete_event(turn_id: &str, last_agent_message: Option<&str>) -> TurnCompleteEvent {
    serde_json::from_value(serde_json::json!({
        "turn_id": turn_id,
        "last_agent_message": last_agent_message,
    }))
    .expect("turn complete event should deserialize")
}

fn submit_composer_text(chat: &mut ChatWidget, text: &str) {
    chat.bottom_pane
        .set_composer_text(text.to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
}

fn queue_composer_text_with_tab(chat: &mut ChatWidget, text: &str) {
    chat.bottom_pane
        .set_composer_text(text.to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
}

fn recall_latest_after_clearing(chat: &mut ChatWidget) -> String {
    chat.bottom_pane
        .set_composer_text(String::new(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    chat.bottom_pane.composer_text()
}

fn next_add_to_history_op(op_rx: &mut tokio::sync::mpsc::UnboundedReceiver<Op>) -> String {
    loop {
        match op_rx.try_recv() {
            Ok(Op::AddToHistory { text }) => return text,
            Ok(_) => continue,
            Err(TryRecvError::Empty) => panic!("expected AddToHistory op but queue was empty"),
            Err(TryRecvError::Disconnected) => {
                panic!("expected AddToHistory op but channel closed")
            }
        }
    }
}

#[tokio::test]
async fn slash_compact_eagerly_queues_follow_up_before_turn_start() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.dispatch_command(SlashCommand::Compact);

    assert!(chat.bottom_pane.is_task_running());
    match rx.try_recv() {
        Ok(AppEvent::CodexOp(Op::Compact)) => {}
        other => panic!("expected compact op to be submitted, got {other:?}"),
    }

    chat.bottom_pane.set_composer_text(
        "queued before compact turn start".to_string(),
        Vec::new(),
        Vec::new(),
    );
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(chat.pending_steers.is_empty());
    assert_eq!(chat.queued_user_messages.len(), 1);
    assert_eq!(
        chat.queued_user_messages.front().unwrap().text,
        "queued before compact turn start"
    );
    assert_matches!(op_rx.try_recv(), Err(TryRecvError::Empty));
}

#[tokio::test]
async fn queued_slash_compact_dispatches_after_active_turn() {
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

    queue_composer_text_with_tab(&mut chat, "/compact");

    assert_eq!(chat.queued_user_messages.len(), 1);
    assert_eq!(
        chat.queued_user_messages.front().unwrap().action,
        QueuedInputAction::ParseSlash
    );
    assert_matches!(rx.try_recv(), Err(TryRecvError::Empty));

    chat.handle_codex_event(Event {
        id: "turn-complete".into(),
        msg: EventMsg::TurnComplete(turn_complete_event("turn-1", Some("done"))),
    });

    let events = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, AppEvent::CodexOp(Op::Compact))),
        "expected queued /compact to submit compact op; events: {events:?}"
    );
}

#[tokio::test]
async fn queued_slash_review_with_args_dispatches_after_active_turn() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
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

    queue_composer_text_with_tab(&mut chat, "/review check regressions");

    chat.handle_codex_event(Event {
        id: "turn-complete".into(),
        msg: EventMsg::TurnComplete(turn_complete_event("turn-1", Some("done"))),
    });

    match op_rx.try_recv() {
        Ok(Op::AddToHistory { .. }) => match op_rx.try_recv() {
            Ok(Op::Review { review_request }) => assert_eq!(
                review_request,
                ReviewRequest {
                    target: ReviewTarget::Custom {
                        instructions: "check regressions".to_string(),
                    },
                    user_facing_hint: None,
                }
            ),
            other => panic!("expected queued /review to submit review op, got {other:?}"),
        },
        Ok(Op::Review { review_request }) => assert_eq!(
            review_request,
            ReviewRequest {
                target: ReviewTarget::Custom {
                    instructions: "check regressions".to_string(),
                },
                user_facing_hint: None,
            }
        ),
        other => panic!("expected queued /review to submit review op, got {other:?}"),
    }
}

#[tokio::test]
async fn queued_slash_review_with_args_restores_for_edit() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
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

    queue_composer_text_with_tab(&mut chat, "/review check regressions");
    chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT));

    assert_eq!(
        chat.bottom_pane.composer_text(),
        "/review check regressions"
    );
}

#[tokio::test]
async fn queued_bang_shell_dispatches_after_active_turn() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
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

    queue_composer_text_with_tab(&mut chat, "!echo hi");

    assert_eq!(chat.queued_user_messages.len(), 1);
    assert_eq!(
        chat.queued_user_messages.front().unwrap().action,
        QueuedInputAction::RunShell
    );
    assert_matches!(op_rx.try_recv(), Err(TryRecvError::Empty));

    chat.handle_codex_event(Event {
        id: "turn-complete".into(),
        msg: EventMsg::TurnComplete(turn_complete_event("turn-1", Some("done"))),
    });

    match op_rx.try_recv() {
        Ok(Op::RunUserShellCommand { command }) => assert_eq!(command, "echo hi"),
        other => panic!("expected queued shell command op, got {other:?}"),
    }
    assert!(chat.queued_user_messages.is_empty());
}

#[tokio::test]
async fn queued_empty_bang_shell_reports_help_when_dequeued_and_drains_next_input() {
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

    queue_composer_text_with_tab(&mut chat, "!");
    queue_composer_text_with_tab(&mut chat, "hello after help");

    assert!(drain_insert_history(&mut rx).is_empty());

    chat.handle_codex_event(Event {
        id: "turn-complete".into(),
        msg: EventMsg::TurnComplete(turn_complete_event("turn-1", Some("done"))),
    });

    let cells = drain_insert_history(&mut rx);
    let rendered = cells
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains(USER_SHELL_COMMAND_HELP_TITLE),
        "expected delayed shell help, got {rendered:?}"
    );

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "hello after help".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected queued message after empty shell command, got {other:?}"),
    }
    assert!(chat.queued_user_messages.is_empty());
}

#[tokio::test]
async fn queued_bang_shell_waits_for_user_shell_completion_before_next_input() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
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

    queue_composer_text_with_tab(&mut chat, "!echo hi");
    queue_composer_text_with_tab(&mut chat, "hello after shell");

    chat.handle_codex_event(Event {
        id: "turn-complete".into(),
        msg: EventMsg::TurnComplete(turn_complete_event("turn-1", Some("done"))),
    });

    match op_rx.try_recv() {
        Ok(Op::RunUserShellCommand { command }) => assert_eq!(command, "echo hi"),
        other => panic!("expected queued shell command op, got {other:?}"),
    }
    assert_matches!(op_rx.try_recv(), Err(TryRecvError::Empty));
    assert_eq!(chat.queued_user_messages.len(), 1);

    let begin = begin_exec_with_source(
        &mut chat,
        "user-shell-echo",
        "echo hi",
        ExecCommandSource::UserShell,
    );
    end_exec(&mut chat, begin, "hi\n", "", /*exit_code*/ 0);

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "hello after shell".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected queued message after shell completion, got {other:?}"),
    }
    assert!(chat.queued_user_messages.is_empty());
}

async fn assert_cancelled_queued_menu_drains_next_input(command: &str, expected_popup_text: &str) {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(Some("gpt-5.2")).await;
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

    queue_composer_text_with_tab(&mut chat, command);
    queue_composer_text_with_tab(&mut chat, "hello after menu");

    chat.handle_codex_event(Event {
        id: "turn-complete".into(),
        msg: EventMsg::TurnComplete(turn_complete_event("turn-1", Some("done"))),
    });

    assert_eq!(chat.queued_user_messages.len(), 1);
    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert!(
        popup.contains(expected_popup_text),
        "expected {command} menu to open; popup:\n{popup}"
    );
    assert_matches!(op_rx.try_recv(), Err(TryRecvError::Empty));

    chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "hello after menu".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected queued message after cancelling {command}, got {other:?}"),
    }
    assert!(chat.queued_user_messages.is_empty());
}

#[tokio::test]
async fn queued_slash_menu_cancel_drains_next_input() {
    assert_cancelled_queued_menu_drains_next_input("/model", "Select Model").await;
    assert_cancelled_queued_menu_drains_next_input("/permissions", "Update Model Permissions")
        .await;
}

#[tokio::test]
async fn queued_slash_menu_selection_drains_next_input() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(Some("gpt-5.2")).await;
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

    queue_composer_text_with_tab(&mut chat, "/permissions");
    queue_composer_text_with_tab(&mut chat, "hello after selection");

    chat.handle_codex_event(Event {
        id: "turn-complete".into(),
        msg: EventMsg::TurnComplete(turn_complete_event("turn-1", Some("done"))),
    });

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert!(
        popup.contains("Update Model Permissions"),
        "expected permissions menu to open; popup:\n{popup}"
    );

    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "hello after selection".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected queued message after permissions selection, got {other:?}"),
    }
    assert!(chat.queued_user_messages.is_empty());
}

#[tokio::test]
async fn queued_bare_rename_drains_next_input_after_name_update() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    chat.handle_codex_event(Event {
        id: "turn-start".into(),
        msg: EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: "turn-1".to_string(),
            started_at: None,
            model_context_window: None,
            collaboration_mode_kind: ModeKind::Default,
        }),
    });

    queue_composer_text_with_tab(&mut chat, "/rename");
    queue_composer_text_with_tab(&mut chat, "hello after rename");

    chat.handle_codex_event(Event {
        id: "turn-complete".into(),
        msg: EventMsg::TurnComplete(turn_complete_event("turn-1", Some("done"))),
    });

    assert_eq!(chat.queued_user_messages.len(), 1);
    assert!(render_bottom_popup(&chat, /*width*/ 80).contains("Name thread"));
    assert_matches!(op_rx.try_recv(), Err(TryRecvError::Empty));

    chat.handle_paste("Queued rename".to_string());
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    let events = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(
        events.iter().any(|event| matches!(
            event,
            AppEvent::CodexOp(Op::SetThreadName { name }) if name == "Queued rename"
        )),
        "expected rename prompt to submit thread name; events: {events:?}"
    );

    chat.handle_codex_event(Event {
        id: "rename".into(),
        msg: EventMsg::ThreadNameUpdated(codex_protocol::protocol::ThreadNameUpdatedEvent {
            thread_id,
            thread_name: Some("Queued rename".to_string()),
        }),
    });

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "hello after rename".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected queued message after /rename, got {other:?}"),
    }
    assert!(chat.queued_user_messages.is_empty());
}

#[tokio::test]
async fn queued_inline_rename_does_not_drain_again_before_turn_started() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    chat.handle_codex_event(Event {
        id: "turn-start".into(),
        msg: EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: "turn-1".to_string(),
            started_at: None,
            model_context_window: None,
            collaboration_mode_kind: ModeKind::Default,
        }),
    });

    queue_composer_text_with_tab(&mut chat, "/rename Queued rename");
    queue_composer_text_with_tab(&mut chat, "first after rename");
    queue_composer_text_with_tab(&mut chat, "second after rename");

    chat.handle_codex_event(Event {
        id: "turn-complete".into(),
        msg: EventMsg::TurnComplete(turn_complete_event("turn-1", Some("done"))),
    });

    let events = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(
        events.iter().any(|event| matches!(
            event,
            AppEvent::CodexOp(Op::SetThreadName { name }) if name == "Queued rename"
        )),
        "expected queued /rename to submit thread name; events: {events:?}"
    );

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "first after rename".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected first queued message after /rename, got {other:?}"),
    }
    assert_matches!(
        op_rx.try_recv(),
        Ok(Op::AddToHistory { text }) if text == "first after rename"
    );
    assert_eq!(
        chat.queued_user_message_texts(),
        vec!["second after rename"]
    );
    let input_state = chat.capture_thread_input_state().unwrap();
    assert!(input_state.user_turn_pending_start);
    chat.restore_thread_input_state(/*input_state*/ None);
    assert!(!chat.user_turn_pending_start);
    chat.restore_thread_input_state(Some(input_state));
    assert!(chat.user_turn_pending_start);
    assert_eq!(
        chat.queued_user_message_texts(),
        vec!["second after rename"]
    );

    chat.handle_codex_event(Event {
        id: "rename".into(),
        msg: EventMsg::ThreadNameUpdated(codex_protocol::protocol::ThreadNameUpdatedEvent {
            thread_id,
            thread_name: Some("Queued rename".to_string()),
        }),
    });

    assert_matches!(op_rx.try_recv(), Err(TryRecvError::Empty));
    assert_eq!(
        chat.queued_user_message_texts(),
        vec!["second after rename"]
    );

    chat.handle_codex_event(Event {
        id: "turn-2-start".into(),
        msg: EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: "turn-2".to_string(),
            started_at: None,
            model_context_window: None,
            collaboration_mode_kind: ModeKind::Default,
        }),
    });
    chat.handle_codex_event(Event {
        id: "turn-2-complete".into(),
        msg: EventMsg::TurnComplete(turn_complete_event("turn-2", Some("done"))),
    });

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "second after rename".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected second queued message after turn complete, got {other:?}"),
    }
    assert!(chat.queued_user_messages.is_empty());
}

#[tokio::test]
async fn queued_unknown_slash_reports_error_when_dequeued() {
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

    queue_composer_text_with_tab(&mut chat, "/does-not-exist");

    assert!(drain_insert_history(&mut rx).is_empty());

    chat.handle_codex_event(Event {
        id: "turn-complete".into(),
        msg: EventMsg::TurnComplete(turn_complete_event("turn-1", Some("done"))),
    });

    let cells = drain_insert_history(&mut rx);
    let rendered = cells
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("Unrecognized command '/does-not-exist'"),
        "expected delayed slash error, got {rendered:?}"
    );
    assert!(chat.queued_user_messages.is_empty());
}

#[tokio::test]
async fn ctrl_d_quits_without_prompt() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_key_event(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
    assert_matches!(rx.try_recv(), Ok(AppEvent::Exit(ExitMode::ShutdownFirst)));
}

#[tokio::test]
async fn ctrl_d_with_modal_open_does_not_quit() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.open_approvals_popup();
    chat.handle_key_event(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));

    assert_matches!(rx.try_recv(), Err(TryRecvError::Empty));
}

#[tokio::test]
async fn slash_init_skips_when_project_doc_exists() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let tempdir = tempdir().unwrap();
    let existing_path = tempdir.path().join(DEFAULT_AGENTS_MD_FILENAME);
    std::fs::write(&existing_path, "existing instructions").unwrap();
    chat.config.cwd = tempdir.path().to_path_buf().abs();

    submit_composer_text(&mut chat, "/init");

    match op_rx.try_recv() {
        Err(TryRecvError::Empty) => {}
        other => panic!("expected no Codex op to be sent, got {other:?}"),
    }

    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 1, "expected one info message");
    let rendered = lines_to_single_string(&cells[0]);
    assert!(
        rendered.contains(DEFAULT_AGENTS_MD_FILENAME),
        "info message should mention the existing file: {rendered:?}"
    );
    assert!(
        rendered.contains("Skipping /init"),
        "info message should explain why /init was skipped: {rendered:?}"
    );
    assert_eq!(
        std::fs::read_to_string(existing_path).unwrap(),
        "existing instructions"
    );
    assert_eq!(recall_latest_after_clearing(&mut chat), "/init");
}

#[tokio::test]
async fn bare_slash_command_is_available_from_local_recall_after_dispatch() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    submit_composer_text(&mut chat, "/diff");

    let _ = drain_insert_history(&mut rx);
    chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    assert_eq!(chat.bottom_pane.composer_text(), "/diff");
}

#[tokio::test]
async fn inline_slash_command_is_available_from_local_recall_after_dispatch() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    submit_composer_text(&mut chat, "/rename Better title");

    let _ = drain_insert_history(&mut rx);
    chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    assert_eq!(chat.bottom_pane.composer_text(), "/rename Better title");
}

#[tokio::test]
async fn goal_slash_command_records_original_command_in_history() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Goals, /*enabled*/ true);
    let command = "/goal keep improving benchmark coverage with a 5K token budget";
    let rollout_file = NamedTempFile::new().unwrap();
    let configured = codex_protocol::protocol::SessionConfiguredEvent {
        session_id: ThreadId::new(),
        forked_from_id: None,
        thread_name: None,
        model: "test-model".to_string(),
        model_provider_id: "test-provider".to_string(),
        service_tier: None,
        approval_policy: AskForApproval::Never,
        approvals_reviewer: ApprovalsReviewer::User,
        sandbox_policy: SandboxPolicy::new_read_only_policy(),
        permission_profile: None,
        cwd: test_path_buf("/home/user/project").abs(),
        reasoning_effort: Some(ReasoningEffortConfig::default()),
        history_log_id: 0,
        history_entry_count: 0,
        initial_messages: None,
        network_proxy: None,
        rollout_path: Some(rollout_file.path().to_path_buf()),
    };
    chat.handle_codex_event(Event {
        id: "initial".into(),
        msg: EventMsg::SessionConfigured(configured),
    });
    drain_insert_history(&mut rx);

    submit_composer_text(&mut chat, command);

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => {
            let [UserInput::Text { text, .. }] = items.as_slice() else {
                panic!("expected one text item, got {items:?}");
            };
            assert!(
                text.starts_with("Set the current thread goal"),
                "model should receive goal-parser prompt, got {text:?}"
            );
        }
        other => panic!("expected user turn, got {other:?}"),
    }
    assert_eq!(next_add_to_history_op(&mut op_rx), command);
    assert_eq!(recall_latest_after_clearing(&mut chat), command);

    let rendered = drain_insert_history(&mut rx)
        .iter()
        .map(|cell| lines_to_single_string(cell))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains(command),
        "expected transcript to render original command, got {rendered:?}"
    );
    assert!(
        !rendered.contains("Set the current thread goal"),
        "transcript should not render the internal parser prompt: {rendered:?}"
    );
}

#[tokio::test]
async fn goal_slash_command_records_mentions_in_history() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Goals, /*enabled*/ true);
    chat.thread_id = Some(ThreadId::new());
    chat.bottom_pane.set_composer_text_with_mention_bindings(
        "/goal use $figma for the mockup".to_string(),
        Vec::new(),
        Vec::new(),
        vec![MentionBinding {
            mention: "figma".to_string(),
            path: "app://figma".to_string(),
        }],
    );

    chat.handle_key_event(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => {
            let [UserInput::Text { text, .. }] = items.as_slice() else {
                panic!("expected one text item, got {items:?}");
            };
            assert!(
                text.starts_with("Set the current thread goal"),
                "model should receive goal-parser prompt, got {text:?}"
            );
        }
        other => panic!("expected user turn, got {other:?}"),
    }
    assert_eq!(
        next_add_to_history_op(&mut op_rx),
        "/goal use [$figma](app://figma) for the mockup"
    );
}

#[tokio::test]
async fn goal_slash_command_preserves_attached_images() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Goals, /*enabled*/ true);
    chat.thread_id = Some(ThreadId::new());
    let remote_url = "https://example.com/goal.png".to_string();
    let local_image = PathBuf::from("/tmp/goal-local.png");
    let placeholder = "[Image #2]";
    let command = format!("/goal describe {placeholder}");
    let placeholder_start = command.find(placeholder).expect("placeholder in command");
    chat.set_remote_image_urls(vec![remote_url.clone()]);
    chat.bottom_pane.set_composer_text(
        command,
        vec![TextElement::new(
            (placeholder_start..placeholder_start + placeholder.len()).into(),
            Some(placeholder.to_string()),
        )],
        vec![local_image.clone()],
    );

    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => {
            assert_eq!(items.len(), 3);
            assert_eq!(
                items[0],
                UserInput::Image {
                    image_url: remote_url
                }
            );
            assert_eq!(items[1], UserInput::LocalImage { path: local_image });
            let UserInput::Text { text, .. } = &items[2] else {
                panic!("expected parser prompt text, got {items:?}");
            };
            assert!(
                text.starts_with("Set the current thread goal"),
                "model should receive goal-parser prompt, got {text:?}"
            );
        }
        other => panic!("expected user turn, got {other:?}"),
    }
}

#[tokio::test]
async fn bare_goal_slash_command_drains_pending_submission_state() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Goals, /*enabled*/ true);
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    let remote_url = "https://example.com/goal-menu.png".to_string();
    let local_image = PathBuf::from("/tmp/goal-menu-local.png");
    chat.set_remote_image_urls(vec![remote_url]);
    chat.bottom_pane
        .set_composer_text("/goal".to_string(), Vec::new(), vec![local_image]);

    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_matches!(
        rx.try_recv(),
        Ok(AppEvent::OpenThreadGoalMenu { thread_id: opened }) if opened == thread_id
    );
    assert!(chat.remote_image_urls().is_empty());
    assert!(chat.bottom_pane.composer_local_image_paths().is_empty());
}

#[tokio::test]
async fn goal_control_slash_commands_emit_goal_events() {
    let cases = [
        ("/goal clear", None),
        ("/goal pause", Some(AppThreadGoalStatus::Paused)),
        ("/goal unpause", Some(AppThreadGoalStatus::Active)),
    ];

    for (command, status) in cases {
        let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
        chat.set_feature_enabled(Feature::Goals, /*enabled*/ true);
        let thread_id = ThreadId::new();
        chat.thread_id = Some(thread_id);

        submit_composer_text(&mut chat, command);

        match status {
            Some(status) => {
                let event = rx.try_recv().expect("expected goal status event");
                let AppEvent::SetThreadGoalStatus {
                    thread_id: actual_thread_id,
                    status: actual_status,
                } = event
                else {
                    panic!("expected SetThreadGoalStatus, got {event:?}");
                };
                assert_eq!(actual_thread_id, thread_id);
                assert_eq!(actual_status, status);
            }
            None => {
                let event = rx.try_recv().expect("expected clear goal event");
                let AppEvent::ClearThreadGoal {
                    thread_id: actual_thread_id,
                } = event
                else {
                    panic!("expected ClearThreadGoal, got {event:?}");
                };
                assert_eq!(actual_thread_id, thread_id);
            }
        }
    }
}

#[tokio::test]
async fn goal_slash_command_keeps_draft_when_model_is_unavailable() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Goals, /*enabled*/ true);
    chat.thread_id = Some(ThreadId::new());
    chat.set_model("");
    let placeholder = "[Image #2]";
    let command = format!("/goal write a cat poem about {placeholder}");
    let placeholder_start = command.find(placeholder).expect("placeholder in command");
    let remote_url = "https://example.com/goal-draft.png".to_string();
    let local_image = PathBuf::from("/tmp/goal-draft-local.png");
    chat.set_remote_image_urls(vec![remote_url.clone()]);
    chat.bottom_pane.set_composer_text(
        command.clone(),
        vec![TextElement::new(
            (placeholder_start..placeholder_start + placeholder.len()).into(),
            Some(placeholder.to_string()),
        )],
        vec![local_image.clone()],
    );

    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_no_submit_op(&mut op_rx);
    assert_eq!(chat.bottom_pane.composer_text(), command);
    assert_eq!(chat.remote_image_urls(), vec![remote_url]);
    assert_eq!(
        chat.bottom_pane.composer_local_image_paths(),
        vec![local_image]
    );
}

#[tokio::test]
async fn queued_goal_slash_command_records_original_command_in_history() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Goals, /*enabled*/ true);
    let command = "/goal write a cat poem";

    submit_composer_text(&mut chat, command);
    assert_matches!(op_rx.try_recv(), Err(TryRecvError::Empty));

    chat.thread_id = Some(ThreadId::new());
    chat.maybe_send_next_queued_input();

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => {
            let [UserInput::Text { text, .. }] = items.as_slice() else {
                panic!("expected one text item, got {items:?}");
            };
            assert!(
                text.starts_with("Set the current thread goal"),
                "model should receive goal-parser prompt, got {text:?}"
            );
        }
        other => panic!("expected user turn, got {other:?}"),
    }
    assert_eq!(next_add_to_history_op(&mut op_rx), command);
}

#[tokio::test]
async fn queued_goal_slash_command_preserves_current_draft_metadata() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Goals, /*enabled*/ true);
    let command = "/goal write a cat poem";

    submit_composer_text(&mut chat, command);
    assert_matches!(op_rx.try_recv(), Err(TryRecvError::Empty));

    let remote_url = "https://example.com/current-draft.png".to_string();
    let local_image = PathBuf::from("/tmp/current-draft-local.png");
    let placeholder = "[Image #3]";
    let draft = format!("draft with {placeholder}");
    let placeholder_start = draft.find(placeholder).expect("placeholder in draft");
    chat.set_remote_image_urls(vec![remote_url.clone()]);
    chat.bottom_pane.set_composer_text(
        draft.clone(),
        vec![TextElement::new(
            (placeholder_start..placeholder_start + placeholder.len()).into(),
            Some(placeholder.to_string()),
        )],
        vec![local_image.clone()],
    );

    chat.thread_id = Some(ThreadId::new());
    chat.maybe_send_next_queued_input();

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => {
            let [UserInput::Text { text, .. }] = items.as_slice() else {
                panic!("expected one text item, got {items:?}");
            };
            assert!(
                text.starts_with("Set the current thread goal"),
                "model should receive goal-parser prompt, got {text:?}"
            );
        }
        other => panic!("expected user turn, got {other:?}"),
    }
    assert_eq!(next_add_to_history_op(&mut op_rx), command);
    assert_eq!(chat.bottom_pane.composer_text(), draft);
    assert_eq!(chat.remote_image_urls(), vec![remote_url]);
    assert_eq!(
        chat.bottom_pane.composer_local_image_paths(),
        vec![local_image]
    );
}

#[tokio::test]
async fn restored_queued_goal_slash_command_records_original_command_in_history() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Goals, /*enabled*/ true);
    let command = "/goal write a cat poem";

    submit_composer_text(&mut chat, command);
    let input_state = chat
        .capture_thread_input_state()
        .expect("expected queued input state");

    let (mut restored_chat, _restored_rx, mut restored_op_rx) =
        make_chatwidget_manual(/*model_override*/ None).await;
    restored_chat.set_feature_enabled(Feature::Goals, /*enabled*/ true);
    restored_chat.restore_thread_input_state(Some(input_state));
    restored_chat.thread_id = Some(ThreadId::new());
    restored_chat.maybe_send_next_queued_input();

    match next_submit_op(&mut restored_op_rx) {
        Op::UserTurn { items, .. } => {
            let [UserInput::Text { text, .. }] = items.as_slice() else {
                panic!("expected one text item, got {items:?}");
            };
            assert!(
                text.starts_with("Set the current thread goal"),
                "model should receive goal-parser prompt, got {text:?}"
            );
        }
        other => panic!("expected user turn, got {other:?}"),
    }
    assert_eq!(next_add_to_history_op(&mut restored_op_rx), command);
}

#[tokio::test]
async fn rejected_goal_slash_command_retry_records_original_command_in_history() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Goals, /*enabled*/ true);
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
    let command = "/goal write a cat poem";

    submit_composer_text(&mut chat, command);

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => {
            let [UserInput::Text { text, .. }] = items.as_slice() else {
                panic!("expected one text item, got {items:?}");
            };
            assert!(
                text.starts_with("Set the current thread goal"),
                "model should receive goal-parser prompt, got {text:?}"
            );
        }
        other => panic!("expected user turn, got {other:?}"),
    }
    assert_eq!(next_add_to_history_op(&mut op_rx), command);

    chat.handle_codex_event(Event {
        id: "steer-rejected".into(),
        msg: EventMsg::Error(ErrorEvent {
            message: "cannot steer this turn".to_string(),
            codex_error_info: Some(CodexErrorInfo::ActiveTurnNotSteerable {
                turn_kind: NonSteerableTurnKind::Review,
            }),
        }),
    });
    chat.handle_codex_event(Event {
        id: "turn-complete".into(),
        msg: EventMsg::TurnComplete(TurnCompleteEvent {
            turn_id: "turn-1".to_string(),
            last_agent_message: None,
            completed_at: None,
            duration_ms: None,
            time_to_first_token_ms: None,
        }),
    });

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => {
            let [UserInput::Text { text, .. }] = items.as_slice() else {
                panic!("expected one text item, got {items:?}");
            };
            assert!(
                text.starts_with("Set the current thread goal"),
                "retried model input should remain goal-parser prompt, got {text:?}"
            );
        }
        other => panic!("expected retried user turn, got {other:?}"),
    }
    assert_eq!(next_add_to_history_op(&mut op_rx), command);
}

#[tokio::test]
async fn interrupted_goal_slash_command_restores_original_command_to_composer() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Goals, /*enabled*/ true);
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
    let command = "/goal write a cat poem";

    submit_composer_text(&mut chat, command);

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => {
            let [UserInput::Text { text, .. }] = items.as_slice() else {
                panic!("expected one text item, got {items:?}");
            };
            assert!(
                text.starts_with("Set the current thread goal"),
                "model should receive goal-parser prompt, got {text:?}"
            );
        }
        other => panic!("expected user turn, got {other:?}"),
    }
    assert_eq!(next_add_to_history_op(&mut op_rx), command);

    chat.handle_codex_event(Event {
        id: "turn-aborted".into(),
        msg: EventMsg::TurnAborted(codex_protocol::protocol::TurnAbortedEvent {
            turn_id: Some("turn-1".to_string()),
            reason: TurnAbortReason::Interrupted,
            completed_at: None,
            duration_ms: None,
        }),
    });

    assert_eq!(chat.bottom_pane.composer_text(), command);
}

#[tokio::test]
async fn committed_pending_goal_slash_command_renders_original_command() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Goals, /*enabled*/ true);
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
    let command = "/goal write a cat poem";

    submit_composer_text(&mut chat, command);

    let submitted_items = match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => {
            let [UserInput::Text { text, .. }] = items.as_slice() else {
                panic!("expected one text item, got {items:?}");
            };
            assert!(
                text.starts_with("Set the current thread goal"),
                "model should receive goal-parser prompt, got {text:?}"
            );
            items
        }
        other => panic!("expected user turn, got {other:?}"),
    };
    assert_eq!(next_add_to_history_op(&mut op_rx), command);

    complete_user_message_for_inputs(&mut chat, "user-goal", submitted_items);

    let rendered = drain_insert_history(&mut rx)
        .iter()
        .map(|cell| lines_to_single_string(cell))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains(command),
        "expected transcript to render original command, got {rendered:?}"
    );
    assert!(
        !rendered.contains("Set the current thread goal"),
        "transcript should not render the internal parser prompt: {rendered:?}"
    );
}

#[tokio::test]
async fn interrupted_goal_slash_command_resubmit_records_original_command_in_history() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Goals, /*enabled*/ true);
    chat.thread_id = Some(ThreadId::new());
    chat.on_task_started();
    chat.on_agent_message_delta("Final answer line\n".to_string());
    let command = "/goal write a cat poem";

    submit_composer_text(&mut chat, command);

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => {
            let [UserInput::Text { text, .. }] = items.as_slice() else {
                panic!("expected one text item, got {items:?}");
            };
            assert!(
                text.starts_with("Set the current thread goal"),
                "model should receive goal-parser prompt, got {text:?}"
            );
        }
        other => panic!("expected user turn, got {other:?}"),
    }
    assert_eq!(next_add_to_history_op(&mut op_rx), command);

    chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    next_interrupt_op(&mut op_rx);
    chat.on_interrupted_turn(TurnAbortReason::Interrupted);

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => {
            let [UserInput::Text { text, .. }] = items.as_slice() else {
                panic!("expected one text item, got {items:?}");
            };
            assert!(
                text.starts_with("Set the current thread goal"),
                "resubmitted model input should remain goal-parser prompt, got {text:?}"
            );
        }
        other => panic!("expected resubmitted user turn, got {other:?}"),
    }
    assert_eq!(next_add_to_history_op(&mut op_rx), command);
}

#[test]
fn merged_history_record_preserves_raw_text_and_rebased_elements() {
    let first = UserMessage {
        text: "Ask $figma".to_string(),
        local_images: Vec::new(),
        remote_image_urls: Vec::new(),
        text_elements: vec![TextElement::new((4..10).into(), Some("$figma".to_string()))],
        mention_bindings: vec![MentionBinding {
            mention: "figma".to_string(),
            path: "app://figma".to_string(),
        }],
    };
    let second = UserMessage::from("internal prompt");

    let (_message, history_record) = merge_user_messages_with_history_record(vec![
        (first, UserMessageHistoryRecord::UserMessageText),
        (
            second,
            UserMessageHistoryRecord::Override(UserMessageHistoryOverride {
                text: "/goal inspect [Image #1]".to_string(),
                text_elements: vec![TextElement::new(
                    (14..24).into(),
                    Some("[Image #1]".to_string()),
                )],
            }),
        ),
    ]);

    assert_eq!(
        history_record,
        UserMessageHistoryRecord::Override(UserMessageHistoryOverride {
            text: "Ask $figma\n/goal inspect [Image #1]".to_string(),
            text_elements: vec![
                TextElement::new((4..10).into(), Some("$figma".to_string())),
                TextElement::new((25..35).into(), Some("[Image #1]".to_string())),
            ],
        })
    );
}

#[test]
fn merged_history_record_remaps_override_image_placeholders() {
    let first_placeholder = "[Image #1]";
    let second_placeholder = "[Image #1]";
    let first = UserMessage {
        text: format!("first {first_placeholder}"),
        local_images: vec![LocalImageAttachment {
            placeholder: first_placeholder.to_string(),
            path: PathBuf::from("/tmp/first.png"),
        }],
        remote_image_urls: Vec::new(),
        text_elements: vec![TextElement::new(
            (6..16).into(),
            Some(first_placeholder.to_string()),
        )],
        mention_bindings: Vec::new(),
    };
    let second = UserMessage {
        text: format!("internal {second_placeholder}"),
        local_images: vec![LocalImageAttachment {
            placeholder: second_placeholder.to_string(),
            path: PathBuf::from("/tmp/second.png"),
        }],
        remote_image_urls: Vec::new(),
        text_elements: vec![TextElement::new(
            (9..19).into(),
            Some(second_placeholder.to_string()),
        )],
        mention_bindings: Vec::new(),
    };

    let (message, history_record) = merge_user_messages_with_history_record(vec![
        (first, UserMessageHistoryRecord::UserMessageText),
        (
            second,
            UserMessageHistoryRecord::Override(UserMessageHistoryOverride {
                text: format!("goal {second_placeholder}"),
                text_elements: vec![TextElement::new(
                    (5..15).into(),
                    Some(second_placeholder.to_string()),
                )],
            }),
        ),
    ]);

    assert_eq!(message.text, "first [Image #1]\ninternal [Image #2]");
    assert_eq!(
        message.text_elements,
        vec![
            TextElement::new((6..16).into(), Some("[Image #1]".to_string())),
            TextElement::new((26..36).into(), Some("[Image #2]".to_string())),
        ]
    );
    assert_eq!(
        message
            .local_images
            .iter()
            .map(|image| image.placeholder.as_str())
            .collect::<Vec<_>>(),
        vec!["[Image #1]", "[Image #2]"]
    );
    assert_eq!(
        history_record,
        UserMessageHistoryRecord::Override(UserMessageHistoryOverride {
            text: "first [Image #1]\ngoal [Image #2]".to_string(),
            text_elements: vec![
                TextElement::new((6..16).into(), Some("[Image #1]".to_string())),
                TextElement::new((22..32).into(), Some("[Image #2]".to_string())),
            ],
        })
    );
}

#[tokio::test]
async fn interrupted_merged_message_history_encodes_mentions_once() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.on_task_started();
    chat.on_agent_message_delta("Final answer line\n".to_string());
    let text = "use $figma now";
    chat.bottom_pane.set_composer_text_with_mention_bindings(
        text.to_string(),
        Vec::new(),
        Vec::new(),
        vec![MentionBinding {
            mention: "figma".to_string(),
            path: "app://figma".to_string(),
        }],
    );

    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => {
            let [
                UserInput::Text {
                    text: submitted, ..
                },
            ] = items.as_slice()
            else {
                panic!("expected text item, got {items:?}");
            };
            assert_eq!(submitted, text);
        }
        other => panic!("expected user turn, got {other:?}"),
    }
    let encoded = "use [$figma](app://figma) now";
    assert_eq!(next_add_to_history_op(&mut op_rx), encoded);

    chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    next_interrupt_op(&mut op_rx);
    chat.on_interrupted_turn(TurnAbortReason::Interrupted);

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => {
            let [
                UserInput::Text {
                    text: submitted, ..
                },
            ] = items.as_slice()
            else {
                panic!("expected resubmitted text item, got {items:?}");
            };
            assert_eq!(submitted, text);
        }
        other => panic!("expected resubmitted user turn, got {other:?}"),
    }
    assert_eq!(next_add_to_history_op(&mut op_rx), encoded);
}

#[tokio::test]
async fn slash_rename_prefills_existing_thread_name() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_name = Some("Current project title".to_string());

    chat.dispatch_command(SlashCommand::Rename);

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert_chatwidget_snapshot!("slash_rename_prefilled_prompt", popup);

    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_matches!(
        rx.try_recv(),
        Ok(AppEvent::CodexOp(Op::SetThreadName { name })) if name == "Current project title"
    );
}

#[tokio::test]
async fn slash_rename_without_existing_thread_name_starts_empty() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.dispatch_command(SlashCommand::Rename);

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert!(popup.contains("Name thread"));
    assert!(popup.contains("Type a name and press Enter"));

    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_matches!(rx.try_recv(), Err(TryRecvError::Empty));
}

#[tokio::test]
async fn usage_error_slash_command_is_available_from_local_recall() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.3-codex")).await;
    chat.set_feature_enabled(Feature::FastMode, /*enabled*/ true);

    submit_composer_text(&mut chat, "/fast maybe");

    assert_eq!(chat.bottom_pane.composer_text(), "");

    let cells = drain_insert_history(&mut rx);
    let rendered = cells
        .iter()
        .map(|cell| lines_to_single_string(cell))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("Usage: /fast [on|off|status]"),
        "expected usage message, got: {rendered:?}"
    );
    assert_eq!(recall_latest_after_clearing(&mut chat), "/fast maybe");
}

#[tokio::test]
async fn unrecognized_slash_command_is_not_added_to_local_recall() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    submit_composer_text(&mut chat, "/does-not-exist");

    let cells = drain_insert_history(&mut rx);
    let rendered = cells
        .iter()
        .map(|cell| lines_to_single_string(cell))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("Unrecognized command '/does-not-exist'"),
        "expected unrecognized-command message, got: {rendered:?}"
    );
    assert_eq!(chat.bottom_pane.composer_text(), "/does-not-exist");
    assert_eq!(recall_latest_after_clearing(&mut chat), "");
}

#[tokio::test]
async fn unavailable_slash_command_is_available_from_local_recall() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.bottom_pane.set_task_running(/*running*/ true);

    submit_composer_text(&mut chat, "/model");

    let cells = drain_insert_history(&mut rx);
    let rendered = cells
        .iter()
        .map(|cell| lines_to_single_string(cell))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("'/model' is disabled while a task is in progress."),
        "expected disabled-command message, got: {rendered:?}"
    );
    assert_eq!(recall_latest_after_clearing(&mut chat), "/model");
}

#[tokio::test]
async fn no_op_stub_slash_command_is_available_from_local_recall() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    submit_composer_text(&mut chat, "/debug-m-drop");

    let cells = drain_insert_history(&mut rx);
    let rendered = cells
        .iter()
        .map(|cell| lines_to_single_string(cell))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("Memory maintenance"),
        "expected stub message, got: {rendered:?}"
    );
    assert_eq!(recall_latest_after_clearing(&mut chat), "/debug-m-drop");
}

#[tokio::test]
async fn slash_quit_requests_exit() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.dispatch_command(SlashCommand::Quit);

    assert_matches!(rx.try_recv(), Ok(AppEvent::Exit(ExitMode::ShutdownFirst)));
}

#[tokio::test]
async fn slash_logout_requests_app_server_logout() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.dispatch_command(SlashCommand::Logout);

    assert_matches!(rx.try_recv(), Ok(AppEvent::Logout));
}

#[tokio::test]
async fn slash_copy_state_tracks_turn_complete_final_reply() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_codex_event(Event {
        id: "turn-1".into(),
        msg: EventMsg::TurnComplete(TurnCompleteEvent {
            turn_id: "turn-1".to_string(),
            last_agent_message: Some("Final reply **markdown**".to_string()),
            completed_at: None,
            duration_ms: None,
            time_to_first_token_ms: None,
        }),
    });

    assert_eq!(
        chat.last_agent_markdown_text(),
        Some("Final reply **markdown**")
    );
}

#[tokio::test]
async fn slash_copy_state_tracks_plan_item_completion() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let plan_text = "## Plan\n\n1. Build it\n2. Test it".to_string();

    chat.handle_codex_event(Event {
        id: "item-plan".into(),
        msg: EventMsg::ItemCompleted(ItemCompletedEvent {
            thread_id: ThreadId::new(),
            turn_id: "turn-1".to_string(),
            item: TurnItem::Plan(PlanItem {
                id: "plan-1".to_string(),
                text: plan_text.clone(),
            }),
        }),
    });
    chat.handle_codex_event(Event {
        id: "turn-1".into(),
        msg: EventMsg::TurnComplete(TurnCompleteEvent {
            turn_id: "turn-1".to_string(),
            last_agent_message: None,
            completed_at: None,
            duration_ms: None,
            time_to_first_token_ms: None,
        }),
    });

    assert_eq!(chat.last_agent_markdown_text(), Some(plan_text.as_str()));
    assert_matches!(
        chat.pending_notification,
        Some(Notification::AgentTurnComplete { ref response }) if response == &plan_text
    );
}

#[tokio::test]
async fn slash_copy_reports_when_no_agent_response_exists() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.dispatch_command(SlashCommand::Copy);

    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 1, "expected one info message");
    let rendered = lines_to_single_string(&cells[0]);
    assert_chatwidget_snapshot!("slash_copy_no_output_info_message", rendered);
    assert!(
        rendered.contains("No agent response to copy"),
        "expected no-output message, got {rendered:?}"
    );
}

#[tokio::test]
async fn ctrl_o_copy_reports_when_no_agent_response_exists() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_key_event(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL));

    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 1, "expected one info message");
    let rendered = lines_to_single_string(&cells[0]);
    assert!(
        rendered.contains("No agent response to copy"),
        "expected no-output message, got {rendered:?}"
    );
}

#[tokio::test]
async fn slash_copy_stores_clipboard_lease_and_preserves_it_on_failure() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.last_agent_markdown = Some("copy me".to_string());

    chat.copy_last_agent_markdown_with(|markdown| {
        assert_eq!(markdown, "copy me");
        Ok(Some(crate::clipboard_copy::ClipboardLease::test()))
    });

    assert!(chat.clipboard_lease.is_some());
    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 1, "expected one success message");
    let rendered = lines_to_single_string(&cells[0]);
    assert!(
        rendered.contains("Copied last message to clipboard"),
        "expected success message, got {rendered:?}"
    );

    chat.copy_last_agent_markdown_with(|markdown| {
        assert_eq!(markdown, "copy me");
        Err("blocked".into())
    });

    assert!(chat.clipboard_lease.is_some());
    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 1, "expected one failure message");
    let rendered = lines_to_single_string(&cells[0]);
    assert!(
        rendered.contains("Copy failed: blocked"),
        "expected failure message, got {rendered:?}"
    );
}

#[tokio::test]
async fn slash_copy_state_is_preserved_during_running_task() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_codex_event(Event {
        id: "turn-1".into(),
        msg: EventMsg::TurnComplete(TurnCompleteEvent {
            turn_id: "turn-1".to_string(),
            last_agent_message: Some("Previous completed reply".to_string()),
            completed_at: None,
            duration_ms: None,
            time_to_first_token_ms: None,
        }),
    });
    chat.on_task_started();

    assert_eq!(
        chat.last_agent_markdown_text(),
        Some("Previous completed reply")
    );
}

#[tokio::test]
async fn slash_copy_tracks_replayed_legacy_agent_message_when_turn_complete_omits_text() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_codex_event_replay(Event {
        id: "turn-1".into(),
        msg: EventMsg::AgentMessage(AgentMessageEvent {
            message: "Legacy final message".into(),
            phase: None,
            memory_citation: None,
        }),
    });
    let _ = drain_insert_history(&mut rx);
    chat.handle_codex_event(Event {
        id: "turn-1".into(),
        msg: EventMsg::TurnComplete(TurnCompleteEvent {
            turn_id: "turn-1".to_string(),
            last_agent_message: None,
            completed_at: None,
            duration_ms: None,
            time_to_first_token_ms: None,
        }),
    });
    let _ = drain_insert_history(&mut rx);

    assert_eq!(
        chat.last_agent_markdown_text(),
        Some("Legacy final message")
    );
}

#[tokio::test]
async fn slash_copy_uses_agent_message_item_when_turn_complete_omits_final_text() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_codex_event(Event {
        id: "turn-1".into(),
        msg: EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: "turn-1".to_string(),
            started_at: None,
            model_context_window: None,
            collaboration_mode_kind: ModeKind::Default,
        }),
    });
    complete_assistant_message(
        &mut chat,
        "msg-1",
        "Legacy item final message",
        /*phase*/ None,
    );
    let _ = drain_insert_history(&mut rx);
    chat.handle_codex_event(Event {
        id: "turn-1".into(),
        msg: EventMsg::TurnComplete(TurnCompleteEvent {
            turn_id: "turn-1".to_string(),
            last_agent_message: None,
            completed_at: None,
            duration_ms: None,
            time_to_first_token_ms: None,
        }),
    });
    let _ = drain_insert_history(&mut rx);

    assert_eq!(
        chat.last_agent_markdown_text(),
        Some("Legacy item final message")
    );
    assert_matches!(
        chat.pending_notification,
        Some(Notification::AgentTurnComplete { ref response }) if response == "Legacy item final message"
    );
}

#[tokio::test]
async fn agent_turn_complete_notification_does_not_reuse_stale_copy_source() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_codex_event(Event {
        id: "turn-1".into(),
        msg: EventMsg::TurnComplete(turn_complete_event("turn-1", Some("Previous reply"))),
    });
    chat.pending_notification = None;

    chat.handle_codex_event(Event {
        id: "turn-2".into(),
        msg: EventMsg::TurnComplete(turn_complete_event(
            "turn-2", /*last_agent_message*/ None,
        )),
    });

    assert_matches!(
        chat.pending_notification,
        Some(Notification::AgentTurnComplete { ref response }) if response.is_empty()
    );
}

#[tokio::test]
async fn active_goal_without_follow_up_sends_agent_turn_complete_notification() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Goals, /*enabled*/ true);
    chat.handle_server_notification(
        ServerNotification::ThreadGoalUpdated(
            codex_app_server_protocol::ThreadGoalUpdatedNotification {
                thread_id: "thread-1".to_string(),
                turn_id: None,
                goal: codex_app_server_protocol::ThreadGoal {
                    thread_id: "thread-1".to_string(),
                    objective: "finish the benchmark".to_string(),
                    status: codex_app_server_protocol::ThreadGoalStatus::Active,
                    token_budget: None,
                    tokens_used: 0,
                    time_used_seconds: 0,
                    created_at: 1,
                    updated_at: 1,
                },
            },
        ),
        /*replay_kind*/ None,
    );

    chat.handle_codex_event(Event {
        id: "turn-1".into(),
        msg: EventMsg::TurnComplete(turn_complete_event("turn-1", Some("Still working"))),
    });

    assert_matches!(
        chat.pending_notification,
        Some(Notification::AgentTurnComplete { ref response }) if response == "Still working"
    );
}

#[tokio::test]
async fn queued_follow_up_suppresses_agent_turn_complete_notification() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.handle_codex_event(Event {
        id: "turn-1".into(),
        msg: EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: "turn-1".to_string(),
            started_at: None,
            model_context_window: None,
            collaboration_mode_kind: ModeKind::Default,
        }),
    });
    chat.queue_user_message("Continue".into());

    chat.handle_codex_event(Event {
        id: "turn-1".into(),
        msg: EventMsg::TurnComplete(turn_complete_event("turn-1", Some("Still working"))),
    });

    assert_matches!(chat.pending_notification, None);
    assert!(chat.queued_user_messages.is_empty());
    assert_matches!(next_submit_op(&mut op_rx), Op::UserTurn { .. });
}

#[tokio::test]
async fn queued_menu_slash_keeps_agent_turn_complete_notification() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(Some("gpt-5.2")).await;
    chat.thread_id = Some(ThreadId::new());
    chat.handle_codex_event(Event {
        id: "turn-1".into(),
        msg: EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: "turn-1".to_string(),
            started_at: None,
            model_context_window: None,
            collaboration_mode_kind: ModeKind::Default,
        }),
    });
    queue_composer_text_with_tab(&mut chat, "/model");

    chat.handle_codex_event(Event {
        id: "turn-1".into(),
        msg: EventMsg::TurnComplete(turn_complete_event("turn-1", Some("Done"))),
    });

    assert_matches!(
        chat.pending_notification,
        Some(Notification::AgentTurnComplete { ref response }) if response == "Done"
    );
    assert!(render_bottom_popup(&chat, /*width*/ 80).contains("Select Model"));
    assert_matches!(op_rx.try_recv(), Err(TryRecvError::Empty));
}

#[tokio::test]
async fn slash_copy_uses_latest_surviving_response_after_rollback() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_codex_event_replay(Event {
        id: "user-1".into(),
        msg: EventMsg::UserMessage(UserMessageEvent {
            message: "foo".to_string(),
            images: None,
            local_images: Vec::new(),
            text_elements: Vec::new(),
        }),
    });
    chat.handle_codex_event_replay(Event {
        id: "agent-1".into(),
        msg: EventMsg::AgentMessage(AgentMessageEvent {
            message: "foo response".to_string(),
            phase: None,
            memory_citation: None,
        }),
    });
    chat.handle_codex_event_replay(Event {
        id: "user-2".into(),
        msg: EventMsg::UserMessage(UserMessageEvent {
            message: "bar".to_string(),
            images: None,
            local_images: Vec::new(),
            text_elements: Vec::new(),
        }),
    });
    chat.handle_codex_event_replay(Event {
        id: "agent-2".into(),
        msg: EventMsg::AgentMessage(AgentMessageEvent {
            message: "bar response".to_string(),
            phase: None,
            memory_citation: None,
        }),
    });
    let _ = drain_insert_history(&mut rx);
    assert_eq!(chat.last_agent_markdown_text(), Some("bar response"));

    chat.truncate_agent_copy_history_to_user_turn_count(/*user_turn_count*/ 1);

    assert_eq!(chat.last_agent_markdown_text(), Some("foo response"));
    chat.copy_last_agent_markdown_with(|markdown| {
        assert_eq!(markdown, "foo response");
        Ok(None)
    });
}

#[tokio::test]
async fn slash_copy_reports_when_rewind_exceeds_retained_copy_history() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_codex_event_replay(Event {
        id: "user-1".into(),
        msg: EventMsg::UserMessage(UserMessageEvent {
            message: "foo".to_string(),
            images: None,
            local_images: Vec::new(),
            text_elements: Vec::new(),
        }),
    });
    chat.handle_codex_event_replay(Event {
        id: "agent-1".into(),
        msg: EventMsg::AgentMessage(AgentMessageEvent {
            message: "foo response".to_string(),
            phase: None,
            memory_citation: None,
        }),
    });
    let _ = drain_insert_history(&mut rx);

    chat.truncate_agent_copy_history_to_user_turn_count(/*user_turn_count*/ 0);
    chat.dispatch_command(SlashCommand::Copy);

    let cells = drain_insert_history(&mut rx);
    let rendered = lines_to_single_string(&cells[0]);
    assert!(
        rendered.contains(
            "Cannot copy that response after rewinding. Only the most recent 32 responses are available to /copy."
        ),
        "expected evicted-history message, got {rendered:?}"
    );
}

#[tokio::test]
async fn slash_exit_requests_exit() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.dispatch_command(SlashCommand::Exit);

    assert_matches!(rx.try_recv(), Ok(AppEvent::Exit(ExitMode::ShutdownFirst)));
}

#[tokio::test]
async fn slash_stop_submits_background_terminal_cleanup() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.dispatch_command(SlashCommand::Stop);

    assert_matches!(op_rx.try_recv(), Ok(Op::CleanBackgroundTerminals));
    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 1, "expected cleanup confirmation message");
    let rendered = lines_to_single_string(&cells[0]);
    assert!(
        rendered.contains("Stopping all background terminals."),
        "expected cleanup confirmation, got {rendered:?}"
    );
}

#[tokio::test]
async fn slash_clear_requests_ui_clear_when_idle() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.dispatch_command(SlashCommand::Clear);

    assert_matches!(rx.try_recv(), Ok(AppEvent::ClearUi));
}

#[tokio::test]
async fn slash_clear_is_disabled_while_task_running() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.bottom_pane.set_task_running(/*running*/ true);

    chat.dispatch_command(SlashCommand::Clear);

    let event = rx.try_recv().expect("expected disabled command error");
    match event {
        AppEvent::InsertHistoryCell(cell) => {
            let rendered = lines_to_single_string(&cell.display_lines(/*width*/ 80));
            assert!(
                rendered.contains("'/clear' is disabled while a task is in progress."),
                "expected /clear task-running error, got {rendered:?}"
            );
        }
        other => panic!("expected InsertHistoryCell error, got {other:?}"),
    }
    assert!(rx.try_recv().is_err(), "expected no follow-up events");
}

#[tokio::test]
async fn slash_memory_drop_reports_stubbed_feature() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.dispatch_command(SlashCommand::MemoryDrop);

    let event = rx.try_recv().expect("expected unsupported-feature error");
    match event {
        AppEvent::InsertHistoryCell(cell) => {
            let rendered = lines_to_single_string(&cell.display_lines(/*width*/ 80));
            assert!(rendered.contains("Memory maintenance: Not available in TUI yet."));
        }
        other => panic!("expected InsertHistoryCell error, got {other:?}"),
    }
    assert!(
        op_rx.try_recv().is_err(),
        "expected no memory op to be sent"
    );
}

#[tokio::test]
async fn slash_mcp_requests_inventory_via_app_server() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.dispatch_command(SlashCommand::Mcp);

    assert!(active_blob(&chat).contains("Loading MCP inventory"));
    assert_matches!(
        rx.try_recv(),
        Ok(AppEvent::FetchMcpInventory {
            detail: McpServerStatusDetail::ToolsAndAuthOnly
        })
    );
    assert!(op_rx.try_recv().is_err(), "expected no core op to be sent");
}

#[tokio::test]
async fn slash_mcp_verbose_requests_full_inventory_via_app_server() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    submit_composer_text(&mut chat, "/mcp verbose");

    assert!(active_blob(&chat).contains("Loading MCP inventory"));
    assert_matches!(
        rx.try_recv(),
        Ok(AppEvent::FetchMcpInventory {
            detail: McpServerStatusDetail::Full
        })
    );
    assert!(op_rx.try_recv().is_err(), "expected no core op to be sent");
}

#[tokio::test]
async fn slash_mcp_invalid_args_show_usage() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    submit_composer_text(&mut chat, "/mcp full");

    let cells = drain_insert_history(&mut rx);
    let rendered = cells
        .iter()
        .map(|cell| lines_to_single_string(cell))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        rendered.contains("Usage: /mcp [verbose]"),
        "expected usage message, got: {rendered:?}"
    );
    assert_eq!(recall_latest_after_clearing(&mut chat), "/mcp full");
    assert!(op_rx.try_recv().is_err(), "expected no core op to be sent");
}

#[tokio::test]
async fn slash_memories_opens_memory_menu() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::MemoryTool, /*enabled*/ true);

    chat.dispatch_command(SlashCommand::Memories);

    assert!(render_bottom_popup(&chat, /*width*/ 80).contains("Use memories"));
    assert_matches!(rx.try_recv(), Err(TryRecvError::Empty));
    assert!(op_rx.try_recv().is_err(), "expected no core op to be sent");
}

#[tokio::test]
async fn slash_memory_update_reports_stubbed_feature() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.dispatch_command(SlashCommand::MemoryUpdate);

    let event = rx.try_recv().expect("expected unsupported-feature error");
    match event {
        AppEvent::InsertHistoryCell(cell) => {
            let rendered = lines_to_single_string(&cell.display_lines(/*width*/ 80));
            assert!(rendered.contains("Memory maintenance: Not available in TUI yet."));
        }
        other => panic!("expected InsertHistoryCell error, got {other:?}"),
    }
    assert!(
        op_rx.try_recv().is_err(),
        "expected no memory op to be sent"
    );
}

#[tokio::test]
async fn slash_resume_opens_picker() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.dispatch_command(SlashCommand::Resume);

    assert_matches!(rx.try_recv(), Ok(AppEvent::OpenResumePicker));
}

#[tokio::test]
async fn slash_resume_with_arg_requests_named_session() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.bottom_pane.set_composer_text(
        "/resume my-saved-thread".to_string(),
        Vec::new(),
        Vec::new(),
    );
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    assert_matches!(
        rx.try_recv(),
        Ok(AppEvent::ResumeSessionByIdOrName(id_or_name)) if id_or_name == "my-saved-thread"
    );
    assert_matches!(op_rx.try_recv(), Err(TryRecvError::Empty));
}

#[tokio::test]
async fn slash_fork_requests_current_fork() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.dispatch_command(SlashCommand::Fork);

    assert_matches!(rx.try_recv(), Ok(AppEvent::ForkCurrentSession));
}

#[tokio::test]
async fn slash_rollout_displays_current_path() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let rollout_path = PathBuf::from("/tmp/codex-test-rollout.jsonl");
    chat.current_rollout_path = Some(rollout_path.clone());

    chat.dispatch_command(SlashCommand::Rollout);

    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 1, "expected info message for rollout path");
    let rendered = lines_to_single_string(&cells[0]);
    assert!(
        rendered.contains(&rollout_path.display().to_string()),
        "expected rollout path to be shown: {rendered}"
    );
}

#[tokio::test]
async fn slash_rollout_handles_missing_path() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.dispatch_command(SlashCommand::Rollout);

    let cells = drain_insert_history(&mut rx);
    assert_eq!(
        cells.len(),
        1,
        "expected info message explaining missing path"
    );
    let rendered = lines_to_single_string(&cells[0]);
    assert!(
        rendered.contains("not available"),
        "expected missing rollout path message: {rendered}"
    );
}

#[tokio::test]
async fn undo_success_events_render_info_messages() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_codex_event(Event {
        id: "turn-1".to_string(),
        msg: EventMsg::UndoStarted(UndoStartedEvent {
            message: Some("Undo requested for the last turn...".to_string()),
        }),
    });
    assert!(
        chat.bottom_pane.status_indicator_visible(),
        "status indicator should be visible during undo"
    );

    chat.handle_codex_event(Event {
        id: "turn-1".to_string(),
        msg: EventMsg::UndoCompleted(UndoCompletedEvent {
            success: true,
            message: None,
        }),
    });

    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 1, "expected final status only");
    assert!(
        !chat.bottom_pane.status_indicator_visible(),
        "status indicator should be hidden after successful undo"
    );

    let completed = lines_to_single_string(&cells[0]);
    assert!(
        completed.contains("Undo completed successfully."),
        "expected default success message, got {completed:?}"
    );
}

#[tokio::test]
async fn undo_failure_events_render_error_message() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_codex_event(Event {
        id: "turn-2".to_string(),
        msg: EventMsg::UndoStarted(UndoStartedEvent { message: None }),
    });
    assert!(
        chat.bottom_pane.status_indicator_visible(),
        "status indicator should be visible during undo"
    );

    chat.handle_codex_event(Event {
        id: "turn-2".to_string(),
        msg: EventMsg::UndoCompleted(UndoCompletedEvent {
            success: false,
            message: Some("Failed to restore workspace state.".to_string()),
        }),
    });

    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 1, "expected final status only");
    assert!(
        !chat.bottom_pane.status_indicator_visible(),
        "status indicator should be hidden after failed undo"
    );

    let completed = lines_to_single_string(&cells[0]);
    assert!(
        completed.contains("Failed to restore workspace state."),
        "expected failure message, got {completed:?}"
    );
}

#[tokio::test]
async fn undo_started_hides_interrupt_hint() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_codex_event(Event {
        id: "turn-hint".to_string(),
        msg: EventMsg::UndoStarted(UndoStartedEvent { message: None }),
    });

    let status = chat
        .bottom_pane
        .status_widget()
        .expect("status indicator should be active");
    assert!(
        !status.interrupt_hint_visible(),
        "undo should hide the interrupt hint because the operation cannot be cancelled"
    );
}

#[tokio::test]
async fn fast_slash_command_updates_and_persists_local_service_tier() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(Some("gpt-5.3-codex")).await;
    chat.set_feature_enabled(Feature::FastMode, /*enabled*/ true);

    chat.dispatch_command(SlashCommand::Fast);

    let events = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(
        events.iter().any(|event| matches!(
            event,
            AppEvent::CodexOp(Op::OverrideTurnContext {
                service_tier: Some(Some(ServiceTier::Fast)),
                ..
            })
        )),
        "expected fast-mode override app event; events: {events:?}"
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            AppEvent::PersistServiceTierSelection {
                service_tier: Some(ServiceTier::Fast),
            }
        )),
        "expected fast-mode persistence app event; events: {events:?}"
    );

    assert_matches!(op_rx.try_recv(), Err(TryRecvError::Empty));
}

#[tokio::test]
async fn user_turn_carries_service_tier_after_fast_toggle() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(Some("gpt-5.3-codex")).await;
    chat.thread_id = Some(ThreadId::new());
    set_chatgpt_auth(&mut chat);
    chat.set_feature_enabled(Feature::FastMode, /*enabled*/ true);

    chat.dispatch_command(SlashCommand::Fast);

    let _events = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();

    chat.bottom_pane
        .set_composer_text("hello".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    match next_submit_op(&mut op_rx) {
        Op::UserTurn {
            service_tier: Some(Some(ServiceTier::Fast)),
            ..
        } => {}
        other => panic!("expected Op::UserTurn with fast service tier, got {other:?}"),
    }
}

#[tokio::test]
async fn queued_fast_slash_applies_before_next_queued_message() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(Some("gpt-5.3-codex")).await;
    chat.thread_id = Some(ThreadId::new());
    set_chatgpt_auth(&mut chat);
    chat.set_feature_enabled(Feature::FastMode, /*enabled*/ true);
    chat.handle_codex_event(Event {
        id: "turn-start".into(),
        msg: EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: "turn-1".to_string(),
            started_at: None,
            model_context_window: None,
            collaboration_mode_kind: ModeKind::Default,
        }),
    });

    queue_composer_text_with_tab(&mut chat, "/fast on");
    queue_composer_text_with_tab(&mut chat, "hello after fast");

    chat.handle_codex_event(Event {
        id: "turn-complete".into(),
        msg: EventMsg::TurnComplete(turn_complete_event("turn-1", Some("done"))),
    });

    let events = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(
        events.iter().any(|event| matches!(
            event,
            AppEvent::CodexOp(Op::OverrideTurnContext {
                service_tier: Some(Some(ServiceTier::Fast)),
                ..
            })
        )),
        "expected queued /fast to update service tier before next turn; events: {events:?}"
    );

    match next_submit_op(&mut op_rx) {
        Op::UserTurn {
            items,
            service_tier: Some(Some(ServiceTier::Fast)),
            ..
        } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "hello after fast".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected queued message to submit with fast tier, got {other:?}"),
    }
}

#[tokio::test]
async fn user_turn_sends_standard_override_after_fast_is_turned_off() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(Some("gpt-5.3-codex")).await;
    chat.thread_id = Some(ThreadId::new());
    set_chatgpt_auth(&mut chat);
    chat.set_feature_enabled(Feature::FastMode, /*enabled*/ true);

    chat.dispatch_command(SlashCommand::Fast);
    let _events = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();

    chat.dispatch_command_with_args(SlashCommand::Fast, "off".to_string(), Vec::new());
    let events = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(
        events.iter().any(|event| matches!(
            event,
            AppEvent::CodexOp(Op::OverrideTurnContext {
                service_tier: Some(None),
                ..
            })
        )),
        "expected fast-mode off override app event; events: {events:?}"
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            AppEvent::PersistServiceTierSelection { service_tier: None }
        )),
        "expected fast-mode opt-out persistence app event; events: {events:?}"
    );

    chat.bottom_pane
        .set_composer_text("hello".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    match next_submit_op(&mut op_rx) {
        Op::UserTurn {
            service_tier: Some(None),
            ..
        } => {}
        other => panic!("expected Op::UserTurn with standard service tier override, got {other:?}"),
    }
}

#[tokio::test]
async fn compact_queues_user_messages_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
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

    chat.submit_user_message(UserMessage::from(
        "Steer submitted while /compact was running.".to_string(),
    ));
    chat.handle_codex_event(Event {
        id: "steer-rejected".into(),
        msg: EventMsg::Error(ErrorEvent {
            message: "cannot steer a compact turn".to_string(),
            codex_error_info: Some(CodexErrorInfo::ActiveTurnNotSteerable {
                turn_kind: NonSteerableTurnKind::Compact,
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
        "compact_queues_user_messages_snapshot",
        normalize_snapshot_paths(term.backend().vt100().screen().contents())
    );
}
