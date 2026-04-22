use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn collab_spawn_end_shows_requested_model_and_effort() {
    let (mut chat, mut rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;
    let sender_thread_id = ThreadId::new();
    let spawned_thread_id = ThreadId::new();

    chat.handle_codex_event(Event {
        id: "spawn-begin".into(),
        msg: EventMsg::CollabAgentSpawnBegin(CollabAgentSpawnBeginEvent {
            call_id: "call-spawn".to_string(),
            sender_thread_id,
            prompt: "Explore the repo".to_string(),
            model: "gpt-5".to_string(),
            reasoning_effort: ReasoningEffortConfig::High,
        }),
    });
    chat.handle_codex_event(Event {
        id: "spawn-end".into(),
        msg: EventMsg::CollabAgentSpawnEnd(CollabAgentSpawnEndEvent {
            call_id: "call-spawn".to_string(),
            sender_thread_id,
            new_thread_id: Some(spawned_thread_id),
            new_agent_nickname: Some("Robie".to_string()),
            new_agent_role: Some("explorer".to_string()),
            prompt: "Explore the repo".to_string(),
            model: "gpt-5".to_string(),
            reasoning_effort: ReasoningEffortConfig::High,
            status: AgentStatus::PendingInit,
        }),
    });

    let cells = drain_insert_history(&mut rx);
    let rendered = cells
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        rendered.contains("Spawned Robie [explorer] (gpt-5 high)"),
        "expected spawn line to include agent metadata and requested model, got {rendered:?}"
    );
}

#[tokio::test]
async fn live_app_server_user_message_item_completed_does_not_duplicate_rendered_prompt() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.thread_id = Some(ThreadId::new());

    chat.bottom_pane
        .set_composer_text("Hi, are you there?".to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    match next_submit_op(&mut op_rx) {
        Op::UserTurn { .. } => {}
        other => panic!("expected Op::UserTurn, got {other:?}"),
    }

    let inserted = drain_insert_history(&mut rx);
    assert_eq!(inserted.len(), 1);
    assert!(lines_to_single_string(&inserted[0]).contains("Hi, are you there?"));

    chat.handle_server_notification(
        ServerNotification::ItemCompleted(ItemCompletedNotification {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item: AppServerThreadItem::UserMessage {
                id: "user-1".to_string(),
                content: vec![AppServerUserInput::Text {
                    text: "Hi, are you there?".to_string(),
                    text_elements: Vec::new(),
                }],
            },
        }),
        /*replay_kind*/ None,
    );

    assert!(drain_insert_history(&mut rx).is_empty());
}

#[tokio::test]
async fn live_app_server_turn_completed_clears_working_status_after_answer_item() {
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
        /*replay_kind*/ None,
    );

    assert!(chat.bottom_pane.is_task_running());
    let status = chat
        .bottom_pane
        .status_widget()
        .expect("status indicator should be visible");
    assert_eq!(status.header(), "Working");

    chat.handle_server_notification(
        ServerNotification::ItemCompleted(ItemCompletedNotification {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item: AppServerThreadItem::AgentMessage {
                id: "msg-1".to_string(),
                text: "Yes. What do you need?".to_string(),
                phase: Some(MessagePhase::FinalAnswer),
                memory_citation: None,
            },
        }),
        /*replay_kind*/ None,
    );

    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 1);
    assert!(lines_to_single_string(&cells[0]).contains("Yes. What do you need?"));
    assert!(chat.bottom_pane.is_task_running());

    chat.handle_server_notification(
        ServerNotification::TurnCompleted(TurnCompletedNotification {
            thread_id: "thread-1".to_string(),
            turn: AppServerTurn {
                id: "turn-1".to_string(),
                items: Vec::new(),
                status: AppServerTurnStatus::Completed,
                error: None,
                started_at: None,
                completed_at: Some(0),
                duration_ms: None,
            },
        }),
        /*replay_kind*/ None,
    );

    assert!(!chat.bottom_pane.is_task_running());
    assert!(chat.bottom_pane.status_widget().is_none());
}

#[tokio::test]
async fn live_app_server_turn_started_sets_feedback_turn_id() {
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
        /*replay_kind*/ None,
    );

    chat.open_feedback_note(
        crate::app_event::FeedbackCategory::Bug,
        /*include_logs*/ false,
    );
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_matches!(
        rx.try_recv(),
        Ok(AppEvent::SubmitFeedback {
            category: crate::app_event::FeedbackCategory::Bug,
            reason: None,
            turn_id: Some(turn_id),
            include_logs: false,
        }) if turn_id == "turn-1"
    );
}

#[tokio::test]
async fn live_app_server_warning_notification_renders_message() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_server_notification(
        ServerNotification::Warning(WarningNotification {
            thread_id: None,
            message: "Warning: Exceeded skills context budget of 2%. All skill descriptions were removed and 2 additional skills were not included in the model-visible skills list.".to_string(),
        }),
        /*replay_kind*/ None,
    );

    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 1, "expected one warning history cell");
    let rendered = lines_to_single_string(&cells[0]);
    let normalized = rendered.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        normalized.contains("Warning: Exceeded skills context budget of 2%."),
        "expected warning notification message, got {rendered}"
    );
    assert!(
        normalized.contains(
            "All skill descriptions were removed and 2 additional skills were not included in the model-visible skills list."
        ),
        "expected warning guidance, got {rendered}"
    );
}

#[tokio::test]
async fn live_app_server_guardian_warning_notification_renders_message() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_server_notification(
        ServerNotification::GuardianWarning(GuardianWarningNotification {
            thread_id: "thread-1".to_string(),
            message: "Automatic approval review denied the requested action.".to_string(),
        }),
        /*replay_kind*/ None,
    );

    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 1, "expected one warning history cell");
    let rendered = lines_to_single_string(&cells[0]);
    assert!(
        rendered.contains("Automatic approval review denied the requested action."),
        "expected guardian warning notification message, got {rendered}"
    );
}

#[tokio::test]
async fn live_app_server_config_warning_prefixes_summary() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_server_notification(
        ServerNotification::ConfigWarning(ConfigWarningNotification {
            summary: "Invalid configuration; using defaults.".to_string(),
            details: None,
            path: None,
            range: None,
        }),
        /*replay_kind*/ None,
    );

    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 1, "expected one warning history cell");
    let rendered = lines_to_single_string(&cells[0]);
    assert!(
        rendered.contains("Invalid configuration; using defaults."),
        "expected config warning summary, got {rendered}"
    );
}

#[tokio::test]
async fn live_app_server_file_change_item_started_preserves_changes() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_server_notification(
        ServerNotification::ItemStarted(ItemStartedNotification {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item: AppServerThreadItem::FileChange {
                id: "patch-1".to_string(),
                changes: vec![FileUpdateChange {
                    path: "foo.txt".to_string(),
                    kind: PatchChangeKind::Add,
                    diff: "hello\n".to_string(),
                }],
                status: AppServerPatchApplyStatus::InProgress,
            },
        }),
        /*replay_kind*/ None,
    );

    let cells = drain_insert_history(&mut rx);
    assert!(!cells.is_empty(), "expected patch history to be rendered");
    let transcript = lines_to_single_string(cells.last().expect("patch cell"));
    assert!(
        transcript.contains("Added foo.txt") || transcript.contains("Edited foo.txt"),
        "expected patch summary to include foo.txt, got: {transcript}"
    );
}

#[tokio::test]
async fn live_app_server_command_execution_strips_shell_wrapper() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let script = r#"python3 -c 'print("Hello, world!")'"#;
    let command =
        shlex::try_join(["/bin/zsh", "-lc", script]).expect("round-trippable shell wrapper");

    chat.handle_server_notification(
        ServerNotification::ItemStarted(ItemStartedNotification {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item: AppServerThreadItem::CommandExecution {
                id: "cmd-1".to_string(),
                command: command.clone(),
                cwd: test_path_buf("/tmp").abs(),
                process_id: None,
                source: AppServerCommandExecutionSource::UserShell,
                status: AppServerCommandExecutionStatus::InProgress,
                command_actions: vec![AppServerCommandAction::Unknown {
                    command: script.to_string(),
                }],
                aggregated_output: None,
                exit_code: None,
                duration_ms: None,
            },
        }),
        /*replay_kind*/ None,
    );
    chat.handle_server_notification(
        ServerNotification::ItemCompleted(ItemCompletedNotification {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item: AppServerThreadItem::CommandExecution {
                id: "cmd-1".to_string(),
                command,
                cwd: test_path_buf("/tmp").abs(),
                process_id: None,
                source: AppServerCommandExecutionSource::UserShell,
                status: AppServerCommandExecutionStatus::Completed,
                command_actions: vec![AppServerCommandAction::Unknown {
                    command: script.to_string(),
                }],
                aggregated_output: Some("Hello, world!\n".to_string()),
                exit_code: Some(0),
                duration_ms: Some(5),
            },
        }),
        /*replay_kind*/ None,
    );

    let cells = drain_insert_history(&mut rx);
    assert_eq!(
        cells.len(),
        1,
        "expected one completed command history cell"
    );
    let blob = lines_to_single_string(cells.first().expect("command cell"));
    assert_chatwidget_snapshot!(
        "live_app_server_command_execution_strips_shell_wrapper",
        blob
    );
}

#[test]
fn app_server_patch_changes_to_core_preserves_diffs() {
    let changes = app_server_patch_changes_to_core(vec![FileUpdateChange {
        path: "foo.txt".to_string(),
        kind: PatchChangeKind::Add,
        diff: "hello\n".to_string(),
    }]);

    assert_eq!(
        changes,
        HashMap::from([(
            PathBuf::from("foo.txt"),
            FileChange::Add {
                content: "hello\n".to_string(),
            },
        )])
    );
}

#[tokio::test]
async fn live_app_server_collab_wait_items_render_history() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let sender_thread_id =
        ThreadId::from_string("019cff70-2599-75e2-af72-b90000000001").expect("valid thread id");
    let receiver_thread_id =
        ThreadId::from_string("019cff70-2599-75e2-af72-b958ce5dc1cc").expect("valid thread id");
    let other_receiver_thread_id =
        ThreadId::from_string("019cff70-2599-75e2-af72-b96db334332d").expect("valid thread id");
    chat.set_collab_agent_metadata(
        receiver_thread_id,
        Some("Robie".to_string()),
        Some("explorer".to_string()),
    );
    chat.set_collab_agent_metadata(
        other_receiver_thread_id,
        Some("Ada".to_string()),
        Some("reviewer".to_string()),
    );

    chat.handle_server_notification(
        ServerNotification::ItemStarted(ItemStartedNotification {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item: AppServerThreadItem::CollabAgentToolCall {
                id: "wait-1".to_string(),
                tool: AppServerCollabAgentTool::Wait,
                status: AppServerCollabAgentToolCallStatus::InProgress,
                sender_thread_id: sender_thread_id.to_string(),
                receiver_thread_ids: vec![
                    receiver_thread_id.to_string(),
                    other_receiver_thread_id.to_string(),
                ],
                prompt: None,
                model: None,
                reasoning_effort: None,
                agents_states: HashMap::new(),
            },
        }),
        /*replay_kind*/ None,
    );

    chat.handle_server_notification(
        ServerNotification::ItemCompleted(ItemCompletedNotification {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item: AppServerThreadItem::CollabAgentToolCall {
                id: "wait-1".to_string(),
                tool: AppServerCollabAgentTool::Wait,
                status: AppServerCollabAgentToolCallStatus::Completed,
                sender_thread_id: sender_thread_id.to_string(),
                receiver_thread_ids: vec![
                    receiver_thread_id.to_string(),
                    other_receiver_thread_id.to_string(),
                ],
                prompt: None,
                model: None,
                reasoning_effort: None,
                agents_states: HashMap::from([
                    (
                        receiver_thread_id.to_string(),
                        AppServerCollabAgentState {
                            status: AppServerCollabAgentStatus::Completed,
                            message: Some("Done".to_string()),
                        },
                    ),
                    (
                        other_receiver_thread_id.to_string(),
                        AppServerCollabAgentState {
                            status: AppServerCollabAgentStatus::Running,
                            message: None,
                        },
                    ),
                ]),
            },
        }),
        /*replay_kind*/ None,
    );

    let combined = drain_insert_history(&mut rx)
        .into_iter()
        .map(|lines| lines_to_single_string(&lines))
        .collect::<Vec<_>>()
        .join("\n");
    assert_chatwidget_snapshot!("app_server_collab_wait_items_render_history", combined);
}

#[tokio::test]
async fn live_app_server_collab_spawn_completed_renders_requested_model_and_effort() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let sender_thread_id =
        ThreadId::from_string("019cff70-2599-75e2-af72-b90000000002").expect("valid thread id");
    let spawned_thread_id =
        ThreadId::from_string("019cff70-2599-75e2-af72-b91781b41a8e").expect("valid thread id");

    chat.handle_server_notification(
        ServerNotification::ItemStarted(ItemStartedNotification {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item: AppServerThreadItem::CollabAgentToolCall {
                id: "spawn-1".to_string(),
                tool: AppServerCollabAgentTool::SpawnAgent,
                status: AppServerCollabAgentToolCallStatus::InProgress,
                sender_thread_id: sender_thread_id.to_string(),
                receiver_thread_ids: Vec::new(),
                prompt: Some("Explore the repo".to_string()),
                model: Some("gpt-5".to_string()),
                reasoning_effort: Some(ReasoningEffortConfig::High),
                agents_states: HashMap::new(),
            },
        }),
        /*replay_kind*/ None,
    );

    chat.handle_server_notification(
        ServerNotification::ItemCompleted(ItemCompletedNotification {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
            item: AppServerThreadItem::CollabAgentToolCall {
                id: "spawn-1".to_string(),
                tool: AppServerCollabAgentTool::SpawnAgent,
                status: AppServerCollabAgentToolCallStatus::Completed,
                sender_thread_id: sender_thread_id.to_string(),
                receiver_thread_ids: vec![spawned_thread_id.to_string()],
                prompt: Some("Explore the repo".to_string()),
                model: Some("gpt-5".to_string()),
                reasoning_effort: Some(ReasoningEffortConfig::High),
                agents_states: HashMap::from([(
                    spawned_thread_id.to_string(),
                    AppServerCollabAgentState {
                        status: AppServerCollabAgentStatus::PendingInit,
                        message: None,
                    },
                )]),
            },
        }),
        /*replay_kind*/ None,
    );

    let combined = drain_insert_history(&mut rx)
        .into_iter()
        .map(|lines| lines_to_single_string(&lines))
        .collect::<Vec<_>>()
        .join("\n");
    assert_chatwidget_snapshot!(
        "app_server_collab_spawn_completed_renders_requested_model_and_effort",
        combined
    );
}

#[tokio::test]
async fn live_app_server_failed_turn_does_not_duplicate_error_history() {
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
        /*replay_kind*/ None,
    );

    chat.handle_server_notification(
        ServerNotification::Error(ErrorNotification {
            error: AppServerTurnError {
                message: "permission denied".to_string(),
                codex_error_info: None,
                additional_details: None,
            },
            will_retry: false,
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
        }),
        /*replay_kind*/ None,
    );

    let first_cells = drain_insert_history(&mut rx);
    assert_eq!(first_cells.len(), 1);
    assert!(lines_to_single_string(&first_cells[0]).contains("permission denied"));

    chat.handle_server_notification(
        ServerNotification::TurnCompleted(TurnCompletedNotification {
            thread_id: "thread-1".to_string(),
            turn: AppServerTurn {
                id: "turn-1".to_string(),
                items: Vec::new(),
                status: AppServerTurnStatus::Failed,
                error: Some(AppServerTurnError {
                    message: "permission denied".to_string(),
                    codex_error_info: None,
                    additional_details: None,
                }),
                started_at: None,
                completed_at: Some(0),
                duration_ms: None,
            },
        }),
        /*replay_kind*/ None,
    );

    assert!(drain_insert_history(&mut rx).is_empty());
    assert!(!chat.bottom_pane.is_task_running());
}

#[tokio::test]
async fn live_app_server_stream_recovery_restores_previous_status_header() {
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
        /*replay_kind*/ None,
    );
    drain_insert_history(&mut rx);

    chat.handle_server_notification(
        ServerNotification::Error(ErrorNotification {
            error: AppServerTurnError {
                message: "Reconnecting... 1/5".to_string(),
                codex_error_info: Some(CodexErrorInfo::Other.into()),
                additional_details: None,
            },
            will_retry: true,
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
        }),
        /*replay_kind*/ None,
    );
    drain_insert_history(&mut rx);

    chat.handle_server_notification(
        ServerNotification::AgentMessageDelta(
            codex_app_server_protocol::AgentMessageDeltaNotification {
                thread_id: "thread-1".to_string(),
                turn_id: "turn-1".to_string(),
                item_id: "item-1".to_string(),
                delta: "hello".to_string(),
            },
        ),
        /*replay_kind*/ None,
    );

    let status = chat
        .bottom_pane
        .status_widget()
        .expect("status indicator should be visible");
    assert_eq!(status.header(), "Working");
    assert_eq!(status.details(), None);
    assert!(chat.retry_status_header.is_none());
}

#[tokio::test]
async fn live_app_server_server_overloaded_error_renders_warning() {
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
        /*replay_kind*/ None,
    );
    drain_insert_history(&mut rx);

    chat.handle_server_notification(
        ServerNotification::Error(ErrorNotification {
            error: AppServerTurnError {
                message: "server overloaded".to_string(),
                codex_error_info: Some(CodexErrorInfo::ServerOverloaded.into()),
                additional_details: None,
            },
            will_retry: false,
            thread_id: "thread-1".to_string(),
            turn_id: "turn-1".to_string(),
        }),
        /*replay_kind*/ None,
    );

    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 1);
    assert_eq!(lines_to_single_string(&cells[0]), "⚠ server overloaded\n");
    assert!(!chat.bottom_pane.is_task_running());
}

#[tokio::test]
async fn live_app_server_invalid_thread_name_update_is_ignored() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    chat.thread_name = Some("original name".to_string());

    chat.handle_server_notification(
        ServerNotification::ThreadNameUpdated(
            codex_app_server_protocol::ThreadNameUpdatedNotification {
                thread_id: "not-a-thread-id".to_string(),
                thread_name: Some("bad update".to_string()),
            },
        ),
        /*replay_kind*/ None,
    );

    assert_eq!(chat.thread_id, Some(thread_id));
    assert_eq!(chat.thread_name, Some("original name".to_string()));
}

#[tokio::test]
async fn live_app_server_thread_name_update_shows_resume_hint() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);

    chat.handle_server_notification(
        ServerNotification::ThreadNameUpdated(
            codex_app_server_protocol::ThreadNameUpdatedNotification {
                thread_id: thread_id.to_string(),
                thread_name: Some("review-fix".to_string()),
            },
        ),
        /*replay_kind*/ None,
    );

    assert_eq!(chat.thread_name, Some("review-fix".to_string()));
    let cells = drain_insert_history(&mut rx);
    assert_eq!(cells.len(), 1);
    let rendered = lines_to_single_string(&cells[0]);
    assert!(rendered.contains("Thread renamed to review-fix"));
    assert!(rendered.contains("codex resume review-fix"));
}

#[tokio::test]
async fn live_app_server_thread_closed_requests_immediate_exit() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_server_notification(
        ServerNotification::ThreadClosed(ThreadClosedNotification {
            thread_id: "thread-1".to_string(),
        }),
        /*replay_kind*/ None,
    );

    assert_matches!(rx.try_recv(), Ok(AppEvent::Exit(ExitMode::Immediate)));
}
