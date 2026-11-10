//! Preset HUD, editor, and save/delete flows preserve drafts and explicit confirmation.

use super::*;
use crate::chatwidget::quick_dials::QuickDialEditorTarget;
use codex_config::types::QuickDial;

#[tokio::test]
async fn current_settings_offer_model_permissions_and_status_layouts() {
    let (mut chat, mut events, _ops) =
        make_chatwidget_manual(/*model_override*/ Some("gpt-5.5")).await;
    chat.thread_id = Some(ThreadId::new());
    chat.local_settings.tui.terminal_title = Some(vec![
        "model-name".into(),
        "project".into(),
        "model-name".into(),
    ]);
    chat.local_settings.tui.status_line = Some(vec![
        "model-name".into(),
        "project-root".into(),
        "model-name".into(),
    ]);
    chat.local_settings.tui.status_line_use_colors = false;
    chat.set_permission_profile_from_session_snapshot(PermissionProfileSnapshot::active(
        PermissionProfile::workspace_write(),
        ActivePermissionProfile::new(":workspace"),
    ))
    .unwrap();

    chat.open_quick_dial_prefills();

    assert_chatwidget_snapshot!(
        "saved_quick_dials_source",
        render_bottom_popup(&chat, /*width*/ 80)
    );
    chat.handle_key_event(KeyCode::Down.into());
    chat.handle_key_event(KeyCode::Enter.into());
    assert_matches!(
        events.try_recv(),
        Ok(AppEvent::OpenQuickDialEditor {
            target: QuickDialEditorTarget::PickSlot,
            dial: Some(QuickDial { input, label: None }),
        }) if input == "/permissions :workspace --approval-policy=on-request --reviewer=user"
    );

    chat.open_quick_dial_prefills();
    chat.handle_key_event(KeyCode::Down.into());
    chat.handle_key_event(KeyCode::Down.into());
    chat.handle_key_event(KeyCode::Enter.into());
    assert_matches!(
        events.try_recv(),
        Ok(AppEvent::OpenQuickDialEditor {
            target: QuickDialEditorTarget::PickSlot,
            dial: Some(QuickDial { input, label: None }),
        }) if input == "/title model project-name model"
    );

    chat.open_quick_dial_prefills();
    chat.handle_key_event(KeyCode::Down.into());
    chat.handle_key_event(KeyCode::Down.into());
    chat.handle_key_event(KeyCode::Down.into());
    chat.handle_key_event(KeyCode::Enter.into());
    assert_matches!(
        events.try_recv(),
        Ok(AppEvent::OpenQuickDialEditor {
            target: QuickDialEditorTarget::PickSlot,
            dial: Some(QuickDial { input, label: None }),
        }) if input == "/statusline model project-name model --no-colors"
    );

    chat.local_settings.tui.terminal_title = Some(Vec::new());
    chat.local_settings.tui.status_line = Some(Vec::new());
    chat.local_settings.tui.status_line_use_colors = true;
    assert_eq!(
        chat.current_terminal_title_quick_dial(),
        Ok(QuickDial {
            input: "/title --clear".into(),
            label: None,
        })
    );
    assert_eq!(
        chat.current_status_line_quick_dial(),
        Ok(QuickDial {
            input: "/statusline --clear --colors".into(),
            label: None,
        })
    );

    chat.set_reasoning_effort(Some(ReasoningEffortConfig::Custom("removed-effort".into())));
    chat.terminal_title_invalid_items_warned
        .store(true, std::sync::atomic::Ordering::Relaxed);
    chat.local_settings.tui.terminal_title = Some(vec!["removed-title-item".into()]);
    chat.local_settings.tui.status_line = Some(vec!["model".into()]);
    chat.config.approvals_reviewer = ApprovalsReviewer::AutoReview;
    chat.open_quick_dial_prefills();
    assert_chatwidget_snapshot!(
        "saved_quick_dials_source_unavailable_model",
        render_bottom_popup(&chat, /*width*/ 80)
    );
    chat.handle_key_event(KeyCode::Enter.into());
    assert_matches!(
        events.try_recv(),
        Ok(AppEvent::OpenQuickDialEditor {
            target: QuickDialEditorTarget::PickSlot,
            dial: Some(QuickDial { input, label: None }),
        }) if input == "/permissions :workspace --approval-policy=on-request --reviewer=auto_review"
    );

    let granular = AskForApproval::Granular {
        sandbox_approval: false,
        rules: true,
        skill_approval: false,
        request_permissions: true,
        mcp_elicitations: false,
    };
    chat.config
        .permissions
        .approval_policy
        .set(granular.to_core())
        .expect("set granular approval policy");
    chat.config.approvals_reviewer = ApprovalsReviewer::User;
    chat.set_permission_profile_from_session_snapshot(PermissionProfileSnapshot::active(
        PermissionProfile::workspace_write(),
        ActivePermissionProfile::new("team review"),
    ))
    .expect("set named permission profile");
    chat.open_quick_dial_prefills();
    chat.handle_key_event(KeyCode::Enter.into());
    let policy = serde_json::to_string(&granular).expect("granular policy serializes");
    assert_matches!(
        events.try_recv(),
        Ok(AppEvent::OpenQuickDialEditor {
            target: QuickDialEditorTarget::PickSlot,
            dial: Some(QuickDial { input, label: None }),
        }) if input == format!(
            "/permissions 'team review' --approval-policy='{policy}' --reviewer=user"
        )
    );
}

#[tokio::test]
async fn saved_quick_dials_save_and_clear_picker_snapshots() {
    const DRAFT: &str = "Review the current changes\nfor safety  ";
    let (mut chat, mut events, _ops) =
        make_chatwidget_manual(/*model_override*/ Some("gpt-5.5")).await;
    chat.thread_id = Some(ThreadId::new());
    chat.set_status_line(Some("GPT-5.5 default · /tmp/project".into()));
    chat.open_quick_dial_menu();
    assert_chatwidget_snapshot!(
        "quick_dial_shortcuts_empty",
        render_bottom_popup(&chat, /*width*/ 100)
    );
    let escape = KeyCode::Esc.into();
    assert!(chat.handle_quick_dial_menu(escape));
    assert!(chat.handle_quick_dial_menu(escape));
    assert!(chat.handle_quick_dial_menu(KeyEvent::new_with_kind(
        KeyCode::Esc,
        KeyModifiers::NONE,
        crossterm::event::KeyEventKind::Release,
    )));
    chat.local_settings.tui.quick_dials = [(
        "1".into(),
        QuickDial {
            input: "/model gpt-5.5 --reasoning=high --speed=fast".into(),
            label: Some("coding".into()),
        },
    )]
    .into();
    chat.bottom_pane.apply_external_edit(DRAFT.into());
    chat.dispatch_command(SlashCommand::Presets);
    let hud = render_bottom_popup(&chat, /*width*/ 80);
    chat.handle_key_event(KeyCode::Enter.into());
    assert_eq!(render_bottom_popup(&chat, /*width*/ 80), hud);
    assert_eq!(chat.bottom_pane.composer_text(), DRAFT);
    assert!(chat.queued_user_message_texts().is_empty());
    chat.cancel_quick_dial_menu();
    chat.open_quick_dial_menu();
    assert_eq!(render_bottom_popup(&chat, /*width*/ 80), hud);
    chat.handle_key_event(KeyCode::Esc.into());
    while events.try_recv().is_ok() {}
    chat.open_quick_dial_editor(QuickDialEditorTarget::PickSlot, /*dial*/ None);
    let editor = render_bottom_popup(&chat, /*width*/ 80);
    assert_chatwidget_snapshot!("saved_quick_dials_editor", editor.clone());
    chat.handle_key_event(KeyCode::Enter.into());
    let edited = std::iter::from_fn(|| events.try_recv().ok())
        .find_map(|event| match event {
            AppEvent::OpenQuickDialSavePicker(dial) => Some(dial),
            _ => None,
        })
        .expect("editor submits a preset draft");
    assert_eq!(
        edited,
        QuickDial {
            input: DRAFT.into(),
            label: None,
        }
    );
    chat.open_quick_dial_save_picker(edited);
    assert_chatwidget_snapshot!(
        "saved_quick_dials_save",
        render_bottom_popup(&chat, /*width*/ 80)
    );
    chat.handle_key_event(KeyCode::Esc.into());
    assert_eq!(render_bottom_popup(&chat, /*width*/ 80), editor);
    chat.handle_key_event(KeyCode::Esc.into());

    let saved = chat.local_settings.tui.quick_dials["1"].clone();
    chat.open_quick_dial_editor(QuickDialEditorTarget::Slot(/*slot*/ 1), Some(saved.clone()));
    chat.handle_key_event(KeyCode::Enter.into());
    assert_matches!(
        std::iter::from_fn(|| events.try_recv().ok())
            .find(|event| matches!(event, AppEvent::ConfirmQuickDial { .. })),
        Some(AppEvent::ConfirmQuickDial {
            slot: 1,
            dial: Some(actual),
        }) if actual == saved
    );
    chat.handle_key_event(KeyCode::Esc.into());

    chat.open_quick_dial_menu();
    chat.handle_key_event(KeyCode::Char('d').into());
    assert_chatwidget_snapshot!(
        "saved_quick_dials_clear",
        render_bottom_popup(&chat, /*width*/ 80)
    );
    chat.handle_key_event(KeyCode::Esc.into());
    while events.try_recv().is_ok() {}
    let replacement = QuickDial {
        input: "/model gpt-5.6-terra --reasoning=high --speed=fast".into(),
        label: None,
    };
    chat.confirm_quick_dial(/*slot*/ 2, Some(replacement.clone()));
    assert_chatwidget_snapshot!(
        "saved_quick_dials_confirm_save",
        render_bottom_popup(&chat, /*width*/ 52)
    );
    chat.handle_key_event(KeyCode::Esc.into());
    assert!(
        !std::iter::from_fn(|| events.try_recv().ok())
            .any(|event| matches!(event, AppEvent::SaveQuickDial { .. }))
    );

    chat.confirm_quick_dial(/*slot*/ 1, Some(replacement.clone()));
    assert_chatwidget_snapshot!(
        "saved_quick_dials_confirm_replace",
        render_bottom_popup(&chat, /*width*/ 52)
    );
    chat.handle_key_event(KeyCode::Enter.into());
    assert_matches!(
        events.try_recv(),
        Ok(AppEvent::SaveQuickDial {
            slot: 1,
            dial: Some(actual),
        }) if actual == replacement
    );

    chat.confirm_quick_dial(/*slot*/ 1, /*dial*/ None);
    assert_chatwidget_snapshot!(
        "saved_quick_dials_confirm_delete",
        render_bottom_popup(&chat, /*width*/ 80)
    );
    chat.handle_key_event(KeyCode::Esc.into());
    assert!(
        !std::iter::from_fn(|| events.try_recv().ok())
            .any(|event| matches!(event, AppEvent::SaveQuickDial { .. }))
    );

    chat.recall_quick_dial(/*slot*/ 1);
    assert_eq!(chat.bottom_pane.composer_text(), DRAFT);
    assert_eq!(
        chat.queued_user_message_texts(),
        ["/model gpt-5.5 --reasoning=high --speed=fast"]
    );

    chat.local_settings.tui.quick_dials.extend(
        [
            ("2", "/permissions workspace-write"),
            (
                "3",
                "Review the current changes and explain every important behavior change",
            ),
            ("5", "/skills simplify-code"),
            ("7", "Plan the requested change"),
            ("0", "/status"),
        ]
        .map(|(slot, input)| {
            (
                slot.into(),
                QuickDial {
                    input: input.into(),
                    label: None,
                },
            )
        }),
    );
    chat.open_quick_dial_menu();
    assert_chatwidget_snapshot!(
        "quick_dial_shortcuts",
        render_bottom_popup(&chat, /*width*/ 100)
    );
    assert_chatwidget_snapshot!(
        "quick_dial_shortcuts_narrow",
        render_bottom_popup(&chat, /*width*/ 48)
    );
    chat.handle_key_event(KeyCode::Esc.into());

    chat.set_reasoning_effort(Some(ReasoningEffortConfig::High));
    chat.effective_service_tier = Some(SERVICE_TIER_DEFAULT_REQUEST_VALUE.into());
    assert_eq!(
        chat.current_model_quick_dial(),
        Ok(QuickDial {
            input: "/model gpt-5.5 --reasoning=high --speed=standard".into(),
            label: None,
        })
    );
    let expected_model = chat.current_model_quick_dial().expect("model command");
    chat.open_quick_dial_menu();
    chat.handle_key_event(KeyCode::Char('s').into());
    chat.handle_key_event(KeyCode::Enter.into());
    assert_matches!(
        std::iter::from_fn(|| events.try_recv().ok())
            .find(|event| matches!(event, AppEvent::OpenQuickDialEditor { .. })),
        Some(AppEvent::OpenQuickDialEditor {
            target: QuickDialEditorTarget::PickSlot,
            dial: Some(actual),
        }) if actual == expected_model
    );
    chat.handle_key_event(KeyCode::Esc.into());
    chat.set_reasoning_effort(Some(ReasoningEffortConfig::Custom("removed-effort".into())));
    assert_eq!(
        chat.current_model_quick_dial(),
        Err("Choose an available reasoning effort with /model before saving.")
    );
    chat.set_reasoning_effort(/*effort*/ None);
    chat.effective_service_tier = Some("removed-speed".into());
    assert_eq!(
        chat.current_model_quick_dial(),
        Err("Choose an available speed with /model before saving.")
    );
}
