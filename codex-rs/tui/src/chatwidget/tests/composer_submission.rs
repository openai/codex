use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn submission_preserves_text_elements_and_local_images() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    let conversation_id = ThreadId::new();
    let rollout_file = NamedTempFile::new().unwrap();
    let configured = codex_protocol::protocol::SessionConfiguredEvent {
        session_id: conversation_id,
        forked_from_id: None,
        thread_name: None,
        model: "test-model".to_string(),
        model_provider_id: "test-provider".to_string(),
        service_tier: None,
        approval_policy: AskForApproval::Never,
        approvals_reviewer: ApprovalsReviewer::User,
        sandbox_policy: SandboxPolicy::new_read_only_policy(),
        permission_profile: None,
        cwd: test_path_buf("/home/user/project").abs(),
        reasoning_effort: Some(ReasoningEffortConfig::default()),
        history_log_id: 0,
        history_entry_count: 0,
        initial_messages: None,
        network_proxy: None,
        rollout_path: Some(rollout_file.path().to_path_buf()),
    };
    chat.handle_codex_event(Event {
        id: "initial".into(),
        msg: EventMsg::SessionConfigured(configured),
    });
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
            path: local_images[0].clone()
        }
    );
    assert_eq!(
        items[1],
        UserInput::Text {
            text: text.clone(),
            text_elements: text_elements.clone(),
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
async fn submission_with_remote_and_local_images_keeps_local_placeholder_numbering() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    let conversation_id = ThreadId::new();
    let rollout_file = NamedTempFile::new().unwrap();
    let configured = codex_protocol::protocol::SessionConfiguredEvent {
        session_id: conversation_id,
        forked_from_id: None,
        thread_name: None,
        model: "test-model".to_string(),
        model_provider_id: "test-provider".to_string(),
        service_tier: None,
        approval_policy: AskForApproval::Never,
        approvals_reviewer: ApprovalsReviewer::User,
        sandbox_policy: SandboxPolicy::new_read_only_policy(),
        permission_profile: None,
        cwd: test_path_buf("/home/user/project").abs(),
        reasoning_effort: Some(ReasoningEffortConfig::default()),
        history_log_id: 0,
        history_entry_count: 0,
        initial_messages: None,
        network_proxy: None,
        rollout_path: Some(rollout_file.path().to_path_buf()),
    };
    chat.handle_codex_event(Event {
        id: "initial".into(),
        msg: EventMsg::SessionConfigured(configured),
    });
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
            image_url: remote_url.clone(),
        }
    );
    assert_eq!(
        items[1],
        UserInput::LocalImage {
            path: local_images[0].clone(),
        }
    );
    assert_eq!(
        items[2],
        UserInput::Text {
            text: text.clone(),
            text_elements: text_elements.clone(),
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

    let conversation_id = ThreadId::new();
    let rollout_file = NamedTempFile::new().unwrap();
    let configured = codex_protocol::protocol::SessionConfiguredEvent {
        session_id: conversation_id,
        forked_from_id: None,
        thread_name: None,
        model: "test-model".to_string(),
        model_provider_id: "test-provider".to_string(),
        service_tier: None,
        approval_policy: AskForApproval::Never,
        approvals_reviewer: ApprovalsReviewer::User,
        sandbox_policy: SandboxPolicy::new_read_only_policy(),
        permission_profile: None,
        cwd: test_path_buf("/home/user/project").abs(),
        reasoning_effort: Some(ReasoningEffortConfig::default()),
        history_log_id: 0,
        history_entry_count: 0,
        initial_messages: None,
        network_proxy: None,
        rollout_path: Some(rollout_file.path().to_path_buf()),
    };
    chat.handle_codex_event(Event {
        id: "initial".into(),
        msg: EventMsg::SessionConfigured(configured),
    });
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
            image_url: remote_url.clone(),
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

    let conversation_id = ThreadId::new();
    let rollout_file = NamedTempFile::new().unwrap();
    let configured = codex_protocol::protocol::SessionConfiguredEvent {
        session_id: conversation_id,
        forked_from_id: None,
        thread_name: None,
        model: "test-model".to_string(),
        model_provider_id: "test-provider".to_string(),
        service_tier: None,
        approval_policy: AskForApproval::Never,
        approvals_reviewer: ApprovalsReviewer::User,
        sandbox_policy: SandboxPolicy::new_read_only_policy(),
        permission_profile: None,
        cwd: test_path_buf("/home/user/project").abs(),
        reasoning_effort: Some(ReasoningEffortConfig::default()),
        history_log_id: 0,
        history_entry_count: 0,
        initial_messages: None,
        network_proxy: None,
        rollout_path: Some(rollout_file.path().to_path_buf()),
    };
    chat.handle_codex_event(Event {
        id: "initial".into(),
        msg: EventMsg::SessionConfigured(configured),
    });
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

    let conversation_id = ThreadId::new();
    let rollout_file = NamedTempFile::new().unwrap();
    let configured = codex_protocol::protocol::SessionConfiguredEvent {
        session_id: conversation_id,
        forked_from_id: None,
        thread_name: None,
        model: "test-model".to_string(),
        model_provider_id: "test-provider".to_string(),
        service_tier: None,
        approval_policy: AskForApproval::Never,
        approvals_reviewer: ApprovalsReviewer::User,
        sandbox_policy: SandboxPolicy::new_read_only_policy(),
        permission_profile: None,
        cwd: test_path_buf("/home/user/project").abs(),
        reasoning_effort: Some(ReasoningEffortConfig::default()),
        history_log_id: 0,
        history_entry_count: 0,
        initial_messages: None,
        network_proxy: None,
        rollout_path: Some(rollout_file.path().to_path_buf()),
    };
    chat.handle_codex_event(Event {
        id: "initial".into(),
        msg: EventMsg::SessionConfigured(configured),
    });
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

    let conversation_id = ThreadId::new();
    let rollout_file = NamedTempFile::new().unwrap();
    let configured = codex_protocol::protocol::SessionConfiguredEvent {
        session_id: conversation_id,
        forked_from_id: None,
        thread_name: None,
        model: "test-model".to_string(),
        model_provider_id: "test-provider".to_string(),
        service_tier: None,
        approval_policy: AskForApproval::Never,
        approvals_reviewer: ApprovalsReviewer::User,
        sandbox_policy: SandboxPolicy::new_read_only_policy(),
        permission_profile: None,
        cwd: test_path_buf("/home/user/project").abs(),
        reasoning_effort: Some(ReasoningEffortConfig::default()),
        history_log_id: 0,
        history_entry_count: 0,
        initial_messages: None,
        network_proxy: None,
        rollout_path: Some(rollout_file.path().to_path_buf()),
    };
    chat.handle_codex_event(Event {
        id: "initial".into(),
        msg: EventMsg::SessionConfigured(configured),
    });
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

    let conversation_id = ThreadId::new();
    let rollout_file = NamedTempFile::new().unwrap();
    let configured = codex_protocol::protocol::SessionConfiguredEvent {
        session_id: conversation_id,
        forked_from_id: None,
        thread_name: None,
        model: "test-model".to_string(),
        model_provider_id: "test-provider".to_string(),
        service_tier: None,
        approval_policy: AskForApproval::Never,
        approvals_reviewer: ApprovalsReviewer::User,
        sandbox_policy: SandboxPolicy::new_read_only_policy(),
        permission_profile: None,
        cwd: test_path_buf("/home/user/project").abs(),
        reasoning_effort: Some(ReasoningEffortConfig::default()),
        history_log_id: 0,
        history_entry_count: 0,
        initial_messages: None,
        network_proxy: None,
        rollout_path: Some(rollout_file.path().to_path_buf()),
    };
    chat.handle_codex_event(Event {
        id: "initial".into(),
        msg: EventMsg::SessionConfigured(configured),
    });
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
            policy: None,
            path_to_skills_md: repo_skill_path,
            scope: SkillScope::Repo,
        },
        SkillMetadata {
            name: "figma".to_string(),
            description: "User skill".to_string(),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            path_to_skills_md: user_skill_path.clone(),
            scope: SkillScope::User,
        },
    ]));

    chat.bottom_pane.set_composer_text_with_mention_bindings(
        "please use $figma now".to_string(),
        Vec::new(),
        Vec::new(),
        vec![MentionBinding {
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
    assert_eq!(chat.bottom_pane.composer_text_elements(), text_elements);
    assert_eq!(chat.bottom_pane.composer_local_images(), local_images);
    assert_eq!(chat.remote_image_urls(), remote_image_urls);
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
    chat.queued_user_messages.push_back(
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

    chat.handle_codex_event(Event {
        id: "interrupt".into(),
        msg: EventMsg::TurnAborted(codex_protocol::protocol::TurnAbortedEvent {
            turn_id: Some("turn-1".to_string()),
            reason: TurnAbortReason::Interrupted,
            completed_at: None,
            duration_ms: None,
        }),
    });

    assert_eq!(chat.bottom_pane.composer_text(), "Implement the plan.");
    assert!(chat.queued_user_messages.is_empty());
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
    assert!(chat.queued_user_messages.is_empty());
}

#[tokio::test]
async fn restore_thread_input_state_syncs_sleep_inhibitor_state() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::PreventIdleSleep, /*enabled*/ true);

    chat.restore_thread_input_state(Some(ThreadInputState {
        composer: None,
        pending_steers: VecDeque::new(),
        rejected_steers_queue: VecDeque::new(),
        queued_user_messages: VecDeque::new(),
        user_turn_pending_start: false,
        current_collaboration_mode: chat.current_collaboration_mode.clone(),
        active_collaboration_mask: chat.active_collaboration_mask.clone(),
        task_running: true,
        agent_turn_running: true,
    }));

    assert!(chat.agent_turn_running);
    assert!(chat.turn_sleep_inhibitor.is_turn_running());
    assert!(chat.bottom_pane.is_task_running());

    chat.restore_thread_input_state(/*input_state*/ None);

    assert!(!chat.agent_turn_running);
    assert!(!chat.turn_sleep_inhibitor.is_turn_running());
    assert!(!chat.bottom_pane.is_task_running());
}

#[tokio::test]
async fn alt_up_edits_most_recent_queued_message() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.queued_message_edit_binding = crate::key_hint::alt(KeyCode::Up);
    chat.bottom_pane
        .set_queued_message_edit_binding(crate::key_hint::alt(KeyCode::Up));

    // Simulate a running task so messages would normally be queued.
    chat.bottom_pane.set_task_running(/*running*/ true);

    // Seed two queued messages.
    chat.queued_user_messages
        .push_back(UserMessage::from("first queued".to_string()).into());
    chat.queued_user_messages
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
    assert_eq!(chat.queued_user_messages.len(), 1);
    assert_eq!(
        chat.queued_user_messages.front().unwrap().text,
        "first queued"
    );
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

    assert_eq!(chat.queued_user_messages.len(), 3);
    for message in chat.queued_user_messages.iter() {
        assert_eq!(message.text, "repeat me");
    }
}

#[test]
fn rendered_user_message_event_from_inputs_matches_flattened_user_message_shape() {
    let local_image = PathBuf::from("/tmp/local.png");
    let rendered = ChatWidget::rendered_user_message_event_from_inputs(&[
        UserInput::Text {
            text: "hello ".to_string(),
            text_elements: vec![TextElement::new((0..5).into(), /*placeholder*/ None)],
        },
        UserInput::Image {
            image_url: "https://example.com/remote.png".to_string(),
        },
        UserInput::LocalImage {
            path: local_image.clone(),
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
            text_elements: vec![TextElement::new((0..5).into(), Some("planet".to_string()))],
        },
    ]);

    assert_eq!(
        rendered,
        ChatWidget::rendered_user_message_event_from_parts(
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

#[tokio::test]
async fn interrupt_restores_queued_messages_into_composer() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    // Simulate a running task to enable queuing of user inputs.
    chat.bottom_pane.set_task_running(/*running*/ true);

    // Queue two user messages while the task is running.
    chat.queued_user_messages
        .push_back(UserMessage::from("first queued".to_string()).into());
    chat.queued_user_messages
        .push_back(UserMessage::from("second queued".to_string()).into());
    chat.refresh_pending_input_preview();

    // Deliver a TurnAborted event with Interrupted reason (as if Esc was pressed).
    chat.handle_codex_event(Event {
        id: "turn-1".into(),
        msg: EventMsg::TurnAborted(codex_protocol::protocol::TurnAbortedEvent {
            turn_id: Some("turn-1".to_string()),
            reason: TurnAbortReason::Interrupted,
            completed_at: None,
            duration_ms: None,
        }),
    });

    // Composer should now contain the queued messages joined by newlines, in order.
    assert_eq!(
        chat.bottom_pane.composer_text(),
        "first queued\nsecond queued"
    );

    // Queue should be cleared and no new user input should have been auto-submitted.
    assert!(chat.queued_user_messages.is_empty());
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

    chat.queued_user_messages
        .push_back(UserMessage::from("first queued".to_string()).into());
    chat.queued_user_messages
        .push_back(UserMessage::from("second queued".to_string()).into());
    chat.refresh_pending_input_preview();

    chat.handle_codex_event(Event {
        id: "turn-1".into(),
        msg: EventMsg::TurnAborted(codex_protocol::protocol::TurnAbortedEvent {
            turn_id: Some("turn-1".to_string()),
            reason: TurnAbortReason::Interrupted,
            completed_at: None,
            duration_ms: None,
        }),
    });

    assert_eq!(
        chat.bottom_pane.composer_text(),
        "first queued\nsecond queued\ncurrent draft"
    );
    assert!(chat.queued_user_messages.is_empty());
    assert!(
        op_rx.try_recv().is_err(),
        "unexpected outbound op after interrupt"
    );

    let _ = drain_insert_history(&mut rx);
}
