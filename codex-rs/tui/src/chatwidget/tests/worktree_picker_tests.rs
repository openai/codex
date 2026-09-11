//! Worktree picker behavior and rendered choices.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn slash_new_and_fork_offer_checkout_choices_inside_local_git_repository() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Worktrees, /*enabled*/ false);
    let checkout = tempdir().expect("temporary checkout");
    std::fs::create_dir(checkout.path().join(".git")).expect("git directory");
    std::fs::write(checkout.path().join(".git/HEAD"), "ref: refs/heads/main\n").expect("git HEAD");
    chat.config.cwd =
        AbsolutePathBuf::from_absolute_path(checkout.path()).expect("absolute checkout");

    chat.dispatch_command(SlashCommand::Fork);
    assert_matches!(
        rx.try_recv(),
        Ok(AppEvent::ForkCurrentSession { name: None })
    );
    chat.dispatch_command(SlashCommand::New);
    assert_matches!(rx.try_recv(), Ok(AppEvent::NewSession { name: None }));

    chat.set_feature_enabled(Feature::Worktrees, /*enabled*/ true);
    chat.dispatch_command(SlashCommand::Fork);

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert_chatwidget_snapshot!("worktrees_fork_choices", popup);
    chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    chat.dispatch_command(SlashCommand::New);
    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert_chatwidget_snapshot!("worktrees_new_choices", popup);
    assert!(popup.contains("Current checkout"), "popup: {popup}");
    assert!(popup.contains("New worktree"), "popup: {popup}");
    assert_matches!(rx.try_recv(), Err(TryRecvError::Empty));
    chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    chat.bottom_pane
        .set_composer_text("/new named".into(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE));
    assert_matches!(rx.try_recv(), Ok(AppEvent::NewSession { name: Some(name) }) if name == "named");
    chat.bottom_pane
        .set_composer_text("/fork named".into(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE));
    assert_matches!(rx.try_recv(), Ok(AppEvent::StartManagedWorktree {
        mode: crate::app_event::ManagedWorktreeMode::Fork,
        name: Some(name),
    }) if name == "named");
    chat.set_local_worktree_operations(/*enabled*/ false);
    chat.dispatch_command(SlashCommand::New);
    assert_matches!(rx.try_recv(), Ok(AppEvent::NewSession { name: None }));
    chat.dispatch_command(SlashCommand::Fork);
    assert_matches!(
        rx.try_recv(),
        Ok(AppEvent::ForkCurrentSession { name: None })
    );
    for (available, snapshot) in [
        (false, "worktrees_command_remote"),
        (true, "worktrees_command_local"),
    ] {
        chat.set_feature_enabled(Feature::Worktrees, /*enabled*/ true);
        chat.set_local_worktree_operations(available);
        chat.bottom_pane
            .set_composer_text("/work".into(), Vec::new(), Vec::new());
        let popup = normalize_snapshot_paths(render_bottom_popup(&chat, /*width*/ 80));
        assert_chatwidget_snapshot!(snapshot, popup);
        assert_eq!(popup.contains("/worktree"), available);
    }
}

#[tokio::test]
async fn slash_worktree_offers_current_or_new_conversation() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let checkout = tempdir().expect("temporary checkout");
    std::fs::create_dir(checkout.path().join(".git")).expect("git directory");
    std::fs::write(checkout.path().join(".git/HEAD"), "ref: refs/heads/main\n").expect("git HEAD");
    chat.config.cwd =
        AbsolutePathBuf::from_absolute_path(checkout.path()).expect("absolute checkout");

    chat.set_feature_enabled(Feature::Worktrees, /*enabled*/ true);
    chat.dispatch_command(SlashCommand::Worktree);

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert_chatwidget_snapshot!("worktrees_conversation_choices", popup);
    assert!(
        popup.contains("Continue current conversation"),
        "popup: {popup}"
    );
    assert!(popup.contains("Start new conversation"), "popup: {popup}");
    assert_matches!(rx.try_recv(), Err(TryRecvError::Empty));
}

#[tokio::test]
async fn worktree_browser_actions_and_stale_results() {
    use crate::worktree_browser::Entry;
    use crate::worktree_browser::Owner;
    use crate::worktree_browser::ThreadSummary;
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let checkout = tempdir().unwrap();
    std::fs::create_dir(checkout.path().join(".git")).unwrap();
    std::fs::write(checkout.path().join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
    chat.config.cwd = AbsolutePathBuf::from_absolute_path(checkout.path()).unwrap();
    chat.set_feature_enabled(Feature::Worktrees, /*enabled*/ true);
    let request = chat.request_managed_worktrees().unwrap();
    assert_chatwidget_snapshot!(
        "worktree_browser_loading",
        render_bottom_popup(&chat, /*width*/ 80)
    );
    let owner = ThreadId::from_string("00000000-0000-0000-0000-000000000001").unwrap();
    let entry = Entry {
        root: PathBuf::from("/repo/worktree"),
        cwd: PathBuf::from("/repo/worktree"),
        owner: Owner::Resumable(ThreadSummary {
            id: owner,
            title: "Fix login form".to_string(),
            updated_at: chrono::Utc::now().timestamp() - 7_200,
        }),
    };
    chat.on_managed_worktrees_loaded(request, Ok(vec![entry.clone()]));
    assert_chatwidget_snapshot!(
        "worktree_browser_list",
        render_bottom_popup(&chat, /*width*/ 80)
    );
    for character in "worktree".chars() {
        chat.handle_key_event(KeyEvent::from(KeyCode::Char(character)));
    }
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    let AppEvent::ShowManagedWorktreeActions {
        request,
        entry: selected,
    } = rx.try_recv().unwrap()
    else {
        panic!("worktree action");
    };
    assert_eq!(selected, entry);
    chat.show_managed_worktree_actions(request.clone(), selected);
    assert_chatwidget_snapshot!(
        "worktree_browser_actions",
        render_bottom_popup(&chat, /*width*/ 80)
    );
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    let AppEvent::ManagedWorktreeAction {
        request: selected_request,
        action,
    } = rx.try_recv().unwrap()
    else {
        panic!("resume action");
    };
    assert_matches!(chat.managed_worktree_action(&selected_request, action.clone()), Some(AppEvent::ResumeSessionByIdOrName(id)) if id == owner.to_string());
    chat.set_local_worktree_operations(/*enabled*/ false);
    assert!(
        chat.managed_worktree_action(&selected_request, action)
            .is_none()
    );
    chat.set_local_worktree_operations(/*enabled*/ true);
    chat.show_managed_worktree_actions(
        request,
        Entry {
            owner: Owner::None,
            ..entry
        },
    );
    assert!(!render_bottom_popup(&chat, /*width*/ 80).contains("Resume owner"));
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    let AppEvent::ManagedWorktreeAction { request, action } = rx.try_recv().unwrap() else {
        panic!("copy action");
    };
    assert_matches!(chat.managed_worktree_action(&request, action), Some(AppEvent::CopySelection { text, .. }) if &*text == "/repo/worktree");
    chat.confirm_managed_worktree_removal(request.clone(), PathBuf::from("/repo/worktree"));
    assert_chatwidget_snapshot!(
        "worktree_browser_delete_confirmation",
        render_bottom_popup(&chat, /*width*/ 80)
    );
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    assert_matches!(rx.try_recv(), Err(TryRecvError::Empty));
    chat.confirm_managed_worktree_removal(request, PathBuf::from("/repo/worktree"));
    chat.handle_key_event(KeyEvent::from(KeyCode::Down));
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    let AppEvent::ManagedWorktreeAction {
        request: selected,
        action,
    } = rx.try_recv().unwrap()
    else {
        panic!("delete action");
    };
    assert_matches!(chat.managed_worktree_action(&selected, action), Some(AppEvent::RemoveManagedWorktree { root, .. }) if root.as_path() == std::path::Path::new("/repo/worktree"));
    let first = chat.request_managed_worktrees().unwrap();
    chat.handle_key_event(KeyEvent::from(KeyCode::Esc));
    chat.on_managed_worktrees_loaded(first, Ok(Vec::new()));
    assert!(!chat.bottom_pane.has_active_view());
    let stale = chat.request_managed_worktrees().unwrap();
    let current = chat.request_managed_worktrees().unwrap();
    chat.on_managed_worktrees_loaded(stale, Err("stale result".to_string()));
    assert_eq!(chat.worktree_popup_request_id, Some(current.id));
    chat.config.cwd = AbsolutePathBuf::from_absolute_path(checkout.path().join("other")).unwrap();
    chat.on_managed_worktrees_loaded(current, Ok(Vec::new()));
    assert!(!chat.bottom_pane.has_active_view());
}

#[tokio::test]
async fn worktree_browser_names_archived_and_missing_owners() {
    use crate::worktree_browser::Entry;
    use crate::worktree_browser::Owner;
    use crate::worktree_browser::ThreadSummary;
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let checkout = tempdir().unwrap();
    std::fs::create_dir(checkout.path().join(".git")).unwrap();
    std::fs::write(checkout.path().join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
    chat.config.cwd = AbsolutePathBuf::from_absolute_path(checkout.path()).unwrap();
    chat.set_feature_enabled(Feature::Worktrees, /*enabled*/ true);
    let request = chat.request_managed_worktrees().unwrap();
    let owner = ThreadId::from_string("00000000-0000-0000-0000-000000000002").unwrap();
    chat.on_managed_worktrees_loaded(
        request.clone(),
        Ok(vec![
            Entry {
                root: PathBuf::from("/repo/archived"),
                cwd: PathBuf::from("/repo/archived"),
                owner: Owner::Archived(ThreadSummary {
                    id: owner,
                    title: "Fix settings".to_string(),
                    updated_at: chrono::Utc::now().timestamp() - 86_400,
                }),
            },
            Entry {
                root: PathBuf::from("/repo/missing"),
                cwd: PathBuf::from("/repo/missing"),
                owner: Owner::Unavailable(owner),
            },
            Entry {
                root: PathBuf::from("/repo/empty"),
                cwd: PathBuf::from("/repo/empty"),
                owner: Owner::None,
            },
        ]),
    );
    assert_chatwidget_snapshot!(
        "worktree_browser_owner_states",
        render_bottom_popup(&chat, /*width*/ 80)
    );
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
    chat.show_managed_worktree_actions(
        request,
        Entry {
            root: PathBuf::from("/repo/archived"),
            cwd: PathBuf::from("/repo/archived"),
            owner: Owner::Archived(ThreadSummary {
                id: owner,
                title: "Fix settings".to_string(),
                updated_at: 0,
            }),
        },
    );
    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert!(popup.contains("Worktree: Fix settings"), "popup: {popup}");
    assert!(!popup.contains("Resume owner"));
}
