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
        chat.config.notices.hide_world_writable_warning = Some(true);
        chat.set_windows_sandbox_mode(Some(WindowsSandboxModeToml::Unelevated));
    }
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
        chat.set_feature_enabled(Feature::WindowsSandbox, /*enabled*/ false);
        chat.set_feature_enabled(Feature::WindowsSandboxElevated, /*enabled*/ false);
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
        codex_config::ConfigRequirementsToml {
            allowed_approvals_reviewers: Some(vec![AutoReview]),
            ..Default::default()
        },
        codex_config::ConfigRequirementsToml {
            auto_review: Some(codex_config::AutoReviewRequirementsToml {
                required_on_models: Some(vec![chat.current_model().to_string()]),
                ..Default::default()
            }),
            ..Default::default()
        },
    ] {
        chat.config.config_layer_stack = requirements_stack(requirements);
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
