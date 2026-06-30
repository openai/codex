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

fn submit_bare_review(chat: &mut ChatWidget) {
    chat.bottom_pane
        .set_composer_text("/review".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
}

fn submit_review_with_args(
    chat: &mut ChatWidget,
    op_rx: &mut tokio::sync::mpsc::UnboundedReceiver<Op>,
    instructions: &str,
) {
    chat.dispatch_command_with_args(SlashCommand::Review, instructions.to_string(), Vec::new());
    assert_matches!(
        op_rx.try_recv(),
        Ok(Op::Review {
            target: ReviewTarget::Custom {
                instructions: actual
            }
        }) if actual == instructions
    );
}

fn assert_bare_review_rejected(
    chat: &mut ChatWidget,
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
    expected_error: &str,
) {
    submit_bare_review(chat);
    let error = drain_insert_history(rx)
        .into_iter()
        .map(|lines| lines_to_single_string(&lines))
        .collect::<String>();
    assert!(error.contains(expected_error), "unexpected error: {error}");
    assert_eq!(chat.bottom_pane.composer_text(), "/review");
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
async fn restored_mcp_only_thread_clears_busy_state_when_startup_finishes() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_mcp_startup_expected_servers(["alpha".to_string()]);
    notify_mcp_status(&mut chat, "alpha", McpServerStartupState::Starting);
    let input_state = chat.capture_thread_input_state();

    chat.restore_thread_input_state(/*input_state*/ None);
    chat.restore_thread_input_state(input_state);

    assert!(!chat.bottom_pane.is_foreground_task_running());
    assert!(chat.bottom_pane.is_mcp_startup_running());

    notify_mcp_status(&mut chat, "alpha", McpServerStartupState::Ready);

    assert!(!chat.bottom_pane.is_task_running());
}

#[tokio::test]
async fn review_command_during_mcp_startup_opens_popup_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    notify_mcp_status(&mut chat, "alpha", McpServerStartupState::Starting);

    submit_bare_review(&mut chat);

    assert!(chat.bottom_pane.composer_text().is_empty());
    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert_chatwidget_snapshot!("review_command_during_mcp_startup_opens_popup", popup);
}

#[tokio::test]
async fn background_mcp_startup_completion_does_not_finish_review_task() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_mcp_startup_expected_servers(["alpha".to_string(), "beta".to_string()]);
    notify_mcp_status(&mut chat, "alpha", McpServerStartupState::Starting);

    submit_review_with_args(&mut chat, &mut op_rx, "check regressions");
    assert!(chat.mcp_startup_status.is_some());
    assert!(chat.bottom_pane.is_foreground_task_running());
    assert!(chat.bottom_pane.is_mcp_startup_running());
    assert!(chat.bottom_pane.is_task_running());
    assert_eq!(chat.status_state.current_status.header, "Working");

    notify_mcp_status(&mut chat, "beta", McpServerStartupState::Starting);
    notify_mcp_status(&mut chat, "alpha", McpServerStartupState::Ready);
    notify_mcp_status(&mut chat, "beta", McpServerStartupState::Ready);

    assert!(chat.mcp_startup_status.is_none());
    assert!(chat.bottom_pane.is_task_running());
    assert_eq!(chat.status_state.current_status.header, "Working");

    handle_entered_review_mode(&mut chat, "current changes");
    handle_exited_review_mode(&mut chat);
    notify_mcp_status(&mut chat, "alpha", McpServerStartupState::Starting);
    notify_mcp_status(&mut chat, "beta", McpServerStartupState::Starting);
    assert!(chat.mcp_startup_status.is_some());
}

#[tokio::test]
async fn mcp_startup_can_complete_after_review_submission() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_mcp_startup_expected_servers(["alpha".to_string()]);

    submit_review_with_args(&mut chat, &mut op_rx, "check regressions");

    notify_mcp_status(&mut chat, "alpha", McpServerStartupState::Starting);
    notify_mcp_status(&mut chat, "alpha", McpServerStartupState::Ready);

    assert!(chat.mcp_startup_status.is_none());
    assert!(chat.bottom_pane.is_task_running());
    assert_eq!(chat.status_state.current_status.header, "Working");
}

#[tokio::test]
async fn failed_review_setup_restores_background_mcp_status() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_mcp_startup_expected_servers(["alpha".to_string()]);
    notify_mcp_status(&mut chat, "alpha", McpServerStartupState::Starting);

    submit_review_with_args(&mut chat, &mut op_rx, "review unrelated branch");
    assert!(chat.mcp_startup_status.is_some());
    assert!(chat.bottom_pane.is_foreground_task_running());
    assert!(chat.bottom_pane.is_mcp_startup_running());

    handle_error(
        &mut chat,
        "failed to resolve merge base",
        Some(CodexErrorInfo::Other),
    );
    notify_mcp_status(&mut chat, "alpha", McpServerStartupState::Starting);

    assert!(chat.mcp_startup_status.is_some());
    assert!(chat.bottom_pane.is_task_running());
    assert!(!chat.bottom_pane.is_foreground_task_running());
    assert!(chat.bottom_pane.is_mcp_startup_running());
    assert!(
        chat.status_state
            .current_status
            .header
            .starts_with("Booting MCP server")
    );
}

#[tokio::test]
async fn partial_mcp_startup_continues_after_failed_review_setup() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_mcp_startup_expected_servers(["alpha".to_string(), "beta".to_string()]);

    submit_review_with_args(&mut chat, &mut op_rx, "review unrelated branch");

    handle_error(
        &mut chat,
        "failed to resolve merge base",
        Some(CodexErrorInfo::Other),
    );
    assert!(!chat.bottom_pane.is_task_running());

    notify_mcp_status(&mut chat, "beta", McpServerStartupState::Ready);

    assert!(chat.mcp_startup_status.is_some());
    assert!(chat.bottom_pane.is_task_running());
    assert!(chat.bottom_pane.is_mcp_startup_running());

    notify_mcp_status(&mut chat, "alpha", McpServerStartupState::Starting);
    notify_mcp_status(&mut chat, "beta", McpServerStartupState::Starting);

    assert!(chat.mcp_startup_status.is_some());
    assert!(chat.bottom_pane.is_task_running());
}

#[tokio::test]
async fn replayed_active_review_keeps_mcp_startup_in_background() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_mcp_startup_expected_servers(["alpha".to_string()]);

    replay_entered_review_mode(&mut chat, "current changes");
    notify_mcp_status(&mut chat, "alpha", McpServerStartupState::Starting);

    assert!(chat.review.is_review_mode);
    assert!(chat.mcp_startup_status.is_some());
    assert!(chat.bottom_pane.is_mcp_startup_running());
    assert!(!chat.bottom_pane.is_foreground_task_running());

    handle_exited_review_mode(&mut chat);
    notify_mcp_status(&mut chat, "alpha", McpServerStartupState::Starting);

    assert!(chat.mcp_startup_status.is_some());
}

#[tokio::test]
async fn completed_replayed_review_does_not_hide_fresh_mcp_round() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_mcp_startup_expected_servers(["alpha".to_string(), "beta".to_string()]);

    replay_entered_review_mode(&mut chat, "current changes");
    chat.replay_thread_item(
        AppServerThreadItem::ExitedReviewMode {
            id: "review-end".to_string(),
            review: String::new(),
        },
        "turn-1".to_string(),
        ReplayKind::ThreadSnapshot,
    );
    notify_mcp_status(&mut chat, "beta", McpServerStartupState::Ready);

    assert!(chat.mcp_startup_status.is_some());
    assert!(chat.bottom_pane.is_task_running());
}

#[tokio::test]
async fn lag_recovery_after_failed_review_tracks_background_mcp_failure() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.show_welcome_banner = false;
    chat.set_mcp_startup_expected_servers(["alpha".to_string(), "beta".to_string()]);

    submit_review_with_args(&mut chat, &mut op_rx, "review unrelated branch");

    chat.finish_mcp_startup_after_lag();
    handle_error(
        &mut chat,
        "failed to resolve merge base",
        Some(CodexErrorInfo::Other),
    );
    let _ = drain_insert_history(&mut rx);

    notify_mcp_status_error(
        &mut chat,
        "alpha",
        "MCP client for `alpha` failed to start: handshake failed",
    );
    notify_mcp_status(&mut chat, "beta", McpServerStartupState::Ready);

    let warning = drain_insert_history(&mut rx)
        .into_iter()
        .map(|lines| lines_to_single_string(&lines))
        .collect::<String>();
    assert!(warning.contains("handshake failed"));
    assert!(chat.mcp_startup_status.is_none());
    assert!(!chat.bottom_pane.is_task_running());
}

#[tokio::test]
async fn bare_review_submission_during_agent_turn_preserves_draft() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    handle_turn_started(&mut chat, "turn-1");

    assert_bare_review_rejected(
        &mut chat,
        &mut rx,
        "'/review' is disabled while a task is in progress.",
    );
}

#[tokio::test]
async fn queued_prompt_blocks_review_during_mcp_startup() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    notify_mcp_status(&mut chat, "alpha", McpServerStartupState::Starting);
    chat.queue_user_message("queued prompt".into());
    let attachment = "https://example.com/review-context.png".to_string();
    chat.set_remote_image_urls(vec![attachment.clone()]);

    assert_bare_review_rejected(
        &mut chat,
        &mut rx,
        "'/review' is disabled while a task is in progress.",
    );
    assert_eq!(chat.remote_image_urls(), vec![attachment]);
}

#[tokio::test]
async fn compact_task_blocks_review_when_mcp_startup_arrives() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.dispatch_command(SlashCommand::Compact);
    assert!(chat.bottom_pane.is_task_running());
    assert_matches!(rx.try_recv(), Ok(AppEvent::CodexOp(Op::Compact)));
    notify_mcp_status(&mut chat, "alpha", McpServerStartupState::Starting);

    assert_bare_review_rejected(
        &mut chat,
        &mut rx,
        "'/review' is disabled while a task is in progress.",
    );
}

#[tokio::test]
async fn side_conversation_rejection_preserves_review_draft_during_mcp_startup() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    notify_mcp_status(&mut chat, "alpha", McpServerStartupState::Starting);
    chat.set_side_conversation_active(/*active*/ true);

    assert_bare_review_rejected(
        &mut chat,
        &mut rx,
        "'/review' is unavailable in side conversations.",
    );
}

#[tokio::test]
async fn side_conversation_rejection_without_mcp_clears_review_attachments() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_side_conversation_active(/*active*/ true);
    chat.set_remote_image_urls(vec!["https://example.com/review-context.png".to_string()]);

    submit_bare_review(&mut chat);

    let error = drain_insert_history(&mut rx)
        .into_iter()
        .map(|lines| lines_to_single_string(&lines))
        .collect::<String>();
    assert!(
        error.contains("'/review' is unavailable in side conversations."),
        "unexpected error: {error}"
    );
    assert_eq!(chat.bottom_pane.composer_text(), "");
    assert_eq!(chat.remote_image_urls(), Vec::<String>::new());
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
