use super::*;
use pretty_assertions::assert_eq;

fn notify_mcp_status(chat: &mut ChatWidget, name: &str, status: McpServerStartupState) {
    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            thread_id: Some("thread-1".to_string()),
            name: name.to_string(),
            status,
            error: None,
            failure_reason: None,
        }),
        /*replay_kind*/ None,
    );
}

fn notify_mcp_status_error(chat: &mut ChatWidget, name: &str, error: &str) {
    chat.handle_server_notification(
        ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
            thread_id: Some("thread-1".to_string()),
            name: name.to_string(),
            status: McpServerStartupState::Failed,
            error: Some(error.to_string()),
            failure_reason: None,
        }),
        /*replay_kind*/ None,
    );
}

#[tokio::test]
async fn mcp_startup_ignores_status_for_other_thread() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.show_welcome_banner = false;
    chat.set_mcp_startup_expected_servers(["sentry".to_string()]);
    let parent_thread_id = ThreadId::new();
    let child_thread_id = ThreadId::new();
    chat.thread_id = Some(parent_thread_id);
    chat.on_stream_error(
        "Connection interrupted, retrying".to_string(),
        /*additional_details*/ None,
    );
    let status_before = chat.status_state.current_status.clone();
    let retry_status_header_before = chat.status_state.retry_status_header.clone();

    for status in [
        McpServerStartupState::Starting,
        McpServerStartupState::Failed,
    ] {
        chat.handle_server_notification(
            ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
                thread_id: Some(child_thread_id.to_string()),
                name: "sentry".to_string(),
                status,
                error: matches!(status, McpServerStartupState::Failed)
                    .then(|| "sentry is not logged in".to_string()),
                failure_reason: None,
            }),
            /*replay_kind*/ None,
        );
    }

    assert!(drain_insert_history(&mut rx).is_empty());
    assert!(!chat.bottom_pane.is_task_running());
    assert!(chat.mcp_startup_status.is_none());
    assert_eq!(chat.status_state.current_status, status_before);
    assert_eq!(
        chat.status_state.retry_status_header,
        retry_status_header_before
    );
}

#[tokio::test]
async fn mcp_apps_readiness_retries_an_in_flight_installed_mention_lookup() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    set_chatgpt_auth(&mut chat);
    chat.set_feature_enabled(Feature::Apps, /*enabled*/ true);

    chat.refresh_connector_mentions(/*force_refresh*/ false);
    chat.on_mcp_server_status_updated(McpServerStatusUpdatedNotification {
        thread_id: None,
        name: "codex_apps".to_string(),
        status: McpServerStartupState::Ready,
        error: None,
        failure_reason: None,
    });

    assert_eq!(chat.connectors.mention_refresh_pending, Some(false));
    let snapshot = crate::app_event::ConnectorsSnapshot { connectors: vec![] };
    chat.on_connector_mentions_loaded(chat.connector_scope_generation(), Ok(snapshot));
    assert!(chat.connectors.mention_refresh_in_flight);
}

#[tokio::test]
async fn mcp_startup_dedupes_same_round_duplicate_failure_warning() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.show_welcome_banner = false;
    chat.set_mcp_startup_expected_servers(["alpha".to_string(), "beta".to_string()]);

    notify_mcp_status(&mut chat, "alpha", McpServerStartupState::Starting);
    notify_mcp_status_error(
        &mut chat,
        "alpha",
        "MCP client for `alpha` failed to start: handshake failed",
    );
    notify_mcp_status_error(
        &mut chat,
        "alpha",
        "MCP client for `alpha` failed to start: handshake failed",
    );

    let failure_text = drain_insert_history(&mut rx)
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    assert_eq!(
        failure_text,
        "⚠ MCP client for `alpha` failed to start: handshake failed\n"
    );

    notify_mcp_status(&mut chat, "beta", McpServerStartupState::Ready);

    let summary_text = drain_insert_history(&mut rx)
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    assert_eq!(summary_text, "⚠ MCP startup incomplete (failed: alpha)\n");
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
    assert_eq!(chat.status_state.current_status.header, "Working");
}

#[tokio::test]
async fn pending_mcp_startup_does_not_block_queued_follow_up() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_mcp_startup_expected_servers(["slow".to_string()]);
    notify_mcp_status(&mut chat, "slow", McpServerStartupState::Starting);
    chat.thread_id = Some(ThreadId::new());
    handle_turn_started(&mut chat, "turn-1");
    chat.queue_user_message("queued follow-up".into());

    handle_turn_completed(&mut chat, "turn-1", /*duration_ms*/ None);

    assert!(chat.mcp_startup_status.is_some());
    assert!(chat.bottom_pane.is_task_running());
    assert!(chat.input_queue.queued_user_messages.is_empty());
    assert_matches!(next_submit_op(&mut op_rx), Op::UserTurn { items, .. } if items == vec![
        UserInput::Text {
            text: "queued follow-up".to_string(),
            text_elements: Vec::new(),
        }
    ]);

    chat.finish_mcp_startup(Vec::new(), Vec::new());

    assert!(chat.input_queue.user_turn_pending_start);
}

#[tokio::test]
async fn pending_mcp_startup_dispatches_queued_slash_commands() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_mcp_startup_expected_servers(["slow".to_string()]);
    notify_mcp_status(&mut chat, "slow", McpServerStartupState::Starting);
    chat.thread_id = Some(ThreadId::new());
    chat.bottom_pane
        .set_composer_text("/resume".to_string(), Vec::new(), Vec::new());

    chat.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));

    assert_matches!(rx.try_recv(), Ok(AppEvent::OpenResumePicker));
    assert_no_submit_op(&mut op_rx);
}

#[tokio::test]
async fn pending_mcp_startup_does_not_reject_queued_compaction() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_mcp_startup_expected_servers(["slow".to_string()]);
    notify_mcp_status(&mut chat, "slow", McpServerStartupState::Starting);
    chat.thread_id = Some(ThreadId::new());
    handle_turn_started(&mut chat, "turn-1");
    chat.bottom_pane
        .set_composer_text("/compact".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));

    handle_turn_completed(&mut chat, "turn-1", /*duration_ms*/ None);

    assert!(
        std::iter::from_fn(|| rx.try_recv().ok())
            .any(|event| matches!(event, AppEvent::CodexOp(Op::Compact)))
    );
}

#[tokio::test]
async fn pending_mcp_startup_does_not_drain_follow_up_before_review_starts() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_mcp_startup_expected_servers(["slow".to_string()]);
    notify_mcp_status(&mut chat, "slow", McpServerStartupState::Starting);
    chat.thread_id = Some(ThreadId::new());
    handle_turn_started(&mut chat, "turn-1");
    for message in ["/review", "queued follow-up"] {
        chat.bottom_pane
            .set_composer_text(message.to_string(), Vec::new(), Vec::new());
        chat.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    }
    handle_turn_completed(&mut chat, "turn-1", /*duration_ms*/ None);

    chat.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_eq!(chat.input_queue.queued_user_messages.len(), 1);
    assert!(
        std::iter::from_fn(|| rx.try_recv().ok())
            .any(|event| { matches!(event, AppEvent::CodexOp(Op::Review { .. })) })
    );
    assert_no_submit_op(&mut op_rx);
}

#[tokio::test]
async fn pending_mcp_startup_does_not_unblock_external_review() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_mcp_startup_expected_servers(["slow".to_string()]);
    notify_mcp_status(&mut chat, "slow", McpServerStartupState::Starting);
    chat.thread_id = Some(ThreadId::new());

    handle_entered_review_mode(&mut chat, "current changes");
    chat.queue_user_message("queued follow-up".into());

    assert!(chat.review.is_review_mode);
    assert!(!chat.turn_lifecycle.agent_turn_running);
    assert_eq!(chat.input_queue.queued_user_messages.len(), 1);
    assert_no_submit_op(&mut op_rx);

    while rx.try_recv().is_ok() {}
    chat.dispatch_command(crate::slash_command::SlashCommand::Fork);
    assert_matches!(rx.try_recv(), Ok(AppEvent::InsertHistoryCell(_)));

    chat.finish_mcp_startup(Vec::new(), Vec::new());
    assert!(chat.bottom_pane.is_task_running());
    assert_eq!(chat.input_queue.queued_user_messages.len(), 1);
    assert_no_submit_op(&mut op_rx);
}

#[tokio::test]
async fn pending_mcp_startup_does_not_unblock_foreground_shell() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_mcp_startup_expected_servers(["slow".to_string()]);
    notify_mcp_status(&mut chat, "slow", McpServerStartupState::Starting);
    chat.thread_id = Some(ThreadId::new());

    chat.queue_user_message_with_options(
        "!echo hi".into(),
        QueuedInputAction::RunShell,
        Vec::new(),
    );

    assert_matches!(op_rx.try_recv(), Ok(Op::RunUserShellCommand { command }) if command == "echo hi");
    chat.bottom_pane
        .set_composer_text("queued follow-up".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(chat.input_queue.queued_user_messages.len(), 1);
    assert_no_submit_op(&mut op_rx);

    chat.finish_mcp_startup(Vec::new(), Vec::new());
    assert!(chat.bottom_pane.is_task_running());
    assert_eq!(chat.input_queue.queued_user_messages.len(), 1);
    assert_no_submit_op(&mut op_rx);

    while rx.try_recv().is_ok() {}
    chat.dispatch_command(crate::slash_command::SlashCommand::Fork);
    assert_matches!(rx.try_recv(), Ok(AppEvent::InsertHistoryCell(_)));
}

#[tokio::test]
async fn pending_mcp_startup_does_not_unblock_foreground_compaction() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.dispatch_command(crate::slash_command::SlashCommand::Compact);
    chat.set_mcp_startup_expected_servers(["slow".to_string()]);
    notify_mcp_status(&mut chat, "slow", McpServerStartupState::Starting);
    chat.thread_id = Some(ThreadId::new());

    chat.queue_user_message("queued follow-up".into());

    assert_eq!(chat.input_queue.queued_user_messages.len(), 1);
    assert_no_submit_op(&mut op_rx);
}

#[tokio::test]
async fn turn_start_preserves_active_mcp_startup_header() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_mcp_startup_expected_servers(["schaltwerk".to_string()]);

    notify_mcp_status(&mut chat, "schaltwerk", McpServerStartupState::Starting);
    handle_turn_started(&mut chat, "turn-1");

    assert!(chat.bottom_pane.is_task_running());
    assert_eq!(
        chat.status_state.current_status.header,
        "Booting MCP server: schaltwerk"
    );

    notify_mcp_status(&mut chat, "schaltwerk", McpServerStartupState::Ready);

    assert_eq!(chat.status_state.current_status.header, "Working");
}

#[tokio::test]
async fn turn_start_replaces_idle_completed_mcp_startup_header() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_mcp_startup_expected_servers(["schaltwerk".to_string()]);

    notify_mcp_status(&mut chat, "schaltwerk", McpServerStartupState::Starting);
    notify_mcp_status(&mut chat, "schaltwerk", McpServerStartupState::Ready);

    assert!(!chat.bottom_pane.is_task_running());
    assert_eq!(
        chat.status_state.current_status.header,
        "Booting MCP server: schaltwerk"
    );

    handle_turn_started(&mut chat, "turn-1");

    assert!(chat.bottom_pane.is_task_running());
    assert_eq!(chat.status_state.current_status.header, "Working");
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
    let vt_height: u16 = ui_height.saturating_add(1).max(10);
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
async fn mcp_startup_failure_restores_running_status_header() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.show_welcome_banner = false;
    chat.set_mcp_startup_expected_servers(["alpha".to_string(), "beta".to_string()]);
    handle_turn_started(&mut chat, "turn-1");

    notify_mcp_status(&mut chat, "alpha", McpServerStartupState::Starting);
    notify_mcp_status(&mut chat, "beta", McpServerStartupState::Starting);
    assert!(
        chat.status_state
            .current_status
            .header
            .starts_with("Starting MCP servers")
    );

    notify_mcp_status_error(
        &mut chat,
        "alpha",
        "MCP client for `alpha` failed to start: handshake failed",
    );
    notify_mcp_status(&mut chat, "beta", McpServerStartupState::Ready);
    let _ = drain_insert_history(&mut rx);

    assert!(chat.bottom_pane.is_task_running());
    assert!(chat.bottom_pane.status_indicator_visible());
    assert_eq!(chat.status_state.current_status.header, "Working");
}

#[tokio::test]
async fn mcp_startup_complete_preserves_review_status() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.show_welcome_banner = false;
    chat.set_mcp_startup_expected_servers(["alpha".to_string()]);
    handle_turn_started(&mut chat, "turn-1");

    notify_mcp_status(&mut chat, "alpha", McpServerStartupState::Starting);
    assert!(
        chat.status_state
            .current_status
            .header
            .starts_with("Booting MCP server")
    );

    chat.on_guardian_assessment(GuardianAssessmentEvent {
        id: "guardian-1".to_string(),
        target_item_id: Some("guardian-target-1".to_string()),
        plugin_id: None,
        script_path: None,
        turn_id: "turn-1".to_string(),
        started_at_ms: 0,
        completed_at_ms: None,
        status: GuardianAssessmentStatus::InProgress,
        risk_level: None,
        user_authorization: None,
        rationale: None,
        decision_source: None,
        action: GuardianAssessmentAction::Command {
            source: GuardianCommandSource::Shell,
            command: "rm -rf '/tmp/guardian target'".to_string(),
            cwd: test_path_buf("/tmp").abs(),
        },
    });

    notify_mcp_status(&mut chat, "alpha", McpServerStartupState::Ready);

    assert!(chat.bottom_pane.is_task_running());
    assert!(chat.bottom_pane.status_indicator_visible());
    assert_eq!(
        chat.status_state.current_status.header,
        "Reviewing approval request"
    );
    assert_eq!(
        chat.status_state.current_status.details,
        Some("rm -rf '/tmp/guardian target'".to_string())
    );
}

#[tokio::test]
async fn app_server_mcp_startup_lag_settles_startup_and_ignores_late_updates() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.show_welcome_banner = false;
    chat.set_mcp_startup_expected_servers([
        "alpha".to_string(),
        "beta".to_string(),
        "gamma".to_string(),
    ]);

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
    insta::assert_snapshot!(summary_text, @r"
⚠ MCP startup incomplete (failed: alpha)
");
    assert!(!chat.bottom_pane.is_task_running());

    notify_mcp_status(&mut chat, "beta", McpServerStartupState::Starting);

    assert!(drain_insert_history(&mut rx).is_empty());
    assert!(!chat.bottom_pane.is_task_running());

    notify_mcp_status(&mut chat, "beta", McpServerStartupState::Ready);

    assert!(drain_insert_history(&mut rx).is_empty());
    assert!(!chat.bottom_pane.is_task_running());
}

#[tokio::test]
async fn app_server_mcp_startup_lag_reports_only_explicit_cancellations() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.show_welcome_banner = false;
    chat.set_mcp_startup_expected_servers([
        "alpha".to_string(),
        "beta".to_string(),
        "gamma".to_string(),
    ]);

    notify_mcp_status(&mut chat, "alpha", McpServerStartupState::Starting);
    notify_mcp_status(&mut chat, "alpha", McpServerStartupState::Cancelled);
    notify_mcp_status(&mut chat, "beta", McpServerStartupState::Starting);

    let _ = drain_insert_history(&mut rx);
    assert!(chat.bottom_pane.is_task_running());

    chat.finish_mcp_startup_after_lag();

    let summary_text = drain_insert_history(&mut rx)
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    assert_eq!(
        summary_text,
        "⚠ MCP startup interrupted. The following servers were not initialized: alpha\n"
    );
    assert!(!chat.bottom_pane.is_task_running());
}

#[tokio::test]
async fn app_server_mcp_startup_reports_failure_after_lag() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.show_welcome_banner = false;
    chat.set_mcp_startup_expected_servers(["alpha".to_string()]);
    notify_mcp_status(&mut chat, "alpha", McpServerStartupState::Starting);

    chat.finish_mcp_startup_after_lag();
    assert!(drain_insert_history(&mut rx).is_empty());

    notify_mcp_status_error(
        &mut chat,
        "alpha",
        "MCP client for `alpha` failed to start: handshake failed",
    );

    let warning_text = drain_insert_history(&mut rx)
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    assert!(warning_text.contains("MCP client for `alpha` failed to start: handshake failed"));
    assert!(warning_text.contains("MCP startup incomplete (failed: alpha)"));
    assert!(!warning_text.contains("MCP startup interrupted"));
}

#[tokio::test]
async fn app_server_mcp_startup_reports_cancellation_after_lag() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.show_welcome_banner = false;
    chat.set_mcp_startup_expected_servers(["alpha".to_string()]);
    notify_mcp_status(&mut chat, "alpha", McpServerStartupState::Starting);

    chat.finish_mcp_startup_after_lag();
    assert!(drain_insert_history(&mut rx).is_empty());

    notify_mcp_status(&mut chat, "alpha", McpServerStartupState::Cancelled);

    let warning_text = drain_insert_history(&mut rx)
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<String>();
    assert_eq!(
        warning_text,
        "⚠ MCP startup interrupted. The following servers were not initialized: alpha\n"
    );
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
