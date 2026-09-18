//! Recovery commands remain usable when only the current conversation is unavailable.

use super::active_reconnect::drain_history;
use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn unavailable_thread_routes_local_and_recovery_commands() -> Result<()> {
    let (mut app, mut events, mut ops) = make_test_app_with_channels().await;
    let id = ThreadId::new();
    app.active_thread_id = Some(id);
    app.primary_thread_id = Some(id);
    app.app_server_target = AppServerTarget::Remote {
        endpoint: crate::resolve_remote_addr("ws://127.0.0.1:1")?,
    };
    app.chat_widget
        .handle_thread_session(test_thread_session(id, app.config.cwd.to_path_buf()));
    app.ensure_thread_channel(id).mark_replay_only();
    app.chat_widget
        .restore_user_message_to_composer("pending input".into());
    app.chat_widget.handle_key_event(KeyCode::Enter.into());
    app.chat_widget
        .restore_user_message_to_composer("keep queued input".into());
    app.chat_widget.handle_key_event(KeyCode::Tab.into());
    assert_eq!(
        app.chat_widget.queued_user_message_texts(),
        vec!["keep queued input"]
    );
    app.chat_widget.pause_unavailable_thread();
    assert_eq!(
        app.chat_widget.queued_user_message_texts(),
        vec!["pending input", "keep queued input"]
    );
    app.chat_widget
        .set_local_worktree_operations(/*enabled*/ false);
    let mut session = start_config_write_test_app_server(&app).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    while events.try_recv().is_ok() {}
    while ops.try_recv().is_ok() {}

    for command in [
        "/new",
        "/clear recovery",
        "/resume",
        "/agents",
        "/subagents",
        "/raw on",
        "/quit",
        "/exit",
    ] {
        app.chat_widget
            .restore_user_message_to_composer(command.into());
        app.handle_tui_event(&mut tui, &mut session, TuiEvent::Key(KeyCode::Enter.into()))
            .await?;
        let mut dispatched = false;
        while let Ok(event) = events.try_recv() {
            dispatched |= matches!((command, &event),
                ("/clear recovery", AppEvent::ClearUi { name: Some(name) }) if name == "recovery"
            );
            dispatched |= matches!(
                (command, &event),
                ("/new", AppEvent::NewSession { name: None })
                    | ("/resume", AppEvent::OpenResumePicker)
                    | ("/agents", AppEvent::OpenAgentsOverview)
                    | ("/subagents", AppEvent::OpenAgentPicker)
                    | ("/raw on", AppEvent::RawOutputModeChanged { enabled: true })
                    | ("/quit" | "/exit", AppEvent::Exit(_))
            );
        }
        assert!(dispatched, "{command}");
        assert!(ops.try_recv().is_err(), "{command}");
    }

    app.chat_widget
        .restore_user_message_to_composer("/pwd".into());
    app.handle_tui_event(&mut tui, &mut session, TuiEvent::Key(KeyCode::Enter.into()))
        .await?;
    let history = drain_history(&mut app, &mut tui, &mut session, &mut events).await?;
    assert!(history.contains("Current working directory:"));
    assert_snapshot!(
        "unavailable_thread_local_command",
        history.replace(&app.config.cwd.display().to_string(), "/project")
    );

    app.chat_widget
        .restore_user_message_to_composer("/status".into());
    app.handle_tui_event(&mut tui, &mut session, TuiEvent::Key(KeyCode::Enter.into()))
        .await?;
    let history = drain_history(&mut app, &mut tui, &mut session, &mut events).await?;
    assert!(history.contains("Model:"), "{history}");
    assert!(app.chat_widget.composer_is_empty());
    assert!(ops.try_recv().is_err());

    let input = app.chat_widget.capture_thread_input_state();
    app.chat_widget
        .restore_user_message_to_composer("/copy".into());
    app.handle_tui_event(&mut tui, &mut session, TuiEvent::Key(KeyCode::Enter.into()))
        .await?;
    app.handle_tui_event(&mut tui, &mut session, TuiEvent::Key(KeyCode::Esc.into()))
        .await?;
    while let Ok(event) = events.try_recv() {
        assert!(!matches!(event, AppEvent::CodexOp(_)));
        app.handle_event(&mut tui, &mut session, event).await?;
    }
    assert_eq!(app.chat_widget.capture_thread_input_state(), input);
    assert!(ops.try_recv().is_err());

    for offline in [false, true] {
        app.reconnect.offline = offline;
        for draft in if offline {
            vec!["/new", "/clear", "/status"]
        } else {
            vec!["keep my draft", "/review", "/rename changed"]
        } {
            app.handle_tui_event(
                &mut tui,
                &mut session,
                TuiEvent::Key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL)),
            )
            .await?;
            app.chat_widget
                .restore_user_message_to_composer(draft.into());
            app.handle_tui_event(&mut tui, &mut session, TuiEvent::Key(KeyCode::Enter.into()))
                .await?;
            assert_eq!(app.chat_widget.composer_text_with_pending(), draft);
            assert!(events.try_recv().is_err());
            assert!(ops.try_recv().is_err());
        }
    }
    session.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn unavailable_thread_new_and_clear_start_a_writable_session() -> Result<()> {
    for (command, activity) in ["/new", "/clear"]
        .into_iter()
        .flat_map(|command| ["idle", "review", "mcp"].map(|activity| (command, activity)))
    {
        let (mut app, mut events, _) = make_test_app_with_channels().await;
        let id = ThreadId::new();
        app.active_thread_id = Some(id);
        app.primary_thread_id = Some(id);
        app.app_server_target = AppServerTarget::Remote {
            endpoint: crate::resolve_remote_addr("ws://127.0.0.1:1")?,
        };
        app.chat_widget
            .handle_thread_session(test_thread_session(id, app.config.cwd.to_path_buf()));
        app.ensure_thread_channel(id).mark_replay_only();
        match activity {
            "review" => app.chat_widget.replay_thread_turns(
                vec![test_turn(
                    "review",
                    TurnStatus::InProgress,
                    vec![ThreadItem::EnteredReviewMode {
                        id: "review-start".into(),
                        review: "changes against main".into(),
                    }],
                )],
                ReplayKind::ResumeInitialMessages,
            ),
            "mcp" => app.chat_widget.handle_server_notification(
                ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
                    thread_id: Some(id.to_string()),
                    name: "slow".into(),
                    status: McpServerStartupState::Starting,
                    error: None,
                    failure_reason: None,
                }),
                /*replay_kind*/ None,
            ),
            _ => {}
        }
        app.chat_widget.pause_unavailable_thread();
        app.chat_widget
            .set_local_worktree_operations(/*enabled*/ false);
        let mut session = start_config_write_test_app_server(&app).await?;
        let mut tui = crate::tui::test_support::make_test_tui()?;
        while events.try_recv().is_ok() {}
        app.chat_widget
            .restore_user_message_to_composer(command.into());
        app.handle_tui_event(&mut tui, &mut session, TuiEvent::Key(KeyCode::Enter.into()))
            .await?;
        while let Ok(event) = events.try_recv() {
            if matches!(
                event,
                AppEvent::NewSession { .. } | AppEvent::ClearUi { .. }
            ) {
                app.handle_event(&mut tui, &mut session, event).await?;
                break;
            }
        }
        let new_id = app.chat_widget.thread_id().expect("new thread");
        assert_ne!(new_id, id, "{command}");
        assert!(!app.thread_unavailable(new_id), "{command}");
        session.shutdown().await?;
    }
    Ok(())
}
