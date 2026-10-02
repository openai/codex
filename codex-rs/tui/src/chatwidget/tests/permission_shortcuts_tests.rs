use super::permissions::requirements_stack;
use super::*;
use ApprovalsReviewer::AutoReview;
use ApprovalsReviewer::User;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn permission_shortcuts_cycle_builtin_modes() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    chat.set_feature_enabled(Feature::GuardianApproval, /*enabled*/ true);
    chat.chat_keymap.next_permission_mode = vec![crate::key_hint::plain(KeyCode::F(8))];
    chat.chat_keymap.previous_permission_mode = vec![crate::key_hint::plain(KeyCode::F(7))];
    #[cfg(target_os = "windows")]
    {
        chat.set_windows_sandbox_mode(Some(WindowsSandboxSetupMode::Unelevated));
    }
    chat.permission_discovery = Some(crate::permission_discovery::PermissionDiscovery::local(
        &chat.config,
    ));
    chat.config.config_layer_stack = requirements_stack(
        serde_json::from_value(serde_json::json!({
            "allowed_approvals_reviewers": ["auto_review"],
            "allowed_permission_profiles": {":workspace": false, ":read-only": false}
        }))
        .unwrap(),
    );
    chat.turn_lifecycle.agent_turn_running = true;
    for (current, reviewer, key, expected, next_reviewer) in [
        (":workspace", User, KeyCode::F(8), ":workspace", AutoReview),
        (":workspace", AutoReview, KeyCode::F(8), ":read-only", User),
        (":read-only", User, KeyCode::F(8), ":workspace", User),
        (":read-only", User, KeyCode::F(7), ":workspace", AutoReview),
    ] {
        let profile = if current == ":read-only" {
            PermissionProfile::read_only()
        } else {
            PermissionProfile::workspace_write()
        };
        chat.config
            .permissions
            .set_permission_profile_from_session_snapshot(PermissionProfileSnapshot::active(
                profile,
                ActivePermissionProfile::new(current),
            ))
            .expect("set current profile");
        chat.config.approvals_reviewer = reviewer;
        chat.handle_key_event(KeyEvent::from(key));
        chat.handle_key_event(KeyEvent::from(key));
        let AppEvent::ApplyPermissionShortcut {
            thread_id: target,
            selection,
        } = rx.try_recv().expect("permission selection")
        else {
            panic!("expected one typed permission selection");
        };
        assert_eq!(
            (
                target,
                selection.profile_id.as_str(),
                selection.approval_policy,
                selection.approvals_reviewer
            ),
            (
                thread_id,
                expected,
                Some(AskForApproval::OnRequest),
                Some(next_reviewer)
            )
        );
        assert!(
            rx.try_recv().is_err(),
            "pending shortcut must not be duplicated"
        );
        chat.complete_permission_shortcut(thread_id);
    }
    #[cfg(target_os = "windows")]
    {
        chat.set_windows_sandbox_mode(/*mode*/ None);
        chat.config
            .permissions
            .set_permission_profile(PermissionProfile::read_only())
            .unwrap();
        chat.config.approvals_reviewer = User;
        chat.handle_key_event(KeyEvent::from(KeyCode::F(8)));
        assert!(matches!(
            rx.try_recv(),
            Ok(AppEvent::ApplyPermissionShortcut {
                selection: PermissionProfileSelection {
                    approvals_reviewer: Some(AutoReview),
                    ..
                },
                ..
            })
        ));
        assert!(rx.try_recv().is_err());
        chat.complete_permission_shortcut(thread_id);
        chat.windows_sandbox_host = crate::app::WindowsSandboxHost::Remote;
        chat.handle_key_event(KeyEvent::from(KeyCode::F(8)));
        assert!(matches!(
            rx.try_recv(),
            Ok(AppEvent::ApplyPermissionShortcut {
                selection: PermissionProfileSelection {
                    profile_id,
                    approvals_reviewer: Some(User),
                    ..
                },
                ..
            }) if profile_id == ":workspace"
        ));
    }
}

#[tokio::test]
async fn permission_shortcuts_respect_managed_mode_requirements() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.set_feature_enabled(Feature::GuardianApproval, /*enabled*/ true);
    chat.config.approvals_reviewer = AutoReview;
    chat.chat_keymap.next_permission_mode = vec![crate::key_hint::plain(KeyCode::F(8))];
    chat.config
        .permissions
        .set_permission_profile_from_session_snapshot(PermissionProfileSnapshot::active(
            PermissionProfile::workspace_write(),
            ActivePermissionProfile::new(":workspace"),
        ))
        .expect("set active profile");

    for requirements in [
        serde_json::json!({"allowedApprovalsReviewers": ["auto_review"]}),
        serde_json::json!({"autoReview": {"requiredOnModels": [chat.current_model()]}}),
        serde_json::json!({"allowedPermissionProfiles": {":workspace": false, ":read-only": false}}),
    ] {
        let mut discovery = crate::permission_discovery::PermissionDiscovery::local(&chat.config);
        discovery.requirements = Some(serde_json::from_value(requirements).unwrap());
        chat.permission_discovery = Some(discovery);
        chat.handle_key_event(KeyEvent::from(KeyCode::F(8)));
        let AppEvent::InsertHistoryCell(cell) = rx.try_recv().expect("unavailable-mode notice")
        else {
            panic!("must not submit a forbidden mode");
        };
        insta::assert_snapshot!(
            "permission_shortcut_no_alternative",
            lines_to_single_string(&cell.display_lines(/*width*/ 80))
        );
        assert!(rx.try_recv().is_err(), "must not submit a forbidden mode");
    }
}

#[tokio::test]
async fn permission_shortcuts_load_once_and_reuse_the_picker_catalog() {
    let (mut chat, mut events, _ops) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    chat.chat_keymap.next_permission_mode = vec![crate::key_hint::plain(KeyCode::F(8))];
    chat.handle_key_event(KeyEvent::from(KeyCode::F(8)));
    let request_id = assert_matches!(events.try_recv(), Ok(AppEvent::FetchPermissionProfiles { request_id, .. }) => request_id);
    assert_chatwidget_snapshot!(
        "permission_shortcut_loading",
        render_bottom_popup(&chat, /*width*/ 80)
    );
    chat.handle_key_event(KeyEvent::from(KeyCode::Esc));
    chat.handle_key_event(KeyEvent::from(KeyCode::F(8)));
    assert!(events.try_recv().is_err(), "reuse pending fetch");
    chat.on_permission_profiles_loaded(
        request_id,
        Ok(crate::permission_discovery::PermissionDiscovery::local(
            &chat.config,
        )),
    );
    chat.handle_key_event(KeyEvent::from(KeyCode::Esc));
    chat.open_permissions_popup();
    assert!(chat.bottom_pane.has_active_view());
    assert!(
        events.try_recv().is_err(),
        "reopening must use the cached catalog"
    );
    chat.handle_key_event(KeyEvent::from(KeyCode::Esc));
    chat.handle_key_event(KeyEvent::from(KeyCode::F(8)));
    assert_matches!(
        events.try_recv(),
        Ok(AppEvent::ApplyPermissionShortcut { .. })
    );
    chat.complete_permission_shortcut(thread_id);
    chat.pause_for_disconnect();
    while events.try_recv().is_ok() {}
    chat.handle_key_event(KeyEvent::from(KeyCode::F(8)));
    assert_matches!(
        events.try_recv(),
        Ok(AppEvent::FetchPermissionProfiles { .. })
    );
}
