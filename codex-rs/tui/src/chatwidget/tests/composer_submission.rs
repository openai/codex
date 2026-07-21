use super::*;
use crate::app_event::ConnectorsSnapshot;
use codex_protocol::models::ManagedFileSystemPermissions;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use pretty_assertions::assert_eq;
use std::collections::VecDeque;

#[tokio::test]
async fn parent_owned_thread_blocks_all_direct_input_entry_points() {
    let (mut chat, mut rx, mut op_rx) =
        make_chatwidget_manual(/*model_override*/ Some("gpt-5")).await;
    chat.thread_id = Some(ThreadId::new());
    drain_insert_history(&mut rx);
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
    chat.set_parent_owned_thread();
    chat.set_side_conversation_active(/*active*/ false);
    chat.bottom_pane
        .set_composer_text("keep this draft".to_string(), Vec::new(), Vec::new());
    let before = chat.bottom_pane.composer_draft_snapshot();

    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_eq!(chat.bottom_pane.composer_draft_snapshot(), before);
    assert_no_submit_op(&mut op_rx);
    let rendered = drain_insert_history(&mut rx)
        .into_iter()
        .flatten()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert_chatwidget_snapshot!("parent_owned_thread_rejects_input", rendered);

    let collaboration_mode_before = chat.active_collaboration_mask.clone();
    let plan_mode = collaboration_modes::mask_for_kind(chat.model_catalog.as_ref(), ModeKind::Plan)
        .expect("expected plan collaboration mode");
    chat.submit_user_message_with_mode("Implement the plan.".to_string(), plan_mode);
    assert_eq!(chat.active_collaboration_mask, collaboration_mode_before);
    assert_no_submit_op(&mut op_rx);

    for command in [
        "/init",
        "/review check this",
        "/side inspect this",
        "/archive",
        "/rename",
        "/agent parent",
        "/diff now",
        "!echo blocked",
        " !echo blocked",
    ] {
        chat.bottom_pane
            .set_composer_text(command.to_string(), Vec::new(), Vec::new());
        let before = chat.bottom_pane.composer_draft_snapshot();
        chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(chat.bottom_pane.composer_draft_snapshot(), before);
        assert_no_submit_op(&mut op_rx);
    }

    assert!(!chat.submit_op(AppCommand::compact()));
    assert_no_submit_op(&mut op_rx);
}

#[tokio::test]
async fn parent_owned_thread_blocks_settings_shortcuts() {
    let (mut chat, mut rx, _op_rx) =
        make_chatwidget_manual(/*model_override*/ Some("gpt-5.4")).await;
    chat.thread_id = Some(ThreadId::new());
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);
    chat.set_reasoning_effort(Some(ReasoningEffortConfig::Medium));
    chat.set_parent_owned_thread();
    drain_insert_history(&mut rx);

    let collaboration_mode_before = chat.active_collaboration_mask.clone();
    let reasoning_effort_before = chat.current_reasoning_effort();

    for key_event in [
        KeyEvent::from(KeyCode::BackTab),
        KeyEvent::new(KeyCode::Char('.'), KeyModifiers::ALT),
        KeyEvent::new(KeyCode::Char(','), KeyModifiers::ALT),
    ] {
        chat.handle_key_event(key_event);
    }

    assert_eq!(chat.active_collaboration_mask, collaboration_mode_before);
    assert_eq!(chat.current_reasoning_effort(), reasoning_effort_before);
    let events = std::iter::from_fn(|| rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(events.iter().all(|event| !matches!(
        event,
        AppEvent::SubmitThreadOp { .. }
            | AppEvent::UpdateModel(_)
            | AppEvent::UpdateReasoningEffort(_)
            | AppEvent::UpdatePlanModeReasoningEffort(_)
    )));

    let rendered = events
        .into_iter()
        .filter_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => Some(cell.display_lines(/*width*/ 80)),
            _ => None,
        })
        .flatten()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert_chatwidget_snapshot!("parent_owned_thread_rejects_settings_shortcuts", rendered);
}

#[tokio::test]
async fn parent_owned_thread_restores_pending_initial_prompt() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ Some("gpt-5")).await;
    let pending_prompt = "keep this startup prompt".to_string();
    chat.initial_user_message = Some(pending_prompt.clone().into());
    chat.set_parent_owned_thread();

    chat.submit_initial_user_message_if_pending();

    assert_eq!(chat.bottom_pane.composer_text(), pending_prompt);
    assert!(chat.initial_user_message.is_none());
    assert_no_submit_op(&mut op_rx);
}

#[tokio::test]
async fn parent_owned_thread_preserves_queued_input_before_draining() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ Some("gpt-5")).await;
    let queued_message = QueuedUserMessage {
        user_message: UserMessage::from("keep this queued prompt"),
        action: QueuedInputAction::Plain,
        pending_pastes: vec![("[Image 1]".to_string(), "pasted contents".to_string())],
    };
    let history_record = UserMessageHistoryRecord::UserMessageText;
    chat.input_queue
        .queued_user_messages
        .push_back(queued_message.clone());
    chat.input_queue
        .queued_user_message_history_records
        .push_back(history_record.clone());
    chat.set_parent_owned_thread();

    assert!(!chat.maybe_send_next_queued_input());
    assert_eq!(
        chat.input_queue.queued_user_messages,
        VecDeque::from([queued_message])
    );
    assert_eq!(
        chat.input_queue.queued_user_message_history_records,
        VecDeque::from([history_record])
    );
    assert_no_submit_op(&mut op_rx);
}

#[tokio::test]
async fn submission_preserves_text_elements_and_local_images() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    let thread_id = ThreadId::new();
    let rollout_file = NamedTempFile::new().unwrap();
    let configured = crate::session_state::ThreadSessionState {
        thread_id,
        forked_from_id: None,
        fork_parent_title: None,
        thread_name: None,
        model: "test-model".to_string(),
        model_provider_id: "test-provider".to_string(),
        service_tier: None,
        approval_policy: AskForApproval::Never,
        approvals_reviewer: ApprovalsReviewer::User,
        permission_profile: PermissionProfile::read_only(),
        active_permission_profile: None,
        cwd: test_path_buf("/home/user/project").abs(),
        runtime_workspace_roots: Vec::new(),
        instruction_source_paths: Vec::new(),
        reasoning_effort: Some(ReasoningEffortConfig::default()),
        collaboration_mode: None,
        personality: None,
        message_history: None,
        network_proxy: None,
        rollout_path: Some(rollout_file.path().to_path_buf()),
    };
    chat.handle_thread_session(configured);
    drain_insert_history(&mut rx);

    let placeholder = "[Image #1]";
    let text = format!("{placeholder} submit");
    let text_elements = vec![TextElement::new(
        (0..placeholder.len()).into(),
        Some(placeholder.to_string()),
    )];
    let local_images = vec![PathBuf::from("/tmp/submitted.png")];

    chat.bottom_pane
        .set_composer_text(text.clone(), text_elements.clone(), local_images.clone());
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    let items = match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => items,
        other => panic!("expected Op::UserTurn, got {other:?}"),
    };
    assert_eq!(items.len(), 2);
    assert_eq!(
        items[0],
        UserInput::LocalImage {
            path: local_images[0].clone(),
            detail: None,
        }
    );
    assert_eq!(
        items[1],
        UserInput::Text {
            text: text.clone(),
            text_elements: text_elements.clone().into_iter().map(Into::into).collect(),
        }
    );

    let mut user_cell = None;
    while let Ok(ev) = rx.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = ev
            && let Some(cell) = cell.as_any().downcast_ref::<UserHistoryCell>()
        {
            user_cell = Some((
                cell.message.clone(),
                cell.text_elements.clone(),
                cell.local_image_paths.clone(),
                cell.remote_image_urls.clone(),
            ));
            break;
        }
    }

    let (stored_message, stored_elements, stored_images, stored_remote_image_urls) =
        user_cell.expect("expected submitted user history cell");
    assert_eq!(stored_message, text);
    assert_eq!(stored_elements, text_elements);
    assert_eq!(stored_images, local_images);
    assert!(stored_remote_image_urls.is_empty());
}

#[tokio::test]
async fn submission_includes_configured_active_permission_profile() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    let thread_id = ThreadId::new();
    let rollout_file = NamedTempFile::new().unwrap();
    let expected_permission_profile: PermissionProfile = PermissionProfile::Managed {
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
                    path: FileSystemPath::GlobPattern {
                        pattern: "/home/user/project/secrets/**".to_string(),
                    },
                    access: FileSystemAccessMode::Deny,
                    missing_path_behavior: None,
                },
            ],
            glob_scan_max_depth: None,
        },
    };
    let expected_active_permission_profile = ActivePermissionProfile::new("custom");
    let configured = crate::session_state::ThreadSessionState {
        thread_id,
        forked_from_id: None,
        fork_parent_title: None,
        thread_name: None,
        model: "test-model".to_string(),
        model_provider_id: "test-provider".to_string(),
        service_tier: None,
        approval_policy: AskForApproval::Never,
        approvals_reviewer: ApprovalsReviewer::User,
        permission_profile: expected_permission_profile,
        active_permission_profile: Some(expected_active_permission_profile.clone()),
        cwd: test_path_buf("/home/user/project").abs(),
        runtime_workspace_roots: Vec::new(),
        instruction_source_paths: Vec::new(),
        reasoning_effort: Some(ReasoningEffortConfig::default()),
        collaboration_mode: None,
        personality: None,
        message_history: None,
        network_proxy: None,
        rollout_path: Some(rollout_file.path().to_path_buf()),
    };
    chat.handle_thread_session(configured);
    drain_insert_history(&mut rx);

    chat.bottom_pane.set_composer_text(
        "submit with configured permissions".to_string(),
        Vec::new(),
        Vec::new(),
    );
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    let active_permission_profile = match next_submit_op(&mut op_rx) {
        Op::UserTurn {
            active_permission_profile,
            ..
        } => active_permission_profile,
        other => panic!("expected Op::UserTurn, got {other:?}"),
    };
    assert_eq!(
        active_permission_profile,
        Some(expected_active_permission_profile)
    );
}

#[tokio::test]
async fn submission_omits_active_permission_profile_for_legacy_snapshot() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    let thread_id = ThreadId::new();
    let rollout_file = NamedTempFile::new().unwrap();
    let expected_permission_profile: PermissionProfile = PermissionProfile::Managed {
        network: NetworkSandboxPolicy::Restricted,
        file_system: ManagedFileSystemPermissions::Unrestricted,
    };
    let configured = crate::session_state::ThreadSessionState {
        thread_id,
        forked_from_id: None,
        fork_parent_title: None,
        thread_name: None,
        model: "test-model".to_string(),
        model_provider_id: "test-provider".to_string(),
        service_tier: None,
        approval_policy: AskForApproval::Never,
        approvals_reviewer: ApprovalsReviewer::User,
        permission_profile: expected_permission_profile,
        active_permission_profile: None,
        cwd: test_path_buf("/home/user/project").abs(),
        runtime_workspace_roots: Vec::new(),
        instruction_source_paths: Vec::new(),
        reasoning_effort: Some(ReasoningEffortConfig::default()),
        collaboration_mode: None,
        personality: None,
        message_history: None,
        network_proxy: None,
        rollout_path: Some(rollout_file.path().to_path_buf()),
    };
    chat.handle_thread_session(configured);
    drain_insert_history(&mut rx);

    chat.bottom_pane
        .set_composer_text("submit".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    let active_permission_profile = match next_submit_op(&mut op_rx) {
        Op::UserTurn {
            active_permission_profile,
            ..
        } => active_permission_profile,
        other => panic!("expected Op::UserTurn, got {other:?}"),
    };
    assert_eq!(active_permission_profile, None);
}

#[tokio::test]
async fn submission_with_remote_and_local_images_keeps_local_placeholder_numbering() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    let thread_id = ThreadId::new();
    let rollout_file = NamedTempFile::new().unwrap();
    let configured = crate::session_state::ThreadSessionState {
        thread_id,
        forked_from_id: None,
        fork_parent_title: None,
        thread_name: None,
        model: "test-model".to_string(),
        model_provider_id: "test-provider".to_string(),
        service_tier: None,
        approval_policy: AskForApproval::Never,
        approvals_reviewer: ApprovalsReviewer::User,
        permission_profile: PermissionProfile::read_only(),
        active_permission_profile: None,
        cwd: test_path_buf("/home/user/project").abs(),
        runtime_workspace_roots: Vec::new(),
        instruction_source_paths: Vec::new(),
        reasoning_effort: Some(ReasoningEffortConfig::default()),
        collaboration_mode: None,
        personality: None,
        message_history: None,
        network_proxy: None,
        rollout_path: Some(rollout_file.path().to_path_buf()),
    };
    chat.handle_thread_session(configured);
    drain_insert_history(&mut rx);

    let remote_url = "https://example.com/remote.png".to_string();
    chat.set_remote_image_urls(vec![remote_url.clone()]);

    let placeholder = "[Image #2]";
    let text = format!("{placeholder} submit mixed");
    let text_elements = vec![TextElement::new(
        (0..placeholder.len()).into(),
        Some(placeholder.to_string()),
    )];
    let local_images = vec![PathBuf::from("/tmp/submitted-mixed.png")];

    chat.bottom_pane
        .set_composer_text(text.clone(), text_elements.clone(), local_images.clone());
    assert_eq!(chat.bottom_pane.composer_text(), "[Image #2] submit mixed");
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    let items = match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => items,
        other => panic!("expected Op::UserTurn, got {other:?}"),
    };
    assert_eq!(items.len(), 3);
    assert_eq!(
        items[0],
        UserInput::Image {
            url: remote_url.clone(),
            detail: None,
        }
    );
    assert_eq!(
        items[1],
        UserInput::LocalImage {
            path: local_images[0].clone(),
            detail: None,
        }
    );
    assert_eq!(
        items[2],
        UserInput::Text {
            text: text.clone(),
            text_elements: text_elements.clone().into_iter().map(Into::into).collect(),
        }
    );
    assert_eq!(text_elements[0].placeholder(&text), Some("[Image #2]"));

    let mut user_cell = None;
    while let Ok(ev) = rx.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = ev
            && let Some(cell) = cell.as_any().downcast_ref::<UserHistoryCell>()
        {
            user_cell = Some((
                cell.message.clone(),
                cell.text_elements.clone(),
                cell.local_image_paths.clone(),
                cell.remote_image_urls.clone(),
            ));
            break;
        }
    }

    let (stored_message, stored_elements, stored_images, stored_remote_image_urls) =
        user_cell.expect("expected submitted user history cell");
    assert_eq!(stored_message, text);
    assert_eq!(stored_elements, text_elements);
    assert_eq!(stored_images, local_images);
    assert_eq!(stored_remote_image_urls, vec![remote_url]);
}

#[tokio::test]
async fn enter_with_only_remote_images_submits_user_turn() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    let thread_id = ThreadId::new();
    let rollout_file = NamedTempFile::new().unwrap();
    let configured = crate::session_state::ThreadSessionState {
        thread_id,
        forked_from_id: None,
        fork_parent_title: None,
        thread_name: None,
        model: "test-model".to_string(),
        model_provider_id: "test-provider".to_string(),
        service_tier: None,
        approval_policy: AskForApproval::Never,
        approvals_reviewer: ApprovalsReviewer::User,
        permission_profile: PermissionProfile::read_only(),
        active_permission_profile: None,
        cwd: test_path_buf("/home/user/project").abs(),
        runtime_workspace_roots: Vec::new(),
        instruction_source_paths: Vec::new(),
        reasoning_effort: Some(ReasoningEffortConfig::default()),
        collaboration_mode: None,
        personality: None,
        message_history: None,
        network_proxy: None,
        rollout_path: Some(rollout_file.path().to_path_buf()),
    };
    chat.handle_thread_session(configured);
    drain_insert_history(&mut rx);

    let remote_url = "https://example.com/remote-only.png".to_string();
    chat.set_remote_image_urls(vec![remote_url.clone()]);
    assert_eq!(chat.bottom_pane.composer_text(), "");

    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    let (items, summary) = match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, summary, .. } => (items, summary),
        other => panic!("expected Op::UserTurn, got {other:?}"),
    };
    assert_eq!(
        items,
        vec![UserInput::Image {
            url: remote_url.clone(),
            detail: None,
        }]
    );
    assert_eq!(summary, None);
    assert!(chat.remote_image_urls().is_empty());

    let mut user_cell = None;
    while let Ok(ev) = rx.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = ev
            && let Some(cell) = cell.as_any().downcast_ref::<UserHistoryCell>()
        {
            user_cell = Some((cell.message.clone(), cell.remote_image_urls.clone()));
            break;
        }
    }

    let (stored_message, stored_remote_image_urls) =
        user_cell.expect("expected submitted user history cell");
    assert_eq!(stored_message, String::new());
    assert_eq!(stored_remote_image_urls, vec![remote_url]);
}

#[tokio::test]
async fn shift_enter_with_only_remote_images_does_not_submit_user_turn() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    let thread_id = ThreadId::new();
    let rollout_file = NamedTempFile::new().unwrap();
    let configured = crate::session_state::ThreadSessionState {
        thread_id,
        forked_from_id: None,
        fork_parent_title: None,
        thread_name: None,
        model: "test-model".to_string(),
        model_provider_id: "test-provider".to_string(),
        service_tier: None,
        approval_policy: AskForApproval::Never,
        approvals_reviewer: ApprovalsReviewer::User,
        permission_profile: PermissionProfile::read_only(),
        active_permission_profile: None,
        cwd: test_path_buf("/home/user/project").abs(),
        runtime_workspace_roots: Vec::new(),
        instruction_source_paths: Vec::new(),
        reasoning_effort: Some(ReasoningEffortConfig::default()),
        collaboration_mode: None,
        personality: None,
        message_history: None,
        network_proxy: None,
        rollout_path: Some(rollout_file.path().to_path_buf()),
    };
    chat.handle_thread_session(configured);
    drain_insert_history(&mut rx);

    let remote_url = "https://example.com/remote-only.png".to_string();
    chat.set_remote_image_urls(vec![remote_url.clone()]);
    assert_eq!(chat.bottom_pane.composer_text(), "");

    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT));

    assert_no_submit_op(&mut op_rx);
    assert_eq!(chat.remote_image_urls(), vec![remote_url]);
}

#[tokio::test]
async fn enter_with_only_remote_images_does_not_submit_when_modal_is_active() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    let thread_id = ThreadId::new();
    let rollout_file = NamedTempFile::new().unwrap();
    let configured = crate::session_state::ThreadSessionState {
        thread_id,
        forked_from_id: None,
        fork_parent_title: None,
        thread_name: None,
        model: "test-model".to_string(),
        model_provider_id: "test-provider".to_string(),
        service_tier: None,
        approval_policy: AskForApproval::Never,
        approvals_reviewer: ApprovalsReviewer::User,
        permission_profile: PermissionProfile::read_only(),
        active_permission_profile: None,
        cwd: test_path_buf("/home/user/project").abs(),
        runtime_workspace_roots: Vec::new(),
        instruction_source_paths: Vec::new(),
        reasoning_effort: Some(ReasoningEffortConfig::default()),
        collaboration_mode: None,
        personality: None,
        message_history: None,
        network_proxy: None,
        rollout_path: Some(rollout_file.path().to_path_buf()),
    };
    chat.handle_thread_session(configured);
    drain_insert_history(&mut rx);

    let remote_url = "https://example.com/remote-only.png".to_string();
    chat.set_remote_image_urls(vec![remote_url.clone()]);

    chat.open_review_popup();
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_eq!(chat.remote_image_urls(), vec![remote_url]);
    assert_no_submit_op(&mut op_rx);
}

#[tokio::test]
async fn enter_with_only_remote_images_does_not_submit_when_input_disabled() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    let thread_id = ThreadId::new();
    let rollout_file = NamedTempFile::new().unwrap();
    let configured = crate::session_state::ThreadSessionState {
        thread_id,
        forked_from_id: None,
        fork_parent_title: None,
        thread_name: None,
        model: "test-model".to_string(),
        model_provider_id: "test-provider".to_string(),
        service_tier: None,
        approval_policy: AskForApproval::Never,
        approvals_reviewer: ApprovalsReviewer::User,
        permission_profile: PermissionProfile::read_only(),
        active_permission_profile: None,
        cwd: test_path_buf("/home/user/project").abs(),
        runtime_workspace_roots: Vec::new(),
        instruction_source_paths: Vec::new(),
        reasoning_effort: Some(ReasoningEffortConfig::default()),
        collaboration_mode: None,
        personality: None,
        message_history: None,
        network_proxy: None,
        rollout_path: Some(rollout_file.path().to_path_buf()),
    };
    chat.handle_thread_session(configured);
    drain_insert_history(&mut rx);

    let remote_url = "https://example.com/remote-only.png".to_string();
    chat.set_remote_image_urls(vec![remote_url.clone()]);
    chat.bottom_pane.set_composer_input_enabled(
        /*enabled*/ false,
        Some("Input disabled for test.".to_string()),
    );

    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_eq!(chat.remote_image_urls(), vec![remote_url]);
    assert_no_submit_op(&mut op_rx);
}

#[tokio::test]
async fn submission_prefers_selected_duplicate_skill_path() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    let thread_id = ThreadId::new();
    let rollout_file = NamedTempFile::new().unwrap();
    let configured = crate::session_state::ThreadSessionState {
        thread_id,
        forked_from_id: None,
        fork_parent_title: None,
        thread_name: None,
        model: "test-model".to_string(),
        model_provider_id: "test-provider".to_string(),
        service_tier: None,
        approval_policy: AskForApproval::Never,
        approvals_reviewer: ApprovalsReviewer::User,
        permission_profile: PermissionProfile::read_only(),
        active_permission_profile: None,
        cwd: test_path_buf("/home/user/project").abs(),
        runtime_workspace_roots: Vec::new(),
        instruction_source_paths: Vec::new(),
        reasoning_effort: Some(ReasoningEffortConfig::default()),
        collaboration_mode: None,
        personality: None,
        message_history: None,
        network_proxy: None,
        rollout_path: Some(rollout_file.path().to_path_buf()),
    };
    chat.handle_thread_session(configured);
    drain_insert_history(&mut rx);

    let repo_skill_path = test_path_buf("/tmp/repo/figma/SKILL.md").abs();
    let user_skill_path = test_path_buf("/tmp/user/figma/SKILL.md").abs();
    chat.set_skills(Some(vec![
        SkillMetadata {
            name: "figma".to_string(),
            description: "Repo skill".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            path: repo_skill_path,
            scope: crate::test_support::skill_scope_repo(),
            enabled: true,
        },
        SkillMetadata {
            name: "figma".to_string(),
            description: "User skill".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            path: user_skill_path.clone(),
            scope: crate::test_support::skill_scope_user(),
            enabled: true,
        },
    ]));

    chat.bottom_pane.set_composer_text_with_mention_bindings(
        "please use $figma now".to_string(),
        Vec::new(),
        Vec::new(),
        vec![MentionBinding {
            sigil: '$',
            mention: "figma".to_string(),
            path: user_skill_path.to_string_lossy().into_owned(),
        }],
    );
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    let items = match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => items,
        other => panic!("expected Op::UserTurn, got {other:?}"),
    };
    let selected_skill_paths = items
        .iter()
        .filter_map(|item| match item {
            UserInput::Skill { path, .. } => Some(path.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(selected_skill_paths, vec![user_skill_path.to_path_buf()]);
}

#[tokio::test]
async fn blocked_image_restore_preserves_mention_bindings() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    let placeholder = "[Image #1]";
    let text = format!("{placeholder} check $file");
    let text_elements = vec![TextElement::new(
        (0..placeholder.len()).into(),
        Some(placeholder.to_string()),
    )];
    let local_images = vec![LocalImageAttachment {
        placeholder: placeholder.to_string(),
        path: PathBuf::from("/tmp/blocked.png"),
    }];
    let mention_bindings = vec![MentionBinding {
        sigil: '$',
        mention: "file".to_string(),
        path: "/tmp/skills/file/SKILL.md".to_string(),
    }];

    chat.restore_blocked_image_submission(
        text.clone(),
        text_elements,
        local_images.clone(),
        mention_bindings.clone(),
        Vec::new(),
    );

    let mention_start = text.find("$file").expect("mention token exists");
    let expected_elements = vec![
        TextElement::new((0..placeholder.len()).into(), Some(placeholder.to_string())),
        TextElement::new(
            (mention_start..mention_start + "$file".len()).into(),
            Some("$file".to_string()),
        ),
    ];
    assert_eq!(chat.bottom_pane.composer_text(), text);
    assert_eq!(chat.bottom_pane.composer_text_elements(), expected_elements);
    assert_eq!(
        chat.bottom_pane.composer_local_image_paths(),
        vec![local_images[0].path.clone()],
    );
    assert_eq!(chat.bottom_pane.take_mention_bindings(), mention_bindings);

    let cells = drain_insert_history(&mut rx);
    let warning = cells
        .last()
        .map(|lines| lines_to_single_string(lines))
        .expect("expected warning cell");
    assert!(
        warning.contains("does not support image inputs"),
        "expected image warning, got: {warning:?}"
    );
}

#[tokio::test]
async fn blocked_image_restore_with_remote_images_keeps_local_placeholder_mapping() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    let first_placeholder = "[Image #2]";
    let second_placeholder = "[Image #3]";
    let text = format!("{first_placeholder} first\n{second_placeholder} second");
    let second_start = text.find(second_placeholder).expect("second placeholder");
    let text_elements = vec![
        TextElement::new(
            (0..first_placeholder.len()).into(),
            Some(first_placeholder.to_string()),
        ),
        TextElement::new(
            (second_start..second_start + second_placeholder.len()).into(),
            Some(second_placeholder.to_string()),
        ),
    ];
    let local_images = vec![
        LocalImageAttachment {
            placeholder: first_placeholder.to_string(),
            path: PathBuf::from("/tmp/blocked-first.png"),
        },
        LocalImageAttachment {
            placeholder: second_placeholder.to_string(),
            path: PathBuf::from("/tmp/blocked-second.png"),
        },
    ];
    let remote_image_urls = vec!["https://example.com/blocked-remote.png".to_string()];

    chat.restore_blocked_image_submission(
        text.clone(),
        text_elements.clone(),
        local_images.clone(),
        Vec::new(),
        remote_image_urls.clone(),
    );

    assert_eq!(chat.bottom_pane.composer_text(), text);
    assert_eq!(chat.bottom_pane.composer_text_elements(), text_elements);
    assert_eq!(chat.bottom_pane.composer_local_images(), local_images);
    assert_eq!(chat.remote_image_urls(), remote_image_urls);
}

#[tokio::test]
async fn queued_restore_with_remote_images_keeps_local_placeholder_mapping() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    let first_placeholder = "[Image #2]";
    let second_placeholder = "[Image #3]";
    let text = format!("{first_placeholder} first\n{second_placeholder} second");
    let second_start = text.find(second_placeholder).expect("second placeholder");
    let text_elements = vec![
        TextElement::new(
            (0..first_placeholder.len()).into(),
            Some(first_placeholder.to_string()),
        ),
        TextElement::new(
            (second_start..second_start + second_placeholder.len()).into(),
            Some(second_placeholder.to_string()),
        ),
    ];
    let local_images = vec![
        LocalImageAttachment {
            placeholder: first_placeholder.to_string(),
            path: PathBuf::from("/tmp/queued-first.png"),
        },
        LocalImageAttachment {
            placeholder: second_placeholder.to_string(),
            path: PathBuf::from("/tmp/queued-second.png"),
        },
    ];
    let remote_image_urls = vec!["https://example.com/queued-remote.png".to_string()];

    chat.restore_user_message_to_composer(UserMessage {
        text: text.clone(),
        local_images: local_images.clone(),
        remote_image_urls: remote_image_urls.clone(),
        text_elements: text_elements.clone(),
        mention_bindings: Vec::new(),
    });

    assert_eq!(chat.bottom_pane.composer_text(), text);
    assert_eq!(chat.bottom_pane.composer_cursor(), text.len());
    assert_eq!(chat.bottom_pane.composer_text_elements(), text_elements);
    assert_eq!(chat.bottom_pane.composer_local_images(), local_images);
    assert_eq!(chat.remote_image_urls(), remote_image_urls);
}

#[tokio::test]
async fn restored_message_preserves_existing_composer_draft_and_attachments() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let retry_image = PathBuf::from("/tmp/retry.png");
    let draft_image = PathBuf::from("/tmp/draft.png");
    let paste_placeholder = "[Pasted Content 5 chars]";
    let draft_text = format!("[Image #1] {paste_placeholder} draft");
    chat.bottom_pane.set_composer_text(
        draft_text,
        vec![TextElement::new(
            (0.."[Image #1]".len()).into(),
            Some("[Image #1]".to_string()),
        )],
        vec![draft_image.clone()],
    );
    chat.bottom_pane
        .set_composer_pending_pastes(vec![(paste_placeholder.to_string(), "hello".to_string())]);

    chat.restore_user_message_to_composer(UserMessage {
        text: "[Image #1] retry prompt".to_string(),
        local_images: vec![LocalImageAttachment {
            placeholder: "[Image #1]".to_string(),
            path: retry_image.clone(),
        }],
        remote_image_urls: Vec::new(),
        text_elements: vec![TextElement::new(
            (0.."[Image #1]".len()).into(),
            Some("[Image #1]".to_string()),
        )],
        mention_bindings: Vec::new(),
    });

    assert_eq!(
        chat.bottom_pane.composer_text(),
        format!("[Image #1] retry prompt\n[Image #2] {paste_placeholder} draft")
    );
    assert_eq!(
        chat.bottom_pane.composer_local_image_paths(),
        vec![retry_image, draft_image]
    );
    assert_eq!(
        chat.bottom_pane.composer_pending_pastes(),
        vec![(paste_placeholder.to_string(), "hello".to_string())]
    );
}

#[tokio::test]
async fn interrupted_turn_restore_keeps_active_mode_for_resubmission() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(Some("gpt-5")).await;
    chat.thread_id = Some(ThreadId::new());
    chat.set_feature_enabled(Feature::CollaborationModes, /*enabled*/ true);

    let plan_mask = collaboration_modes::plan_mask(chat.model_catalog.as_ref())
        .expect("expected plan collaboration mode");
    let expected_mode = plan_mask
        .mode
        .expect("expected mode kind on plan collaboration mode");

    chat.set_collaboration_mask(plan_mask);
    chat.on_task_started();
    chat.input_queue.queued_user_messages.push_back(
        UserMessage {
            text: "Implement the plan.".to_string(),
            local_images: Vec::new(),
            remote_image_urls: Vec::new(),
            text_elements: Vec::new(),
            mention_bindings: Vec::new(),
        }
        .into(),
    );
    chat.refresh_pending_input_preview();

    handle_turn_interrupted(&mut chat, "turn-1");

    assert_eq!(chat.bottom_pane.composer_text(), "Implement the plan.");
    assert!(chat.input_queue.queued_user_messages.is_empty());
    assert_eq!(chat.active_collaboration_mode_kind(), expected_mode);

    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    match next_submit_op(&mut op_rx) {
        Op::UserTurn {
            collaboration_mode: Some(CollaborationMode { mode, .. }),
            personality: None,
            ..
        } => assert_eq!(mode, expected_mode),
        other => {
            panic!("expected Op::UserTurn with active mode, got {other:?}")
        }
    }
    assert_eq!(chat.active_collaboration_mode_kind(), expected_mode);
}

#[tokio::test]
async fn remap_placeholders_uses_attachment_labels() {
    let placeholder_one = "[Image #1]";
    let placeholder_two = "[Image #2]";
    let text = format!("{placeholder_two} before {placeholder_one}");
    let elements = vec![
        TextElement::new(
            (0..placeholder_two.len()).into(),
            Some(placeholder_two.to_string()),
        ),
        TextElement::new(
            ("[Image #2] before ".len().."[Image #2] before [Image #1]".len()).into(),
            Some(placeholder_one.to_string()),
        ),
    ];

    let attachments = vec![
        LocalImageAttachment {
            placeholder: placeholder_one.to_string(),
            path: PathBuf::from("/tmp/one.png"),
        },
        LocalImageAttachment {
            placeholder: placeholder_two.to_string(),
            path: PathBuf::from("/tmp/two.png"),
        },
    ];
    let message = UserMessage {
        text,
        text_elements: elements,
        local_images: attachments,
        remote_image_urls: vec!["https://example.com/a.png".to_string()],
        mention_bindings: Vec::new(),
    };
    let mut next_label = 3usize;
    let remapped = remap_placeholders_for_message(message, &mut next_label);

    assert_eq!(remapped.text, "[Image #4] before [Image #3]");
    assert_eq!(
        remapped.text_elements,
        vec![
            TextElement::new(
                (0.."[Image #4]".len()).into(),
                Some("[Image #4]".to_string()),
            ),
            TextElement::new(
                ("[Image #4] before ".len().."[Image #4] before [Image #3]".len()).into(),
                Some("[Image #3]".to_string()),
            ),
        ]
    );
    assert_eq!(
        remapped.local_images,
        vec![
            LocalImageAttachment {
                placeholder: "[Image #3]".to_string(),
                path: PathBuf::from("/tmp/one.png"),
            },
            LocalImageAttachment {
                placeholder: "[Image #4]".to_string(),
                path: PathBuf::from("/tmp/two.png"),
            },
        ]
    );
    assert_eq!(
        remapped.remote_image_urls,
        vec!["https://example.com/a.png".to_string()]
    );
}

#[tokio::test]
async fn remap_placeholders_uses_byte_ranges_when_placeholder_missing() {
    let placeholder_one = "[Image #1]";
    let placeholder_two = "[Image #2]";
    let text = format!("{placeholder_two} before {placeholder_one}");
    let elements = vec![
        TextElement::new((0..placeholder_two.len()).into(), /*placeholder*/ None),
        TextElement::new(
            ("[Image #2] before ".len().."[Image #2] before [Image #1]".len()).into(),
            /*placeholder*/ None,
        ),
    ];

    let attachments = vec![
        LocalImageAttachment {
            placeholder: placeholder_one.to_string(),
            path: PathBuf::from("/tmp/one.png"),
        },
        LocalImageAttachment {
            placeholder: placeholder_two.to_string(),
            path: PathBuf::from("/tmp/two.png"),
        },
    ];
    let message = UserMessage {
        text,
        text_elements: elements,
        local_images: attachments,
        remote_image_urls: Vec::new(),
        mention_bindings: Vec::new(),
    };
    let mut next_label = 3usize;
    let remapped = remap_placeholders_for_message(message, &mut next_label);

    assert_eq!(remapped.text, "[Image #4] before [Image #3]");
    assert_eq!(
        remapped.text_elements,
        vec![
            TextElement::new(
                (0.."[Image #4]".len()).into(),
                Some("[Image #4]".to_string()),
            ),
            TextElement::new(
                ("[Image #4] before ".len().."[Image #4] before [Image #3]".len()).into(),
                Some("[Image #3]".to_string()),
            ),
        ]
    );
    assert_eq!(
        remapped.local_images,
        vec![
            LocalImageAttachment {
                placeholder: "[Image #3]".to_string(),
                path: PathBuf::from("/tmp/one.png"),
            },
            LocalImageAttachment {
                placeholder: "[Image #4]".to_string(),
                path: PathBuf::from("/tmp/two.png"),
            },
        ]
    );
}

#[tokio::test]
async fn empty_enter_during_task_does_not_queue() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    // Simulate running task so submissions would normally be queued.
    chat.bottom_pane.set_task_running(/*running*/ true);

    // Press Enter with an empty composer.
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    // Ensure nothing was queued.
    assert!(chat.input_queue.queued_user_messages.is_empty());
}

fn interrupted_history(
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
    prompt: &str,
) -> (bool, String) {
    let mut saw_prompt = false;
    let mut history = Vec::new();
    while let Ok(event) = rx.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event {
            if let Some(cell) = cell.as_any().downcast_ref::<UserHistoryCell>() {
                assert_eq!(cell.message, prompt);
                saw_prompt = true;
            }
            history.push(lines_to_single_string(&cell.display_lines(/*width*/ 80)));
        }
    }
    let history = history.join("\n");
    assert!(
        history.contains("Conversation interrupted - tell the model what to do differently."),
        "expected normal interruption notice, got {history:?}"
    );
    (saw_prompt, history)
}

#[tokio::test]
async fn output_free_esc_interrupt_keeps_prompt_and_opens_blank_composer() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let prompt = "revise this prompt";
    chat.thread_id = Some(ThreadId::new());
    chat.submit_user_message(UserMessage::from(prompt));
    assert_matches!(next_submit_op(&mut op_rx), Op::UserTurn { .. });
    handle_turn_started(&mut chat, "turn-1");
    chat.bottom_pane.ensure_status_indicator();

    let esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    assert!(chat.bottom_pane.should_interrupt_running_task(esc));
    chat.handle_key_event(esc);

    let mut saw_prompt = false;
    loop {
        match rx.try_recv() {
            Ok(AppEvent::InsertHistoryCell(cell)) => {
                if let Some(cell) = cell.as_any().downcast_ref::<UserHistoryCell>() {
                    assert_eq!(cell.message, prompt);
                    saw_prompt = true;
                }
            }
            Ok(AppEvent::CodexOp(Op::Interrupt)) => break,
            Ok(_) => {}
            Err(error) => panic!("expected Esc interrupt command, got {error:?}"),
        }
    }

    handle_turn_interrupted(&mut chat, "turn-1");

    let (prompt_after_interrupt, _) = interrupted_history(&mut rx, prompt);
    assert!(saw_prompt || prompt_after_interrupt);
    assert!(chat.bottom_pane.composer_is_empty());
}

#[tokio::test]
async fn output_free_ctrl_c_interrupt_keeps_prompt_and_opens_blank_composer() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let prompt = "revise this prompt";
    chat.thread_id = Some(ThreadId::new());
    chat.submit_user_message(UserMessage::from(prompt));
    assert_matches!(next_submit_op(&mut op_rx), Op::UserTurn { .. });
    handle_turn_started(&mut chat, "turn-1");

    chat.handle_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));

    next_interrupt_op(&mut op_rx);
    handle_turn_interrupted(&mut chat, "turn-1");

    let (saw_prompt, interrupted_history) = interrupted_history(&mut rx, prompt);
    assert!(saw_prompt);
    assert!(chat.bottom_pane.composer_is_empty());
    insta::assert_snapshot!(
        "output_free_ctrl_c_interrupt_keeps_prompt_and_blank_composer",
        format!(
            "history:\n{interrupted_history}\ncomposer:\n{}",
            chat.bottom_pane.composer_text()
        )
    );
}

#[tokio::test]
async fn pending_steer_esc_does_not_steal_vim_insert_escape() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);

    chat.bottom_pane.set_task_running(/*running*/ true);
    chat.input_queue
        .pending_steers
        .push_back(pending_steer("queued steer"));
    chat.toggle_vim_mode_and_notify();
    chat.handle_key_event(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE));

    assert!(chat.should_handle_vim_insert_escape(esc));
    chat.handle_key_event(esc);

    assert!(!chat.should_handle_vim_insert_escape(esc));
    assert_eq!(chat.input_queue.pending_steers.len(), 1);
    assert!(!chat.input_queue.submit_pending_steers_after_interrupt);
    assert!(op_rx.try_recv().is_err());

    chat.handle_key_event(esc);

    match op_rx.try_recv() {
        Ok(Op::Interrupt) => {}
        other => panic!("expected Op::Interrupt, got {other:?}"),
    }
    assert!(chat.input_queue.submit_pending_steers_after_interrupt);
}

#[tokio::test]
async fn pending_steer_interrupt_uses_remapped_binding() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let mut keymap = crate::keymap::RuntimeKeymap::defaults();
    keymap.chat.interrupt_turn = vec![crate::key_hint::plain(KeyCode::F(12))];
    chat.chat_keymap = keymap.chat.clone();
    chat.bottom_pane.set_keymap_bindings(&keymap);
    chat.bottom_pane.set_task_running(/*running*/ true);
    chat.input_queue
        .pending_steers
        .push_back(pending_steer("queued steer"));

    chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

    assert!(!chat.input_queue.submit_pending_steers_after_interrupt);
    assert!(op_rx.try_recv().is_err());

    chat.handle_key_event(KeyEvent::new(KeyCode::F(12), KeyModifiers::NONE));

    match op_rx.try_recv() {
        Ok(Op::Interrupt) => {}
        other => panic!("expected Op::Interrupt, got {other:?}"),
    }
    assert!(chat.input_queue.submit_pending_steers_after_interrupt);
}

#[tokio::test]
async fn restore_thread_input_state_applies_running_state_policy() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    chat.set_feature_enabled(Feature::PreventIdleSleep, /*enabled*/ true);

    let pending_history = UserMessageHistoryRecord::Override(UserMessageHistoryOverride {
        text: "submitted history".to_string(),
        text_elements: Vec::new(),
    });
    let queued_history = UserMessageHistoryRecord::Override(UserMessageHistoryOverride {
        text: "queued history".to_string(),
        text_elements: Vec::new(),
    });
    let input_state = ThreadInputState {
        composer: Some(ThreadComposerState {
            text: "composer draft".to_string(),
            ..Default::default()
        }),
        safety_buffering_prompt: Some(UserMessage::from("buffered prompt")),
        pending_steers: VecDeque::from([UserMessage::from("submitted to the interrupted turn")]),
        pending_steer_history_records: VecDeque::from([pending_history.clone()]),
        pending_steer_compare_keys: VecDeque::new(),
        rejected_steers_queue: VecDeque::new(),
        rejected_steer_history_records: VecDeque::new(),
        queued_user_messages: VecDeque::from([UserMessage::from("already queued").into()]),
        queued_user_message_history_records: VecDeque::from([queued_history.clone()]),
        user_turn_pending_start: true,
        submit_pending_steers_after_interrupt: true,
        current_collaboration_mode: chat.current_collaboration_mode.clone(),
        active_collaboration_mask: chat.active_collaboration_mask.clone(),
        task_running: true,
        agent_turn_running: true,
    };
    chat.restore_thread_input_state(
        Some(input_state.clone()),
        ThreadInputStateRestoreMode {
            preserve_in_flight_turn: true,
        },
    );

    assert!(chat.turn_lifecycle.agent_turn_running);
    assert!(chat.turn_lifecycle.sleep_inhibitor.is_turn_running());
    assert!(chat.bottom_pane.is_task_running());
    assert!(chat.input_queue.user_turn_pending_start);
    assert!(chat.input_queue.submit_pending_steers_after_interrupt);
    let captured_input_state = chat
        .capture_thread_input_state()
        .expect("thread input state");
    assert!(captured_input_state.submit_pending_steers_after_interrupt);
    assert_eq!(
        captured_input_state.safety_buffering_prompt,
        Some(UserMessage::from("buffered prompt"))
    );
    assert_eq!(chat.input_queue.pending_steers.len(), 1);
    assert_eq!(
        chat.safety_buffering_prompt,
        Some(UserMessage::from("buffered prompt"))
    );

    chat.restore_thread_input_state(
        Some(input_state),
        ThreadInputStateRestoreMode {
            preserve_in_flight_turn: false,
        },
    );

    assert!(!chat.turn_lifecycle.agent_turn_running);
    assert!(!chat.turn_lifecycle.sleep_inhibitor.is_turn_running());
    assert!(!chat.bottom_pane.is_task_running());
    assert!(!chat.input_queue.user_turn_pending_start);
    assert!(!chat.input_queue.submit_pending_steers_after_interrupt);
    assert!(chat.input_queue.pending_steers.is_empty());
    assert_eq!(chat.bottom_pane.composer_text(), "composer draft");
    assert_eq!(
        chat.safety_buffering_prompt,
        Some(UserMessage::from("buffered prompt"))
    );
    assert_eq!(
        chat.queued_user_message_texts(),
        vec!["submitted to the interrupted turn", "already queued"]
    );
    assert_eq!(
        chat.input_queue.queued_user_message_history_records,
        VecDeque::from([pending_history, queued_history])
    );
    assert!(chat.maybe_send_next_queued_input());
    assert_matches!(next_submit_op(&mut op_rx), Op::UserTurn { .. });
    assert_eq!(chat.queued_user_message_texts(), vec!["already queued"]);

    chat.restore_thread_input_state(
        /*input_state*/ None,
        ThreadInputStateRestoreMode {
            preserve_in_flight_turn: true,
        },
    );

    assert!(!chat.turn_lifecycle.agent_turn_running);
    assert!(!chat.turn_lifecycle.sleep_inhibitor.is_turn_running());
    assert!(!chat.bottom_pane.is_task_running());
    assert_eq!(chat.safety_buffering_prompt, None);
}

#[tokio::test]
async fn alt_up_edits_most_recent_queued_message() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.chat_keymap.edit_queued_message = vec![crate::key_hint::alt(KeyCode::Up)];
    chat.queued_message_edit_hint_binding = Some(crate::key_hint::alt(KeyCode::Up));
    chat.bottom_pane
        .set_queued_message_edit_binding(chat.queued_message_edit_hint_binding);

    // Simulate a running task so messages would normally be queued.
    chat.bottom_pane.set_task_running(/*running*/ true);

    // Seed two queued messages.
    chat.input_queue
        .queued_user_messages
        .push_back(UserMessage::from("first queued".to_string()).into());
    chat.input_queue
        .queued_user_messages
        .push_back(UserMessage::from("second queued".to_string()).into());
    chat.refresh_pending_input_preview();

    // Press Alt+Up to edit the most recent (last) queued message.
    chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT));

    // Composer should now contain the last queued message.
    assert_eq!(
        chat.bottom_pane.composer_text(),
        "second queued".to_string()
    );
    // And the queue should now contain only the remaining (older) item.
    assert_eq!(chat.input_queue.queued_user_messages.len(), 1);
    assert_eq!(
        chat.input_queue.queued_user_messages.front().unwrap().text,
        "first queued"
    );
}

#[tokio::test]
async fn unbound_queued_message_edit_does_not_fall_back_to_alt_up() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.chat_keymap.edit_queued_message = Vec::new();
    chat.queued_message_edit_hint_binding = None;
    chat.bottom_pane
        .set_queued_message_edit_binding(chat.queued_message_edit_hint_binding);
    chat.bottom_pane.set_task_running(/*running*/ true);
    chat.input_queue
        .queued_user_messages
        .push_back(UserMessage::from("queued".to_string()).into());
    chat.refresh_pending_input_preview();

    chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT));

    assert!(chat.bottom_pane.composer_text().is_empty());
    assert_eq!(chat.input_queue.queued_user_messages.len(), 1);
}

#[tokio::test]
async fn shift_left_edits_most_recent_queued_message_in_apple_terminal() {
    assert_shift_left_edits_most_recent_queued_message_for_terminal(TerminalInfo {
        name: TerminalName::AppleTerminal,
        term_program: None,
        version: None,
        term: None,
        multiplexer: None,
    })
    .await;
}

#[tokio::test]
async fn shift_left_edits_most_recent_queued_message_in_warp_terminal() {
    assert_shift_left_edits_most_recent_queued_message_for_terminal(TerminalInfo {
        name: TerminalName::WarpTerminal,
        term_program: None,
        version: None,
        term: None,
        multiplexer: None,
    })
    .await;
}

#[tokio::test]
async fn shift_left_edits_most_recent_queued_message_in_vscode_terminal() {
    assert_shift_left_edits_most_recent_queued_message_for_terminal(TerminalInfo {
        name: TerminalName::VsCode,
        term_program: None,
        version: None,
        term: None,
        multiplexer: None,
    })
    .await;
}

#[tokio::test]
async fn shift_left_edits_most_recent_queued_message_in_tmux() {
    assert_shift_left_edits_most_recent_queued_message_for_terminal(TerminalInfo {
        name: TerminalName::Iterm2,
        term_program: None,
        version: None,
        term: None,
        multiplexer: Some(Multiplexer::Tmux { version: None }),
    })
    .await;
}

#[test]
fn queued_message_edit_binding_mapping_covers_special_terminals_and_tmux() {
    assert_eq!(
        queued_message_edit_binding_for_terminal(TerminalInfo {
            name: TerminalName::AppleTerminal,
            term_program: None,
            version: None,
            term: None,
            multiplexer: None,
        }),
        crate::key_hint::shift(KeyCode::Left)
    );
    assert_eq!(
        queued_message_edit_binding_for_terminal(TerminalInfo {
            name: TerminalName::WarpTerminal,
            term_program: None,
            version: None,
            term: None,
            multiplexer: None,
        }),
        crate::key_hint::shift(KeyCode::Left)
    );
    assert_eq!(
        queued_message_edit_binding_for_terminal(TerminalInfo {
            name: TerminalName::VsCode,
            term_program: None,
            version: None,
            term: None,
            multiplexer: None,
        }),
        crate::key_hint::shift(KeyCode::Left)
    );
    assert_eq!(
        queued_message_edit_binding_for_terminal(TerminalInfo {
            name: TerminalName::Iterm2,
            term_program: None,
            version: None,
            term: None,
            multiplexer: Some(Multiplexer::Tmux { version: None }),
        }),
        crate::key_hint::shift(KeyCode::Left)
    );
    assert_eq!(
        queued_message_edit_binding_for_terminal(TerminalInfo {
            name: TerminalName::Iterm2,
            term_program: None,
            version: None,
            term: None,
            multiplexer: None,
        }),
        crate::key_hint::alt(KeyCode::Up)
    );
}

/// Pressing Up to recall the most recent history entry and immediately queuing
/// it while a task is running should always enqueue the same text, even when it
/// is queued repeatedly.
#[tokio::test]
async fn enqueueing_history_prompt_multiple_times_is_stable() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());

    // Submit an initial prompt to seed history.
    chat.bottom_pane
        .set_composer_text("repeat me".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    // Simulate an active task so further submissions are queued.
    chat.bottom_pane.set_task_running(/*running*/ true);

    for _ in 0..3 {
        // Recall the prompt from history and ensure it is what we expect.
        chat.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        assert_eq!(chat.bottom_pane.composer_text(), "repeat me");

        // Queue the prompt while the task is running.
        chat.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    }

    assert_eq!(chat.input_queue.queued_user_messages.len(), 3);
    for message in chat.input_queue.queued_user_messages.iter() {
        assert_eq!(message.text, "repeat me");
    }
}

#[tokio::test]
async fn submit_user_message_ignores_inaccessible_app_mentions_from_bindings() {
    let (mut chat, _rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());
    set_chatgpt_auth(&mut chat);
    chat.config
        .features
        .enable(Feature::Apps)
        .expect("test config should allow feature update");

    chat.on_connectors_loaded(
        Ok(ConnectorsSnapshot {
            connectors: vec![AppInfo {
                id: "arabica_uae".to_string(),
                name: "% Arabica UAE".to_string(),
                description: Some("Directory-only app".to_string()),
                logo_url: None,
                logo_url_dark: None,
                icon_assets: None,
                icon_dark_assets: None,
                distribution_channel: None,
                branding: None,
                app_metadata: None,
                labels: None,
                install_url: Some("https://example.test/arabica".to_string()),
                is_accessible: false,
                is_enabled: true,
                plugin_display_names: Vec::new(),
            }],
        }),
        /*is_final*/ false,
    );

    chat.submit_user_message(UserMessage {
        text: "$arabica-uae".to_string(),
        local_images: Vec::new(),
        remote_image_urls: Vec::new(),
        text_elements: Vec::new(),
        mention_bindings: vec![MentionBinding {
            sigil: '$',
            mention: "arabica-uae".to_string(),
            path: "app://arabica_uae".to_string(),
        }],
    });

    let items = match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => items,
        other => panic!("expected Op::UserTurn, got {other:?}"),
    };
    assert_eq!(
        items,
        vec![UserInput::Text {
            text: "$arabica-uae".to_string(),
            text_elements: Vec::new(),
        }]
    );
}

#[test]
fn user_message_display_from_inputs_matches_flattened_user_message_shape() {
    let local_image = PathBuf::from("/tmp/local.png");
    let rendered = ChatWidget::user_message_display_from_inputs(&[
        UserInput::Text {
            text: "hello ".to_string(),
            text_elements: vec![TextElement::new((0..5).into(), /*placeholder*/ None).into()],
        },
        UserInput::Image {
            url: "https://example.com/remote.png".to_string(),
            detail: None,
        },
        UserInput::LocalImage {
            path: local_image.clone(),
            detail: None,
        },
        UserInput::Skill {
            name: "demo".to_string(),
            path: PathBuf::from("/tmp/skill/SKILL.md"),
        },
        UserInput::Mention {
            name: "repo".to_string(),
            path: "app://repo".to_string(),
        },
        UserInput::Text {
            text: "world".to_string(),
            text_elements: vec![TextElement::new((0..5).into(), Some("planet".to_string())).into()],
        },
    ]);

    assert_eq!(
        rendered,
        ChatWidget::user_message_display_from_parts(
            "hello world".to_string(),
            vec![
                TextElement::new((0..5).into(), Some("hello".to_string())),
                TextElement::new((6..11).into(), Some("planet".to_string())),
            ],
            vec![local_image],
            vec!["https://example.com/remote.png".to_string()],
        )
    );
}

#[test]
fn user_message_display_from_inputs_hides_prompt_context() {
    let raw_message = "# Context from my IDE setup:\n\n## Active file: src/lib.rs\n\n## My request for Codex:\nAsk $figma";
    let mention_start = raw_message.find("$figma").expect("mention in raw message");
    let rendered = ChatWidget::user_message_display_from_inputs(&[UserInput::Text {
        text: raw_message.to_string(),
        text_elements: vec![
            TextElement::new(
                (mention_start..mention_start + "$figma".len()).into(),
                Some("$figma".to_string()),
            )
            .into(),
        ],
    }]);

    assert_eq!(
        rendered,
        ChatWidget::user_message_display_from_parts(
            "Ask $figma".to_string(),
            vec![TextElement::new((4..10).into(), Some("$figma".to_string()))],
            Vec::new(),
            Vec::new(),
        )
    );
}

#[tokio::test]
async fn committed_user_message_with_hidden_prompt_context_renders_local_images() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let local_image = PathBuf::from("/tmp/context-image.png");
    let raw_message =
        "# Context from my IDE setup:\n\n## Active file: src/lib.rs\n\n## My request for Codex:\n";

    complete_user_message_for_inputs(
        &mut chat,
        "user-1",
        vec![
            UserInput::Text {
                text: raw_message.to_string(),
                text_elements: Vec::new(),
            },
            UserInput::LocalImage {
                path: local_image.clone(),
                detail: None,
            },
        ],
    );

    let mut user_cell = None;
    while let Ok(event) = rx.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event
            && let Some(cell) = cell.as_any().downcast_ref::<UserHistoryCell>()
        {
            user_cell = Some((cell.message.clone(), cell.local_image_paths.clone()));
            break;
        }
    }

    let (message, local_images) = user_cell.expect("expected user history cell");
    assert_eq!(message, "");
    assert_eq!(local_images, vec![local_image]);
}

#[tokio::test]
async fn interrupt_restores_queued_messages_into_composer() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    // Simulate a running task to enable queuing of user inputs.
    chat.bottom_pane.set_task_running(/*running*/ true);

    // Queue two user messages while the task is running.
    chat.input_queue
        .queued_user_messages
        .push_back(UserMessage::from("first queued".to_string()).into());
    chat.input_queue
        .queued_user_messages
        .push_back(UserMessage::from("second queued".to_string()).into());
    chat.refresh_pending_input_preview();

    // Deliver an interrupted turn notification as if Esc was pressed.
    handle_turn_interrupted(&mut chat, "turn-1");

    // Composer should now contain the queued messages joined by newlines, in order.
    assert_eq!(
        chat.bottom_pane.composer_text(),
        "first queued\nsecond queued"
    );

    // Queue should be cleared and no new user input should have been auto-submitted.
    assert!(chat.input_queue.queued_user_messages.is_empty());
    assert!(
        op_rx.try_recv().is_err(),
        "unexpected outbound op after interrupt"
    );

    // Drain rx to avoid unused warnings.
    let _ = drain_insert_history(&mut rx);
}

#[tokio::test]
async fn interrupt_prepends_queued_messages_before_existing_composer_text() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.bottom_pane.set_task_running(/*running*/ true);
    chat.bottom_pane
        .set_composer_text("current draft".to_string(), Vec::new(), Vec::new());

    chat.input_queue
        .queued_user_messages
        .push_back(UserMessage::from("first queued".to_string()).into());
    chat.input_queue
        .queued_user_messages
        .push_back(UserMessage::from("second queued".to_string()).into());
    chat.refresh_pending_input_preview();

    handle_turn_interrupted(&mut chat, "turn-1");

    assert_eq!(
        chat.bottom_pane.composer_text(),
        "first queued\nsecond queued\ncurrent draft"
    );
    assert!(chat.input_queue.queued_user_messages.is_empty());
    assert!(
        op_rx.try_recv().is_err(),
        "unexpected outbound op after interrupt"
    );

    let _ = drain_insert_history(&mut rx);
}
