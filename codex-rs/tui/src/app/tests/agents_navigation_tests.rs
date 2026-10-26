use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn agents_navigation_requires_shared_app_server() -> Result<()> {
    let endpoint = crate::RemoteAppServerEndpoint::UnixSocket {
        socket_path: AbsolutePathBuf::relative_to_current_dir("codex.sock")?,
    };
    for (target, enabled) in [
        (AppServerTarget::Embedded, false),
        (
            AppServerTarget::Remote {
                endpoint: endpoint.clone(),
            },
            true,
        ),
        (
            AppServerTarget::LocalDaemon {
                endpoint,
                allow_embedded_fallback: true,
            },
            true,
        ),
    ] {
        let (mut app, mut events, _op_rx) = make_test_app_with_channels().await;
        let mut tui = crate::tui::test_support::make_test_tui()?;
        let mut app_server = start_config_write_test_app_server(&app).await?;
        app.app_server_target = target;
        let init = app.chatwidget_init_for_forked_or_resumed_thread(
            &mut tui,
            app.config.clone(),
            /*initial_user_message*/ None,
        );
        app.replace_chat_widget(ChatWidget::new_with_app_event(init));
        while events.try_recv().is_ok() {}
        let composer = render_bottom_popup(&app.chat_widget, /*width*/ 96);
        assert_eq!(composer.contains("← for agents"), enabled);
        if matches!(app.app_server_target, AppServerTarget::Remote { .. }) {
            let footer = composer.lines().last().expect("composer footer").trim();
            insta::assert_snapshot!(normalize_snapshot_paths(footer), @"GPT-5.6-Sol default · /tmp/project · ← for agents");
        }
        app.handle_tui_event(
            &mut tui,
            &mut app_server,
            TuiEvent::Key(KeyCode::Left.into()),
        )
        .await?;
        if enabled {
            let event = events.try_recv()?;
            assert_matches!(event, AppEvent::OpenAgentsOverview);
            app.handle_event(&mut tui, &mut app_server, event).await?;
            assert!(app.chat_widget.active_view_is(AGENTS_OVERVIEW_VIEW_ID));
        } else {
            assert!(events.try_recv().is_err());
            assert!(app.chat_widget.no_modal_or_popup_active());
        }
        app_server.shutdown().await?;
    }
    Ok(())
}

#[tokio::test]
async fn externally_archived_thread_keeps_advertised_agents_navigation() -> Result<()> {
    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server = start_config_write_test_app_server(&app).await?;
    app.app_server_target = AppServerTarget::LocalDaemon {
        endpoint: crate::RemoteAppServerEndpoint::UnixSocket {
            socket_path: AbsolutePathBuf::relative_to_current_dir("codex.sock")?,
        },
        allow_embedded_fallback: true,
    };
    let init = app.chatwidget_init_for_forked_or_resumed_thread(
        &mut tui,
        app.config.clone(),
        /*initial_user_message*/ None,
    );
    app.replace_chat_widget(ChatWidget::new_with_app_event(init));
    let id = ThreadId::new();
    app.active_thread_id = Some(id);
    app.primary_thread_id = Some(id);
    app.chat_widget
        .handle_thread_session(test_thread_session(id, app.config.cwd.to_path_buf()));
    app.ensure_thread_channel(id);
    app.handle_app_server_event(
        &app_server,
        codex_app_server_client::AppServerEvent::ServerNotification(Box::new(
            ServerNotification::ThreadArchived(ThreadArchivedNotification {
                thread_id: id.to_string(),
            }),
        )),
    )
    .await;
    while events.try_recv().is_ok() {}

    // Left must still edit an existing draft before it can navigate from an empty composer.
    app.chat_widget
        .restore_user_message_to_composer("ab".into());
    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Key(KeyCode::Left.into()),
    )
    .await?;
    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Key(KeyCode::Char('!').into()),
    )
    .await?;
    assert_eq!(app.chat_widget.composer_text_with_pending(), "a!b");
    assert!(
        !std::iter::from_fn(|| events.try_recv().ok())
            .any(|event| matches!(event, AppEvent::OpenAgentsOverview))
    );
    for key in [
        KeyCode::End.into(),
        KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL),
    ] {
        app.handle_tui_event(&mut tui, &mut app_server, TuiEvent::Key(key))
            .await?;
    }
    assert!(app.chat_widget.composer_is_empty());
    assert!(render_bottom_popup(&app.chat_widget, /*width*/ 96).contains("for agents"));
    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Key(KeyEvent::new_with_kind(
            KeyCode::Left,
            KeyModifiers::NONE,
            KeyEventKind::Repeat,
        )),
    )
    .await?;
    assert!(
        !std::iter::from_fn(|| events.try_recv().ok())
            .any(|event| matches!(event, AppEvent::OpenAgentsOverview))
    );
    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Key(KeyCode::Left.into()),
    )
    .await?;
    let event = events.try_recv()?;
    assert_matches!(event, AppEvent::OpenAgentsOverview);
    app.handle_event(&mut tui, &mut app_server, event).await?;
    insta::assert_snapshot!(
        "externally_archived_thread_agents",
        render_bottom_popup(&app.chat_widget, /*width*/ 96)
    );
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn pending_windows_sandbox_setup_blocks_thread_replacement() -> Result<()> {
    use crate::app_event::WindowsSandboxEnableMode;

    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    app.chat_widget.windows_sandbox_local_server = true;
    while events.try_recv().is_ok() {}
    let mut app_server = start_config_write_test_app_server(&app).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let current = ThreadId::new();
    let other = ThreadId::new();
    app.active_thread_id = Some(current);
    app.primary_thread_id = Some(current);
    let preset = codex_utils_approval_presets::builtin_approval_presets()
        .into_iter()
        .find(|preset| preset.id == "auto")
        .expect("auto preset");
    app.windows_sandbox.pending_setup = Some((WindowsSandboxEnableMode::Elevated, preset, None));
    let prompt = crate::chatwidget::create_initial_user_message(
        Some("review this".to_string()),
        Vec::new(),
        Vec::new(),
    );
    app.chat_widget.initial_user_message = prompt.clone();

    app.select_agent_thread(&mut tui, &mut app_server, other)
        .await?;
    assert_eq!(app.active_thread_id, Some(current));
    assert_eq!(app.chat_widget.initial_user_message, prompt);
    let cell = match events.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => cell,
        other => panic!("expected setup navigation message, got {other:?}"),
    };
    let rendered = lines_to_single_string(&cell.display_lines(/*width*/ 100));
    insta::assert_snapshot!(rendered, @"• Finish Windows sandbox setup before switching threads.");

    app.chat_widget.initial_user_message = None;
    app.run_agents_overview_action(
        &mut tui,
        &mut app_server,
        current,
        crate::app_event::AgentsOverviewAction::Archive,
    )
    .await?;
    assert_eq!(app.active_thread_id, Some(current));
    assert!(events.try_recv().is_err());

    app_server.shutdown().await?;
    Ok(())
}
