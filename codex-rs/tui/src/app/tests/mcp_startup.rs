use super::*;
use pretty_assertions::assert_eq;

fn configure_mcp_servers(app: &mut App) {
    let config: codex_config::types::McpServerConfig =
        toml::from_str::<toml::Value>("command = 'true'")
            .expect("test MCP config should parse")
            .try_into()
            .expect("test MCP config should deserialize");
    app.config
        .mcp_servers
        .set(HashMap::from([
            ("eager".to_string(), config.clone()),
            ("deferred".to_string(), config),
        ]))
        .expect("test MCP servers should accept any configuration");
}

#[tokio::test]
async fn remote_mcp_startup_settles_when_attaching_mid_round() -> Result<()> {
    use crate::terminal_program_status::ProgramStatus;
    use McpServerStartupState::Ready;
    use McpServerStartupState::Starting;
    use codex_app_server_client::AppServerEvent;

    let mut tui = crate::tui::test_support::make_test_tui()?;
    // Subscribe before startup, during startup, or after every server is ready.
    for missed_updates in [0, 2, 5, 6] {
        let (mut app, mut events, _ops) = make_test_app_with_channels().await;
        let mut app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;
        app.app_server_target = AppServerTarget::Remote {
            endpoint: crate::resolve_remote_addr("ws://127.0.0.1:4500")?,
        };
        configure_mcp_servers(&mut app);
        let id = ThreadId::new();
        app.enqueue_primary_thread_session(
            test_thread_session(id, app.config.cwd.to_path_buf()),
            Vec::new(),
        )
        .await?;

        let update = |name: &str, status| {
            ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
                environment_id: None,
                thread_id: Some(id.to_string()),
                name: name.to_string(),
                status,
                error: None,
                failure_reason: None,
            })
        };
        // Only the app server has `late` in its configuration.
        let startup = [
            (update("eager", Starting), true),
            (update("deferred", Starting), true),
            (update("late", Starting), true),
            (update("eager", Ready), true),
            (update("deferred", Ready), true),
            (update("late", Ready), false),
        ];
        for (notification, running) in startup.into_iter().skip(missed_updates).chain([
            (turn_started_notification(id, "turn"), true),
            (
                turn_completed_notification(id, "turn", TurnStatus::Completed),
                false,
            ),
            // A later reconnect can report Ready without a corresponding Starting.
            (update("late", Ready), false),
        ]) {
            app.handle_app_server_event(
                &app_server,
                AppServerEvent::ServerNotification(Box::new(notification)),
            )
            .await;
            app.drain_active_thread_events(&mut tui).await?;
            assert_eq!(
                app.chat_widget.is_task_running_for_test(),
                running,
                "missed {missed_updates} startup updates"
            );
        }
        assert_eq!(
            app.chat_widget.desired_program_status(),
            ProgramStatus::Done
        );
        app.transcript_cells
            .push(Arc::new(history_cell::new_user_prompt(
                "Previous prompt".into(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )));
        app.handle_key_event(&mut tui, &mut app_server, KeyCode::Esc.into())
            .await;
        let composer = render_bottom_popup(&app.chat_widget, /*width*/ 80);
        let hint = composer
            .lines()
            .find(|line| line.contains("edit previous message"))
            .unwrap_or_default()
            .trim();
        insta::allow_duplicates! {
            insta::assert_snapshot!(hint, @"esc again to edit previous message");
        }
        app.handle_key_event(
            &mut tui,
            &mut app_server,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        )
        .await;
        assert!(
            std::iter::from_fn(|| events.try_recv().ok())
                .any(|event| matches!(event, AppEvent::Exit(ExitMode::ShutdownFirst)))
        );
        app_server.shutdown().await?;
    }
    Ok(())
}

#[tokio::test]
async fn remote_mcp_startup_reports_terminal_only_failures_and_cancellations() -> Result<()> {
    use McpServerStartupState::Cancelled;
    use McpServerStartupState::Failed;
    use McpServerStartupState::Ready;
    use McpServerStartupState::Starting;
    use codex_app_server_client::AppServerEvent;
    use codex_app_server_protocol::McpServerStartupFailureReason::ReauthenticationRequired;

    let mut tui = crate::tui::test_support::make_test_tui()?;
    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    let app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;
    app.app_server_target = AppServerTarget::Remote {
        endpoint: crate::resolve_remote_addr("ws://127.0.0.1:4500")?,
    };
    configure_mcp_servers(&mut app);
    let id = ThreadId::new();
    app.enqueue_primary_thread_session(
        test_thread_session(id, app.config.cwd.to_path_buf()),
        Vec::new(),
    )
    .await?;

    let error = "Session expired";
    let failed = "MCP startup incomplete (failed: deferred)";
    let both_failed = "MCP startup incomplete (failed: deferred, eager)";
    let cancelled = "MCP startup interrupted. The following servers were not initialized: other";
    for (name, status, running, expected) in [
        ("eager", Ready, false, &[][..]),
        ("deferred", Failed, false, &[error, failed]),
        ("deferred", Failed, false, &[]),
        ("other", Cancelled, false, &[cancelled]),
        // An observed retry emits a fresh diagnostic even if the error is unchanged.
        ("deferred", Starting, true, &[]),
        ("deferred", Failed, false, &[error, failed]),
        // Only eager's Starting was delivered for the next reload. Both failures count.
        ("eager", Starting, true, &[]),
        ("deferred", Failed, true, &[]),
        ("other", Cancelled, true, &[]),
        ("eager", Failed, false, &[error, cancelled, both_failed]),
    ] {
        let notification =
            ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
                environment_id: None,
                thread_id: Some(id.to_string()),
                name: name.to_string(),
                error: (status == Failed).then(|| error.to_string()),
                failure_reason: (status == Failed).then_some(ReauthenticationRequired),
                status,
            });
        app.handle_app_server_event(
            &app_server,
            AppServerEvent::ServerNotification(Box::new(notification)),
        )
        .await;
        app.drain_active_thread_events(&mut tui).await?;
        let mut emitted = Vec::new();
        while let Ok(event) = events.try_recv() {
            if let AppEvent::InsertHistoryCell(cell) = event {
                if let Some(warning) = cell
                    .as_any()
                    .downcast_ref::<history_cell::StartupWarningsCell>()
                {
                    emitted.push(warning.messages.join("\n"));
                }
                app.insert_history_cell(&mut tui, cell);
            }
        }
        assert_eq!(emitted, expected, "warning events for {name}");
        assert_eq!(
            app.chat_widget.is_task_running_for_test(),
            running,
            "progress for {name}"
        );
    }
    let warnings = history_cell::warning_entries(&app.transcript_cells)
        .into_iter()
        .map(|entry| format!("{}\n{}", entry.source, entry.details))
        .collect::<Vec<_>>();
    insta::assert_snapshot!(warnings.join("\n\n"), @r"
    MCP · deferred
    Session expired

    MCP startup incomplete (failed: deferred)

    MCP startup incomplete (failed: deferred, eager)

    Sign-in required.

    MCP · eager
    Session expired

    MCP startup incomplete (failed: deferred, eager)

    Sign-in required.

    MCP · other
    MCP startup interrupted. The following servers were not initialized: other
    ");
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn subagent_mcp_startup_settles_while_cached_servers_remain_deferred() {
    let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
    configure_mcp_servers(&mut app);
    let app_server = crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
        .await
        .expect("embedded app server");
    let root_thread_id = ThreadId::new();
    let subagent_thread_id = ThreadId::new();
    app.primary_thread_id = Some(root_thread_id);
    app.upsert_agent_picker_thread(
        subagent_thread_id,
        /*agent_nickname*/ None,
        /*agent_role*/ None,
        /*is_closed*/ false,
    );
    app.ensure_thread_channel(subagent_thread_id);
    app.activate_thread_channel(subagent_thread_id).await;
    app.replay_thread_snapshot(
        ThreadEventSnapshot {
            delegated_turns: Vec::new(),
            session: Some(test_thread_session(
                subagent_thread_id,
                test_path_buf("/tmp/subagent"),
            )),
            turns: Vec::new(),
            events: Vec::new(),
            active_reasoning_item: None,
            input_state: None,
        },
        /*resume_restored_queue*/ false,
    );

    let mut visible_startup_states = Vec::new();
    for (name, status, task_running) in [
        ("eager", McpServerStartupState::Starting, true),
        ("eager", McpServerStartupState::Ready, false),
        ("deferred", McpServerStartupState::Starting, true),
        ("deferred", McpServerStartupState::Ready, false),
    ] {
        app.handle_app_server_event(
            &app_server,
            codex_app_server_client::AppServerEvent::ServerNotification(Box::new(
                ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
                    environment_id: None,
                    thread_id: Some(subagent_thread_id.to_string()),
                    name: name.to_string(),
                    status,
                    error: None,
                    failure_reason: None,
                }),
            )),
        )
        .await;
        let event = app
            .active_thread_rx
            .as_mut()
            .expect("subagent receiver should be active")
            .try_recv()
            .expect("MCP startup update should reach the active subagent");
        app.handle_thread_event_now(event);

        assert_eq!(app.chat_widget.is_task_running_for_test(), task_running);
        let rendered = render_bottom_popup(&app.chat_widget, /*width*/ 80);
        let visible_status = rendered
            .lines()
            .find(|line| line.contains("Booting MCP server:"))
            .and_then(|line| line.split_once(" ("))
            .map_or("idle", |(status, _)| status);
        visible_startup_states.push(format!("{name}: {visible_status}"));
    }

    insta::assert_snapshot!(visible_startup_states.join("\n"), @r"
    eager: idle
    eager: idle
    deferred: idle
    deferred: idle
    ");
}

#[tokio::test]
async fn resumed_subagent_mcp_startup_settles_while_cached_servers_remain_deferred() {
    let mut app = make_test_app().await;
    configure_mcp_servers(&mut app);
    let subagent_thread_id = ThreadId::new();
    app.primary_thread_id = Some(subagent_thread_id);
    app.active_thread_id = Some(subagent_thread_id);
    app.agent_navigation.mark_parent_owned(subagent_thread_id);

    for status in [
        McpServerStartupState::Starting,
        McpServerStartupState::Ready,
    ] {
        app.chat_widget.handle_server_notification(
            ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
                environment_id: None,
                thread_id: Some(subagent_thread_id.to_string()),
                name: "eager".to_string(),
                status,
                error: None,
                failure_reason: None,
            }),
            /*replay_kind*/ None,
        );
    }

    assert!(!app.chat_widget.is_task_running_for_test());
}

#[tokio::test]
async fn side_conversations_settle_despite_unreported_locally_configured_servers() {
    let mut app = make_test_app().await;
    configure_mcp_servers(&mut app);
    let root_thread_id = ThreadId::new();
    let side_thread_id = ThreadId::new();
    app.primary_thread_id = Some(root_thread_id);
    app.side_threads
        .insert(side_thread_id, SideThreadState::new(root_thread_id));
    app.active_thread_id = Some(side_thread_id);

    for status in [
        McpServerStartupState::Starting,
        McpServerStartupState::Ready,
    ] {
        app.chat_widget.handle_server_notification(
            ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
                environment_id: None,
                thread_id: Some(side_thread_id.to_string()),
                name: "eager".to_string(),
                status,
                error: None,
                failure_reason: None,
            }),
            /*replay_kind*/ None,
        );
    }

    assert!(!app.chat_widget.is_task_running_for_test());
}
