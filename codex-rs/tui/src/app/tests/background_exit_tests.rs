use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn daemon_disconnect_exit_summary_includes_reconnect_and_stop_instructions() -> Result<()> {
    let (mut app, _, _) = make_test_app_with_channels().await;
    let thread_id = prepare_running_local_daemon(&mut app)?;
    app.keymap.agents.stop = vec![crate::key_hint::plain(KeyCode::F(10))];
    let mut exit_info = app.exit_info(ExitReason::UserRequested);
    exit_info.token_usage = TokenUsage {
        input_tokens: 10,
        output_tokens: 2,
        total_tokens: 12,
        ..Default::default()
    };
    let output = exit_info
        .format_exit_messages(/*color_enabled*/ false)
        .join("\n")
        .replace(&thread_id.to_string(), "THREAD_ID");
    assert_snapshot!("daemon_disconnect_exit", output);
    Ok(())
}

#[tokio::test]
async fn remote_disconnect_exit_summary_does_not_require_a_local_rollout_or_print_credentials() {
    let (mut app, _, _) = make_test_app_with_channels().await;
    app.app_server_target = AppServerTarget::Remote {
        endpoint: crate::RemoteAppServerEndpoint::WebSocket {
            websocket_url: "wss://user:secret@example.com:443/?token=private#secret".to_string(),
            auth_token: Some("secret-token".to_string()),
        },
    };
    let thread_id = ThreadId::from_string("123e4567-e89b-12d3-a456-426614174000").unwrap();
    app.active_thread_id = Some(thread_id);
    app.chat_widget.handle_thread_session(test_thread_session(
        thread_id,
        test_path_buf("/tmp/project"),
    ));
    let exit_info = app.exit_info(ExitReason::Fatal("connection lost".to_string()));
    let lines = exit_info.format_exit_messages(/*color_enabled*/ false);
    let command = shlex::split(lines[1].strip_prefix("Reconnect: ").unwrap()).unwrap();
    assert_eq!(
        crate::resolve_remote_addr(&command[2]).unwrap(),
        crate::RemoteAppServerEndpoint::WebSocket {
            websocket_url: "wss://example.com/".to_string(),
            auth_token: None,
        }
    );
    assert_snapshot!("remote_disconnect_exit", lines.join("\n"));
}

#[tokio::test]
async fn embedded_exit_keeps_the_session_summary() {
    let (mut app, _, _) = make_test_app_with_channels().await;
    prepare_local_daemon_thread(&mut app).unwrap();
    app.app_server_target = AppServerTarget::Embedded;
    let mut exit_info = app.exit_info(ExitReason::UserRequested);
    exit_info.token_usage = TokenUsage {
        output_tokens: 2,
        total_tokens: 2,
        ..Default::default()
    };
    exit_info.resume_hint = Some("codex resume THREAD_ID".to_string());
    assert_eq!(
        exit_info.format_exit_messages(/*color_enabled*/ false),
        vec![
            "Token usage: total=2 input=0 output=2",
            "To continue this session, run codex resume THREAD_ID",
        ]
    );
}

fn prepare_local_daemon_thread(app: &mut App) -> Result<ThreadId> {
    app.app_server_target = AppServerTarget::LocalDaemon {
        endpoint: crate::RemoteAppServerEndpoint::UnixSocket {
            socket_path: AbsolutePathBuf::relative_to_current_dir("codex.sock")?,
        },
    };
    let thread_id = ThreadId::new();
    app.active_thread_id = Some(thread_id);
    app.chat_widget.handle_thread_session(test_thread_session(
        thread_id,
        test_path_buf("/tmp/project"),
    ));
    Ok(thread_id)
}

fn prepare_running_local_daemon(app: &mut App) -> Result<ThreadId> {
    let thread_id = prepare_local_daemon_thread(app)?;
    app.chat_widget.handle_server_notification(
        turn_started_notification(thread_id, "turn-1"),
        /*replay_kind*/ None,
    );
    Ok(thread_id)
}

async fn open_running_task_exit_menu(
    app: &mut App,
    tui: &mut tui::Tui,
    app_server: &mut AppServerSession,
) {
    app.handle_key_event(
        tui,
        app_server,
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
    )
    .await;
}

async fn prepare_background_exit_test(
    app: &App,
    app_event_rx: &mut mpsc::UnboundedReceiver<AppEvent>,
    op_rx: &mut mpsc::UnboundedReceiver<Op>,
) -> Result<(AppServerSession, tui::Tui)> {
    while app_event_rx.try_recv().is_ok() {}
    while op_rx.try_recv().is_ok() {}
    let app_server =
        crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref()).await?;
    Ok((app_server, crate::tui::test_support::make_test_tui()?))
}

#[tokio::test]
async fn daemon_ctrl_c_shows_background_exit_menu_and_escape_dismisses_it() -> Result<()> {
    let (mut app, mut app_event_rx, mut op_rx) = make_test_app_with_channels().await;
    prepare_running_local_daemon(&mut app)?;
    let (mut app_server, mut tui) =
        prepare_background_exit_test(&app, &mut app_event_rx, &mut op_rx).await?;

    open_running_task_exit_menu(&mut app, &mut tui, &mut app_server).await;

    assert!(!app.chat_widget.no_modal_or_popup_active());
    assert_snapshot!(render_bottom_popup(&app.chat_widget, /*width*/ 90), @r"
      Task is still running
      Choose what happens to the current task.

    › 1. Cancel task        Stop the current task and stay in Codex
      2. Run in background  Exit Codex and leave the task running
      3. Exit               Stop the current task and exit Codex

      Press enter to confirm or esc to go back
    ");

    app.handle_key_event(
        &mut tui,
        &mut app_server,
        KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
    )
    .await;

    assert!(app.chat_widget.no_modal_or_popup_active());
    assert!(op_rx.try_recv().is_err());
    assert!(app_event_rx.try_recv().is_err());
    Ok(())
}

#[tokio::test]
async fn cancel_task_interrupts_without_exiting() -> Result<()> {
    let (mut app, mut app_event_rx, mut op_rx) = make_test_app_with_channels().await;
    prepare_running_local_daemon(&mut app)?;
    let (mut app_server, mut tui) =
        prepare_background_exit_test(&app, &mut app_event_rx, &mut op_rx).await?;
    open_running_task_exit_menu(&mut app, &mut tui, &mut app_server).await;

    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    let event = app_event_rx.try_recv()?;
    assert_matches!(
        event,
        AppEvent::RunningTaskExit {
            action: RunningTaskExitAction::CancelTask,
            ..
        }
    );
    let control = app.handle_event(&mut tui, &mut app_server, event).await?;

    assert_matches!(control, AppRunControl::Continue);
    assert_matches!(op_rx.try_recv(), Ok(Op::Interrupt));
    assert!(app_event_rx.try_recv().is_err());
    Ok(())
}

#[tokio::test]
async fn run_in_background_detaches_without_interrupting_main_or_side_threads() -> Result<()> {
    let (mut app, mut app_event_rx, mut op_rx) = make_test_app_with_channels().await;
    let parent_thread_id = prepare_running_local_daemon(&mut app)?;
    let side_thread_id = ThreadId::new();
    app.side_threads
        .insert(side_thread_id, SideThreadState::new(parent_thread_id));
    let (mut app_server, mut tui) =
        prepare_background_exit_test(&app, &mut app_event_rx, &mut op_rx).await?;
    open_running_task_exit_menu(&mut app, &mut tui, &mut app_server).await;

    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    let event = app_event_rx.try_recv()?;
    assert_matches!(
        event,
        AppEvent::RunningTaskExit {
            action: RunningTaskExitAction::RunInBackground,
            thread_id,
        } if thread_id == parent_thread_id
    );
    let control = app.handle_event(&mut tui, &mut app_server, event).await?;

    assert_matches!(control, AppRunControl::Exit(ExitReason::UserRequested));
    assert!(app.side_threads.contains_key(&side_thread_id));
    assert!(op_rx.try_recv().is_err());
    assert!(app_event_rx.try_recv().is_err());
    Ok(())
}

#[tokio::test]
async fn exit_interrupts_before_requesting_shutdown() -> Result<()> {
    let (mut app, mut app_event_rx, mut op_rx) = make_test_app_with_channels().await;
    prepare_running_local_daemon(&mut app)?;
    app.chat_widget
        .set_feature_enabled(Feature::Goals, /*enabled*/ true);
    let (mut app_server, mut tui) =
        prepare_background_exit_test(&app, &mut app_event_rx, &mut op_rx).await?;
    let started = app_server
        .start_thread(app.chat_widget.config_ref())
        .await?;
    let thread_id = started.session.thread_id;
    app.active_thread_id = Some(thread_id);
    app.chat_widget
        .handle_thread_session(started.session.clone());
    let goal = app_server
        .thread_goal_set(
            thread_id,
            Some("test goal".to_string()),
            Some(codex_app_server_protocol::ThreadGoalStatus::Paused),
            /*token_budget*/ None,
        )
        .await?
        .goal;
    app.chat_widget.handle_server_notification(
        ServerNotification::ThreadGoalUpdated(
            codex_app_server_protocol::ThreadGoalUpdatedNotification {
                thread_id: thread_id.to_string(),
                turn_id: None,
                goal: codex_app_server_protocol::ThreadGoal {
                    status: codex_app_server_protocol::ThreadGoalStatus::Active,
                    ..goal
                },
            },
        ),
        /*replay_kind*/ None,
    );
    let command = if cfg!(windows) {
        "Start-Sleep -Seconds 30"
    } else {
        "sleep 30"
    };
    app_server
        .thread_shell_command(thread_id, command.to_string())
        .await?;
    let turn_id = loop {
        let event = time::timeout(Duration::from_secs(/*secs*/ 5), app_server.next_event())
            .await
            .expect("app-server should emit a turn/start event")
            .expect("app-server event stream should remain open");
        if let codex_app_server_client::AppServerEvent::ServerNotification(notification) = event
            && let ServerNotification::TurnStarted(notification) = notification.as_ref()
            && notification.thread_id == thread_id.to_string()
        {
            break notification.turn.id.clone();
        }
    };
    app.thread_event_channels.insert(
        thread_id,
        ThreadEventChannel::new_with_session(
            THREAD_EVENT_CHANNEL_CAPACITY,
            started.session,
            vec![test_turn(&turn_id, TurnStatus::InProgress, Vec::new())],
        ),
    );
    app.chat_widget.handle_server_notification(
        turn_started_notification(thread_id, &turn_id),
        /*replay_kind*/ None,
    );
    while app_event_rx.try_recv().is_ok() {}
    while op_rx.try_recv().is_ok() {}
    let control = app
        .handle_event(
            &mut tui,
            &mut app_server,
            AppEvent::RunningTaskExit {
                action: RunningTaskExitAction::Exit,
                thread_id,
            },
        )
        .await?;

    assert_matches!(control, AppRunControl::Continue);
    assert!(op_rx.try_recv().is_err());
    let exit_event = app_event_rx.try_recv()?;
    assert_matches!(exit_event, AppEvent::Exit(ExitMode::ShutdownAfterInterrupt));
    let AppRunControl::Exit(reason) = app
        .handle_event(&mut tui, &mut app_server, exit_event)
        .await?
    else {
        panic!("stop-and-exit must exit");
    };
    assert_matches!(reason, ExitReason::TurnInterrupted);
    let output = app
        .exit_info(reason)
        .format_exit_messages(/*color_enabled*/ false)
        .join("\n")
        .replace(&thread_id.to_string(), "THREAD_ID");
    assert_snapshot!("interrupted_disconnect_exit", output);
    Ok(())
}

#[tokio::test]
async fn daemon_ctrl_c_closes_running_side_thread_and_returns_to_parent() -> Result<()> {
    let (mut app, mut app_event_rx, mut op_rx) = make_test_app_with_channels().await;
    let side_thread_id = prepare_running_local_daemon(&mut app)?;
    let (mut app_server, mut tui) =
        prepare_background_exit_test(&app, &mut app_event_rx, &mut op_rx).await?;
    let started = app_server
        .start_thread(app.chat_widget.config_ref())
        .await?;
    let parent_thread_id = started.session.thread_id;
    app.primary_thread_id = Some(parent_thread_id);
    app.side_threads
        .insert(side_thread_id, SideThreadState::new(parent_thread_id));
    app.thread_event_channels.insert(
        parent_thread_id,
        ThreadEventChannel::new_with_session(
            THREAD_EVENT_CHANNEL_CAPACITY,
            started.session,
            started.turns,
        ),
    );

    open_running_task_exit_menu(&mut app, &mut tui, &mut app_server).await;

    assert!(app.chat_widget.no_modal_or_popup_active());
    assert_eq!(app.active_thread_id, Some(parent_thread_id));
    assert!(!app.side_threads.contains_key(&side_thread_id));
    assert!(op_rx.try_recv().is_err());
    Ok(())
}

#[tokio::test]
async fn daemon_ctrl_c_hides_background_exit_for_running_background_side_thread() -> Result<()> {
    let (mut app, mut app_event_rx, mut op_rx) = make_test_app_with_channels().await;
    let parent_thread_id = prepare_local_daemon_thread(&mut app)?;
    let side_thread_id = ThreadId::new();
    app.side_threads
        .insert(side_thread_id, SideThreadState::new(parent_thread_id));
    app.thread_event_channels.insert(
        side_thread_id,
        ThreadEventChannel::new_with_session(
            THREAD_EVENT_CHANNEL_CAPACITY,
            test_thread_session(side_thread_id, test_path_buf("/tmp/project")),
            vec![test_turn("side-turn", TurnStatus::InProgress, Vec::new())],
        ),
    );
    let (mut app_server, mut tui) =
        prepare_background_exit_test(&app, &mut app_event_rx, &mut op_rx).await?;

    open_running_task_exit_menu(&mut app, &mut tui, &mut app_server).await;
    assert_snapshot!(render_bottom_popup(&app.chat_widget, /*width*/ 90), @r"
      Task is still running
      Choose what happens to the current task.

    › 1. Cancel task  Stop the current task and stay in Codex
      2. Exit         Stop the current task and exit Codex

      Press enter to confirm or esc to go back
    ");
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    let event = app_event_rx.try_recv()?;

    assert_matches!(
        event,
        AppEvent::RunningTaskExit {
            action: RunningTaskExitAction::Exit,
            thread_id,
        } if thread_id == side_thread_id
    );
    assert!(op_rx.try_recv().is_err());
    Ok(())
}

#[tokio::test]
async fn daemon_ctrl_c_hides_background_exit_with_queued_follow_up() -> Result<()> {
    let (mut app, mut app_event_rx, mut op_rx) = make_test_app_with_channels().await;
    prepare_running_local_daemon(&mut app)?;
    app.chat_widget
        .apply_external_edit("queued follow-up".to_string());
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    assert_eq!(
        app.chat_widget.queued_user_message_texts(),
        vec!["queued follow-up".to_string()]
    );
    let (mut app_server, mut tui) =
        prepare_background_exit_test(&app, &mut app_event_rx, &mut op_rx).await?;

    open_running_task_exit_menu(&mut app, &mut tui, &mut app_server).await;

    assert!(!render_bottom_popup(&app.chat_widget, /*width*/ 90).contains("Run in background"));
    assert!(op_rx.try_recv().is_err());
    assert!(app_event_rx.try_recv().is_err());
    Ok(())
}

#[tokio::test]
async fn non_daemon_ctrl_c_keeps_interrupt_behavior() -> Result<()> {
    for target in [
        AppServerTarget::Embedded,
        AppServerTarget::Remote {
            endpoint: crate::RemoteAppServerEndpoint::WebSocket {
                websocket_url: "ws://127.0.0.1:4500".to_string(),
                auth_token: None,
            },
        },
    ] {
        let (mut app, mut app_event_rx, mut op_rx) = make_test_app_with_channels().await;
        prepare_running_local_daemon(&mut app)?;
        app.app_server_target = target;
        let (mut app_server, mut tui) =
            prepare_background_exit_test(&app, &mut app_event_rx, &mut op_rx).await?;

        open_running_task_exit_menu(&mut app, &mut tui, &mut app_server).await;

        assert!(app.chat_widget.no_modal_or_popup_active());
        assert_matches!(op_rx.try_recv(), Ok(Op::Interrupt));
        assert!(app_event_rx.try_recv().is_err());
    }
    Ok(())
}

#[tokio::test]
async fn daemon_ctrl_c_with_draft_preserves_composer_cancellation() -> Result<()> {
    let (mut app, mut app_event_rx, mut op_rx) = make_test_app_with_channels().await;
    prepare_running_local_daemon(&mut app)?;
    app.chat_widget.apply_external_edit("draft".to_string());
    let (mut app_server, mut tui) =
        prepare_background_exit_test(&app, &mut app_event_rx, &mut op_rx).await?;

    open_running_task_exit_menu(&mut app, &mut tui, &mut app_server).await;

    assert!(app.chat_widget.no_modal_or_popup_active());
    assert!(app.chat_widget.composer_is_empty());
    assert!(op_rx.try_recv().is_err());
    assert_matches!(
        app_event_rx.try_recv(),
        Ok(AppEvent::AppendMessageHistoryEntry { text, .. }) if text == "draft"
    );
    Ok(())
}

#[tokio::test]
async fn daemon_ctrl_c_during_paste_burst_does_not_show_exit_menu() -> Result<()> {
    let (mut app, mut app_event_rx, mut op_rx) = make_test_app_with_channels().await;
    prepare_running_local_daemon(&mut app)?;
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
    assert!(app.chat_widget.composer_text_with_pending().is_empty());
    assert!(!app.chat_widget.composer_is_empty());
    let (mut app_server, mut tui) =
        prepare_background_exit_test(&app, &mut app_event_rx, &mut op_rx).await?;

    open_running_task_exit_menu(&mut app, &mut tui, &mut app_server).await;

    assert!(app.chat_widget.no_modal_or_popup_active());
    assert!(!app.chat_widget.composer_is_empty());
    assert_matches!(op_rx.try_recv(), Ok(Op::Interrupt));
    assert!(app_event_rx.try_recv().is_err());
    Ok(())
}

#[tokio::test]
async fn daemon_ctrl_c_during_mcp_startup_does_not_show_background_exit_menu() -> Result<()> {
    let (mut app, mut app_event_rx, mut op_rx) = make_test_app_with_channels().await;
    let thread_id = prepare_local_daemon_thread(&mut app)?;
    app.chat_widget
        .set_mcp_startup_expected_servers(["slow".to_string()]);
    app.chat_widget.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            thread_id: Some(thread_id.to_string()),
            name: "slow".to_string(),
            status: McpServerStartupState::Starting,
            error: None,
            failure_reason: None,
        }),
        /*replay_kind*/ None,
    );
    assert!(app.chat_widget.is_task_running_for_test());
    let (mut app_server, mut tui) =
        prepare_background_exit_test(&app, &mut app_event_rx, &mut op_rx).await?;

    open_running_task_exit_menu(&mut app, &mut tui, &mut app_server).await;

    assert!(app.chat_widget.no_modal_or_popup_active());
    assert_matches!(op_rx.try_recv(), Ok(Op::Interrupt));
    assert!(app_event_rx.try_recv().is_err());
    Ok(())
}
