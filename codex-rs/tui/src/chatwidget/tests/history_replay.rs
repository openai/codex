use super::*;
use codex_protocol::protocol::FileSystemAccessMode;
use codex_protocol::protocol::FileSystemPath;
use codex_protocol::protocol::FileSystemSandboxEntry;
use codex_protocol::protocol::FileSystemSandboxKind;
use codex_protocol::protocol::FileSystemSandboxPolicy;
use codex_protocol::protocol::FileSystemSpecialPath;
use codex_protocol::protocol::NetworkAccess;
use codex_protocol::protocol::NetworkSandboxPolicy;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn resumed_initial_messages_render_history() {
    let (mut chat, mut rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;

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
        initial_messages: Some(vec![
            EventMsg::UserMessage(UserMessageEvent {
                message: "hello from user".to_string(),
                images: None,
                text_elements: Vec::new(),
                local_images: Vec::new(),
            }),
            EventMsg::AgentMessage(AgentMessageEvent {
                message: "assistant reply".to_string(),
                phase: None,
                memory_citation: None,
            }),
        ]),
        network_proxy: None,
        rollout_path: Some(rollout_file.path().to_path_buf()),
    };

    chat.handle_codex_event(Event {
        id: "initial".into(),
        msg: EventMsg::SessionConfigured(configured),
    });

    let cells = drain_insert_history(&mut rx);
    let mut merged_lines = Vec::new();
    for lines in cells {
        let text = lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.clone())
            .collect::<String>();
        merged_lines.push(text);
    }

    let text_blob = merged_lines.join("\n");
    assert!(
        text_blob.contains("hello from user"),
        "expected replayed user message",
    );
    assert!(
        text_blob.contains("assistant reply"),
        "expected replayed agent message",
    );
}

#[tokio::test]
async fn thread_snapshot_replay_does_not_duplicate_agent_message_history() {
    let (mut chat, mut rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_codex_event_replay(Event {
        id: "turn-1".into(),
        msg: EventMsg::ItemCompleted(ItemCompletedEvent {
            thread_id: ThreadId::new(),
            turn_id: "turn-1".to_string(),
            item: TurnItem::AgentMessage(AgentMessageItem {
                id: "msg-1".to_string(),
                content: vec![AgentMessageContent::Text {
                    text: "assistant reply".to_string(),
                }],
                phase: None,
                memory_citation: None,
            }),
        }),
    });
    chat.handle_codex_event_replay(Event {
        id: "turn-1".into(),
        msg: EventMsg::AgentMessage(AgentMessageEvent {
            message: "assistant reply".to_string(),
            phase: None,
            memory_citation: None,
        }),
    });

    let cells = drain_insert_history(&mut rx);
    assert_eq!(
        cells.len(),
        1,
        "expected replayed assistant message to render once"
    );
    let rendered = lines_to_single_string(&cells[0]);
    assert!(
        rendered.contains("assistant reply"),
        "expected replayed assistant message, got {rendered:?}"
    );
}

#[tokio::test]
async fn replayed_user_message_preserves_text_elements_and_local_images() {
    let (mut chat, mut rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;

    let placeholder = "[Image #1]";
    let message = format!("{placeholder} replayed");
    let text_elements = vec![TextElement::new(
        (0..placeholder.len()).into(),
        Some(placeholder.to_string()),
    )];
    let local_images = vec![PathBuf::from("/tmp/replay.png")];

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
        initial_messages: Some(vec![EventMsg::UserMessage(UserMessageEvent {
            message: message.clone(),
            images: None,
            text_elements: text_elements.clone(),
            local_images: local_images.clone(),
        })]),
        network_proxy: None,
        rollout_path: Some(rollout_file.path().to_path_buf()),
    };

    chat.handle_codex_event(Event {
        id: "initial".into(),
        msg: EventMsg::SessionConfigured(configured),
    });

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
        user_cell.expect("expected a replayed user history cell");
    assert_eq!(stored_message, message);
    assert_eq!(stored_elements, text_elements);
    assert_eq!(stored_images, local_images);
    assert!(stored_remote_image_urls.is_empty());
}

#[tokio::test]
async fn replayed_user_message_preserves_remote_image_urls() {
    let (mut chat, mut rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;

    let message = "replayed with remote image".to_string();
    let remote_image_urls = vec!["https://example.com/image.png".to_string()];

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
        initial_messages: Some(vec![EventMsg::UserMessage(UserMessageEvent {
            message: message.clone(),
            images: Some(remote_image_urls.clone()),
            text_elements: Vec::new(),
            local_images: Vec::new(),
        })]),
        network_proxy: None,
        rollout_path: Some(rollout_file.path().to_path_buf()),
    };

    chat.handle_codex_event(Event {
        id: "initial".into(),
        msg: EventMsg::SessionConfigured(configured),
    });

    let mut user_cell = None;
    while let Ok(ev) = rx.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = ev
            && let Some(cell) = cell.as_any().downcast_ref::<UserHistoryCell>()
        {
            user_cell = Some((
                cell.message.clone(),
                cell.local_image_paths.clone(),
                cell.remote_image_urls.clone(),
            ));
            break;
        }
    }

    let (stored_message, stored_local_images, stored_remote_image_urls) =
        user_cell.expect("expected a replayed user history cell");
    assert_eq!(stored_message, message);
    assert!(stored_local_images.is_empty());
    assert_eq!(stored_remote_image_urls, remote_image_urls);
}

#[tokio::test]
async fn session_configured_syncs_widget_config_permissions_and_cwd() {
    let (mut chat, _rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;

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
    chat.config.cwd = test_path_buf("/home/user/main").abs();

    let legacy_fallback_sandbox = SandboxPolicy::new_read_only_policy();
    let expected_cwd = test_path_buf("/home/user/sub-agent").abs();
    let expected_file_system_policy = FileSystemSandboxPolicy::restricted(vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Read,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::GlobPattern {
                pattern: "**/.secret".to_string(),
            },
            access: FileSystemAccessMode::None,
        },
    ]);
    let expected_permission_profile =
        codex_protocol::models::PermissionProfile::from_runtime_permissions(
            &expected_file_system_policy,
            NetworkSandboxPolicy::Restricted,
        );
    let expected_sandbox = expected_permission_profile
        .to_legacy_sandbox_policy(expected_cwd.as_path())
        .expect("permission profile should project to legacy sandbox policy");
    let configured = codex_protocol::protocol::SessionConfiguredEvent {
        session_id: ThreadId::new(),
        forked_from_id: None,
        thread_name: None,
        model: "test-model".to_string(),
        model_provider_id: "test-provider".to_string(),
        service_tier: None,
        approval_policy: AskForApproval::Never,
        approvals_reviewer: ApprovalsReviewer::User,
        sandbox_policy: legacy_fallback_sandbox,
        permission_profile: Some(expected_permission_profile.clone()),
        cwd: expected_cwd.clone(),
        reasoning_effort: Some(ReasoningEffortConfig::default()),
        history_log_id: 0,
        history_entry_count: 0,
        initial_messages: None,
        network_proxy: None,
        rollout_path: None,
    };

    chat.handle_codex_event(Event {
        id: "session-configured".into(),
        msg: EventMsg::SessionConfigured(configured),
    });

    assert_eq!(
        chat.config_ref().permissions.approval_policy.value(),
        AskForApproval::Never
    );
    assert_eq!(
        chat.config_ref().permissions.sandbox_policy.get(),
        &expected_sandbox
    );
    assert_eq!(
        chat.config_ref().permissions.permission_profile(),
        expected_permission_profile
    );
    assert_eq!(&chat.config_ref().cwd, &expected_cwd);

    let updated_sandbox = SandboxPolicy::new_workspace_write_policy();
    chat.set_sandbox_policy(updated_sandbox.clone())
        .expect("set sandbox policy");
    let updated_file_system_policy = FileSystemSandboxPolicy::from_legacy_sandbox_policy_for_cwd(
        &updated_sandbox,
        &expected_cwd,
    );
    assert_eq!(
        chat.config_ref().permissions.permission_profile(),
        codex_protocol::models::PermissionProfile::from_runtime_permissions_with_enforcement(
            codex_protocol::models::SandboxEnforcement::from_legacy_sandbox_policy(
                &updated_sandbox
            ),
            &updated_file_system_policy,
            NetworkSandboxPolicy::from(&updated_sandbox),
        ),
        "local sandbox changes should replace SessionConfigured profile-derived runtime permissions using the widget cwd"
    );
}

#[tokio::test]
async fn session_configured_external_sandbox_keeps_external_runtime_policy() {
    let (mut chat, _rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;

    let expected_sandbox = SandboxPolicy::ExternalSandbox {
        network_access: NetworkAccess::Restricted,
    };
    let configured = codex_protocol::protocol::SessionConfiguredEvent {
        session_id: ThreadId::new(),
        forked_from_id: None,
        thread_name: None,
        model: "test-model".to_string(),
        model_provider_id: "test-provider".to_string(),
        service_tier: None,
        approval_policy: AskForApproval::Never,
        approvals_reviewer: ApprovalsReviewer::User,
        sandbox_policy: expected_sandbox.clone(),
        permission_profile: None,
        cwd: test_path_buf("/home/user/external").abs(),
        reasoning_effort: Some(ReasoningEffortConfig::default()),
        history_log_id: 0,
        history_entry_count: 0,
        initial_messages: None,
        network_proxy: None,
        rollout_path: None,
    };

    chat.handle_codex_event(Event {
        id: "session-configured".into(),
        msg: EventMsg::SessionConfigured(configured),
    });

    assert_eq!(
        chat.config_ref().permissions.sandbox_policy.get(),
        &expected_sandbox
    );
    assert_eq!(
        chat.config_ref()
            .permissions
            .file_system_sandbox_policy
            .kind,
        FileSystemSandboxKind::ExternalSandbox,
    );
    assert_eq!(
        chat.config_ref().permissions.network_sandbox_policy,
        NetworkSandboxPolicy::Restricted,
    );
}

#[tokio::test]
async fn replayed_user_message_with_only_remote_images_renders_history_cell() {
    let (mut chat, mut rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;

    let remote_image_urls = vec!["https://example.com/remote-only.png".to_string()];

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
        initial_messages: Some(vec![EventMsg::UserMessage(UserMessageEvent {
            message: String::new(),
            images: Some(remote_image_urls.clone()),
            text_elements: Vec::new(),
            local_images: Vec::new(),
        })]),
        network_proxy: None,
        rollout_path: Some(rollout_file.path().to_path_buf()),
    };

    chat.handle_codex_event(Event {
        id: "initial".into(),
        msg: EventMsg::SessionConfigured(configured),
    });

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
        user_cell.expect("expected a replayed remote-image-only user history cell");
    assert!(stored_message.is_empty());
    assert_eq!(stored_remote_image_urls, remote_image_urls);
}

#[tokio::test]
async fn replayed_user_message_with_only_local_images_does_not_render_history_cell() {
    let (mut chat, mut rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;

    let local_images = vec![PathBuf::from("/tmp/replay-local-only.png")];

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
        initial_messages: Some(vec![EventMsg::UserMessage(UserMessageEvent {
            message: String::new(),
            images: None,
            text_elements: Vec::new(),
            local_images,
        })]),
        network_proxy: None,
        rollout_path: Some(rollout_file.path().to_path_buf()),
    };

    chat.handle_codex_event(Event {
        id: "initial".into(),
        msg: EventMsg::SessionConfigured(configured),
    });

    let mut found_user_history_cell = false;
    while let Ok(ev) = rx.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = ev
            && cell.as_any().downcast_ref::<UserHistoryCell>().is_some()
        {
            found_user_history_cell = true;
            break;
        }
    }

    assert!(!found_user_history_cell);
}

#[tokio::test]
async fn forked_thread_history_line_includes_name_and_id_snapshot() {
    let (chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let mut chat = chat;

    let forked_from_id =
        ThreadId::from_string("e9f18a88-8081-4e51-9d4e-8af5cde2d8dd").expect("forked id");

    chat.emit_forked_thread_event(forked_from_id, Some("named-thread".to_string()));

    let history_cell = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            match rx.recv().await {
                Some(AppEvent::InsertHistoryCell(cell)) => break cell,
                Some(_) => continue,
                None => panic!("app event channel closed before forked thread history was emitted"),
            }
        }
    })
    .await
    .expect("timed out waiting for forked thread history");
    let combined = lines_to_single_string(&history_cell.display_lines(/*width*/ 80));

    assert!(
        combined.contains("Thread forked from"),
        "expected forked thread message in history"
    );
    assert_chatwidget_snapshot!("forked_thread_history_line", combined);
}

#[tokio::test]
async fn forked_thread_history_line_without_name_shows_id_once_snapshot() {
    let (chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let mut chat = chat;
    let temp = tempdir().expect("tempdir");
    chat.config.codex_home =
        codex_utils_absolute_path::AbsolutePathBuf::from_absolute_path(temp.path())
            .expect("temp dir is absolute");

    let forked_from_id =
        ThreadId::from_string("019c2d47-4935-7423-a190-05691f566092").expect("forked id");
    chat.emit_forked_thread_event(forked_from_id, /*fork_parent_title*/ None);

    let history_cell = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            match rx.recv().await {
                Some(AppEvent::InsertHistoryCell(cell)) => break cell,
                Some(_) => continue,
                None => panic!("app event channel closed before forked thread history was emitted"),
            }
        }
    })
    .await
    .expect("timed out waiting for forked thread history");
    let combined = lines_to_single_string(&history_cell.display_lines(/*width*/ 80));

    assert_chatwidget_snapshot!("forked_thread_history_line_without_name", combined);
}

#[tokio::test]
async fn app_server_forked_thread_history_line_uses_app_server_title_snapshot() {
    let (chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let mut chat = chat;
    let temp = tempdir().expect("tempdir");
    chat.config.codex_home =
        codex_utils_absolute_path::AbsolutePathBuf::from_absolute_path(temp.path())
            .expect("temp dir is absolute");

    let forked_from_id =
        ThreadId::from_string("e9f18a88-8081-4e51-9d4e-8af5cde2d8dd").expect("forked id");
    let session_index_entry = format!(
        "{{\"id\":\"{forked_from_id}\",\"thread_name\":\"stale-local-thread\",\"updated_at\":\"2024-01-02T00:00:00Z\"}}\n"
    );
    std::fs::write(temp.path().join("session_index.jsonl"), session_index_entry)
        .expect("write session index");

    chat.emit_forked_thread_event(forked_from_id, Some("app-server-parent-thread".to_string()));

    let history_cell = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            match rx.recv().await {
                Some(AppEvent::InsertHistoryCell(cell)) => break cell,
                Some(_) => continue,
                None => panic!("app event channel closed before forked thread history was emitted"),
            }
        }
    })
    .await
    .expect("timed out waiting for forked thread history");
    let combined = lines_to_single_string(&history_cell.display_lines(/*width*/ 80));

    assert!(combined.contains("app-server-parent-thread"));
    assert!(
        !combined.contains("stale-local-thread"),
        "app-server fork title lookup should not read local CODEX_HOME"
    );
    assert_chatwidget_snapshot!("app_server_forked_thread_history_line", combined);
}

#[tokio::test]
async fn app_server_forked_thread_history_line_without_app_server_name_ignores_local_snapshot() {
    let (chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let mut chat = chat;
    let temp = tempdir().expect("tempdir");
    chat.config.codex_home =
        codex_utils_absolute_path::AbsolutePathBuf::from_absolute_path(temp.path())
            .expect("temp dir is absolute");

    let forked_from_id =
        ThreadId::from_string("019c2d47-4935-7423-a190-05691f566092").expect("forked id");
    let session_index_entry = format!(
        "{{\"id\":\"{forked_from_id}\",\"thread_name\":\"stale-local-thread\",\"updated_at\":\"2024-01-02T00:00:00Z\"}}\n"
    );
    std::fs::write(temp.path().join("session_index.jsonl"), session_index_entry)
        .expect("write session index");

    chat.emit_forked_thread_event(forked_from_id, /*fork_parent_title*/ None);

    let history_cell = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            match rx.recv().await {
                Some(AppEvent::InsertHistoryCell(cell)) => break cell,
                Some(_) => continue,
                None => panic!("app event channel closed before forked thread history was emitted"),
            }
        }
    })
    .await
    .expect("timed out waiting for forked thread history");
    let combined = lines_to_single_string(&history_cell.display_lines(/*width*/ 80));

    assert!(
        !combined.contains("stale-local-thread"),
        "app-server fork title lookup should not read local CODEX_HOME"
    );
    assert_chatwidget_snapshot!(
        "app_server_forked_thread_history_line_without_app_server_name",
        combined
    );
}

#[tokio::test]
async fn thread_snapshot_replay_preserves_agent_message_during_review_mode() {
    let (mut chat, mut rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_codex_event_replay(Event {
        id: "review-start".into(),
        msg: EventMsg::EnteredReviewMode(ReviewRequest {
            target: ReviewTarget::UncommittedChanges,
            user_facing_hint: None,
        }),
    });
    let _ = drain_insert_history(&mut rx);

    chat.handle_codex_event_replay(Event {
        id: "review-message".into(),
        msg: EventMsg::AgentMessage(AgentMessageEvent {
            message: "Review progress update".to_string(),
            phase: None,
            memory_citation: None,
        }),
    });

    let inserted = drain_insert_history(&mut rx);
    assert_eq!(inserted.len(), 1);
    assert!(lines_to_single_string(&inserted[0]).contains("Review progress update"));
}

#[tokio::test]
async fn replayed_thread_rollback_emits_ordered_app_event() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(Some("gpt-5")).await;

    chat.replay_initial_messages(vec![EventMsg::ThreadRolledBack(ThreadRolledBackEvent {
        num_turns: 2,
    })]);

    let mut saw = false;
    while let Ok(event) = rx.try_recv() {
        if let AppEvent::ApplyThreadRollback { num_turns } = event {
            saw = true;
            assert_eq!(num_turns, 2);
            break;
        }
    }

    assert!(saw, "expected replay rollback app event");
}

#[tokio::test]
async fn live_legacy_agent_message_after_item_completed_does_not_duplicate_assistant_message() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    complete_assistant_message(
        &mut chat,
        "msg-live",
        "hello",
        Some(MessagePhase::FinalAnswer),
    );
    let inserted = drain_insert_history(&mut rx);
    assert_eq!(inserted.len(), 1);
    assert!(lines_to_single_string(&inserted[0]).contains("hello"));

    chat.handle_codex_event(Event {
        id: "legacy-live".into(),
        msg: EventMsg::AgentMessage(AgentMessageEvent {
            message: "hello".into(),
            phase: Some(MessagePhase::FinalAnswer),
            memory_citation: None,
        }),
    });

    assert!(drain_insert_history(&mut rx).is_empty());
}

#[tokio::test]
async fn replayed_retryable_app_server_error_keeps_turn_running() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_server_notification(
        ServerNotification::TurnStarted(TurnStartedNotification {
            thread_id: "thread-1".to_string(),
            turn: AppServerTurn {
                id: "turn-1".to_string(),
                items: Vec::new(),
                status: AppServerTurnStatus::InProgress,
                error: None,
                started_at: Some(0),
                completed_at: None,
                duration_ms: None,
            },
        }),
        Some(ReplayKind::ThreadSnapshot),
    );
    drain_insert_history(&mut rx);

    chat.handle_server_notification(
        ServerNotification::Error(ErrorNotification {
            error: AppServerTurnError {
                message: "Reconnecting... 1/5".to_string(),
                codex_error_info: None,
                additional_details: Some("Idle timeout waiting for SSE".to_string()),
            },
            will_retry: true,
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
        }),
        Some(ReplayKind::ThreadSnapshot),
    );

    assert!(drain_insert_history(&mut rx).is_empty());
    assert!(chat.bottom_pane.is_task_running());
    let status = chat
        .bottom_pane
        .status_widget()
        .expect("status indicator should be visible");
    assert_eq!(status.header(), "Working");
    assert_eq!(status.details(), None);
}

#[tokio::test]
async fn replayed_thread_closed_notification_does_not_exit_tui() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_server_notification(
        ServerNotification::ThreadClosed(ThreadClosedNotification {
            thread_id: "thread-1".to_string(),
        }),
        Some(ReplayKind::ThreadSnapshot),
    );

    assert_matches!(rx.try_recv(), Err(TryRecvError::Empty));
}

#[tokio::test]
async fn replayed_reasoning_item_hides_raw_reasoning_when_disabled() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.config.show_raw_agent_reasoning = false;
    chat.handle_codex_event(Event {
        id: "configured".into(),
        msg: EventMsg::SessionConfigured(SessionConfiguredEvent {
            session_id: ThreadId::new(),
            forked_from_id: None,
            thread_name: None,
            model: "test-model".to_string(),
            model_provider_id: "test-provider".to_string(),
            service_tier: None,
            approval_policy: AskForApproval::Never,
            approvals_reviewer: ApprovalsReviewer::User,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            permission_profile: None,
            cwd: test_project_path().abs(),
            reasoning_effort: None,
            history_log_id: 0,
            history_entry_count: 0,
            initial_messages: None,
            network_proxy: None,
            rollout_path: None,
        }),
    });
    let _ = drain_insert_history(&mut rx);

    chat.replay_thread_item(
        AppServerThreadItem::Reasoning {
            id: "reasoning-1".to_string(),
            summary: vec!["Summary only".to_string()],
            content: vec!["Raw reasoning".to_string()],
        },
        "turn-1".to_string(),
        ReplayKind::ThreadSnapshot,
    );

    let rendered = match rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => {
            lines_to_single_string(&cell.transcript_lines(/*width*/ 80))
        }
        other => panic!("expected InsertHistoryCell, got {other:?}"),
    };
    assert!(!rendered.trim().is_empty());
    assert!(!rendered.contains("Raw reasoning"));
}

#[tokio::test]
async fn replayed_reasoning_item_shows_raw_reasoning_when_enabled() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.config.show_raw_agent_reasoning = true;
    chat.handle_codex_event(Event {
        id: "configured".into(),
        msg: EventMsg::SessionConfigured(SessionConfiguredEvent {
            session_id: ThreadId::new(),
            forked_from_id: None,
            thread_name: None,
            model: "test-model".to_string(),
            model_provider_id: "test-provider".to_string(),
            service_tier: None,
            approval_policy: AskForApproval::Never,
            approvals_reviewer: ApprovalsReviewer::User,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            permission_profile: None,
            cwd: test_project_path().abs(),
            reasoning_effort: None,
            history_log_id: 0,
            history_entry_count: 0,
            initial_messages: None,
            network_proxy: None,
            rollout_path: None,
        }),
    });
    let _ = drain_insert_history(&mut rx);

    chat.replay_thread_item(
        AppServerThreadItem::Reasoning {
            id: "reasoning-1".to_string(),
            summary: vec!["Summary only".to_string()],
            content: vec!["Raw reasoning".to_string()],
        },
        "turn-1".to_string(),
        ReplayKind::ThreadSnapshot,
    );

    let rendered = match rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => {
            lines_to_single_string(&cell.transcript_lines(/*width*/ 80))
        }
        other => panic!("expected InsertHistoryCell, got {other:?}"),
    };
    assert!(rendered.contains("Raw reasoning"));
}

#[tokio::test]
async fn live_reasoning_summary_is_not_rendered_twice_when_item_completes() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.show_welcome_banner = false;

    chat.handle_server_notification(
        ServerNotification::TurnStarted(TurnStartedNotification {
            thread_id: "thread-1".to_string(),
            turn: AppServerTurn {
                id: "turn-1".to_string(),
                items: Vec::new(),
                status: AppServerTurnStatus::InProgress,
                error: None,
                started_at: Some(0),
                completed_at: None,
                duration_ms: None,
            },
        }),
        /*replay_kind*/ None,
    );
    let _ = drain_insert_history(&mut rx);

    chat.handle_server_notification(
        ServerNotification::ReasoningSummaryTextDelta(ReasoningSummaryTextDeltaNotification {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item_id: "reasoning-1".to_string(),
            delta: "Summary only".to_string(),
            summary_index: 0,
        }),
        /*replay_kind*/ None,
    );

    chat.handle_server_notification(
        ServerNotification::ItemCompleted(ItemCompletedNotification {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item: AppServerThreadItem::Reasoning {
                id: "reasoning-1".to_string(),
                summary: vec!["Summary only".to_string()],
                content: Vec::new(),
            },
        }),
        /*replay_kind*/ None,
    );

    let rendered = match rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => {
            lines_to_single_string(&cell.transcript_lines(/*width*/ 80))
        }
        other => panic!("expected InsertHistoryCell, got {other:?}"),
    };
    assert_eq!(rendered.matches("Summary only").count(), 1);
}

#[tokio::test]
async fn replayed_turn_started_does_not_mark_task_running() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.replay_initial_messages(vec![EventMsg::TurnStarted(TurnStartedEvent {
        turn_id: "turn-1".to_string(),
        started_at: None,
        model_context_window: None,
        collaboration_mode_kind: ModeKind::Default,
    })]);

    assert!(!chat.bottom_pane.is_task_running());
    assert!(chat.bottom_pane.status_widget().is_none());
}

#[tokio::test]
async fn thread_snapshot_replayed_turn_started_marks_task_running() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_codex_event_replay(Event {
        id: "turn-1".into(),
        msg: EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: "turn-1".to_string(),
            started_at: None,
            model_context_window: None,
            collaboration_mode_kind: ModeKind::Default,
        }),
    });

    drain_insert_history(&mut rx);
    assert!(chat.bottom_pane.is_task_running());
    let status = chat
        .bottom_pane
        .status_widget()
        .expect("status indicator should be visible");
    assert_eq!(status.header(), "Working");
}

#[tokio::test]
async fn replayed_in_progress_turn_marks_task_running() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.replay_thread_turns(
        vec![AppServerTurn {
            id: "turn-1".to_string(),
            items: Vec::new(),
            status: AppServerTurnStatus::InProgress,
            error: None,
            started_at: None,
            completed_at: None,
            duration_ms: None,
        }],
        ReplayKind::ResumeInitialMessages,
    );

    assert!(drain_insert_history(&mut rx).is_empty());
    assert!(chat.bottom_pane.is_task_running());
    let status = chat
        .bottom_pane
        .status_widget()
        .expect("status indicator should be visible");
    assert_eq!(status.header(), "Working");
}

#[tokio::test]
async fn replayed_stream_error_does_not_set_retry_status_or_status_indicator() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_status_header("Idle".to_string());

    chat.replay_initial_messages(vec![EventMsg::StreamError(StreamErrorEvent {
        message: "Reconnecting... 2/5".to_string(),
        codex_error_info: Some(CodexErrorInfo::Other),
        additional_details: Some("Idle timeout waiting for SSE".to_string()),
    })]);

    let cells = drain_insert_history(&mut rx);
    assert!(
        cells.is_empty(),
        "expected no history cell for replayed StreamError event"
    );
    assert_eq!(chat.current_status.header, "Idle");
    assert!(chat.retry_status_header.is_none());
    assert!(chat.bottom_pane.status_widget().is_none());
}

#[tokio::test]
async fn thread_snapshot_replayed_stream_recovery_restores_previous_status_header() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_codex_event_replay(Event {
        id: "task".into(),
        msg: EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: "turn-1".to_string(),
            started_at: None,
            model_context_window: None,
            collaboration_mode_kind: ModeKind::Default,
        }),
    });
    drain_insert_history(&mut rx);

    chat.handle_codex_event_replay(Event {
        id: "retry".into(),
        msg: EventMsg::StreamError(StreamErrorEvent {
            message: "Reconnecting... 1/5".to_string(),
            codex_error_info: Some(CodexErrorInfo::Other),
            additional_details: None,
        }),
    });
    drain_insert_history(&mut rx);

    chat.handle_codex_event_replay(Event {
        id: "delta".into(),
        msg: EventMsg::AgentMessageDelta(AgentMessageDeltaEvent {
            delta: "hello".to_string(),
        }),
    });

    let status = chat
        .bottom_pane
        .status_widget()
        .expect("status indicator should be visible");
    assert_eq!(status.header(), "Working");
    assert_eq!(status.details(), None);
    assert!(chat.retry_status_header.is_none());
}

#[tokio::test]
async fn resume_replay_interrupted_reconnect_does_not_leave_stale_working_state() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_status_header("Idle".to_string());

    chat.replay_initial_messages(vec![
        EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: "turn-1".to_string(),
            started_at: None,
            model_context_window: None,
            collaboration_mode_kind: ModeKind::Default,
        }),
        EventMsg::StreamError(StreamErrorEvent {
            message: "Reconnecting... 1/5".to_string(),
            codex_error_info: Some(CodexErrorInfo::Other),
            additional_details: None,
        }),
        EventMsg::AgentMessageDelta(AgentMessageDeltaEvent {
            delta: "hello".to_string(),
        }),
    ]);

    let cells = drain_insert_history(&mut rx);
    assert!(
        cells.is_empty(),
        "expected no history cells for replayed interrupted reconnect sequence"
    );
    assert!(!chat.bottom_pane.is_task_running());
    assert!(chat.bottom_pane.status_widget().is_none());
    assert_eq!(chat.current_status.header, "Idle");
    assert!(chat.retry_status_header.is_none());
}

#[tokio::test]
async fn replayed_interrupted_reconnect_footer_row_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.replay_initial_messages(vec![
        EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: "turn-1".to_string(),
            started_at: None,
            model_context_window: None,
            collaboration_mode_kind: ModeKind::Default,
        }),
        EventMsg::StreamError(StreamErrorEvent {
            message: "Reconnecting... 2/5".to_string(),
            codex_error_info: Some(CodexErrorInfo::Other),
            additional_details: Some("Idle timeout waiting for SSE".to_string()),
        }),
    ]);

    let header = render_bottom_first_row(&chat, /*width*/ 80);
    assert!(
        !header.contains("Reconnecting") && !header.contains("Working"),
        "expected replayed interrupted reconnect to avoid active status row, got {header:?}"
    );
    assert_chatwidget_snapshot!("replayed_interrupted_reconnect_footer_row", header);
}

#[tokio::test]
async fn stream_recovery_restores_previous_status_header() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.handle_codex_event(Event {
        id: "task".into(),
        msg: EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: "turn-1".to_string(),
            started_at: None,
            model_context_window: None,
            collaboration_mode_kind: ModeKind::Default,
        }),
    });
    drain_insert_history(&mut rx);
    chat.handle_codex_event(Event {
        id: "retry".into(),
        msg: EventMsg::StreamError(StreamErrorEvent {
            message: "Reconnecting... 1/5".to_string(),
            codex_error_info: Some(CodexErrorInfo::Other),
            additional_details: None,
        }),
    });
    drain_insert_history(&mut rx);
    chat.handle_codex_event(Event {
        id: "delta".into(),
        msg: EventMsg::AgentMessageDelta(AgentMessageDeltaEvent {
            delta: "hello".to_string(),
        }),
    });

    let status = chat
        .bottom_pane
        .status_widget()
        .expect("status indicator should be visible");
    assert_eq!(status.header(), "Working");
    assert_eq!(status.details(), None);
    assert!(chat.retry_status_header.is_none());
}
