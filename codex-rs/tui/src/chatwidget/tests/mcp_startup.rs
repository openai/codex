use super::*;
use pretty_assertions::assert_eq;

fn notify_mcp_status(chat: &mut ChatWidget, name: &str, status: McpServerStartupState) {
    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            name: name.to_string(),
            status,
            error: None,
        }),
        /*replay_kind*/ None,
    );
}

fn notify_mcp_status_error(chat: &mut ChatWidget, name: &str, error: &str) {
    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            name: name.to_string(),
            status: McpServerStartupState::Failed,
            error: Some(error.to_string()),
        }),
        /*replay_kind*/ None,
    );
}

#[tokio::test]
async fn mcp_startup_header_booting_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.show_welcome_banner = false;

    notify_mcp_status(&mut chat, "alpha", McpServerStartupState::Starting);

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

    handle_turn_started(&mut chat, "turn-1");

    assert!(chat.bottom_pane.is_task_running());
    assert!(chat.bottom_pane.status_indicator_visible());

    chat.set_mcp_startup_expected_servers(["schaltwerk".to_string()]);
    notify_mcp_status(&mut chat, "schaltwerk", McpServerStartupState::Starting);
    notify_mcp_status(&mut chat, "schaltwerk", McpServerStartupState::Ready);

    assert!(chat.bottom_pane.is_task_running());
    assert!(chat.bottom_pane.status_indicator_visible());
}

#[tokio::test]
async fn app_server_mcp_startup_failure_renders_warning_history() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.show_welcome_banner = false;
    chat.set_mcp_startup_expected_servers(["alpha".to_string(), "beta".to_string()]);

    notify_mcp_status(&mut chat, "alpha", McpServerStartupState::Starting);

    assert!(drain_insert_history(&mut rx).is_empty());
    assert!(chat.bottom_pane.is_task_running());

    notify_mcp_status_error(
        &mut chat,
        "alpha",
        "MCP client for `alpha` failed to start: handshake failed",
    );

    let failure_cells = drain_insert_history(&mut rx);
    let failure_text = failure_cells
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    assert!(failure_text.contains("MCP client for `alpha` failed to start: handshake failed"));
    assert!(!failure_text.contains("MCP startup incomplete"));
    assert!(chat.bottom_pane.is_task_running());

    notify_mcp_status(&mut chat, "beta", McpServerStartupState::Starting);

    assert!(drain_insert_history(&mut rx).is_empty());
    assert!(chat.bottom_pane.is_task_running());

    notify_mcp_status(&mut chat, "beta", McpServerStartupState::Ready);

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

    notify_mcp_status(&mut chat, "alpha", McpServerStartupState::Starting);
    notify_mcp_status_error(
        &mut chat,
        "alpha",
        "MCP client for `alpha` failed to start: handshake failed",
    );
    notify_mcp_status(&mut chat, "beta", McpServerStartupState::Starting);

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

    notify_mcp_status(&mut chat, "beta", McpServerStartupState::Starting);

    assert!(drain_insert_history(&mut rx).is_empty());
    assert!(!chat.bottom_pane.is_task_running());

    notify_mcp_status(&mut chat, "beta", McpServerStartupState::Ready);

    assert!(drain_insert_history(&mut rx).is_empty());
    assert!(!chat.bottom_pane.is_task_running());
}

#[tokio::test]
async fn app_server_mcp_startup_after_lag_can_settle_without_starting_updates() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.show_welcome_banner = false;
    chat.set_mcp_startup_expected_servers(["alpha".to_string(), "beta".to_string()]);

    chat.finish_mcp_startup_after_lag();

    notify_mcp_status_error(
        &mut chat,
        "alpha",
        "MCP client for `alpha` failed to start: handshake failed",
    );

    let failure_text = drain_insert_history(&mut rx)
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    assert!(failure_text.contains("MCP client for `alpha` failed to start: handshake failed"));
    assert!(chat.bottom_pane.is_task_running());

    notify_mcp_status(&mut chat, "beta", McpServerStartupState::Ready);

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

    notify_mcp_status(&mut chat, "alpha", McpServerStartupState::Starting);
    notify_mcp_status_error(
        &mut chat,
        "alpha",
        "MCP client for `alpha` failed to start: handshake failed",
    );
    notify_mcp_status(&mut chat, "beta", McpServerStartupState::Starting);
    let _ = drain_insert_history(&mut rx);

    chat.finish_mcp_startup_after_lag();
    let _ = drain_insert_history(&mut rx);
    assert!(!chat.bottom_pane.is_task_running());

    notify_mcp_status_error(
        &mut chat,
        "alpha",
        "MCP client for `alpha` failed to start: handshake failed",
    );

    assert!(drain_insert_history(&mut rx).is_empty());
    assert!(!chat.bottom_pane.is_task_running());

    chat.finish_mcp_startup_after_lag();

    notify_mcp_status(&mut chat, "beta", McpServerStartupState::Ready);

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

    notify_mcp_status(&mut chat, "alpha", McpServerStartupState::Starting);
    notify_mcp_status_error(
        &mut chat,
        "alpha",
        "MCP client for `alpha` failed to start: handshake failed",
    );
    notify_mcp_status(&mut chat, "beta", McpServerStartupState::Starting);
    let _ = drain_insert_history(&mut rx);

    chat.finish_mcp_startup_after_lag();
    let _ = drain_insert_history(&mut rx);
    assert!(!chat.bottom_pane.is_task_running());

    notify_mcp_status_error(
        &mut chat,
        "alpha",
        "MCP client for `alpha` failed to start: stale handshake failed",
    );
    assert!(drain_insert_history(&mut rx).is_empty());

    notify_mcp_status(&mut chat, "beta", McpServerStartupState::Starting);
    assert!(drain_insert_history(&mut rx).is_empty());
    assert!(!chat.bottom_pane.is_task_running());

    notify_mcp_status(&mut chat, "alpha", McpServerStartupState::Ready);
    assert!(drain_insert_history(&mut rx).is_empty());
    assert!(chat.bottom_pane.is_task_running());

    notify_mcp_status(&mut chat, "beta", McpServerStartupState::Ready);

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

    notify_mcp_status(&mut chat, "alpha", McpServerStartupState::Starting);
    assert!(drain_insert_history(&mut rx).is_empty());

    notify_mcp_status_error(
        &mut chat,
        "alpha",
        "MCP client for `alpha` failed to start: handshake failed",
    );

    let failure_text = drain_insert_history(&mut rx)
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    assert!(failure_text.contains("MCP client for `alpha` failed to start: handshake failed"));

    notify_mcp_status(&mut chat, "beta", McpServerStartupState::Starting);
    assert!(drain_insert_history(&mut rx).is_empty());
    assert!(chat.bottom_pane.is_task_running());

    notify_mcp_status(&mut chat, "beta", McpServerStartupState::Ready);

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

    notify_mcp_status(&mut chat, "runtime", McpServerStartupState::Starting);
    assert!(drain_insert_history(&mut rx).is_empty());
    assert!(chat.bottom_pane.is_task_running());

    notify_mcp_status_error(
        &mut chat,
        "runtime",
        "MCP client for `runtime` failed to start: handshake failed",
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
async fn app_server_mcp_startup_after_lag_includes_runtime_servers_with_expected_set() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.show_welcome_banner = false;
    chat.set_mcp_startup_expected_servers(["alpha".to_string()]);

    notify_mcp_status_error(
        &mut chat,
        "runtime",
        "MCP client for `runtime` failed to start: handshake failed",
    );

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

    notify_mcp_status(&mut chat, "alpha", McpServerStartupState::Starting);
    notify_mcp_status_error(
        &mut chat,
        "alpha",
        "MCP client for `alpha` failed to start: handshake failed",
    );
    notify_mcp_status(&mut chat, "beta", McpServerStartupState::Starting);
    let _ = drain_insert_history(&mut rx);

    chat.finish_mcp_startup_after_lag();
    let _ = drain_insert_history(&mut rx);
    assert!(!chat.bottom_pane.is_task_running());

    notify_mcp_status_error(
        &mut chat,
        "alpha",
        "MCP client for `alpha` failed to start: stale handshake failed",
    );
    assert!(drain_insert_history(&mut rx).is_empty());

    chat.finish_mcp_startup_after_lag();

    notify_mcp_status_error(
        &mut chat,
        "alpha",
        "MCP client for `alpha` failed to start: handshake failed",
    );

    let failure_text = drain_insert_history(&mut rx)
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    assert!(failure_text.is_empty());
    assert!(!chat.bottom_pane.is_task_running());

    notify_mcp_status(&mut chat, "beta", McpServerStartupState::Ready);

    let summary_text = drain_insert_history(&mut rx)
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    assert!(summary_text.contains("MCP client for `alpha` failed to start: handshake failed"));
    assert!(summary_text.contains("MCP startup incomplete (failed: alpha)"));
    assert!(!chat.bottom_pane.is_task_running());
}
