use super::*;
use crate::chatwidget::UserMessage;
use codex_app_server_protocol::CodexErrorInfo;
use codex_app_server_protocol::ErrorNotification;
use codex_app_server_protocol::ThreadGoalStatus;
use codex_protocol::models::ManagedFileSystemPermissions;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn worktree_creation_event_requires_feature() -> Result<()> {
    let (mut app, mut events, _op_rx) = make_test_app_with_channels().await;
    app.config.features.disable(Feature::Worktrees)?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    while events.try_recv().is_ok() {}
    app.handle_event(
        &mut tui,
        &mut app_server,
        AppEvent::StartManagedWorktree {
            mode: crate::app_event::ManagedWorktreeMode::New,
            name: None,
        },
    )
    .await?;
    drain_managed_worktree_start(&mut app, &mut app_server).await;
    let message = std::iter::from_fn(|| events.try_recv().ok())
        .find_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => {
                Some(lines_to_single_string(&cell.display_lines(/*width*/ 100)))
            }
            _ => None,
        })
        .expect("disabled feature message");
    insta::assert_snapshot!(message, @"■ Enable worktrees in your Codex configuration to create a worktree.");
    assert!(!app.config.codex_home.join("worktrees").exists());
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn worktree_creation_rejects_untrusted_source_before_allocation() -> Result<()> {
    use crate::app_event::ManagedWorktreeMode;

    let (mut app, mut events, _op_rx) = make_test_app_with_channels().await;
    app.config.features.enable(Feature::Worktrees)?;
    app.config.active_project.trust_level =
        Some(codex_protocol::config_types::TrustLevel::Untrusted);
    let thread_id = ThreadId::new();
    app.primary_thread_id = Some(thread_id);
    app.chat_widget
        .handle_thread_session_quiet(test_thread_session(thread_id, app.config.cwd.to_path_buf()));
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    for mode in [ManagedWorktreeMode::New, ManagedWorktreeMode::Fork] {
        if mode == ManagedWorktreeMode::Fork {
            app.config.active_project.trust_level =
                Some(codex_protocol::config_types::TrustLevel::Trusted);
            crate::legacy_core::config::set_project_trust_level(
                &app.config.codex_home,
                app.config.cwd.as_path(),
                codex_protocol::config_types::TrustLevel::Untrusted,
            )
            .map_err(|error| color_eyre::eyre::eyre!(error.to_string()))?;
        }
        while events.try_recv().is_ok() {}
        app.handle_event(
            &mut tui,
            &mut app_server,
            AppEvent::StartManagedWorktree { mode, name: None },
        )
        .await?;
        drain_managed_worktree_start(&mut app, &mut app_server).await;
        let message = std::iter::from_fn(|| events.try_recv().ok())
            .find_map(|event| match event {
                AppEvent::InsertHistoryCell(cell) => {
                    Some(lines_to_single_string(&cell.display_lines(/*width*/ 100)))
                }
                _ => None,
            })
            .expect("source trust error");
        insta::allow_duplicates! {
            insta::assert_snapshot!(message, @"■ Cannot create a worktree from an explicitly untrusted source.");
        }
        assert!(!app.config.codex_home.join("worktrees").exists());
    }
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn worktree_creation_rejects_running_agent_before_allocation() -> Result<()> {
    let (mut app, mut events, _op_rx) = make_test_app_with_channels().await;
    let home = tempfile::tempdir()?;
    app.config.codex_home = home.path().to_path_buf().abs();
    app.config.features.enable(Feature::Worktrees)?;
    app.config.active_project.trust_level = Some(codex_protocol::config_types::TrustLevel::Trusted);
    let primary = ThreadId::new();
    app.primary_thread_id = Some(primary);
    app.chat_widget
        .handle_thread_session_quiet(test_thread_session(primary, app.config.cwd.to_path_buf()));
    let child = ThreadId::new();
    app.agent_navigation.upsert(
        child, /*agent_nickname*/ None, /*agent_role*/ None, /*is_closed*/ false,
    );
    app.agent_navigation.set_running(child, /*is_running*/ true);
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    while events.try_recv().is_ok() {}

    app.handle_event(
        &mut tui,
        &mut server,
        AppEvent::StartManagedWorktree {
            mode: crate::app_event::ManagedWorktreeMode::New,
            name: None,
        },
    )
    .await?;
    drain_managed_worktree_start(&mut app, &mut server).await;

    assert!(!app.pending_managed_worktree_creation);
    assert!(!home.path().join("worktrees").exists());
    let message = std::iter::from_fn(|| events.try_recv().ok())
        .find_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => {
                Some(lines_to_single_string(&cell.display_lines(/*width*/ 100)))
            }
            _ => None,
        })
        .expect("running agent message");
    insta::assert_snapshot!(message, @"■ Cannot change: another agent is running.");
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn turn_start_failure_is_shown_without_exiting() -> Result<()> {
    let (mut app, mut app_event_rx, mut op_rx) = make_test_app_with_channels().await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let thread_id = ThreadId::from_string("123e4567-e89b-12d3-a456-426614174000")?;
    app.active_thread_id = Some(thread_id);
    app.chat_widget
        .handle_thread_session(test_thread_session(thread_id, app.config.cwd.to_path_buf()));
    while app_event_rx.try_recv().is_ok() {}

    app.chat_widget
        .restore_user_message_to_composer(UserMessage::from("hello"));
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    let op = next_user_turn_op(&mut op_rx);
    while let Ok(event) = app_event_rx.try_recv() {
        app.handle_event(&mut tui, &mut app_server, event).await?;
        drain_managed_worktree_start(&mut app, &mut app_server).await;
    }

    let control = app
        .handle_event(&mut tui, &mut app_server, AppEvent::CodexOp(op))
        .await?;

    assert!(matches!(control, AppRunControl::Continue));
    let error_cell = std::iter::from_fn(|| app_event_rx.try_recv().ok())
        .find_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => Some(cell),
            _ => None,
        })
        .expect("turn/start failure should be added to history");
    let transcript = app
        .transcript_cells
        .iter()
        .map(|cell| lines_to_single_string(&cell.display_lines(/*width*/ 80)))
        .chain(std::iter::once(lines_to_single_string(
            &error_cell.display_lines(/*width*/ 80),
        )))
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(transcript, @r"
    › hello

    ■ Failed to start turn: turn/start failed in TUI: turn/start failed: thread not found: 123e4567-e89b-12d3-a456-426614174000 (code -32600)
    ");

    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn misalignment_policy_blocks_queued_turns_and_goal_resumption() -> Result<()> {
    let (mut app, _app_event_rx, mut op_rx) = make_test_app_with_channels().await;
    let checkout = tempfile::tempdir()?;
    app.config.cwd = AbsolutePathBuf::from_absolute_path(checkout.path())?;
    for args in [
        vec!["init", "--quiet"],
        vec![
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            "initial",
        ],
    ] {
        assert!(
            std::process::Command::new("git")
                .args(args)
                .current_dir(checkout.path())
                .status()?
                .success()
        );
    }
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;
    let started = app_server.start_thread(&app.config).await?;
    let thread_id = started.session.thread_id;
    let session = started.session.clone();
    app.enqueue_primary_thread_session(started.session, started.turns)
        .await?;
    app_server
        .thread_goal_set(
            thread_id,
            Some("Do not restart this stopped goal".to_string()),
            Some(ThreadGoalStatus::Blocked),
            /*token_budget*/ None,
        )
        .await?;
    app.chat_widget
        .restore_user_message_to_composer("queued before terminal failure".into());
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    let queued_turn = next_user_turn_op(&mut op_rx);
    let failed_turn = Turn {
        error: Some(AppServerTurnError {
            misalignment: None,
            message: "misalignment policy violation".to_string(),
            codex_error_info: Some(CodexErrorInfo::MisalignmentPolicyViolation),
            additional_details: None,
        }),
        ..test_turn("turn-1", TurnStatus::Failed, Vec::new())
    };
    app.enqueue_primary_thread_session(session, vec![failed_turn])
        .await?;
    let precaution = render_bottom_popup(&app.chat_widget, /*width*/ 80);
    let AppServerRequestId::Integer(next_request_id) = app_server.next_request_id() else {
        unreachable!("embedded app-server request IDs are integers");
    };

    app.maybe_prompt_resume_paused_goal_after_resume(&mut app_server, thread_id)
        .await;
    for event in [
        AppEvent::CodexOp(queued_turn),
        AppEvent::CodexOp(crate::app_command::AppCommand::ExecApproval {
            id: "queued-approval".to_string(),
            turn_id: None,
            decision: codex_app_server_protocol::CommandExecutionApprovalDecision::Accept,
        }),
        AppEvent::SetThreadGoalStatus {
            thread_id,
            status: ThreadGoalStatus::Active,
        },
        AppEvent::SetThreadGoalDraft {
            thread_id,
            draft: crate::goal_files::GoalDraft {
                objective: "Do not replace this stopped goal".to_string(),
                ..Default::default()
            },
            mode: crate::app_event::ThreadGoalSetMode::ReplaceExisting,
        },
        AppEvent::StartSide {
            parent_thread_id: thread_id,
            user_message: Some("Do not fork this stopped thread".into()),
        },
        AppEvent::ForkCurrentSession { name: None },
        AppEvent::StartManagedWorktree {
            mode: crate::app_event::ManagedWorktreeMode::Fork,
            name: None,
        },
        AppEvent::OpenAgentPicker,
        AppEvent::SelectAgentThread(thread_id),
    ] {
        app.handle_event(&mut tui, &mut app_server, event).await?;
    }

    assert!(!app.config.codex_home.join("worktrees").exists());
    assert_eq!(
        app_server.next_request_id(),
        AppServerRequestId::Integer(next_request_id + 1)
    );
    assert_eq!(
        render_bottom_popup(&app.chat_widget, /*width*/ 80),
        precaution
    );
    assert_eq!(
        app_server
            .thread_goal_get(thread_id)
            .await?
            .goal
            .unwrap()
            .status,
        ThreadGoalStatus::Blocked
    );

    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn misalignment_policy_in_parent_stops_active_side_conversation() -> Result<()> {
    let (mut app, _app_event_rx, mut op_rx) = make_test_app_with_channels().await;
    let parent_thread_id = ThreadId::new();
    let side_thread_id = ThreadId::new();
    app.primary_thread_id = Some(parent_thread_id);
    app.active_thread_id = Some(side_thread_id);
    app.side_threads
        .insert(side_thread_id, SideThreadState::new(parent_thread_id));
    app.chat_widget.handle_thread_session(test_thread_session(
        side_thread_id,
        app.config.cwd.to_path_buf(),
    ));
    app.chat_widget
        .restore_user_message_to_composer("pending side turn".into());
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    while op_rx.try_recv().is_ok() {}

    app.enqueue_thread_notification(
        parent_thread_id,
        ServerNotification::Error(ErrorNotification {
            error: AppServerTurnError {
                misalignment: None,
                message: "misalignment policy violation".to_string(),
                codex_error_info: Some(CodexErrorInfo::MisalignmentPolicyViolation),
                additional_details: None,
            },
            will_retry: false,
            thread_id: parent_thread_id.to_string(),
            turn_id: "parent-turn".to_string(),
        }),
    )
    .await?;

    assert!(app.chat_widget.has_misalignment_policy_violation());
    assert!(
        render_bottom_popup(&app.chat_widget, /*width*/ 80)
            .contains("Chat stopped as a precaution")
    );
    assert_matches!(op_rx.try_recv(), Ok(Op::Interrupt));

    Ok(())
}

#[tokio::test]
async fn unsupported_legacy_permissions_are_shown_without_exiting() -> Result<()> {
    let (mut app, mut app_event_rx, mut op_rx) = make_test_app_with_channels().await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let thread_id = ThreadId::new();
    app.active_thread_id = Some(thread_id);
    app.chat_widget
        .handle_thread_session(test_thread_session(thread_id, app.config.cwd.to_path_buf()));

    let extra_root = codex_utils_absolute_path::AbsolutePathBuf::resolve_path_against_base(
        "extra",
        app.config.cwd.as_path(),
    );
    let permission_profile = PermissionProfile::Managed {
        network: NetworkSandboxPolicy::Restricted,
        file_system: ManagedFileSystemPermissions::Restricted {
            entries: vec![
                FileSystemSandboxEntry {
                    path: FileSystemPath::Special {
                        value: FileSystemSpecialPath::Root,
                    },
                    access: FileSystemAccessMode::Read,
                    missing_path_behavior: None,
                },
                FileSystemSandboxEntry {
                    path: FileSystemPath::Path {
                        path: extra_root.into(),
                    },
                    access: FileSystemAccessMode::Write,
                    missing_path_behavior: None,
                },
            ],
            glob_scan_max_depth: None,
        },
    };
    app.config
        .permissions
        .set_permission_profile(permission_profile.clone())?;
    app.chat_widget.set_permission_profile_with_active_profile(
        permission_profile,
        /*active_permission_profile*/ None,
    )?;
    app.runtime_permission_profile_override =
        Some(RuntimePermissionProfileOverride::from_config(&app.config));
    while app_event_rx.try_recv().is_ok() {}

    app.chat_widget
        .restore_user_message_to_composer(UserMessage::from("hello"));
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    let op = next_user_turn_op(&mut op_rx);
    app.chat_widget
        .restore_user_message_to_composer(UserMessage::from("queued follow-up"));
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.chat_widget.has_queued_follow_up_messages());
    while let Ok(event) = app_event_rx.try_recv() {
        app.handle_event(&mut tui, &mut app_server, event).await?;
    }

    let control = app
        .handle_event(&mut tui, &mut app_server, AppEvent::CodexOp(op))
        .await?;

    assert!(matches!(control, AppRunControl::Continue));
    assert!(app.chat_widget.has_queued_follow_up_messages());
    let error_cell = std::iter::from_fn(|| app_event_rx.try_recv().ok())
        .find_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => Some(cell),
            _ => None,
        })
        .expect("unsupported permissions should be added to history");
    insta::assert_snapshot!(
        lines_to_single_string(&error_cell.display_lines(/*width*/ 200)),
        @"■ Failed to start turn: the selected permission profile cannot be safely represented by the legacy app-server sandbox policy; select a named or legacy-compatible permission profile"
    );

    app.handle_event(
        &mut tui,
        &mut app_server,
        AppEvent::SettingsSelectionSettled,
    )
    .await?;
    assert!(app.chat_widget.has_queued_follow_up_messages());

    app.config
        .permissions
        .set_permission_profile(PermissionProfile::read_only())?;
    app.chat_widget.set_permission_profile_with_active_profile(
        PermissionProfile::read_only(),
        /*active_permission_profile*/ None,
    )?;
    app.runtime_permission_profile_override =
        Some(RuntimePermissionProfileOverride::from_config(&app.config));
    app.handle_event(
        &mut tui,
        &mut app_server,
        AppEvent::SettingsSelectionSettled,
    )
    .await?;
    assert!(!app.chat_widget.has_queued_follow_up_messages());

    app_server.shutdown().await?;
    Ok(())
}
