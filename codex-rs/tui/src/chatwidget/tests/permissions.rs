use super::*;
use crate::legacy_core::config::CustomPermissionProfileSummary;
use codex_protocol::models::ActivePermissionProfile;
use codex_protocol::models::ManagedFileSystemPermissions;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use pretty_assertions::assert_eq;

fn app_server_workspace_write_profile(extra_root: AbsolutePathBuf) -> PermissionProfile {
    PermissionProfile::Managed {
        network: NetworkSandboxPolicy::Restricted,
        file_system: ManagedFileSystemPermissions::Restricted {
            entries: vec![
                FileSystemSandboxEntry {
                    path: FileSystemPath::Special {
                        value: FileSystemSpecialPath::Root,
                    },
                    access: FileSystemAccessMode::Read,
                },
                FileSystemSandboxEntry {
                    path: FileSystemPath::Special {
                        value: FileSystemSpecialPath::ProjectRoots { subpath: None },
                    },
                    access: FileSystemAccessMode::Write,
                },
                FileSystemSandboxEntry {
                    path: FileSystemPath::Special {
                        value: FileSystemSpecialPath::SlashTmp,
                    },
                    access: FileSystemAccessMode::Write,
                },
                FileSystemSandboxEntry {
                    path: FileSystemPath::Special {
                        value: FileSystemSpecialPath::Tmpdir,
                    },
                    access: FileSystemAccessMode::Write,
                },
                FileSystemSandboxEntry {
                    path: FileSystemPath::Path { path: extra_root },
                    access: FileSystemAccessMode::Write,
                },
            ],
            glob_scan_max_depth: None,
        },
    }
}

fn windows_sandbox_requirements_stack(
    allowed_sandbox_implementations: Vec<WindowsSandboxModeToml>,
) -> ConfigLayerStack {
    let requirements_toml = codex_config::ConfigRequirementsToml {
        windows: Some(codex_config::WindowsRequirementsToml {
            allowed_sandbox_implementations: Some(allowed_sandbox_implementations),
            sandbox_private_desktop: None,
        }),
        ..Default::default()
    };
    let mut requirements_with_sources = codex_config::ConfigRequirementsWithSources::default();
    requirements_with_sources
        .merge_unset_fields(RequirementSource::Unknown, requirements_toml.clone());
    let requirements = codex_config::ConfigRequirements::try_from(requirements_with_sources)
        .expect("windows sandbox requirements");

    ConfigLayerStack::new(Vec::new(), requirements, requirements_toml)
        .expect("test config layer stack")
}

#[tokio::test]
async fn approvals_selection_popup_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.set_feature_enabled(Feature::GuardianApproval, /*enabled*/ false);
    chat.config.notices.hide_full_access_warning = None;
    chat.open_approvals_popup();

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    #[cfg(target_os = "windows")]
    insta::with_settings!({ snapshot_suffix => "windows" }, {
        assert_chatwidget_snapshot!("approvals_selection_popup", popup);
    });
    #[cfg(not(target_os = "windows"))]
    assert_chatwidget_snapshot!("approvals_selection_popup", popup);
}

#[tokio::test]
async fn profile_permissions_selection_popup_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.config.explicit_permission_profile_mode = true;
    chat.config
        .permissions
        .set_permission_profile_from_session_snapshot(PermissionProfileSnapshot::active(
            PermissionProfile::workspace_write(),
            ActivePermissionProfile::new(":workspace"),
        ))
        .expect("set active profile");

    chat.open_permissions_popup();

    assert_chatwidget_snapshot!(
        "profile_permissions_selection_popup",
        render_bottom_popup(&chat, /*width*/ 80)
    );
}

#[tokio::test]
async fn profile_permissions_selection_popup_with_custom_profiles_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.config.explicit_permission_profile_mode = true;
    chat.config.custom_permission_profiles = vec![
        CustomPermissionProfileSummary {
            id: "locked-down".to_string(),
            description: Some("Inspect and patch only approved workspace files.".to_string()),
        },
        CustomPermissionProfileSummary {
            id: "web-enabled".to_string(),
            description: Some("Workspace profile with network access.".to_string()),
        },
    ];
    chat.config
        .permissions
        .set_permission_profile_from_session_snapshot(PermissionProfileSnapshot::active(
            PermissionProfile::workspace_write(),
            ActivePermissionProfile::new("locked-down"),
        ))
        .expect("set active profile");

    chat.open_permissions_popup();

    assert_chatwidget_snapshot!(
        "profile_permissions_selection_popup_with_custom_profiles",
        render_bottom_popup(&chat, /*width*/ 80)
    );
}

#[tokio::test]
async fn profile_permissions_selection_emits_named_profile_event_only() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    #[cfg(target_os = "windows")]
    {
        chat.set_windows_sandbox_mode(Some(WindowsSandboxModeToml::Unelevated));
    }
    chat.config.explicit_permission_profile_mode = true;
    chat.config
        .permissions
        .set_permission_profile_from_session_snapshot(PermissionProfileSnapshot::active(
            PermissionProfile::workspace_write(),
            ActivePermissionProfile::new(":workspace"),
        ))
        .expect("set active profile");

    chat.open_permissions_popup();
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    let events = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert_eq!(events.len(), 1);
    assert!(matches!(
        &events[0],
        AppEvent::SelectPermissionProfile(PermissionProfileSelection {
            profile_id,
            approval_policy: Some(AskForApproval::OnRequest),
            approvals_reviewer: Some(ApprovalsReviewer::User),
            display_label,
        }) if profile_id == ":workspace" && display_label == "Ask for approval"
    ));
}

#[tokio::test]
async fn profile_permissions_selection_emits_active_custom_profile() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.config.explicit_permission_profile_mode = true;
    chat.config.custom_permission_profiles = vec![CustomPermissionProfileSummary {
        id: "locked-down".to_string(),
        description: None,
    }];
    chat.config
        .permissions
        .set_permission_profile_from_session_snapshot(PermissionProfileSnapshot::active(
            PermissionProfile::workspace_write(),
            ActivePermissionProfile::new("locked-down"),
        ))
        .expect("set active profile");

    chat.open_permissions_popup();
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    let events = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert_eq!(events.len(), 1);
    assert!(matches!(
        &events[0],
        AppEvent::SelectPermissionProfile(PermissionProfileSelection {
            profile_id,
            approval_policy: None,
            approvals_reviewer: None,
            display_label,
        }) if profile_id == "locked-down" && display_label == "locked-down"
    ));
}

#[tokio::test]
async fn profile_permissions_selection_emits_auto_review_mode_event() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    #[cfg(target_os = "windows")]
    {
        chat.set_windows_sandbox_mode(Some(WindowsSandboxModeToml::Unelevated));
    }
    chat.config.explicit_permission_profile_mode = true;
    chat.config
        .permissions
        .set_permission_profile_from_session_snapshot(PermissionProfileSnapshot::active(
            PermissionProfile::workspace_write(),
            ActivePermissionProfile::new(":workspace"),
        ))
        .expect("set active profile");

    chat.open_permissions_popup();
    chat.handle_key_event(KeyEvent::from(KeyCode::Down));
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    let events = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert_eq!(events.len(), 1);
    assert!(matches!(
        &events[0],
        AppEvent::SelectPermissionProfile(PermissionProfileSelection {
            profile_id,
            approval_policy: Some(AskForApproval::OnRequest),
            approvals_reviewer: Some(ApprovalsReviewer::AutoReview),
            display_label,
        }) if profile_id == ":workspace" && display_label == "Approve for me"
    ));
}

#[tokio::test]
async fn profile_permissions_full_access_opens_confirmation() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.config.explicit_permission_profile_mode = true;
    chat.set_feature_enabled(Feature::GuardianApproval, /*enabled*/ false);
    chat.config.notices.hide_full_access_warning = None;

    chat.open_permissions_popup();
    chat.handle_key_event(KeyEvent::from(KeyCode::Up));
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    let events = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert_eq!(events.len(), 1);
    assert!(matches!(
        &events[0],
        AppEvent::OpenFullAccessConfirmation {
            preset,
            return_to_permissions: true,
            profile_selection: Some(PermissionProfileSelection {
                profile_id,
                approval_policy: Some(AskForApproval::Never),
                approvals_reviewer: Some(ApprovalsReviewer::User),
                display_label,
            }),
        } if preset.id == "full-access"
            && profile_id == ":danger-no-sandbox"
            && display_label == "Full Access"
    ));
}

#[cfg(target_os = "windows")]
#[tokio::test]
#[serial]
async fn approvals_selection_popup_snapshot_windows_degraded_sandbox() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.config.notices.hide_full_access_warning = None;
    chat.set_feature_enabled(Feature::WindowsSandbox, /*enabled*/ true);
    chat.set_feature_enabled(Feature::WindowsSandboxElevated, /*enabled*/ false);

    chat.open_approvals_popup();

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert!(
        popup.contains("Ask for approval (non-admin sandbox)"),
        "expected degraded sandbox label in approvals popup: {popup}"
    );
    assert!(
        popup.contains("/setup-default-sandbox"),
        "expected setup hint in approvals popup: {popup}"
    );
    assert!(
        popup.contains("non-admin sandbox"),
        "expected degraded sandbox note in approvals popup: {popup}"
    );
}

#[tokio::test]
async fn preset_matching_accepts_workspace_write_with_extra_roots() {
    let preset = builtin_approval_presets()
        .into_iter()
        .find(|p| p.id == "auto")
        .expect("auto preset exists");
    let current_profile = app_server_workspace_write_profile(test_path_buf("/tmp/extra").abs());
    let cwd = test_path_buf("/tmp/project").abs();

    assert!(
        ChatWidget::preset_matches_current(
            AskForApproval::OnRequest,
            &current_profile,
            cwd.as_path(),
            &preset
        ),
        "WorkspaceWrite with extra roots should still match the Ask for approval preset"
    );
    assert!(
        !ChatWidget::preset_matches_current(
            AskForApproval::Never,
            &current_profile,
            cwd.as_path(),
            &preset
        ),
        "approval mismatch should prevent matching the preset"
    );
}

#[tokio::test]
async fn preset_matching_does_not_treat_non_cwd_writable_profile_as_read_only() {
    let preset = builtin_approval_presets()
        .into_iter()
        .find(|p| p.id == "read-only")
        .expect("read-only preset exists");
    let current_profile: PermissionProfile = PermissionProfile::Managed {
        network: NetworkSandboxPolicy::Restricted,
        file_system: ManagedFileSystemPermissions::Restricted {
            entries: vec![
                FileSystemSandboxEntry {
                    path: FileSystemPath::Special {
                        value: FileSystemSpecialPath::Root,
                    },
                    access: FileSystemAccessMode::Read,
                },
                FileSystemSandboxEntry {
                    path: FileSystemPath::Path {
                        path: test_path_buf("/tmp/writable").abs(),
                    },
                    access: FileSystemAccessMode::Write,
                },
            ],
            glob_scan_max_depth: None,
        },
    };
    let cwd = test_path_buf("/tmp/project").abs();

    assert!(
        !ChatWidget::preset_matches_current(
            AskForApproval::OnRequest,
            &current_profile,
            cwd.as_path(),
            &preset
        ),
        "profiles with any writable root should not be classified as Read Only"
    );
}

#[tokio::test]
async fn full_access_confirmation_popup_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    let preset = builtin_approval_presets()
        .into_iter()
        .find(|preset| preset.id == "full-access")
        .expect("full access preset");
    chat.open_full_access_confirmation(
        preset, /*return_to_permissions*/ false, /*profile_selection*/ None,
    );

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert_chatwidget_snapshot!("full_access_confirmation_popup", popup);
}

#[cfg(target_os = "windows")]
#[tokio::test]
async fn windows_auto_mode_prompt_requests_enabling_sandbox_feature() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    let preset = builtin_approval_presets()
        .into_iter()
        .find(|preset| preset.id == "auto")
        .expect("auto preset");
    chat.open_windows_sandbox_enable_prompt(preset, /*profile_selection*/ None);

    let popup = render_bottom_popup(&chat, /*width*/ 120);
    assert!(
        popup.contains("requires Administrator permissions"),
        "expected auto mode prompt to mention Administrator permissions, popup: {popup}"
    );
    assert!(
        popup.contains("Use non-admin sandbox"),
        "expected auto mode prompt to include non-admin fallback option, popup: {popup}"
    );
}

#[cfg(target_os = "windows")]
#[tokio::test]
async fn startup_prompts_for_windows_sandbox_when_agent_requested() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.set_feature_enabled(Feature::WindowsSandbox, /*enabled*/ false);
    chat.set_feature_enabled(Feature::WindowsSandboxElevated, /*enabled*/ false);

    chat.maybe_prompt_windows_sandbox_enable(/*show_now*/ true);

    let popup = render_bottom_popup(&chat, /*width*/ 120);
    assert!(
        popup.contains("requires Administrator permissions"),
        "expected startup prompt to mention Administrator permissions: {popup}"
    );
    assert!(
        popup.contains("Set up default sandbox"),
        "expected startup prompt to offer default sandbox setup: {popup}"
    );
    assert!(
        popup.contains("Use non-admin sandbox"),
        "expected startup prompt to offer non-admin fallback: {popup}"
    );
    assert!(
        popup.contains("Quit"),
        "expected startup prompt to offer quit action: {popup}"
    );
}

#[cfg(target_os = "windows")]
#[tokio::test]
async fn startup_windows_sandbox_prompt_blocks_disallowed_unelevated_fallback() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.set_feature_enabled(Feature::WindowsSandbox, /*enabled*/ false);
    chat.set_feature_enabled(Feature::WindowsSandboxElevated, /*enabled*/ false);
    chat.config.config_layer_stack =
        windows_sandbox_requirements_stack(vec![WindowsSandboxModeToml::Elevated]);

    chat.maybe_prompt_windows_sandbox_enable(/*show_now*/ true);

    let popup = render_bottom_popup(&chat, /*width*/ 120);
    assert!(
        popup.contains("Your organization requires the default Codex agent sandbox"),
        "expected required sandbox prompt copy: {popup}"
    );
    assert!(
        !popup.contains("Use non-admin sandbox"),
        "expected required sandbox prompt to hide non-admin fallback: {popup}"
    );
}

#[tokio::test]
async fn windows_sandbox_required_enable_prompt_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.config.config_layer_stack =
        windows_sandbox_requirements_stack(vec![WindowsSandboxModeToml::Elevated]);
    let preset = builtin_approval_presets()
        .into_iter()
        .find(|preset| preset.id == "auto")
        .expect("auto preset");

    chat.open_windows_sandbox_enable_prompt(preset, /*profile_selection*/ None);

    assert_chatwidget_snapshot!(
        "windows_sandbox_required_enable_prompt",
        render_bottom_popup(&chat, /*width*/ 120)
    );
}

#[tokio::test]
async fn windows_sandbox_required_enable_prompt_reopens_on_cancel_when_unelevated_allowed() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.config.permissions.windows_sandbox_mode = Some(WindowsSandboxModeToml::Elevated);
    chat.config.config_layer_stack = windows_sandbox_requirements_stack(vec![
        WindowsSandboxModeToml::Elevated,
        WindowsSandboxModeToml::Unelevated,
    ]);
    let preset = builtin_approval_presets()
        .into_iter()
        .find(|preset| preset.id == "auto")
        .expect("auto preset");

    chat.open_windows_sandbox_enable_prompt(preset, /*profile_selection*/ None);
    chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

    assert!(matches!(
        rx.try_recv(),
        Ok(AppEvent::OpenWindowsSandboxEnablePrompt { .. })
    ));
}

#[tokio::test]
async fn required_windows_sandbox_setup_defers_configured_initial_prompt() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let initial_prompt = "fix required sandbox startup".to_string();

    chat.config.permissions.windows_sandbox_mode = Some(WindowsSandboxModeToml::Elevated);
    chat.config.config_layer_stack = windows_sandbox_requirements_stack(vec![
        WindowsSandboxModeToml::Elevated,
        WindowsSandboxModeToml::Unelevated,
    ]);
    chat.initial_user_message =
        create_initial_user_message(Some(initial_prompt.clone()), Vec::new(), Vec::new());

    chat.handle_thread_session(crate::session_state::ThreadSessionState {
        thread_id: ThreadId::new(),
        forked_from_id: None,
        fork_parent_title: None,
        thread_name: None,
        model: "gpt-test".to_string(),
        model_provider_id: "test-provider".to_string(),
        service_tier: None,
        approval_policy: AskForApproval::OnRequest,
        approvals_reviewer: ApprovalsReviewer::User,
        permission_profile: PermissionProfile::workspace_write(),
        active_permission_profile: None,
        cwd: test_project_path().abs(),
        runtime_workspace_roots: Vec::new(),
        instruction_source_paths: Vec::new(),
        reasoning_effort: None,
        collaboration_mode: None,
        personality: None,
        message_history: None,
        network_proxy: None,
        rollout_path: Some(PathBuf::new()),
    });
    drain_insert_history(&mut rx);

    assert!(chat.initial_user_message.is_some());
    while let Ok(op) = op_rx.try_recv() {
        assert!(
            !matches!(op, Op::UserTurn { .. }),
            "required sandbox setup should hold the configured initial prompt"
        );
    }

    chat.set_windows_sandbox_mode(Some(WindowsSandboxModeToml::Unelevated));
    chat.submit_initial_user_message_if_pending();

    let Op::UserTurn { items, .. } = next_submit_op(&mut op_rx) else {
        panic!("expected initial prompt submission after setup is no longer required");
    };
    assert_eq!(
        items,
        vec![UserInput::Text {
            text: initial_prompt,
            text_elements: Vec::new(),
        }]
    );
}

#[tokio::test]
async fn windows_sandbox_required_fallback_prompt_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.config.config_layer_stack =
        windows_sandbox_requirements_stack(vec![WindowsSandboxModeToml::Elevated]);
    let preset = builtin_approval_presets()
        .into_iter()
        .find(|preset| preset.id == "auto")
        .expect("auto preset");

    chat.open_windows_sandbox_fallback_prompt(preset, /*profile_selection*/ None);

    let popup = render_bottom_popup(&chat, /*width*/ 120);
    assert_chatwidget_snapshot!("windows_sandbox_required_fallback_prompt", popup);
}

#[cfg(target_os = "windows")]
#[tokio::test]
async fn startup_does_not_prompt_for_windows_sandbox_when_not_requested() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.set_feature_enabled(Feature::WindowsSandbox, /*enabled*/ false);
    chat.set_feature_enabled(Feature::WindowsSandboxElevated, /*enabled*/ false);
    chat.maybe_prompt_windows_sandbox_enable(/*show_now*/ false);

    assert!(
        chat.bottom_pane.no_modal_or_popup_active(),
        "expected no startup sandbox NUX popup when startup trigger is false"
    );
}

#[tokio::test]
async fn approvals_popup_shows_disabled_presets() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.config.permissions.approval_policy = Constrained::new(
        AskForApproval::OnRequest.to_core(),
        |candidate| match AskForApproval::from(*candidate) {
            AskForApproval::OnRequest => Ok(()),
            _ => Err(invalid_value(
                candidate.to_string(),
                "this message should be printed in the description",
            )),
        },
    )
    .expect("construct constrained approval policy");
    chat.open_approvals_popup();

    let width = 80;
    let height = chat.desired_height(width);
    let mut terminal =
        ratatui::Terminal::new(VT100Backend::new(width, height)).expect("create terminal");
    terminal.set_viewport_area(Rect::new(0, 0, width, height));
    terminal
        .draw(|f| chat.render(f.area(), f.buffer_mut()))
        .expect("render approvals popup");

    let screen = terminal.backend().vt100().screen().contents();
    let collapsed = screen.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        collapsed.contains("(disabled)"),
        "disabled preset label should be shown"
    );
    assert!(
        collapsed.contains("this message should be printed in the description"),
        "disabled preset reason should be shown"
    );
}

#[tokio::test]
async fn approvals_popup_navigation_skips_disabled() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::GuardianApproval, /*enabled*/ false);

    chat.config.permissions.approval_policy = Constrained::new(
        AskForApproval::OnRequest.to_core(),
        |candidate| match AskForApproval::from(*candidate) {
            AskForApproval::OnRequest => Ok(()),
            _ => Err(invalid_value(candidate.to_string(), "[on-request]")),
        },
    )
    .expect("construct constrained approval policy");
    chat.open_approvals_popup();

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    let mut disabled_shortcut = None;
    let mut row_number = 0;
    for line in popup.lines() {
        let row = line
            .trim_start()
            .strip_prefix('\u{203a}')
            .unwrap_or_else(|| line.trim_start())
            .trim_start();
        let mut chars = row.chars();
        let has_numeric_shortcut =
            chars.next().is_some_and(|ch| ch.is_ascii_digit()) && chars.next() == Some('.');
        if has_numeric_shortcut || row.contains("(disabled)") {
            row_number += 1;
            if row.contains("(disabled)") {
                disabled_shortcut = char::from_digit(row_number, 10);
                break;
            }
        }
    }
    let disabled_shortcut = disabled_shortcut
        .unwrap_or_else(|| panic!("expected at least one disabled selection row: {popup}"));

    for _ in 0..10 {
        chat.handle_key_event(KeyEvent::from(KeyCode::Down));
        let popup = render_bottom_popup(&chat, /*width*/ 80);
        let selected_disabled = popup
            .lines()
            .find(|line| line.trim_start().starts_with('\u{203a}'))
            .expect("expected a selected selection row")
            .contains("(disabled)");
        assert!(
            !selected_disabled,
            "navigation should skip disabled rows: {popup}"
        );
    }

    // Press the hidden numeric shortcut for a disabled row; it should not close
    // the popup or accept the preset.
    chat.handle_key_event(KeyEvent::from(KeyCode::Char(disabled_shortcut)));

    // Ensure the popup remains open and no selection actions were sent.
    let screen = render_bottom_popup(&chat, /*width*/ 80);
    assert!(
        screen.contains("Update Model Permissions"),
        "popup should remain open after selecting a disabled entry"
    );
    assert!(
        op_rx.try_recv().is_err(),
        "no actions should be dispatched yet"
    );
    assert!(rx.try_recv().is_err(), "no history should be emitted");

    // Press Enter; selection should land on an enabled preset and dispatch updates.
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    let mut app_events = Vec::new();
    while let Ok(ev) = rx.try_recv() {
        app_events.push(ev);
    }
    assert!(
        app_events.iter().any(|ev| matches!(
            ev,
            AppEvent::CodexOp(Op::OverrideTurnContext {
                approval_policy: Some(AskForApproval::OnRequest),
                personality: None,
                ..
            })
        )),
        "enter should select an enabled preset"
    );
    assert!(
        !app_events.iter().any(|ev| matches!(
            ev,
            AppEvent::CodexOp(Op::OverrideTurnContext {
                approval_policy: Some(AskForApproval::Never),
                personality: None,
                ..
            })
        )),
        "disabled preset should not be selected"
    );
}

#[tokio::test]
async fn permissions_selection_emits_history_cell_when_selection_changes() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    #[cfg(target_os = "windows")]
    {
        chat.config.notices.hide_world_writable_warning = Some(true);
        chat.set_windows_sandbox_mode(Some(WindowsSandboxModeToml::Unelevated));
    }
    chat.config.notices.hide_full_access_warning = Some(true);

    chat.open_permissions_popup();
    chat.handle_key_event(KeyEvent::from(KeyCode::Down));
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    let cells = drain_insert_history(&mut rx);
    assert_eq!(
        cells.len(),
        1,
        "expected one permissions selection history cell"
    );
    let rendered = lines_to_single_string(&cells[0]);
    assert!(
        rendered.contains("Permissions updated to"),
        "expected permissions selection history message, got: {rendered}"
    );
}

#[tokio::test]
async fn permissions_selection_history_snapshot_after_mode_switch() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    #[cfg(target_os = "windows")]
    {
        chat.config.notices.hide_world_writable_warning = Some(true);
        chat.set_windows_sandbox_mode(Some(WindowsSandboxModeToml::Unelevated));
    }
    chat.set_feature_enabled(Feature::GuardianApproval, /*enabled*/ false);
    chat.config.notices.hide_full_access_warning = Some(true);

    chat.open_permissions_popup();
    chat.handle_key_event(KeyEvent::from(KeyCode::Down));
    #[cfg(target_os = "windows")]
    chat.handle_key_event(KeyEvent::from(KeyCode::Down));
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 1, "expected one mode-switch history cell");
    assert_chatwidget_snapshot!(
        "permissions_selection_history_after_mode_switch",
        lines_to_single_string(&cells[0])
    );
}

#[tokio::test]
async fn permissions_selection_history_snapshot_full_access_to_default() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    #[cfg(target_os = "windows")]
    {
        chat.config.notices.hide_world_writable_warning = Some(true);
        chat.set_windows_sandbox_mode(Some(WindowsSandboxModeToml::Unelevated));
    }
    chat.config.notices.hide_full_access_warning = Some(true);
    chat.config
        .permissions
        .approval_policy
        .set(AskForApproval::Never.to_core())
        .expect("set approval policy");
    chat.config
        .permissions
        .set_permission_profile(PermissionProfile::Disabled)
        .expect("set permission profile");

    chat.open_permissions_popup();
    let popup = render_bottom_popup(&chat, /*width*/ 120);
    chat.handle_key_event(KeyEvent::from(KeyCode::Up));
    if popup.contains("Approve for me") {
        chat.handle_key_event(KeyEvent::from(KeyCode::Up));
    }
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 1, "expected one mode-switch history cell");
    #[cfg(target_os = "windows")]
    insta::with_settings!({ snapshot_suffix => "windows" }, {
        assert_chatwidget_snapshot!(
            "permissions_selection_history_full_access_to_default",
            lines_to_single_string(&cells[0])
        );
    });
    #[cfg(not(target_os = "windows"))]
    assert_chatwidget_snapshot!(
        "permissions_selection_history_full_access_to_default",
        lines_to_single_string(&cells[0])
    );
}

#[tokio::test]
async fn permissions_selection_emits_history_cell_when_current_is_selected() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    #[cfg(target_os = "windows")]
    {
        chat.config.notices.hide_world_writable_warning = Some(true);
        chat.set_windows_sandbox_mode(Some(WindowsSandboxModeToml::Unelevated));
    }
    chat.config
        .permissions
        .approval_policy
        .set(AskForApproval::OnRequest.to_core())
        .expect("set approval policy");
    chat.config
        .permissions
        .set_permission_profile(PermissionProfile::workspace_write())
        .expect("set permission profile");

    chat.open_permissions_popup();
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    let cells = drain_insert_history(&mut rx);
    assert_eq!(
        cells.len(),
        1,
        "expected history cell even when selecting current permissions"
    );
    let rendered = lines_to_single_string(&cells[0]);
    assert!(
        rendered.contains("Permissions updated to"),
        "expected permissions update history message, got: {rendered}"
    );
}

#[tokio::test]
async fn permissions_selection_hides_auto_review_when_feature_disabled() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    #[cfg(target_os = "windows")]
    {
        chat.config.notices.hide_world_writable_warning = Some(true);
        chat.set_windows_sandbox_mode(Some(WindowsSandboxModeToml::Unelevated));
    }
    chat.set_feature_enabled(Feature::GuardianApproval, /*enabled*/ false);
    chat.config.notices.hide_full_access_warning = Some(true);

    chat.open_permissions_popup();
    let popup = render_bottom_popup(&chat, /*width*/ 120);

    assert!(
        !popup.contains("Approve for me"),
        "expected Approve for me to stay hidden until the feature is enabled: {popup}"
    );
}

#[tokio::test]
async fn permissions_selection_hides_auto_review_when_feature_disabled_even_if_auto_review_is_active()
 {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    #[cfg(target_os = "windows")]
    {
        chat.config.notices.hide_world_writable_warning = Some(true);
        chat.set_windows_sandbox_mode(Some(WindowsSandboxModeToml::Unelevated));
    }
    chat.set_feature_enabled(Feature::GuardianApproval, /*enabled*/ false);
    chat.config.notices.hide_full_access_warning = Some(true);
    chat.config.approvals_reviewer = ApprovalsReviewer::AutoReview;
    chat.config
        .permissions
        .approval_policy
        .set(AskForApproval::OnRequest.to_core())
        .expect("set approval policy");
    chat.config
        .permissions
        .set_permission_profile(PermissionProfile::workspace_write())
        .expect("set permission profile");

    chat.open_permissions_popup();
    let popup = render_bottom_popup(&chat, /*width*/ 120);

    assert!(
        !popup.contains("Approve for me"),
        "expected Approve for me to stay hidden when the feature is disabled: {popup}"
    );
}

#[tokio::test]
async fn permissions_selection_marks_auto_review_current_after_session_configured() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    #[cfg(target_os = "windows")]
    {
        chat.config.notices.hide_world_writable_warning = Some(true);
        chat.set_windows_sandbox_mode(Some(WindowsSandboxModeToml::Unelevated));
    }
    chat.config.notices.hide_full_access_warning = Some(true);
    let _ = chat
        .config
        .features
        .set_enabled(Feature::GuardianApproval, /*enabled*/ true);

    chat.handle_thread_session(crate::session_state::ThreadSessionState {
        thread_id: ThreadId::new(),
        forked_from_id: None,
        fork_parent_title: None,
        thread_name: None,
        model: "gpt-test".to_string(),
        model_provider_id: "test-provider".to_string(),
        service_tier: None,
        approval_policy: AskForApproval::OnRequest,
        approvals_reviewer: ApprovalsReviewer::AutoReview,
        permission_profile: PermissionProfile::workspace_write(),
        active_permission_profile: None,
        cwd: test_project_path().abs(),
        runtime_workspace_roots: Vec::new(),
        instruction_source_paths: Vec::new(),
        reasoning_effort: None,
        collaboration_mode: None,
        personality: None,
        message_history: None,
        network_proxy: None,
        rollout_path: Some(PathBuf::new()),
    });

    chat.open_permissions_popup();
    let popup = render_bottom_popup(&chat, /*width*/ 120);

    assert!(
        popup.contains("Approve for me (current)"),
        "expected Approve for me to be current after SessionConfigured sync: {popup}"
    );
}

#[tokio::test]
async fn permissions_selection_marks_auto_review_current_with_custom_workspace_write_details() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    #[cfg(target_os = "windows")]
    {
        chat.config.notices.hide_world_writable_warning = Some(true);
        chat.set_windows_sandbox_mode(Some(WindowsSandboxModeToml::Unelevated));
    }
    chat.config.notices.hide_full_access_warning = Some(true);
    let _ = chat
        .config
        .features
        .set_enabled(Feature::GuardianApproval, /*enabled*/ true);

    let extra_root = test_path_buf("/tmp/guardian-approvals-extra").abs();
    let cwd = test_project_path().abs();
    let permission_profile = app_server_workspace_write_profile(extra_root);

    chat.handle_thread_session(crate::session_state::ThreadSessionState {
        thread_id: ThreadId::new(),
        forked_from_id: None,
        fork_parent_title: None,
        thread_name: None,
        model: "gpt-test".to_string(),
        model_provider_id: "test-provider".to_string(),
        service_tier: None,
        approval_policy: AskForApproval::OnRequest,
        approvals_reviewer: ApprovalsReviewer::AutoReview,
        permission_profile,
        active_permission_profile: None,
        cwd,
        runtime_workspace_roots: Vec::new(),
        instruction_source_paths: Vec::new(),
        reasoning_effort: None,
        collaboration_mode: None,
        personality: None,
        message_history: None,
        network_proxy: None,
        rollout_path: Some(PathBuf::new()),
    });

    chat.open_permissions_popup();
    let popup = render_bottom_popup(&chat, /*width*/ 120);

    assert!(
        popup.contains("Approve for me (current)"),
        "expected Approve for me to be current even with custom workspace-write details: {popup}"
    );
}

#[tokio::test]
async fn permissions_selection_can_disable_auto_review() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    #[cfg(target_os = "windows")]
    {
        chat.config.notices.hide_world_writable_warning = Some(true);
        chat.set_windows_sandbox_mode(Some(WindowsSandboxModeToml::Unelevated));
    }
    chat.config.notices.hide_full_access_warning = Some(true);
    chat.set_feature_enabled(Feature::GuardianApproval, /*enabled*/ true);
    chat.config
        .permissions
        .approval_policy
        .set(AskForApproval::OnRequest.to_core())
        .expect("set approval policy");
    chat.config
        .permissions
        .set_permission_profile(PermissionProfile::workspace_write())
        .expect("set permission profile");

    chat.open_permissions_popup();
    chat.handle_key_event(KeyEvent::from(KeyCode::Up));
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    let events = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(
        events.iter().any(|event| matches!(
            event,
            AppEvent::UpdateApprovalsReviewer(ApprovalsReviewer::User)
        )),
        "expected selecting Ask for approval from Approve for me to switch back to manual approval review: {events:?}"
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, AppEvent::UpdateFeatureFlags { .. })),
        "expected permissions selection to leave feature flags unchanged: {events:?}"
    );
}

#[tokio::test]
async fn permissions_selection_sends_approvals_reviewer_in_override_turn_context() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    #[cfg(target_os = "windows")]
    {
        chat.config.notices.hide_world_writable_warning = Some(true);
        chat.set_windows_sandbox_mode(Some(WindowsSandboxModeToml::Unelevated));
    }
    chat.config.notices.hide_full_access_warning = Some(true);
    chat.set_feature_enabled(Feature::GuardianApproval, /*enabled*/ true);
    chat.config
        .permissions
        .approval_policy
        .set(AskForApproval::OnRequest.to_core())
        .expect("set approval policy");
    chat.config
        .permissions
        .set_permission_profile(PermissionProfile::workspace_write())
        .expect("set permission profile");
    chat.set_approvals_reviewer(ApprovalsReviewer::User);

    chat.open_permissions_popup();
    let popup = render_bottom_popup(&chat, /*width*/ 120);
    assert!(
        popup
            .lines()
            .any(|line| line.contains("(current)") && line.contains('›')),
        "expected permissions popup to open with the current preset selected: {popup}"
    );

    chat.handle_key_event(KeyEvent::from(KeyCode::Down));
    let popup = render_bottom_popup(&chat, /*width*/ 120);
    assert!(
        popup
            .lines()
            .any(|line| line.contains("Approve for me") && line.contains('›')),
        "expected one Down from Ask for approval to select Approve for me: {popup}"
    );
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    let op = std::iter::from_fn(|| rx.try_recv().ok())
        .find_map(|event| match event {
            AppEvent::CodexOp(op @ Op::OverrideTurnContext { .. }) => Some(op),
            _ => None,
        })
        .expect("expected OverrideTurnContext op");

    assert_eq!(
        op,
        Op::OverrideTurnContext {
            cwd: None,
            approval_policy: Some(AskForApproval::OnRequest),
            approvals_reviewer: Some(ApprovalsReviewer::AutoReview),
            permission_profile: Some(PermissionProfile::workspace_write()),
            active_permission_profile: Some(ActivePermissionProfile::new(
                BUILT_IN_PERMISSION_PROFILE_WORKSPACE,
            )),
            windows_sandbox_level: None,
            model: None,
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        }
    );

    let active_permission_profile_update = std::iter::from_fn(|| rx.try_recv().ok())
        .find_map(|event| match event {
            AppEvent::UpdateActivePermissionProfile(active_permission_profile) => {
                Some(active_permission_profile)
            }
            _ => None,
        })
        .expect("expected UpdateActivePermissionProfile event");

    assert_eq!(
        active_permission_profile_update,
        ActivePermissionProfile::new(BUILT_IN_PERMISSION_PROFILE_WORKSPACE)
    );
}

#[tokio::test]
async fn permissions_full_access_history_cell_emitted_only_after_confirmation() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    #[cfg(target_os = "windows")]
    {
        chat.config.notices.hide_world_writable_warning = Some(true);
        chat.set_windows_sandbox_mode(Some(WindowsSandboxModeToml::Unelevated));
    }
    chat.set_feature_enabled(Feature::GuardianApproval, /*enabled*/ false);
    chat.config.notices.hide_full_access_warning = None;

    chat.open_permissions_popup();
    chat.handle_key_event(KeyEvent::from(KeyCode::Down));
    #[cfg(target_os = "windows")]
    chat.handle_key_event(KeyEvent::from(KeyCode::Down));
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    let mut open_confirmation_event = None;
    let mut cells_before_confirmation = Vec::new();
    while let Ok(event) = rx.try_recv() {
        match event {
            AppEvent::InsertHistoryCell(cell) => {
                cells_before_confirmation.push(cell.display_lines(/*width*/ 80));
            }
            AppEvent::OpenFullAccessConfirmation {
                preset,
                return_to_permissions,
                profile_selection,
            } => {
                open_confirmation_event = Some((preset, return_to_permissions, profile_selection));
            }
            _ => {}
        }
    }
    if cfg!(not(target_os = "windows")) {
        assert!(
            cells_before_confirmation.is_empty(),
            "did not expect history cell before confirming full access"
        );
    }
    let (preset, return_to_permissions, profile_selection) =
        open_confirmation_event.expect("expected full access confirmation event");
    chat.open_full_access_confirmation(preset, return_to_permissions, profile_selection);

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert!(
        popup.contains("Enable full access?"),
        "expected full access confirmation popup, got: {popup}"
    );

    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    let cells_after_confirmation = drain_insert_history(&mut rx);
    let total_history_cells = cells_before_confirmation.len() + cells_after_confirmation.len();
    assert_eq!(
        total_history_cells, 1,
        "expected one full access history cell total"
    );
    let rendered = if !cells_before_confirmation.is_empty() {
        lines_to_single_string(&cells_before_confirmation[0])
    } else {
        lines_to_single_string(&cells_after_confirmation[0])
    };
    assert!(
        rendered.contains("Permissions updated to Full Access"),
        "expected full access update history message, got: {rendered}"
    );
}
