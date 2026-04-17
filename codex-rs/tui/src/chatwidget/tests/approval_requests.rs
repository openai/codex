//! Approval-request focused tests extracted from the main chatwidget test file
//! to keep the primary module under blob-size policy limits.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn exec_approval_emits_proposed_command_and_decision_history() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    // Trigger an exec approval request with a short, single-line command.
    let ev = ExecApprovalRequestEvent {
        call_id: "call-short".into(),
        approval_id: Some("call-short".into()),
        turn_id: "turn-short".into(),
        command: vec!["bash".into(), "-lc".into(), "echo hello world".into()],
        cwd: AbsolutePathBuf::current_dir().expect("current dir"),
        reason: Some(
            "this is a test reason such as one that would be produced by the model".into(),
        ),
        network_approval_context: None,
        proposed_execpolicy_amendment: None,
        proposed_network_policy_amendments: None,
        additional_permissions: None,
        available_decisions: None,
        parsed_cmd: vec![],
    };
    chat.handle_codex_event(Event {
        id: "sub-short".into(),
        msg: EventMsg::ExecApprovalRequest(ev),
    });

    let proposed_cells = drain_insert_history(&mut rx);
    assert!(
        proposed_cells.is_empty(),
        "expected approval request to render via modal without emitting history cells"
    );

    let area = Rect::new(0, 0, 80, chat.desired_height(/*width*/ 80));
    let mut buf = ratatui::buffer::Buffer::empty(area);
    chat.render(area, &mut buf);
    assert_snapshot!("exec_approval_modal_exec", format!("{buf:?}"));

    chat.handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
    let decision = drain_insert_history(&mut rx)
        .pop()
        .expect("expected decision cell in history");
    assert_snapshot!(
        "exec_approval_history_decision_approved_short",
        lines_to_single_string(&decision)
    );
}

#[tokio::test]
async fn terminal_title_shows_action_required_while_exec_approval_is_pending() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.bottom_pane.set_task_running(/*running*/ true);
    chat.refresh_terminal_title();

    let request = ExecApprovalRequestEvent {
        call_id: "call-action-required".into(),
        approval_id: Some("call-action-required".into()),
        turn_id: "turn-action-required".into(),
        command: vec!["bash".into(), "-lc".into(), "echo hello".into()],
        cwd: AbsolutePathBuf::current_dir().expect("current dir"),
        reason: Some("need confirmation".into()),
        network_approval_context: None,
        proposed_execpolicy_amendment: None,
        proposed_network_policy_amendments: None,
        additional_permissions: None,
        available_decisions: None,
        parsed_cmd: vec![],
    };
    chat.handle_codex_event(Event {
        id: "sub-action-required".into(),
        msg: EventMsg::ExecApprovalRequest(request),
    });

    chat.pre_draw_tick();

    assert_eq!(
        chat.last_terminal_title,
        Some("[ ! ] Action Required | project".to_string())
    );
    assert!(!chat.should_animate_terminal_title_spinner());

    chat.handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
    chat.pre_draw_tick();

    let title = chat
        .last_terminal_title
        .as_deref()
        .expect("terminal title should be restored after approval");
    assert!(title.contains("project"));
    assert!(!title.contains("Action Required"));
    assert!(chat.should_animate_terminal_title_spinner());
}

#[tokio::test]
async fn terminal_title_action_required_respects_spinner_setting() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.config.tui_terminal_title = Some(vec!["project".to_string()]);
    chat.bottom_pane.set_task_running(/*running*/ true);
    chat.refresh_terminal_title();

    let request = ExecApprovalRequestEvent {
        call_id: "call-no-spinner".into(),
        approval_id: Some("call-no-spinner".into()),
        turn_id: "turn-no-spinner".into(),
        command: vec!["bash".into(), "-lc".into(), "echo hello".into()],
        cwd: AbsolutePathBuf::current_dir().expect("current dir"),
        reason: Some("need confirmation".into()),
        network_approval_context: None,
        proposed_execpolicy_amendment: None,
        proposed_network_policy_amendments: None,
        additional_permissions: None,
        available_decisions: None,
        parsed_cmd: vec![],
    };
    chat.handle_codex_event(Event {
        id: "sub-no-spinner".into(),
        msg: EventMsg::ExecApprovalRequest(request),
    });

    chat.pre_draw_tick();

    assert_eq!(chat.last_terminal_title, Some("project".to_string()));
    assert!(!chat.should_animate_terminal_title_action_required());
}

#[tokio::test]
async fn terminal_title_action_required_blinks_when_animations_are_enabled() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.bottom_pane.set_task_running(/*running*/ true);
    chat.terminal_title_animation_origin = Instant::now() - std::time::Duration::from_millis(1500);
    chat.refresh_terminal_title();

    let request = ExecApprovalRequestEvent {
        call_id: "call-blink".into(),
        approval_id: Some("call-blink".into()),
        turn_id: "turn-blink".into(),
        command: vec!["bash".into(), "-lc".into(), "echo hello".into()],
        cwd: AbsolutePathBuf::current_dir().expect("current dir"),
        reason: Some("need confirmation".into()),
        network_approval_context: None,
        proposed_execpolicy_amendment: None,
        proposed_network_policy_amendments: None,
        additional_permissions: None,
        available_decisions: None,
        parsed_cmd: vec![],
    };
    chat.handle_codex_event(Event {
        id: "sub-blink".into(),
        msg: EventMsg::ExecApprovalRequest(request),
    });

    chat.pre_draw_tick();

    assert_eq!(
        chat.last_terminal_title,
        Some("[ . ] Action Required | project".to_string())
    );
    assert!(chat.should_animate_terminal_title_action_required());
}

#[test]
fn app_server_exec_approval_request_splits_shell_wrapped_command() {
    let script = r#"python3 -c 'print("Hello, world!")'"#;
    let request = exec_approval_request_from_params(
        AppServerCommandExecutionRequestApprovalParams {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item_id: "item-1".to_string(),
            approval_id: Some("approval-1".to_string()),
            reason: None,
            network_approval_context: None,
            command: Some(
                shlex::try_join(["/bin/zsh", "-lc", script])
                    .expect("round-trippable shell wrapper"),
            ),
            cwd: Some(test_path_buf("/tmp").abs()),
            command_actions: None,
            additional_permissions: None,
            proposed_execpolicy_amendment: None,
            proposed_network_policy_amendments: None,
            available_decisions: None,
        },
        &test_path_buf("/tmp").abs(),
    );

    assert_eq!(
        request.command,
        vec![
            "/bin/zsh".to_string(),
            "-lc".to_string(),
            script.to_string(),
        ]
    );
}

#[test]
fn app_server_exec_approval_request_preserves_permissions_context() {
    let read_path = AbsolutePathBuf::try_from(PathBuf::from(test_path_display("/tmp/read-only")))
        .expect("absolute read path");
    let write_path = AbsolutePathBuf::try_from(PathBuf::from(test_path_display("/tmp/write")))
        .expect("absolute write path");
    let request = exec_approval_request_from_params(
        AppServerCommandExecutionRequestApprovalParams {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item_id: "item-1".to_string(),
            approval_id: Some("approval-1".to_string()),
            reason: None,
            network_approval_context: Some(codex_app_server_protocol::NetworkApprovalContext {
                host: "example.com".to_string(),
                protocol: codex_app_server_protocol::NetworkApprovalProtocol::Socks5Tcp,
            }),
            command: Some("ls".to_string()),
            cwd: Some(test_path_buf("/tmp").abs()),
            command_actions: None,
            additional_permissions: Some(AppServerAdditionalPermissionProfile {
                network: Some(AppServerAdditionalNetworkPermissions {
                    enabled: Some(true),
                }),
                file_system: Some(AppServerAdditionalFileSystemPermissions {
                    read: Some(vec![read_path.clone()]),
                    write: Some(vec![write_path.clone()]),
                    glob_scan_max_depth: None,
                    entries: None,
                }),
            }),
            proposed_execpolicy_amendment: None,
            proposed_network_policy_amendments: None,
            available_decisions: None,
        },
        &test_path_buf("/tmp").abs(),
    );

    assert_eq!(
        request.network_approval_context,
        Some(codex_protocol::protocol::NetworkApprovalContext {
            host: "example.com".to_string(),
            protocol: codex_protocol::protocol::NetworkApprovalProtocol::Socks5Tcp,
        })
    );
    assert_eq!(
        request.additional_permissions,
        Some(PermissionProfile {
            network: Some(NetworkPermissions {
                enabled: Some(true),
            }),
            file_system: Some(FileSystemPermissions::from_read_write_roots(
                Some(vec![read_path]),
                Some(vec![write_path]),
            )),
        })
    );
}

#[test]
fn app_server_request_permissions_preserves_file_system_permissions() {
    let read_path = AbsolutePathBuf::try_from(PathBuf::from(test_path_display("/tmp/read-only")))
        .expect("absolute read path");
    let write_path = AbsolutePathBuf::try_from(PathBuf::from(test_path_display("/tmp/write")))
        .expect("absolute write path");
    let cwd =
        AbsolutePathBuf::try_from(PathBuf::from(test_path_display("/tmp"))).expect("absolute cwd");

    let request = request_permissions_from_params(AppServerPermissionsRequestApprovalParams {
        thread_id: "thread-1".to_string(),
        turn_id: "turn-1".to_string(),
        item_id: "item-1".to_string(),
        cwd: cwd.clone(),
        reason: Some("Select a workspace root".to_string()),
        permissions: codex_app_server_protocol::RequestPermissionProfile {
            network: Some(AppServerAdditionalNetworkPermissions {
                enabled: Some(true),
            }),
            file_system: Some(AppServerAdditionalFileSystemPermissions {
                read: Some(vec![read_path.clone()]),
                write: Some(vec![write_path.clone()]),
                glob_scan_max_depth: None,
                entries: None,
            }),
        },
    });

    assert_eq!(
        request.permissions,
        RequestPermissionProfile {
            network: Some(NetworkPermissions {
                enabled: Some(true),
            }),
            file_system: Some(FileSystemPermissions::from_read_write_roots(
                Some(vec![read_path]),
                Some(vec![write_path]),
            )),
        }
    );
    assert_eq!(request.cwd, Some(cwd));
}

#[tokio::test]
async fn exec_approval_uses_approval_id_when_present() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_codex_event(Event {
        id: "sub-short".into(),
        msg: EventMsg::ExecApprovalRequest(ExecApprovalRequestEvent {
            call_id: "call-parent".into(),
            approval_id: Some("approval-subcommand".into()),
            turn_id: "turn-short".into(),
            command: vec!["bash".into(), "-lc".into(), "echo hello world".into()],
            cwd: AbsolutePathBuf::current_dir().expect("current dir"),
            reason: Some(
                "this is a test reason such as one that would be produced by the model".into(),
            ),
            network_approval_context: None,
            proposed_execpolicy_amendment: None,
            proposed_network_policy_amendments: None,
            additional_permissions: None,
            available_decisions: None,
            parsed_cmd: vec![],
        }),
    });

    chat.handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));

    let mut found = false;
    while let Ok(app_ev) = rx.try_recv() {
        if let AppEvent::SubmitThreadOp {
            op: Op::ExecApproval { id, decision, .. },
            ..
        } = app_ev
        {
            assert_eq!(id, "approval-subcommand");
            assert_matches!(decision, codex_protocol::protocol::ReviewDecision::Approved);
            found = true;
            break;
        }
    }
    assert!(found, "expected ExecApproval op to be sent");
}

#[tokio::test]
async fn exec_approval_decision_truncates_multiline_and_long_commands() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    let ev_multi = ExecApprovalRequestEvent {
        call_id: "call-multi".into(),
        approval_id: Some("call-multi".into()),
        turn_id: "turn-multi".into(),
        command: vec!["bash".into(), "-lc".into(), "echo line1\necho line2".into()],
        cwd: AbsolutePathBuf::current_dir().expect("current dir"),
        reason: Some(
            "this is a test reason such as one that would be produced by the model".into(),
        ),
        network_approval_context: None,
        proposed_execpolicy_amendment: None,
        proposed_network_policy_amendments: None,
        additional_permissions: None,
        available_decisions: None,
        parsed_cmd: vec![],
    };
    chat.handle_codex_event(Event {
        id: "sub-multi".into(),
        msg: EventMsg::ExecApprovalRequest(ev_multi),
    });
    let proposed_multi = drain_insert_history(&mut rx);
    assert!(
        proposed_multi.is_empty(),
        "expected multiline approval request to render via modal without emitting history cells"
    );

    let area = Rect::new(0, 0, 80, chat.desired_height(/*width*/ 80));
    let mut buf = ratatui::buffer::Buffer::empty(area);
    chat.render(area, &mut buf);
    let mut saw_first_line = false;
    for y in 0..area.height {
        let mut row = String::new();
        for x in 0..area.width {
            row.push(buf[(x, y)].symbol().chars().next().unwrap_or(' '));
        }
        if row.contains("echo line1") {
            saw_first_line = true;
            break;
        }
    }
    assert!(
        saw_first_line,
        "expected modal to show first line of multiline snippet"
    );

    chat.handle_key_event(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE));
    let aborted_multi = drain_insert_history(&mut rx)
        .pop()
        .expect("expected aborted decision cell (multiline)");
    assert_snapshot!(
        "exec_approval_history_decision_aborted_multiline",
        lines_to_single_string(&aborted_multi)
    );

    let long = format!("echo {}", "a".repeat(200));
    let ev_long = ExecApprovalRequestEvent {
        call_id: "call-long".into(),
        approval_id: Some("call-long".into()),
        turn_id: "turn-long".into(),
        command: vec!["bash".into(), "-lc".into(), long],
        cwd: AbsolutePathBuf::current_dir().expect("current dir"),
        reason: None,
        network_approval_context: None,
        proposed_execpolicy_amendment: None,
        proposed_network_policy_amendments: None,
        additional_permissions: None,
        available_decisions: None,
        parsed_cmd: vec![],
    };
    chat.handle_codex_event(Event {
        id: "sub-long".into(),
        msg: EventMsg::ExecApprovalRequest(ev_long),
    });
    let proposed_long = drain_insert_history(&mut rx);
    assert!(
        proposed_long.is_empty(),
        "expected long approval request to avoid emitting history cells before decision"
    );
    chat.handle_key_event(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE));
    let aborted_long = drain_insert_history(&mut rx)
        .pop()
        .expect("expected aborted decision cell (long)");
    assert_snapshot!(
        "exec_approval_history_decision_aborted_long",
        lines_to_single_string(&aborted_long)
    );
}
