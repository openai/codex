use super::*;
use pretty_assertions::assert_eq;

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
        popup.contains("Default (non-admin sandbox)"),
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
    let current_sandbox = SandboxPolicy::WorkspaceWrite {
        writable_roots: vec![PathBuf::from("C:\\extra").abs()],
        read_only_access: Default::default(),
        network_access: false,
        exclude_tmpdir_env_var: false,
        exclude_slash_tmp: false,
    };

    assert!(
        ChatWidget::preset_matches_current(AskForApproval::OnRequest, &current_sandbox, &preset),
        "WorkspaceWrite with extra roots should still match the Default preset"
    );
    assert!(
        !ChatWidget::preset_matches_current(AskForApproval::Never, &current_sandbox, &preset),
        "approval mismatch should prevent matching the preset"
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
        preset, /*return_to_permissions*/ false, /*permission_preset_context*/ None,
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
    chat.open_windows_sandbox_enable_prompt(preset);

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

    chat.config.permissions.approval_policy =
        Constrained::new(AskForApproval::OnRequest, |candidate| match candidate {
            AskForApproval::OnRequest => Ok(()),
            _ => Err(invalid_value(
                candidate.to_string(),
                "this message should be printed in the description",
            )),
        })
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

    chat.config.permissions.approval_policy =
        Constrained::new(AskForApproval::OnRequest, |candidate| match candidate {
            AskForApproval::OnRequest => Ok(()),
            _ => Err(invalid_value(candidate.to_string(), "[on-request]")),
        })
        .expect("construct constrained approval policy");
    chat.open_approvals_popup();

    // The approvals popup is the active bottom-pane view; drive navigation via chat handle_key_event.
    // Start selected at idx 0 (enabled), move down twice; the disabled option should be skipped
    // and selection should wrap back to idx 0 (also enabled).
    chat.handle_key_event(KeyEvent::from(KeyCode::Down));
    chat.handle_key_event(KeyEvent::from(KeyCode::Down));

    // Press numeric shortcut for the disabled row (3 => idx 2); should not close or accept.
    chat.handle_key_event(KeyEvent::from(KeyCode::Char('3')));

    // Ensure the popup remains open and no selection actions were sent.
    let width = 80;
    let height = chat.desired_height(width);
    let mut terminal =
        ratatui::Terminal::new(VT100Backend::new(width, height)).expect("create terminal");
    terminal.set_viewport_area(Rect::new(0, 0, width, height));
    terminal
        .draw(|f| chat.render(f.area(), f.buffer_mut()))
        .expect("render approvals popup after disabled selection");
    let screen = terminal.backend().vt100().screen().contents();
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
        .set(AskForApproval::Never)
        .expect("set approval policy");
    chat.config.permissions.sandbox_policy =
        Constrained::allow_any(SandboxPolicy::DangerFullAccess);

    chat.open_permissions_popup();
    let popup = render_bottom_popup(&chat, /*width*/ 120);
    chat.handle_key_event(KeyEvent::from(KeyCode::Up));
    if popup.contains("Guardian Approvals") {
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
        .set(AskForApproval::OnRequest)
        .expect("set approval policy");
    chat.config
        .permissions
        .sandbox_policy
        .set(SandboxPolicy::new_workspace_write_policy())
        .expect("set sandbox policy");

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
async fn permissions_selection_hides_guardian_approvals_when_feature_disabled() {
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
        !popup.contains("Guardian Approvals"),
        "expected Guardian Approvals to stay hidden until the experimental feature is enabled: {popup}"
    );
}

#[tokio::test]
async fn permissions_selection_hides_guardian_approvals_when_feature_disabled_even_if_auto_review_is_active()
 {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    #[cfg(target_os = "windows")]
    {
        chat.config.notices.hide_world_writable_warning = Some(true);
        chat.set_windows_sandbox_mode(Some(WindowsSandboxModeToml::Unelevated));
    }
    chat.set_feature_enabled(Feature::GuardianApproval, /*enabled*/ false);
    chat.config.notices.hide_full_access_warning = Some(true);
    chat.config.approvals_reviewer = ApprovalsReviewer::GuardianSubagent;
    chat.config
        .permissions
        .approval_policy
        .set(AskForApproval::OnRequest)
        .expect("set approval policy");
    chat.config
        .permissions
        .sandbox_policy
        .set(SandboxPolicy::new_workspace_write_policy())
        .expect("set sandbox policy");

    chat.open_permissions_popup();
    let popup = render_bottom_popup(&chat, /*width*/ 120);

    assert!(
        !popup.contains("Guardian Approvals"),
        "expected Guardian Approvals to stay hidden when the experimental feature is disabled: {popup}"
    );
}

#[tokio::test]
async fn permissions_selection_marks_guardian_approvals_current_after_session_configured() {
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

    chat.handle_codex_event(Event {
        id: "session-configured".to_string(),
        msg: EventMsg::SessionConfigured(SessionConfiguredEvent {
            session_id: ThreadId::new(),
            forked_from_id: None,
            thread_name: None,
            model: "gpt-test".to_string(),
            model_provider_id: "test-provider".to_string(),
            service_tier: None,
            approval_policy: AskForApproval::OnRequest,
            approvals_reviewer: ApprovalsReviewer::GuardianSubagent,
            sandbox_policy: SandboxPolicy::new_workspace_write_policy(),
            cwd: test_project_path(),
            reasoning_effort: None,
            history_log_id: 0,
            history_entry_count: 0,
            initial_messages: None,
            network_proxy: None,
            rollout_path: Some(PathBuf::new()),
        }),
    });

    chat.open_permissions_popup();
    let popup = render_bottom_popup(&chat, /*width*/ 120);

    assert!(
        popup.contains("Guardian Approvals (current)"),
        "expected Guardian Approvals to be current after SessionConfigured sync: {popup}"
    );
}

#[tokio::test]
async fn permissions_selection_marks_guardian_approvals_current_with_custom_workspace_write_details()
 {
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

    let extra_root = PathBuf::from("/tmp/guardian-approvals-extra").abs();

    chat.handle_codex_event(Event {
        id: "session-configured-custom-workspace".to_string(),
        msg: EventMsg::SessionConfigured(SessionConfiguredEvent {
            session_id: ThreadId::new(),
            forked_from_id: None,
            thread_name: None,
            model: "gpt-test".to_string(),
            model_provider_id: "test-provider".to_string(),
            service_tier: None,
            approval_policy: AskForApproval::OnRequest,
            approvals_reviewer: ApprovalsReviewer::GuardianSubagent,
            sandbox_policy: SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![extra_root],
                read_only_access: ReadOnlyAccess::FullAccess,
                network_access: false,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: false,
            },
            cwd: test_project_path(),
            reasoning_effort: None,
            history_log_id: 0,
            history_entry_count: 0,
            initial_messages: None,
            network_proxy: None,
            rollout_path: Some(PathBuf::new()),
        }),
    });

    chat.open_permissions_popup();
    let popup = render_bottom_popup(&chat, /*width*/ 120);

    assert!(
        popup.contains("Guardian Approvals (current)"),
        "expected Guardian Approvals to be current even with custom workspace-write details: {popup}"
    );
}

#[tokio::test]
async fn permissions_selection_can_disable_guardian_approvals() {
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
        .set(AskForApproval::OnRequest)
        .expect("set approval policy");
    chat.config
        .permissions
        .sandbox_policy
        .set(SandboxPolicy::new_workspace_write_policy())
        .expect("set sandbox policy");

    chat.open_permissions_popup();
    chat.handle_key_event(KeyEvent::from(KeyCode::Up));
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    let events = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(
        events.iter().any(|event| matches!(
            event,
            AppEvent::UpdateApprovalsReviewer(ApprovalsReviewer::User)
        )),
        "expected selecting Default from Guardian Approvals to switch back to manual approval review: {events:?}"
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
        .set(AskForApproval::OnRequest)
        .expect("set approval policy");
    chat.config
        .permissions
        .sandbox_policy
        .set(SandboxPolicy::new_workspace_write_policy())
        .expect("set sandbox policy");
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
            .any(|line| line.contains("Guardian Approvals") && line.contains('›')),
        "expected one Down from Default to select Guardian Approvals: {popup}"
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
            approvals_reviewer: Some(ApprovalsReviewer::GuardianSubagent),
            sandbox_policy: Some(SandboxPolicy::new_workspace_write_policy()),
            windows_sandbox_level: None,
            model: None,
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        }
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
                permission_preset_context,
            } => {
                open_confirmation_event =
                    Some((preset, return_to_permissions, permission_preset_context));
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
    let (preset, return_to_permissions, permission_preset_context) =
        open_confirmation_event.expect("expected full access confirmation event");
    chat.open_full_access_confirmation(preset, return_to_permissions, permission_preset_context);

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
