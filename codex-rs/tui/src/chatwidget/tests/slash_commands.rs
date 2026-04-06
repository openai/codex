use super::*;
use pretty_assertions::assert_eq;

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
    let existing_path = tempdir.path().join(DEFAULT_PROJECT_DOC_FILENAME);
    std::fs::write(&existing_path, "existing instructions").unwrap();
    chat.config.cwd = tempdir.path().to_path_buf().abs();

    chat.dispatch_command(SlashCommand::Init);

    match op_rx.try_recv() {
        Err(TryRecvError::Empty) => {}
        other => panic!("expected no Codex op to be sent, got {other:?}"),
    }

    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 1, "expected one info message");
    let rendered = lines_to_single_string(&cells[0]);
    assert!(
        rendered.contains(DEFAULT_PROJECT_DOC_FILENAME),
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
}

#[tokio::test]
async fn slash_quit_requests_exit() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.dispatch_command(SlashCommand::Quit);

    assert_matches!(rx.try_recv(), Ok(AppEvent::Exit(ExitMode::ShutdownFirst)));
}

#[tokio::test]
async fn slash_cwd_requires_inline_path() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.dispatch_command(SlashCommand::Cwd);

    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 1, "expected one error message");
    let rendered = lines_to_single_string(&cells[0]);
    assert!(rendered.contains("Usage: /cwd <directory>"));
}

#[tokio::test]
async fn slash_cwd_dispatches_change_event_with_args() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.bottom_pane
        .set_composer_text("/cwd ../other".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_matches!(
        rx.try_recv(),
        Ok(AppEvent::ChangeCwd { path }) if path == "../other"
    );
}

#[tokio::test]
async fn slash_copy_state_tracks_turn_complete_final_reply() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_codex_event(Event {
        id: "turn-1".into(),
        msg: EventMsg::TurnComplete(TurnCompleteEvent {
            turn_id: "turn-1".to_string(),
            last_agent_message: Some("Final reply **markdown**".to_string()),
        }),
    });

    assert_eq!(
        chat.last_copyable_output,
        Some("Final reply **markdown**".to_string())
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
        }),
    });

    assert_eq!(chat.last_copyable_output, Some(plan_text));
}

#[tokio::test]
async fn slash_copy_reports_when_no_copyable_output_exists() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.dispatch_command(SlashCommand::Copy);

    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 1, "expected one info message");
    let rendered = lines_to_single_string(&cells[0]);
    assert_chatwidget_snapshot!("slash_copy_no_output_info_message", rendered);
    assert!(
        rendered.contains(
            "`/copy` is unavailable before the first Codex output or right after a rollback."
        ),
        "expected no-output message, got {rendered:?}"
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
        }),
    });
    chat.on_task_started();

    assert_eq!(
        chat.last_copyable_output,
        Some("Previous completed reply".to_string())
    );
}

#[tokio::test]
async fn slash_copy_state_clears_on_thread_rollback() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_codex_event(Event {
        id: "turn-1".into(),
        msg: EventMsg::TurnComplete(TurnCompleteEvent {
            turn_id: "turn-1".to_string(),
            last_agent_message: Some("Reply that will be rolled back".to_string()),
        }),
    });
    chat.handle_codex_event(Event {
        id: "rollback-1".into(),
        msg: EventMsg::ThreadRolledBack(ThreadRolledBackEvent { num_turns: 1 }),
    });

    assert_eq!(chat.last_copyable_output, None);
}

#[tokio::test]
async fn slash_copy_is_unavailable_when_legacy_agent_message_is_not_repeated_on_turn_complete() {
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
        }),
    });
    let _ = drain_insert_history(&mut rx);

    chat.dispatch_command(SlashCommand::Copy);

    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 1, "expected one info message");
    let rendered = lines_to_single_string(&cells[0]);
    assert!(
        rendered.contains(
            "`/copy` is unavailable before the first Codex output or right after a rollback."
        ),
        "expected unavailable message, got {rendered:?}"
    );
}

#[tokio::test]
async fn slash_copy_uses_agent_message_item_when_turn_complete_omits_final_text() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_codex_event(Event {
        id: "turn-1".into(),
        msg: EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: "turn-1".to_string(),
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
        }),
    });
    let _ = drain_insert_history(&mut rx);

    chat.dispatch_command(SlashCommand::Copy);

    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 1, "expected one info message");
    let rendered = lines_to_single_string(&cells[0]);
    assert!(
        !rendered.contains(
            "`/copy` is unavailable before the first Codex output or right after a rollback."
        ),
        "expected copy state to be available, got {rendered:?}"
    );
    assert_eq!(
        chat.last_copyable_output,
        Some("Legacy item final message".to_string())
    );
}

#[tokio::test]
async fn slash_copy_does_not_return_stale_output_after_thread_rollback() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_codex_event(Event {
        id: "turn-1".into(),
        msg: EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: "turn-1".to_string(),
            model_context_window: None,
            collaboration_mode_kind: ModeKind::Default,
        }),
    });
    complete_assistant_message(
        &mut chat,
        "msg-1",
        "Reply that will be rolled back",
        /*phase*/ None,
    );
    let _ = drain_insert_history(&mut rx);
    chat.handle_codex_event(Event {
        id: "turn-1".into(),
        msg: EventMsg::TurnComplete(TurnCompleteEvent {
            turn_id: "turn-1".to_string(),
            last_agent_message: None,
        }),
    });
    let _ = drain_insert_history(&mut rx);

    chat.handle_codex_event(Event {
        id: "rollback-1".into(),
        msg: EventMsg::ThreadRolledBack(ThreadRolledBackEvent { num_turns: 1 }),
    });
    let _ = drain_insert_history(&mut rx);

    chat.dispatch_command(SlashCommand::Copy);

    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 1, "expected one info message");
    let rendered = lines_to_single_string(&cells[0]);
    assert!(
        rendered.contains(
            "`/copy` is unavailable before the first Codex output or right after a rollback."
        ),
        "expected rollback-cleared copy state message, got {rendered:?}"
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
    assert_matches!(rx.try_recv(), Ok(AppEvent::FetchMcpInventory));
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
async fn user_turn_clears_service_tier_after_fast_is_turned_off() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(Some("gpt-5.3-codex")).await;
    chat.thread_id = Some(ThreadId::new());
    set_chatgpt_auth(&mut chat);
    chat.set_feature_enabled(Feature::FastMode, /*enabled*/ true);

    chat.dispatch_command(SlashCommand::Fast);
    let _events = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();

    chat.dispatch_command_with_args(SlashCommand::Fast, "off".to_string(), Vec::new());
    let _events = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();

    chat.bottom_pane
        .set_composer_text("hello".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    match next_submit_op(&mut op_rx) {
        Op::UserTurn {
            service_tier: Some(None),
            ..
        } => {}
        other => panic!("expected Op::UserTurn to clear service tier, got {other:?}"),
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
