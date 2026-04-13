//! Approval-request focused tests extracted from the main chatwidget test file
//! to keep the primary module under blob-size policy limits.

use super::*;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::request_permission_preset::PermissionPresetId;
use codex_protocol::request_permission_preset::RequestPermissionPresetEvent;
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
        cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
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
async fn permission_preset_request_opens_permissions_picker_with_requested_preset_selected() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    #[cfg(target_os = "windows")]
    {
        chat.config.notices.hide_world_writable_warning = Some(true);
        chat.set_windows_sandbox_mode(Some(WindowsSandboxModeToml::Unelevated));
    }
    chat.set_feature_enabled(Feature::GuardianApproval, /*enabled*/ false);

    let ev = RequestPermissionPresetEvent {
        call_id: "preset-call".into(),
        turn_id: "preset-turn".into(),
        preset: PermissionPresetId::FullAccess,
        label: "Full access".into(),
        description: "Run commands without filesystem sandboxing; network access enabled.".into(),
        approval_policy: AskForApproval::Never,
        approvals_reviewer: ApprovalsReviewer::User,
        sandbox_policy: SandboxPolicy::DangerFullAccess,
        reason: Some("the user asked to switch into full access mode".into()),
    };
    chat.handle_codex_event(Event {
        id: "sub-preset".into(),
        msg: EventMsg::RequestPermissionPreset(ev),
    });

    let proposed_cells = drain_insert_history(&mut rx);
    assert!(
        proposed_cells.is_empty(),
        "expected preset request to open the picker without emitting history cells"
    );

    let popup = render_bottom_popup(&chat, /*width*/ 100);
    assert!(
        popup
            .lines()
            .any(|line| line.contains('›') && line.contains("Full Access")),
        "expected Full Access to be preselected in the permissions picker: {popup}"
    );
    assert_snapshot!("permission_preset_request_picker", popup);

    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    let events = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(
        events.iter().any(|event| matches!(
            event,
            AppEvent::OpenFullAccessConfirmation {
                permission_preset_context: Some(_),
                ..
            }
        )),
        "expected selecting Full Access to invoke the existing confirmation flow: {events:?}"
    );
    assert!(
        !events.iter().any(|event| matches!(
            event,
            AppEvent::SubmitThreadOp {
                op: Op::RequestPermissionPresetResponse { .. },
                ..
            }
        )),
        "expected no permission preset response until the user confirms Full Access: {events:?}"
    );
}

#[test]
fn app_server_exec_approval_request_splits_shell_wrapped_command() {
    let script = r#"python3 -c 'print("Hello, world!")'"#;
    let request =
        exec_approval_request_from_params(AppServerCommandExecutionRequestApprovalParams {
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
            cwd: Some(PathBuf::from("/tmp")),
            command_actions: None,
            additional_permissions: None,
            proposed_execpolicy_amendment: None,
            proposed_network_policy_amendments: None,
            available_decisions: None,
        });

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
    let request =
        exec_approval_request_from_params(AppServerCommandExecutionRequestApprovalParams {
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
            cwd: Some(PathBuf::from("/tmp")),
            command_actions: None,
            additional_permissions: Some(AppServerAdditionalPermissionProfile {
                network: Some(AppServerAdditionalNetworkPermissions {
                    enabled: Some(true),
                }),
                file_system: Some(AppServerAdditionalFileSystemPermissions {
                    read: Some(vec![read_path.clone()]),
                    write: Some(vec![write_path.clone()]),
                }),
            }),
            proposed_execpolicy_amendment: None,
            proposed_network_policy_amendments: None,
            available_decisions: None,
        });

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
            file_system: Some(FileSystemPermissions {
                read: Some(vec![read_path]),
                write: Some(vec![write_path]),
            }),
        })
    );
}

#[test]
fn app_server_request_permissions_preserves_file_system_permissions() {
    let read_path = AbsolutePathBuf::try_from(PathBuf::from(test_path_display("/tmp/read-only")))
        .expect("absolute read path");
    let write_path = AbsolutePathBuf::try_from(PathBuf::from(test_path_display("/tmp/write")))
        .expect("absolute write path");

    let request = request_permissions_from_params(AppServerPermissionsRequestApprovalParams {
        thread_id: "thread-1".to_string(),
        turn_id: "turn-1".to_string(),
        item_id: "item-1".to_string(),
        reason: Some("Select a workspace root".to_string()),
        permissions: codex_app_server_protocol::RequestPermissionProfile {
            network: Some(AppServerAdditionalNetworkPermissions {
                enabled: Some(true),
            }),
            file_system: Some(AppServerAdditionalFileSystemPermissions {
                read: Some(vec![read_path.clone()]),
                write: Some(vec![write_path.clone()]),
            }),
        },
        suggested_scope: codex_app_server_protocol::PermissionGrantScope::Turn,
    });

    assert_eq!(
        request.permissions,
        RequestPermissionProfile {
            network: Some(NetworkPermissions {
                enabled: Some(true),
            }),
            file_system: Some(FileSystemPermissions {
                read: Some(vec![read_path]),
                write: Some(vec![write_path]),
            }),
        }
    );
    assert_eq!(
        request.suggested_scope,
        codex_protocol::request_permissions::PermissionGrantScope::Turn
    );
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
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
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
        cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
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
        cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
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
