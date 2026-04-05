use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn mcp_startup_header_booting_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.show_welcome_banner = false;

    chat.handle_codex_event(Event {
        id: "mcp-1".into(),
        msg: EventMsg::McpStartupUpdate(McpStartupUpdateEvent {
            server: "alpha".into(),
            status: McpStartupStatus::Starting,
        }),
    });

    let height = chat.desired_height(/*width*/ 80);
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, height))
        .expect("create terminal");
    terminal
        .draw(|f| chat.render(f.area(), f.buffer_mut()))
        .expect("draw chat widget");
    assert_chatwidget_snapshot!(
        "mcp_startup_header_booting",
        normalized_backend_snapshot(terminal.backend())
    );
}

#[tokio::test]
async fn mcp_startup_complete_does_not_clear_running_task() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_codex_event(Event {
        id: "task-1".into(),
        msg: EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: "turn-1".to_string(),
            created_at: None,
            model_context_window: None,
            collaboration_mode_kind: ModeKind::Default,
        }),
    });

    assert!(chat.bottom_pane.is_task_running());
    assert!(chat.bottom_pane.status_indicator_visible());

    chat.handle_codex_event(Event {
        id: "mcp-1".into(),
        msg: EventMsg::McpStartupComplete(McpStartupCompleteEvent {
            ready: vec!["schaltwerk".into()],
            ..Default::default()
        }),
    });

    assert!(chat.bottom_pane.is_task_running());
    assert!(chat.bottom_pane.status_indicator_visible());
}

#[tokio::test]
async fn app_server_mcp_startup_failure_renders_warning_history() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.show_welcome_banner = false;
    chat.set_mcp_startup_expected_servers(["alpha".to_string(), "beta".to_string()]);

    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            name: "alpha".to_string(),
            status: McpServerStartupState::Starting,
            error: None,
        }),
        /*replay_kind*/ None,
    );

    assert!(drain_insert_history(&mut rx).is_empty());
    assert!(chat.bottom_pane.is_task_running());

    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            name: "alpha".to_string(),
            status: McpServerStartupState::Failed,
            error: Some("MCP client for `alpha` failed to start: handshake failed".to_string()),
        }),
        /*replay_kind*/ None,
    );

    let failure_cells = drain_insert_history(&mut rx);
    let failure_text = failure_cells
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    assert!(failure_text.contains("MCP client for `alpha` failed to start: handshake failed"));
    assert!(!failure_text.contains("MCP startup incomplete"));
    assert!(chat.bottom_pane.is_task_running());

    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            name: "beta".to_string(),
            status: McpServerStartupState::Starting,
            error: None,
        }),
        /*replay_kind*/ None,
    );

    assert!(drain_insert_history(&mut rx).is_empty());
    assert!(chat.bottom_pane.is_task_running());

    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            name: "beta".to_string(),
            status: McpServerStartupState::Ready,
            error: None,
        }),
        /*replay_kind*/ None,
    );

    let summary_cells = drain_insert_history(&mut rx);
    let summary_text = summary_cells
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    assert_eq!(summary_text, "⚠ MCP startup incomplete (failed: alpha)\n");
    assert!(!chat.bottom_pane.is_task_running());

    let width: u16 = 120;
    let ui_height: u16 = chat.desired_height(width);
    let vt_height: u16 = 10;
    let viewport = Rect::new(0, vt_height - ui_height - 1, width, ui_height);

    let backend = VT100Backend::new(width, vt_height);
    let mut term = crate::custom_terminal::Terminal::with_options(backend).expect("terminal");
    term.set_viewport_area(viewport);

    for lines in failure_cells.into_iter().chain(summary_cells) {
        crate::insert_history::insert_history_lines(&mut term, lines)
            .expect("Failed to insert history lines in test");
    }

    term.draw(|f| {
        chat.render(f.area(), f.buffer_mut());
    })
    .expect("draw MCP startup warning history");

    assert_chatwidget_snapshot!(
        "app_server_mcp_startup_failure_renders_warning_history",
        normalize_snapshot_paths(term.backend().vt100().screen().contents())
    );
}

#[tokio::test]
async fn app_server_mcp_startup_lag_settles_startup_and_ignores_late_updates() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.show_welcome_banner = false;
    chat.set_mcp_startup_expected_servers(["alpha".to_string(), "beta".to_string()]);

    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            name: "alpha".to_string(),
            status: McpServerStartupState::Starting,
            error: None,
        }),
        /*replay_kind*/ None,
    );
    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            name: "alpha".to_string(),
            status: McpServerStartupState::Failed,
            error: Some("MCP client for `alpha` failed to start: handshake failed".to_string()),
        }),
        /*replay_kind*/ None,
    );
    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            name: "beta".to_string(),
            status: McpServerStartupState::Starting,
            error: None,
        }),
        /*replay_kind*/ None,
    );

    let _ = drain_insert_history(&mut rx);
    assert!(chat.bottom_pane.is_task_running());

    chat.finish_mcp_startup_after_lag();

    let summary_text = drain_insert_history(&mut rx)
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    assert!(summary_text.contains("MCP startup interrupted"));
    assert!(summary_text.contains("beta"));
    assert!(summary_text.contains("MCP startup incomplete (failed: alpha)"));
    assert!(!chat.bottom_pane.is_task_running());

    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            name: "beta".to_string(),
            status: McpServerStartupState::Starting,
            error: None,
        }),
        /*replay_kind*/ None,
    );

    assert!(drain_insert_history(&mut rx).is_empty());
    assert!(!chat.bottom_pane.is_task_running());

    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            name: "beta".to_string(),
            status: McpServerStartupState::Ready,
            error: None,
        }),
        /*replay_kind*/ None,
    );

    assert!(drain_insert_history(&mut rx).is_empty());
    assert!(!chat.bottom_pane.is_task_running());
}

#[tokio::test]
async fn app_server_mcp_startup_after_lag_can_settle_without_starting_updates() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.show_welcome_banner = false;
    chat.set_mcp_startup_expected_servers(["alpha".to_string(), "beta".to_string()]);

    chat.finish_mcp_startup_after_lag();

    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            name: "alpha".to_string(),
            status: McpServerStartupState::Failed,
            error: Some("MCP client for `alpha` failed to start: handshake failed".to_string()),
        }),
        /*replay_kind*/ None,
    );

    let failure_text = drain_insert_history(&mut rx)
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    assert!(failure_text.contains("MCP client for `alpha` failed to start: handshake failed"));
    assert!(chat.bottom_pane.is_task_running());

    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            name: "beta".to_string(),
            status: McpServerStartupState::Ready,
            error: None,
        }),
        /*replay_kind*/ None,
    );

    let summary_text = drain_insert_history(&mut rx)
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    assert_eq!(summary_text, "⚠ MCP startup incomplete (failed: alpha)\n");
    assert!(!chat.bottom_pane.is_task_running());
}

#[tokio::test]
async fn app_server_mcp_startup_after_lag_preserves_partial_terminal_only_round() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.show_welcome_banner = false;
    chat.set_mcp_startup_expected_servers(["alpha".to_string(), "beta".to_string()]);

    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            name: "alpha".to_string(),
            status: McpServerStartupState::Starting,
            error: None,
        }),
        /*replay_kind*/ None,
    );
    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            name: "alpha".to_string(),
            status: McpServerStartupState::Failed,
            error: Some("MCP client for `alpha` failed to start: handshake failed".to_string()),
        }),
        /*replay_kind*/ None,
    );
    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            name: "beta".to_string(),
            status: McpServerStartupState::Starting,
            error: None,
        }),
        /*replay_kind*/ None,
    );
    let _ = drain_insert_history(&mut rx);

    chat.finish_mcp_startup_after_lag();
    let _ = drain_insert_history(&mut rx);
    assert!(!chat.bottom_pane.is_task_running());

    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            name: "alpha".to_string(),
            status: McpServerStartupState::Failed,
            error: Some("MCP client for `alpha` failed to start: handshake failed".to_string()),
        }),
        /*replay_kind*/ None,
    );

    assert!(drain_insert_history(&mut rx).is_empty());
    assert!(!chat.bottom_pane.is_task_running());

    chat.finish_mcp_startup_after_lag();

    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            name: "beta".to_string(),
            status: McpServerStartupState::Ready,
            error: None,
        }),
        /*replay_kind*/ None,
    );

    let summary_text = drain_insert_history(&mut rx)
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    assert!(summary_text.contains("MCP client for `alpha` failed to start: handshake failed"));
    assert!(summary_text.contains("MCP startup incomplete (failed: alpha)"));
    assert!(!chat.bottom_pane.is_task_running());
}

#[tokio::test]
async fn app_server_mcp_startup_next_round_discards_stale_terminal_updates() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.show_welcome_banner = false;
    chat.set_mcp_startup_expected_servers(["alpha".to_string(), "beta".to_string()]);

    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            name: "alpha".to_string(),
            status: McpServerStartupState::Starting,
            error: None,
        }),
        /*replay_kind*/ None,
    );
    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            name: "alpha".to_string(),
            status: McpServerStartupState::Failed,
            error: Some("MCP client for `alpha` failed to start: handshake failed".to_string()),
        }),
        /*replay_kind*/ None,
    );
    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            name: "beta".to_string(),
            status: McpServerStartupState::Starting,
            error: None,
        }),
        /*replay_kind*/ None,
    );
    let _ = drain_insert_history(&mut rx);

    chat.finish_mcp_startup_after_lag();
    let _ = drain_insert_history(&mut rx);
    assert!(!chat.bottom_pane.is_task_running());

    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            name: "alpha".to_string(),
            status: McpServerStartupState::Failed,
            error: Some(
                "MCP client for `alpha` failed to start: stale handshake failed".to_string(),
            ),
        }),
        /*replay_kind*/ None,
    );
    assert!(drain_insert_history(&mut rx).is_empty());

    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            name: "beta".to_string(),
            status: McpServerStartupState::Starting,
            error: None,
        }),
        /*replay_kind*/ None,
    );
    assert!(drain_insert_history(&mut rx).is_empty());
    assert!(!chat.bottom_pane.is_task_running());

    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            name: "alpha".to_string(),
            status: McpServerStartupState::Ready,
            error: None,
        }),
        /*replay_kind*/ None,
    );
    assert!(drain_insert_history(&mut rx).is_empty());
    assert!(chat.bottom_pane.is_task_running());

    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            name: "beta".to_string(),
            status: McpServerStartupState::Ready,
            error: None,
        }),
        /*replay_kind*/ None,
    );

    let summary_text = drain_insert_history(&mut rx)
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    assert!(summary_text.is_empty());
    assert!(!chat.bottom_pane.is_task_running());
}

#[tokio::test]
async fn app_server_mcp_startup_next_round_keeps_terminal_statuses_after_starting() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.show_welcome_banner = false;
    chat.set_mcp_startup_expected_servers(["alpha".to_string(), "beta".to_string()]);

    chat.finish_mcp_startup_after_lag();

    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            name: "alpha".to_string(),
            status: McpServerStartupState::Starting,
            error: None,
        }),
        /*replay_kind*/ None,
    );
    assert!(drain_insert_history(&mut rx).is_empty());

    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            name: "alpha".to_string(),
            status: McpServerStartupState::Failed,
            error: Some("MCP client for `alpha` failed to start: handshake failed".to_string()),
        }),
        /*replay_kind*/ None,
    );

    let failure_text = drain_insert_history(&mut rx)
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    assert!(failure_text.contains("MCP client for `alpha` failed to start: handshake failed"));

    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            name: "beta".to_string(),
            status: McpServerStartupState::Starting,
            error: None,
        }),
        /*replay_kind*/ None,
    );
    assert!(drain_insert_history(&mut rx).is_empty());
    assert!(chat.bottom_pane.is_task_running());

    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            name: "beta".to_string(),
            status: McpServerStartupState::Ready,
            error: None,
        }),
        /*replay_kind*/ None,
    );

    let summary_text = drain_insert_history(&mut rx)
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    assert_eq!(summary_text, "⚠ MCP startup incomplete (failed: alpha)\n");
    assert!(!chat.bottom_pane.is_task_running());
}

#[tokio::test]
async fn app_server_mcp_startup_next_round_with_empty_expected_servers_reactivates() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.show_welcome_banner = false;
    chat.set_mcp_startup_expected_servers(std::iter::empty::<String>());
    chat.finish_mcp_startup(Vec::new(), Vec::new());

    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            name: "runtime".to_string(),
            status: McpServerStartupState::Starting,
            error: None,
        }),
        /*replay_kind*/ None,
    );
    assert!(drain_insert_history(&mut rx).is_empty());
    assert!(chat.bottom_pane.is_task_running());

    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            name: "runtime".to_string(),
            status: McpServerStartupState::Failed,
            error: Some("MCP client for `runtime` failed to start: handshake failed".to_string()),
        }),
        /*replay_kind*/ None,
    );

    let summary_text = drain_insert_history(&mut rx)
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    assert!(summary_text.contains("MCP client for `runtime` failed to start: handshake failed"));
    assert!(summary_text.contains("MCP startup incomplete (failed: runtime)"));
    assert!(!chat.bottom_pane.is_task_running());
}

#[tokio::test]
async fn app_server_mcp_startup_after_lag_with_empty_expected_servers_preserves_failures() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.show_welcome_banner = false;
    chat.set_mcp_startup_expected_servers(std::iter::empty::<String>());

    chat.on_mcp_startup_update(McpStartupUpdateEvent {
        server: "runtime".to_string(),
        status: McpStartupStatus::Starting,
    });
    chat.on_mcp_startup_update(McpStartupUpdateEvent {
        server: "runtime".to_string(),
        status: McpStartupStatus::Failed {
            error: "MCP client for `runtime` failed to start: handshake failed".to_string(),
        },
    });

    let warning_text = drain_insert_history(&mut rx)
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    assert!(warning_text.contains("MCP client for `runtime` failed to start: handshake failed"));
    assert!(chat.bottom_pane.is_task_running());

    chat.finish_mcp_startup_after_lag();

    let summary_text = drain_insert_history(&mut rx)
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    assert!(summary_text.contains("MCP startup incomplete (failed: runtime)"));
    assert!(!chat.bottom_pane.is_task_running());
}

#[tokio::test]
async fn app_server_mcp_startup_after_lag_includes_runtime_servers_with_expected_set() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.show_welcome_banner = false;
    chat.set_mcp_startup_expected_servers(["alpha".to_string()]);

    chat.on_mcp_startup_update(McpStartupUpdateEvent {
        server: "alpha".to_string(),
        status: McpStartupStatus::Ready,
    });
    chat.on_mcp_startup_update(McpStartupUpdateEvent {
        server: "runtime".to_string(),
        status: McpStartupStatus::Failed {
            error: "MCP client for `runtime` failed to start: handshake failed".to_string(),
        },
    });

    let warning_text = drain_insert_history(&mut rx)
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    assert!(warning_text.contains("MCP client for `runtime` failed to start: handshake failed"));
    assert!(chat.bottom_pane.is_task_running());

    chat.finish_mcp_startup_after_lag();

    let summary_text = drain_insert_history(&mut rx)
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    assert!(summary_text.contains("MCP startup incomplete (failed: runtime)"));
    assert!(!chat.bottom_pane.is_task_running());
}

#[tokio::test]
async fn app_server_mcp_startup_next_round_after_lag_can_settle_without_starting_updates() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.show_welcome_banner = false;
    chat.set_mcp_startup_expected_servers(["alpha".to_string(), "beta".to_string()]);

    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            name: "alpha".to_string(),
            status: McpServerStartupState::Starting,
            error: None,
        }),
        /*replay_kind*/ None,
    );
    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            name: "alpha".to_string(),
            status: McpServerStartupState::Failed,
            error: Some("MCP client for `alpha` failed to start: handshake failed".to_string()),
        }),
        /*replay_kind*/ None,
    );
    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            name: "beta".to_string(),
            status: McpServerStartupState::Starting,
            error: None,
        }),
        /*replay_kind*/ None,
    );
    let _ = drain_insert_history(&mut rx);

    chat.finish_mcp_startup_after_lag();
    let _ = drain_insert_history(&mut rx);
    assert!(!chat.bottom_pane.is_task_running());

    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            name: "alpha".to_string(),
            status: McpServerStartupState::Failed,
            error: Some(
                "MCP client for `alpha` failed to start: stale handshake failed".to_string(),
            ),
        }),
        /*replay_kind*/ None,
    );
    assert!(drain_insert_history(&mut rx).is_empty());

    chat.finish_mcp_startup_after_lag();

    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            name: "alpha".to_string(),
            status: McpServerStartupState::Failed,
            error: Some("MCP client for `alpha` failed to start: handshake failed".to_string()),
        }),
        /*replay_kind*/ None,
    );

    let failure_text = drain_insert_history(&mut rx)
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    assert!(failure_text.is_empty());
    assert!(!chat.bottom_pane.is_task_running());

    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            name: "beta".to_string(),
            status: McpServerStartupState::Ready,
            error: None,
        }),
        /*replay_kind*/ None,
    );

    let summary_text = drain_insert_history(&mut rx)
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    assert!(summary_text.contains("MCP client for `alpha` failed to start: handshake failed"));
    assert!(summary_text.contains("MCP startup incomplete (failed: alpha)"));
    assert!(!chat.bottom_pane.is_task_running());
}
