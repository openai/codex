//! App-level orchestration tests for the TUI.

mod model_catalog;

use super::*;
use crate::app_backtrack::BacktrackSelection;
use crate::app_backtrack::BacktrackState;
use crate::app_backtrack::user_count;

use crate::chatwidget::ChatWidgetInit;
use crate::chatwidget::create_initial_user_message;
use crate::chatwidget::tests::make_chatwidget_manual_with_sender;
use crate::chatwidget::tests::set_chatgpt_auth;
use crate::chatwidget::tests::set_fast_mode_test_catalog;
use crate::file_search::FileSearchManager;
use crate::history_cell::AgentMessageCell;
use crate::history_cell::HistoryCell;
use crate::history_cell::UserHistoryCell;
use crate::history_cell::new_session_info;
use crate::multi_agents::AgentPickerThreadEntry;
use assert_matches::assert_matches;

use crate::legacy_core::config::ConfigBuilder;
use crate::legacy_core::config::ConfigOverrides;
use codex_app_server_protocol::AdditionalFileSystemPermissions;
use codex_app_server_protocol::AdditionalNetworkPermissions;
use codex_app_server_protocol::AdditionalPermissionProfile;
use codex_app_server_protocol::AgentMessageDeltaNotification;
use codex_app_server_protocol::CommandExecutionRequestApprovalParams;
use codex_app_server_protocol::ConfigWarningNotification;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::McpServerStartupState;
use codex_app_server_protocol::McpServerStatusUpdatedNotification;
use codex_app_server_protocol::NetworkApprovalContext as AppServerNetworkApprovalContext;
use codex_app_server_protocol::NetworkApprovalProtocol as AppServerNetworkApprovalProtocol;
use codex_app_server_protocol::NetworkPolicyAmendment as AppServerNetworkPolicyAmendment;
use codex_app_server_protocol::NetworkPolicyRuleAction as AppServerNetworkPolicyRuleAction;
use codex_app_server_protocol::NonSteerableTurnKind as AppServerNonSteerableTurnKind;
use codex_app_server_protocol::PermissionsRequestApprovalParams;
use codex_app_server_protocol::RequestId as AppServerRequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::Thread;
use codex_app_server_protocol::ThreadClosedNotification;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadStartedNotification;
use codex_app_server_protocol::ThreadTokenUsage;
use codex_app_server_protocol::ThreadTokenUsageUpdatedNotification;
use codex_app_server_protocol::TokenUsageBreakdown;
use codex_app_server_protocol::ToolRequestUserInputParams;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnError as AppServerTurnError;
use codex_app_server_protocol::TurnStartedNotification;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput as AppServerUserInput;
use codex_app_server_protocol::WarningNotification;
use codex_otel::SessionTelemetry;
use codex_protocol::ThreadId;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::CollaborationModeMask;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::models::NetworkPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::NetworkApprovalContext;
use codex_protocol::protocol::NetworkApprovalProtocol;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionConfiguredEvent;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::TurnContextItem;
use codex_protocol::request_permissions::RequestPermissionProfile;
use codex_protocol::user_input::TextElement;
use codex_protocol::user_input::UserInput;
use codex_utils_absolute_path::AbsolutePathBuf;
use crossterm::event::KeyModifiers;
use insta::assert_snapshot;
use pretty_assertions::assert_eq;
use ratatui::prelude::Line;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tempfile::tempdir;
use tokio::time;

macro_rules! assert_app_snapshot {
    ($name:expr, $value:expr $(,)?) => {
        insta::with_settings!({snapshot_path => "../snapshots"}, {
            assert_snapshot!($name, $value);
        });
    };
}

fn test_absolute_path(path: &str) -> AbsolutePathBuf {
    AbsolutePathBuf::try_from(PathBuf::from(path)).expect("absolute test path")
}

#[tokio::test]
async fn handle_mcp_inventory_result_clears_committed_loading_cell() {
    let mut app = make_test_app().await;
    app.transcript_cells
        .push(Arc::new(history_cell::new_mcp_inventory_loading(
            /*animations_enabled*/ false,
        )));

    app.handle_mcp_inventory_result(
        Ok(vec![McpServerStatus {
            name: "docs".to_string(),
            tools: HashMap::new(),
            resources: Vec::new(),
            resource_templates: Vec::new(),
            auth_status: codex_app_server_protocol::McpAuthStatus::Unsupported,
        }]),
        McpServerStatusDetail::ToolsAndAuthOnly,
    );

    assert_eq!(app.transcript_cells.len(), 0);
}

#[test]
fn startup_waiting_gate_is_only_for_fresh_or_exit_session_selection() {
    assert_eq!(
        App::should_wait_for_initial_session(&SessionSelection::StartFresh),
        true
    );
    assert_eq!(
        App::should_wait_for_initial_session(&SessionSelection::Exit),
        true
    );
    assert_eq!(
        App::should_wait_for_initial_session(&SessionSelection::Resume(
            crate::resume_picker::SessionTarget {
                path: Some(PathBuf::from("/tmp/restore")),
                thread_id: ThreadId::new(),
            }
        )),
        false
    );
    assert_eq!(
        App::should_wait_for_initial_session(&SessionSelection::Fork(
            crate::resume_picker::SessionTarget {
                path: Some(PathBuf::from("/tmp/fork")),
                thread_id: ThreadId::new(),
            }
        )),
        false
    );
}

#[test]
fn startup_waiting_gate_holds_active_thread_events_until_primary_thread_configured() {
    let mut wait_for_initial_session =
        App::should_wait_for_initial_session(&SessionSelection::StartFresh);
    assert_eq!(wait_for_initial_session, true);
    assert_eq!(
        App::should_handle_active_thread_events(
            wait_for_initial_session,
            /*has_active_thread_receiver*/ true
        ),
        false
    );

    assert_eq!(
        App::should_stop_waiting_for_initial_session(
            wait_for_initial_session,
            /*primary_thread_id*/ None
        ),
        false
    );
    if App::should_stop_waiting_for_initial_session(wait_for_initial_session, Some(ThreadId::new()))
    {
        wait_for_initial_session = false;
    }
    assert_eq!(wait_for_initial_session, false);

    assert_eq!(
        App::should_handle_active_thread_events(
            wait_for_initial_session,
            /*has_active_thread_receiver*/ true
        ),
        true
    );
}

#[test]
fn startup_waiting_gate_not_applied_for_resume_or_fork_session_selection() {
    let wait_for_resume = App::should_wait_for_initial_session(&SessionSelection::Resume(
        crate::resume_picker::SessionTarget {
            path: Some(PathBuf::from("/tmp/restore")),
            thread_id: ThreadId::new(),
        },
    ));
    assert_eq!(
        App::should_handle_active_thread_events(
            wait_for_resume,
            /*has_active_thread_receiver*/ true
        ),
        true
    );
    let wait_for_fork = App::should_wait_for_initial_session(&SessionSelection::Fork(
        crate::resume_picker::SessionTarget {
            path: Some(PathBuf::from("/tmp/fork")),
            thread_id: ThreadId::new(),
        },
    ));
    assert_eq!(
        App::should_handle_active_thread_events(
            wait_for_fork,
            /*has_active_thread_receiver*/ true
        ),
        true
    );
}

#[tokio::test]
async fn ignore_same_thread_resume_reports_noop_for_current_thread() {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let thread_id = ThreadId::new();
    let session = test_thread_session(thread_id, test_path_buf("/tmp/project"));
    app.chat_widget.handle_thread_session(session.clone());
    app.thread_event_channels.insert(
        thread_id,
        ThreadEventChannel::new_with_session(THREAD_EVENT_CHANNEL_CAPACITY, session, Vec::new()),
    );
    app.activate_thread_channel(thread_id).await;
    while app_event_rx.try_recv().is_ok() {}

    let ignored = app.ignore_same_thread_resume(&crate::resume_picker::SessionTarget {
        path: Some(test_path_buf("/tmp/project")),
        thread_id,
    });

    assert!(ignored);
    let cell = match app_event_rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => cell,
        other => panic!("expected info message after same-thread resume, saw {other:?}"),
    };
    let rendered = lines_to_single_string(&cell.display_lines(/*width*/ 80));
    assert!(rendered.contains(&format!(
        "Already viewing {}.",
        test_path_display("/tmp/project")
    )));
}

#[tokio::test]
async fn ignore_same_thread_resume_allows_reattaching_displayed_inactive_thread() {
    let mut app = make_test_app().await;
    let thread_id = ThreadId::new();
    let session = test_thread_session(thread_id, test_path_buf("/tmp/project"));
    app.chat_widget.handle_thread_session(session);

    let ignored = app.ignore_same_thread_resume(&crate::resume_picker::SessionTarget {
        path: Some(test_path_buf("/tmp/project")),
        thread_id,
    });

    assert!(!ignored);
    assert!(app.transcript_cells.is_empty());
}

#[tokio::test]
async fn enqueue_primary_thread_session_replays_buffered_approval_after_attach() -> Result<()> {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let thread_id = ThreadId::new();
    let approval_request =
        exec_approval_request(thread_id, "turn-1", "call-1", /*approval_id*/ None);

    assert_eq!(
        app.pending_app_server_requests
            .note_server_request(&approval_request),
        None
    );
    app.enqueue_primary_thread_request(approval_request).await?;
    app.enqueue_primary_thread_session(
        test_thread_session(thread_id, test_path_buf("/tmp/project")),
        Vec::new(),
    )
    .await?;

    let rx = app
        .active_thread_rx
        .as_mut()
        .expect("primary thread receiver should be active");
    let event = time::timeout(Duration::from_millis(50), rx.recv())
        .await
        .expect("timed out waiting for buffered approval event")
        .expect("channel closed unexpectedly");

    assert!(matches!(
        &event,
        ThreadBufferedEvent::Request(ServerRequest::CommandExecutionRequestApproval {
            params,
            ..
        }) if params.turn_id == "turn-1"
    ));

    app.handle_thread_event_now(event);
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));

    while let Ok(app_event) = app_event_rx.try_recv() {
        if let AppEvent::SubmitThreadOp {
            thread_id: op_thread_id,
            ..
        } = app_event
        {
            assert_eq!(op_thread_id, thread_id);
            return Ok(());
        }
    }

    panic!("expected approval action to submit a thread-scoped op");
}

#[tokio::test]
async fn resolved_buffered_approval_does_not_become_actionable_after_drain() -> Result<()> {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let thread_id = ThreadId::new();
    let approval_request =
        exec_approval_request(thread_id, "turn-1", "call-1", /*approval_id*/ None);

    app.enqueue_primary_thread_session(
        test_thread_session(thread_id, test_path_buf("/tmp/project")),
        Vec::new(),
    )
    .await?;
    while app_event_rx.try_recv().is_ok() {}

    assert_eq!(
        app.pending_app_server_requests
            .note_server_request(&approval_request),
        None
    );
    app.enqueue_thread_request(thread_id, approval_request)
        .await?;

    let resolved = app
        .pending_app_server_requests
        .resolve_notification(&AppServerRequestId::Integer(1))
        .expect("matching app-server request should resolve");
    app.chat_widget.dismiss_app_server_request(&resolved);
    while app_event_rx.try_recv().is_ok() {}

    let rx = app
        .active_thread_rx
        .as_mut()
        .expect("primary thread receiver should be active");
    let event = time::timeout(Duration::from_millis(50), rx.recv())
        .await
        .expect("timed out waiting for buffered approval event")
        .expect("channel closed unexpectedly");

    assert!(matches!(
        &event,
        ThreadBufferedEvent::Request(ServerRequest::CommandExecutionRequestApproval {
            params,
            ..
        }) if params.turn_id == "turn-1"
    ));

    app.handle_thread_event_now(event);
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));

    while let Ok(app_event) = app_event_rx.try_recv() {
        assert!(
            !matches!(app_event, AppEvent::SubmitThreadOp { .. }),
            "resolved buffered approval should not become actionable"
        );
    }

    Ok(())
}

#[tokio::test]
async fn enqueue_primary_thread_session_replays_turns_before_initial_prompt_submit() -> Result<()> {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let thread_id = ThreadId::new();
    let initial_prompt = "follow-up after replay".to_string();
    let config = app.config.clone();
    let model = crate::legacy_core::test_support::get_model_offline(config.model.as_deref());
    app.chat_widget = ChatWidget::new_with_app_event(ChatWidgetInit {
        config,
        frame_requester: crate::tui::FrameRequester::test_dummy(),
        app_event_tx: app.app_event_tx.clone(),
        initial_user_message: create_initial_user_message(
            Some(initial_prompt.clone()),
            Vec::new(),
            Vec::new(),
        ),
        enhanced_keys_supported: false,
        has_chatgpt_account: false,
        model_catalog: app.model_catalog.clone(),
        feedback: codex_feedback::CodexFeedback::new(),
        is_first_run: false,
        status_account_display: None,
        initial_plan_type: None,
        model: Some(model),
        startup_tooltip_override: None,
        status_line_invalid_items_warned: app.status_line_invalid_items_warned.clone(),
        terminal_title_invalid_items_warned: app.terminal_title_invalid_items_warned.clone(),
        session_telemetry: app.session_telemetry.clone(),
    });

    app.enqueue_primary_thread_session(
        test_thread_session(thread_id, test_path_buf("/tmp/project")),
        vec![test_turn(
            "turn-1",
            TurnStatus::Completed,
            vec![ThreadItem::UserMessage {
                id: "user-1".to_string(),
                content: vec![AppServerUserInput::Text {
                    text: "earlier prompt".to_string(),
                    text_elements: Vec::new(),
                }],
            }],
        )],
    )
    .await?;

    let mut saw_replayed_answer = false;
    let mut submitted_items = None;
    while let Ok(event) = app_event_rx.try_recv() {
        match event {
            AppEvent::InsertHistoryCell(cell) => {
                let transcript = lines_to_single_string(&cell.transcript_lines(/*width*/ 80));
                saw_replayed_answer |= transcript.contains("earlier prompt");
            }
            AppEvent::SubmitThreadOp {
                thread_id: op_thread_id,
                op: Op::UserTurn { items, .. },
            } => {
                assert_eq!(op_thread_id, thread_id);
                submitted_items = Some(items);
            }
            AppEvent::CodexOp(Op::UserTurn { items, .. }) => {
                submitted_items = Some(items);
            }
            _ => {}
        }
    }
    assert!(
        saw_replayed_answer,
        "expected replayed history before initial prompt submit"
    );
    assert_eq!(
        submitted_items,
        Some(vec![UserInput::Text {
            text: initial_prompt,
            text_elements: Vec::new(),
        }])
    );

    Ok(())
}

#[tokio::test]
async fn reset_thread_event_state_aborts_listener_tasks() {
    struct NotifyOnDrop(Option<tokio::sync::oneshot::Sender<()>>);

    impl Drop for NotifyOnDrop {
        fn drop(&mut self) {
            if let Some(tx) = self.0.take() {
                let _ = tx.send(());
            }
        }
    }

    let mut app = make_test_app().await;
    let thread_id = ThreadId::new();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (dropped_tx, dropped_rx) = tokio::sync::oneshot::channel();
    let handle = tokio::spawn(async move {
        let _notify_on_drop = NotifyOnDrop(Some(dropped_tx));
        let _ = started_tx.send(());
        std::future::pending::<()>().await;
    });
    app.thread_event_listener_tasks.insert(thread_id, handle);
    started_rx
        .await
        .expect("listener task should report it started");

    app.reset_thread_event_state();

    assert_eq!(app.thread_event_listener_tasks.is_empty(), true);
    time::timeout(Duration::from_millis(50), dropped_rx)
        .await
        .expect("timed out waiting for listener task abort")
        .expect("listener task drop notification should succeed");
}

#[tokio::test]
async fn history_lookup_response_is_routed_to_requesting_thread() -> Result<()> {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let thread_id = ThreadId::new();

    let handled = app
        .try_handle_local_history_op(
            thread_id,
            &Op::GetHistoryEntryRequest {
                offset: 0,
                log_id: 1,
            }
            .into(),
        )
        .await?;

    assert!(handled);

    let app_event = tokio::time::timeout(Duration::from_secs(1), app_event_rx.recv())
        .await
        .expect("history lookup should emit an app event")
        .expect("app event channel should stay open");

    let AppEvent::ThreadHistoryEntryResponse {
        thread_id: routed_thread_id,
        event,
    } = app_event
    else {
        panic!("expected thread-routed history response");
    };
    assert_eq!(routed_thread_id, thread_id);
    assert_eq!(event.offset, 0);
    assert_eq!(event.log_id, 1);
    assert!(event.entry.is_none());

    Ok(())
}

#[tokio::test]
async fn enqueue_thread_event_does_not_block_when_channel_full() -> Result<()> {
    let mut app = make_test_app().await;
    let thread_id = ThreadId::new();
    app.thread_event_channels
        .insert(thread_id, ThreadEventChannel::new(/*capacity*/ 1));
    app.set_thread_active(thread_id, /*active*/ true).await;

    let event = thread_closed_notification(thread_id);

    app.enqueue_thread_notification(thread_id, event.clone())
        .await?;
    time::timeout(
        Duration::from_millis(50),
        app.enqueue_thread_notification(thread_id, event),
    )
    .await
    .expect("enqueue_thread_notification blocked on a full channel")?;

    let mut rx = app
        .thread_event_channels
        .get_mut(&thread_id)
        .expect("missing thread channel")
        .receiver
        .take()
        .expect("missing receiver");

    time::timeout(Duration::from_millis(50), rx.recv())
        .await
        .expect("timed out waiting for first event")
        .expect("channel closed unexpectedly");
    time::timeout(Duration::from_millis(50), rx.recv())
        .await
        .expect("timed out waiting for second event")
        .expect("channel closed unexpectedly");

    Ok(())
}

#[tokio::test]
async fn replay_thread_snapshot_restores_draft_and_queued_input() {
    let mut app = make_test_app().await;
    let thread_id = ThreadId::new();
    let session = test_thread_session(thread_id, test_path_buf("/tmp/project"));
    app.thread_event_channels.insert(
        thread_id,
        ThreadEventChannel::new_with_session(
            THREAD_EVENT_CHANNEL_CAPACITY,
            session.clone(),
            Vec::new(),
        ),
    );
    app.activate_thread_channel(thread_id).await;
    app.chat_widget.handle_thread_session(session.clone());

    app.chat_widget
        .apply_external_edit("draft prompt".to_string());
    app.chat_widget.submit_user_message_with_mode(
        "queued follow-up".to_string(),
        CollaborationModeMask {
            name: "Default".to_string(),
            mode: None,
            model: None,
            reasoning_effort: None,
            developer_instructions: None,
        },
    );
    let expected_input_state = app
        .chat_widget
        .capture_thread_input_state()
        .expect("expected thread input state");

    app.store_active_thread_receiver().await;

    let snapshot = {
        let channel = app
            .thread_event_channels
            .get(&thread_id)
            .expect("thread channel should exist");
        let store = channel.store.lock().await;
        assert_eq!(store.input_state, Some(expected_input_state));
        store.snapshot()
    };

    let (chat_widget, _app_event_tx, _rx, mut new_op_rx) =
        make_chatwidget_manual_with_sender().await;
    app.chat_widget = chat_widget;

    app.replay_thread_snapshot(snapshot, /*resume_restored_queue*/ true);

    assert_eq!(app.chat_widget.composer_text_with_pending(), "draft prompt");
    assert!(app.chat_widget.queued_user_message_texts().is_empty());
    while let Ok(op) = new_op_rx.try_recv() {
        assert!(
            !matches!(op, Op::UserTurn { .. }),
            "draft-only replay should not auto-submit queued input"
        );
    }
}

#[tokio::test]
async fn active_turn_id_for_thread_uses_snapshot_turns() {
    let mut app = make_test_app().await;
    let thread_id = ThreadId::new();
    let session = test_thread_session(thread_id, test_path_buf("/tmp/project"));
    app.thread_event_channels.insert(
        thread_id,
        ThreadEventChannel::new_with_session(
            THREAD_EVENT_CHANNEL_CAPACITY,
            session,
            vec![test_turn("turn-1", TurnStatus::InProgress, Vec::new())],
        ),
    );

    assert_eq!(
        app.active_turn_id_for_thread(thread_id).await,
        Some("turn-1".to_string())
    );
}

#[tokio::test]
async fn replayed_turn_complete_submits_restored_queued_follow_up() {
    let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let thread_id = ThreadId::new();
    let session = test_thread_session(thread_id, test_path_buf("/tmp/project"));
    app.chat_widget.handle_thread_session(session.clone());
    app.chat_widget.handle_server_notification(
        turn_started_notification(thread_id, "turn-1"),
        /*replay_kind*/ None,
    );
    app.chat_widget.handle_server_notification(
        agent_message_delta_notification(thread_id, "turn-1", "agent-1", "streaming"),
        /*replay_kind*/ None,
    );
    app.chat_widget
        .apply_external_edit("queued follow-up".to_string());
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    let input_state = app
        .chat_widget
        .capture_thread_input_state()
        .expect("expected queued follow-up state");

    let (chat_widget, _app_event_tx, _rx, mut new_op_rx) =
        make_chatwidget_manual_with_sender().await;
    app.chat_widget = chat_widget;
    app.chat_widget.handle_thread_session(session.clone());
    while new_op_rx.try_recv().is_ok() {}
    app.replay_thread_snapshot(
        ThreadEventSnapshot {
            session: None,
            turns: Vec::new(),
            events: vec![ThreadBufferedEvent::Notification(
                turn_completed_notification(thread_id, "turn-1", TurnStatus::Completed),
            )],
            input_state: Some(input_state),
        },
        /*resume_restored_queue*/ true,
    );

    match next_user_turn_op(&mut new_op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "queued follow-up".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected queued follow-up submission, got {other:?}"),
    }
}

#[tokio::test]
async fn replay_only_thread_keeps_restored_queue_visible() {
    let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let thread_id = ThreadId::new();
    let session = test_thread_session(thread_id, test_path_buf("/tmp/project"));
    app.chat_widget.handle_thread_session(session.clone());
    app.chat_widget.handle_server_notification(
        turn_started_notification(thread_id, "turn-1"),
        /*replay_kind*/ None,
    );
    app.chat_widget.handle_server_notification(
        agent_message_delta_notification(thread_id, "turn-1", "agent-1", "streaming"),
        /*replay_kind*/ None,
    );
    app.chat_widget
        .apply_external_edit("queued follow-up".to_string());
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    let input_state = app
        .chat_widget
        .capture_thread_input_state()
        .expect("expected queued follow-up state");

    let (chat_widget, _app_event_tx, _rx, mut new_op_rx) =
        make_chatwidget_manual_with_sender().await;
    app.chat_widget = chat_widget;
    app.chat_widget.handle_thread_session(session.clone());
    while new_op_rx.try_recv().is_ok() {}

    app.replay_thread_snapshot(
        ThreadEventSnapshot {
            session: None,
            turns: Vec::new(),
            events: vec![ThreadBufferedEvent::Notification(
                turn_completed_notification(thread_id, "turn-1", TurnStatus::Completed),
            )],
            input_state: Some(input_state),
        },
        /*resume_restored_queue*/ false,
    );

    assert_eq!(
        app.chat_widget.queued_user_message_texts(),
        vec!["queued follow-up".to_string()]
    );
    assert!(
        new_op_rx.try_recv().is_err(),
        "replay-only threads should not auto-submit restored queue"
    );
}

#[tokio::test]
async fn replay_thread_snapshot_keeps_queue_when_running_state_only_comes_from_snapshot() {
    let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let thread_id = ThreadId::new();
    let session = test_thread_session(thread_id, test_path_buf("/tmp/project"));
    app.chat_widget.handle_thread_session(session.clone());
    app.chat_widget.handle_server_notification(
        turn_started_notification(thread_id, "turn-1"),
        /*replay_kind*/ None,
    );
    app.chat_widget.handle_server_notification(
        agent_message_delta_notification(thread_id, "turn-1", "agent-1", "streaming"),
        /*replay_kind*/ None,
    );
    app.chat_widget
        .apply_external_edit("queued follow-up".to_string());
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    let input_state = app
        .chat_widget
        .capture_thread_input_state()
        .expect("expected queued follow-up state");

    let (chat_widget, _app_event_tx, _rx, mut new_op_rx) =
        make_chatwidget_manual_with_sender().await;
    app.chat_widget = chat_widget;
    app.chat_widget.handle_thread_session(session.clone());
    while new_op_rx.try_recv().is_ok() {}

    app.replay_thread_snapshot(
        ThreadEventSnapshot {
            session: None,
            turns: Vec::new(),
            events: vec![],
            input_state: Some(input_state),
        },
        /*resume_restored_queue*/ true,
    );

    assert_eq!(
        app.chat_widget.queued_user_message_texts(),
        vec!["queued follow-up".to_string()]
    );
    assert!(
        new_op_rx.try_recv().is_err(),
        "restored queue should stay queued when replay did not prove the turn finished"
    );
}

#[tokio::test]
async fn replay_thread_snapshot_in_progress_turn_restores_running_queue_state() {
    let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let thread_id = ThreadId::new();
    let session = test_thread_session(thread_id, test_path_buf("/tmp/project"));
    app.chat_widget.handle_thread_session(session.clone());
    app.chat_widget.handle_server_notification(
        turn_started_notification(thread_id, "turn-1"),
        /*replay_kind*/ None,
    );
    app.chat_widget.handle_server_notification(
        agent_message_delta_notification(thread_id, "turn-1", "agent-1", "streaming"),
        /*replay_kind*/ None,
    );
    app.chat_widget
        .apply_external_edit("queued follow-up".to_string());
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    let input_state = app
        .chat_widget
        .capture_thread_input_state()
        .expect("expected queued follow-up state");

    let (chat_widget, _app_event_tx, _rx, mut new_op_rx) =
        make_chatwidget_manual_with_sender().await;
    app.chat_widget = chat_widget;
    app.chat_widget.handle_thread_session(session.clone());
    while new_op_rx.try_recv().is_ok() {}

    app.replay_thread_snapshot(
        ThreadEventSnapshot {
            session: None,
            turns: vec![test_turn("turn-1", TurnStatus::InProgress, Vec::new())],
            events: Vec::new(),
            input_state: Some(input_state),
        },
        /*resume_restored_queue*/ true,
    );

    assert_eq!(
        app.chat_widget.queued_user_message_texts(),
        vec!["queued follow-up".to_string()]
    );
    assert!(
        new_op_rx.try_recv().is_err(),
        "restored queue should stay queued while replayed turn is still running"
    );
}

#[tokio::test]
async fn replay_thread_snapshot_in_progress_turn_restores_running_state_without_input_state() {
    let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let thread_id = ThreadId::new();
    let session = test_thread_session(thread_id, test_path_buf("/tmp/project"));
    let (chat_widget, _app_event_tx, _rx, _new_op_rx) = make_chatwidget_manual_with_sender().await;
    app.chat_widget = chat_widget;
    app.chat_widget.handle_thread_session(session);

    app.replay_thread_snapshot(
        ThreadEventSnapshot {
            session: None,
            turns: vec![test_turn("turn-1", TurnStatus::InProgress, Vec::new())],
            events: Vec::new(),
            input_state: None,
        },
        /*resume_restored_queue*/ false,
    );

    assert!(app.chat_widget.is_task_running_for_test());
}

#[tokio::test]
async fn replay_thread_snapshot_does_not_submit_queue_before_replay_catches_up() {
    let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let thread_id = ThreadId::new();
    let session = test_thread_session(thread_id, test_path_buf("/tmp/project"));
    app.chat_widget.handle_thread_session(session.clone());
    app.chat_widget.handle_server_notification(
        turn_started_notification(thread_id, "turn-1"),
        /*replay_kind*/ None,
    );
    app.chat_widget.handle_server_notification(
        agent_message_delta_notification(thread_id, "turn-1", "agent-1", "streaming"),
        /*replay_kind*/ None,
    );
    app.chat_widget
        .apply_external_edit("queued follow-up".to_string());
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    let input_state = app
        .chat_widget
        .capture_thread_input_state()
        .expect("expected queued follow-up state");

    let (chat_widget, _app_event_tx, _rx, mut new_op_rx) =
        make_chatwidget_manual_with_sender().await;
    app.chat_widget = chat_widget;
    app.chat_widget.handle_thread_session(session.clone());
    while new_op_rx.try_recv().is_ok() {}

    app.replay_thread_snapshot(
        ThreadEventSnapshot {
            session: None,
            turns: Vec::new(),
            events: vec![
                ThreadBufferedEvent::Notification(turn_completed_notification(
                    thread_id,
                    "turn-0",
                    TurnStatus::Completed,
                )),
                ThreadBufferedEvent::Notification(turn_started_notification(thread_id, "turn-1")),
            ],
            input_state: Some(input_state),
        },
        /*resume_restored_queue*/ true,
    );

    assert!(
        new_op_rx.try_recv().is_err(),
        "queued follow-up should stay queued until the latest turn completes"
    );
    assert_eq!(
        app.chat_widget.queued_user_message_texts(),
        vec!["queued follow-up".to_string()]
    );

    app.chat_widget.handle_server_notification(
        turn_completed_notification(thread_id, "turn-1", TurnStatus::Completed),
        /*replay_kind*/ None,
    );

    match next_user_turn_op(&mut new_op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "queued follow-up".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected queued follow-up submission, got {other:?}"),
    }
}

#[tokio::test]
async fn replay_thread_snapshot_restores_pending_pastes_for_submit() {
    let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let thread_id = ThreadId::new();
    let session = test_thread_session(thread_id, test_path_buf("/tmp/project"));
    app.thread_event_channels.insert(
        thread_id,
        ThreadEventChannel::new_with_session(
            THREAD_EVENT_CHANNEL_CAPACITY,
            session.clone(),
            Vec::new(),
        ),
    );
    app.activate_thread_channel(thread_id).await;
    app.chat_widget.handle_thread_session(session);

    let large = "x".repeat(1005);
    app.chat_widget.handle_paste(large.clone());
    let expected_input_state = app
        .chat_widget
        .capture_thread_input_state()
        .expect("expected thread input state");

    app.store_active_thread_receiver().await;

    let snapshot = {
        let channel = app
            .thread_event_channels
            .get(&thread_id)
            .expect("thread channel should exist");
        let store = channel.store.lock().await;
        assert_eq!(store.input_state, Some(expected_input_state));
        store.snapshot()
    };

    let (chat_widget, _app_event_tx, _rx, mut new_op_rx) =
        make_chatwidget_manual_with_sender().await;
    app.chat_widget = chat_widget;
    app.replay_thread_snapshot(snapshot, /*resume_restored_queue*/ true);

    assert_eq!(app.chat_widget.composer_text_with_pending(), large);

    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    match next_user_turn_op(&mut new_op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: large,
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected restored paste submission, got {other:?}"),
    }
}

#[tokio::test]
async fn replay_thread_snapshot_restores_collaboration_mode_for_draft_submit() {
    let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let thread_id = ThreadId::new();
    let session = test_thread_session(thread_id, test_path_buf("/tmp/project"));
    app.chat_widget.handle_thread_session(session.clone());
    app.chat_widget
        .set_reasoning_effort(Some(ReasoningEffortConfig::High));
    app.chat_widget
        .set_collaboration_mask(CollaborationModeMask {
            name: "Plan".to_string(),
            mode: Some(ModeKind::Plan),
            model: Some("gpt-restored".to_string()),
            reasoning_effort: Some(Some(ReasoningEffortConfig::High)),
            developer_instructions: None,
        });
    app.chat_widget
        .apply_external_edit("draft prompt".to_string());
    let input_state = app
        .chat_widget
        .capture_thread_input_state()
        .expect("expected draft input state");

    let (chat_widget, _app_event_tx, _rx, mut new_op_rx) =
        make_chatwidget_manual_with_sender().await;
    app.chat_widget = chat_widget;
    app.chat_widget.handle_thread_session(session.clone());
    app.chat_widget
        .set_reasoning_effort(Some(ReasoningEffortConfig::Low));
    app.chat_widget
        .set_collaboration_mask(CollaborationModeMask {
            name: "Default".to_string(),
            mode: Some(ModeKind::Default),
            model: Some("gpt-replacement".to_string()),
            reasoning_effort: Some(Some(ReasoningEffortConfig::Low)),
            developer_instructions: None,
        });
    while new_op_rx.try_recv().is_ok() {}

    app.replay_thread_snapshot(
        ThreadEventSnapshot {
            session: None,
            turns: Vec::new(),
            events: vec![],
            input_state: Some(input_state),
        },
        /*resume_restored_queue*/ true,
    );
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    match next_user_turn_op(&mut new_op_rx) {
        Op::UserTurn {
            items,
            model,
            effort,
            collaboration_mode,
            ..
        } => {
            assert_eq!(
                items,
                vec![UserInput::Text {
                    text: "draft prompt".to_string(),
                    text_elements: Vec::new(),
                }]
            );
            assert_eq!(model, "gpt-restored".to_string());
            assert_eq!(effort, Some(ReasoningEffortConfig::High));
            assert_eq!(
                collaboration_mode,
                Some(CollaborationMode {
                    mode: ModeKind::Plan,
                    settings: Settings {
                        model: "gpt-restored".to_string(),
                        reasoning_effort: Some(ReasoningEffortConfig::High),
                        developer_instructions: None,
                    },
                })
            );
        }
        other => panic!("expected restored draft submission, got {other:?}"),
    }
}

#[tokio::test]
async fn replay_thread_snapshot_restores_collaboration_mode_without_input() {
    let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let thread_id = ThreadId::new();
    let session = test_thread_session(thread_id, test_path_buf("/tmp/project"));
    app.chat_widget.handle_thread_session(session.clone());
    app.chat_widget
        .set_reasoning_effort(Some(ReasoningEffortConfig::High));
    app.chat_widget
        .set_collaboration_mask(CollaborationModeMask {
            name: "Plan".to_string(),
            mode: Some(ModeKind::Plan),
            model: Some("gpt-restored".to_string()),
            reasoning_effort: Some(Some(ReasoningEffortConfig::High)),
            developer_instructions: None,
        });
    let input_state = app
        .chat_widget
        .capture_thread_input_state()
        .expect("expected collaboration-only input state");

    let (chat_widget, _app_event_tx, _rx, _new_op_rx) = make_chatwidget_manual_with_sender().await;
    app.chat_widget = chat_widget;
    app.chat_widget.handle_thread_session(session.clone());
    app.chat_widget
        .set_reasoning_effort(Some(ReasoningEffortConfig::Low));
    app.chat_widget
        .set_collaboration_mask(CollaborationModeMask {
            name: "Default".to_string(),
            mode: Some(ModeKind::Default),
            model: Some("gpt-replacement".to_string()),
            reasoning_effort: Some(Some(ReasoningEffortConfig::Low)),
            developer_instructions: None,
        });

    app.replay_thread_snapshot(
        ThreadEventSnapshot {
            session: None,
            turns: Vec::new(),
            events: vec![],
            input_state: Some(input_state),
        },
        /*resume_restored_queue*/ true,
    );

    assert_eq!(
        app.chat_widget.active_collaboration_mode_kind(),
        ModeKind::Plan
    );
    assert_eq!(app.chat_widget.current_model(), "gpt-restored");
    assert_eq!(
        app.chat_widget.current_reasoning_effort(),
        Some(ReasoningEffortConfig::High)
    );
}

#[tokio::test]
async fn replayed_interrupted_turn_restores_queued_input_to_composer() {
    let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let thread_id = ThreadId::new();
    let session = test_thread_session(thread_id, test_path_buf("/tmp/project"));
    app.chat_widget.handle_thread_session(session.clone());
    app.chat_widget.handle_server_notification(
        turn_started_notification(thread_id, "turn-1"),
        /*replay_kind*/ None,
    );
    app.chat_widget.handle_server_notification(
        agent_message_delta_notification(thread_id, "turn-1", "agent-1", "streaming"),
        /*replay_kind*/ None,
    );
    app.chat_widget
        .apply_external_edit("queued follow-up".to_string());
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    let input_state = app
        .chat_widget
        .capture_thread_input_state()
        .expect("expected queued follow-up state");

    let (chat_widget, _app_event_tx, _rx, mut new_op_rx) =
        make_chatwidget_manual_with_sender().await;
    app.chat_widget = chat_widget;
    app.chat_widget.handle_thread_session(session.clone());
    while new_op_rx.try_recv().is_ok() {}

    app.replay_thread_snapshot(
        ThreadEventSnapshot {
            session: None,
            turns: Vec::new(),
            events: vec![ThreadBufferedEvent::Notification(
                turn_completed_notification(thread_id, "turn-1", TurnStatus::Interrupted),
            )],
            input_state: Some(input_state),
        },
        /*resume_restored_queue*/ true,
    );

    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "queued follow-up"
    );
    assert!(app.chat_widget.queued_user_message_texts().is_empty());
    assert!(
        new_op_rx.try_recv().is_err(),
        "replayed interrupted turns should restore queued input for editing, not submit it"
    );
}

#[tokio::test]
async fn token_usage_update_refreshes_status_line_with_runtime_context_window() {
    let mut app = make_test_app().await;
    app.chat_widget
        .setup_status_line(vec![crate::bottom_pane::StatusLineItem::ContextWindowSize]);

    assert_eq!(app.chat_widget.status_line_text(), None);

    app.handle_thread_event_now(ThreadBufferedEvent::Notification(token_usage_notification(
        ThreadId::new(),
        "turn-1",
        Some(950_000),
    )));

    assert_eq!(
        app.chat_widget.status_line_text(),
        Some("950K window".into())
    );
}

#[tokio::test]
async fn open_agent_picker_keeps_missing_threads_for_replay() -> Result<()> {
    let mut app = make_test_app().await;
    let mut app_server = crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
        .await
        .expect("embedded app server");
    let thread_id = ThreadId::new();
    app.thread_event_channels
        .insert(thread_id, ThreadEventChannel::new(/*capacity*/ 1));

    app.open_agent_picker(&mut app_server).await;

    assert_eq!(app.thread_event_channels.contains_key(&thread_id), true);
    assert_eq!(
        app.agent_navigation.get(&thread_id),
        Some(&AgentPickerThreadEntry {
            agent_nickname: None,
            agent_role: None,
            is_closed: true,
        })
    );
    assert_eq!(app.agent_navigation.ordered_thread_ids(), vec![thread_id]);
    Ok(())
}

#[tokio::test]
async fn open_agent_picker_preserves_cached_metadata_for_replay_threads() -> Result<()> {
    let mut app = make_test_app().await;
    let mut app_server = crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
        .await
        .expect("embedded app server");
    let thread_id = ThreadId::new();
    app.thread_event_channels
        .insert(thread_id, ThreadEventChannel::new(/*capacity*/ 1));
    app.agent_navigation.upsert(
        thread_id,
        Some("Robie".to_string()),
        Some("explorer".to_string()),
        /*is_closed*/ true,
    );

    app.open_agent_picker(&mut app_server).await;

    assert_eq!(app.thread_event_channels.contains_key(&thread_id), true);
    assert_eq!(
        app.agent_navigation.get(&thread_id),
        Some(&AgentPickerThreadEntry {
            agent_nickname: Some("Robie".to_string()),
            agent_role: Some("explorer".to_string()),
            is_closed: true,
        })
    );
    Ok(())
}

#[tokio::test]
async fn open_agent_picker_prunes_terminal_metadata_only_threads() -> Result<()> {
    let mut app = make_test_app().await;
    let mut app_server = crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
        .await
        .expect("embedded app server");
    let thread_id = ThreadId::new();
    app.agent_navigation.upsert(
        thread_id,
        Some("Ghost".to_string()),
        Some("worker".to_string()),
        /*is_closed*/ false,
    );

    app.open_agent_picker(&mut app_server).await;

    assert_eq!(app.agent_navigation.get(&thread_id), None);
    assert!(app.agent_navigation.is_empty());
    Ok(())
}

#[tokio::test]
async fn open_agent_picker_marks_terminal_read_errors_closed() -> Result<()> {
    let mut app = make_test_app().await;
    let mut app_server = crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
        .await
        .expect("embedded app server");
    let thread_id = ThreadId::new();
    app.thread_event_channels
        .insert(thread_id, ThreadEventChannel::new(/*capacity*/ 1));
    app.agent_navigation.upsert(
        thread_id,
        Some("Robie".to_string()),
        Some("explorer".to_string()),
        /*is_closed*/ false,
    );

    app.open_agent_picker(&mut app_server).await;

    assert_eq!(
        app.agent_navigation.get(&thread_id),
        Some(&AgentPickerThreadEntry {
            agent_nickname: Some("Robie".to_string()),
            agent_role: Some("explorer".to_string()),
            is_closed: true,
        })
    );
    Ok(())
}

#[tokio::test]
async fn open_agent_picker_marks_loaded_threads_open() -> Result<()> {
    let mut app = make_test_app().await;
    let mut app_server = crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
        .await
        .expect("embedded app server");
    let started = app_server
        .start_thread(app.chat_widget.config_ref())
        .await?;
    let thread_id = started.session.thread_id;
    app.thread_event_channels
        .insert(thread_id, ThreadEventChannel::new(/*capacity*/ 1));

    app.open_agent_picker(&mut app_server).await;

    assert_eq!(
        app.agent_navigation.get(&thread_id),
        Some(&AgentPickerThreadEntry {
            agent_nickname: None,
            agent_role: None,
            is_closed: false,
        })
    );
    Ok(())
}

#[tokio::test]
async fn attach_live_thread_for_selection_rejects_empty_non_ephemeral_fallback_threads()
-> Result<()> {
    let mut app = make_test_app().await;
    let mut app_server = crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
        .await
        .expect("embedded app server");
    let started = app_server
        .start_thread(app.chat_widget.config_ref())
        .await?;
    let thread_id = started.session.thread_id;
    app.agent_navigation.upsert(
        thread_id,
        Some("Scout".to_string()),
        Some("worker".to_string()),
        /*is_closed*/ false,
    );

    let err = app
        .attach_live_thread_for_selection(&mut app_server, thread_id)
        .await
        .expect_err("empty fallback should not attach as a blank replay-only thread");

    assert_eq!(
        err.to_string(),
        format!("Agent thread {thread_id} is not yet available for replay or live attach.")
    );
    assert!(!app.thread_event_channels.contains_key(&thread_id));
    Ok(())
}

#[tokio::test]
async fn attach_live_thread_for_selection_rejects_unmaterialized_fallback_threads() -> Result<()> {
    let mut app = make_test_app().await;
    let mut app_server = crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
        .await
        .expect("embedded app server");
    let mut ephemeral_config = app.chat_widget.config_ref().clone();
    ephemeral_config.ephemeral = true;
    let started = app_server.start_thread(&ephemeral_config).await?;
    let thread_id = started.session.thread_id;
    app.agent_navigation.upsert(
        thread_id,
        Some("Scout".to_string()),
        Some("worker".to_string()),
        /*is_closed*/ false,
    );

    let err = app
        .attach_live_thread_for_selection(&mut app_server, thread_id)
        .await
        .expect_err("ephemeral fallback should not attach as a blank live thread");

    assert_eq!(
        err.to_string(),
        format!("Agent thread {thread_id} is not yet available for replay or live attach.")
    );
    assert!(!app.thread_event_channels.contains_key(&thread_id));
    Ok(())
}

#[tokio::test]
async fn should_attach_live_thread_for_selection_skips_closed_metadata_only_threads() {
    let mut app = make_test_app().await;
    let thread_id = ThreadId::new();
    app.agent_navigation.upsert(
        thread_id,
        Some("Ghost".to_string()),
        Some("worker".to_string()),
        /*is_closed*/ true,
    );

    assert!(!app.should_attach_live_thread_for_selection(thread_id));

    app.agent_navigation.upsert(
        thread_id,
        Some("Ghost".to_string()),
        Some("worker".to_string()),
        /*is_closed*/ false,
    );
    assert!(app.should_attach_live_thread_for_selection(thread_id));

    app.thread_event_channels
        .insert(thread_id, ThreadEventChannel::new(/*capacity*/ 1));
    assert!(!app.should_attach_live_thread_for_selection(thread_id));
}

#[tokio::test]
async fn refresh_agent_picker_thread_liveness_prunes_closed_metadata_only_threads() -> Result<()> {
    let mut app = make_test_app().await;
    let mut app_server = crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
        .await
        .expect("embedded app server");
    let thread_id = ThreadId::new();
    app.agent_navigation.upsert(
        thread_id,
        Some("Ghost".to_string()),
        Some("worker".to_string()),
        /*is_closed*/ false,
    );

    let is_available = app
        .refresh_agent_picker_thread_liveness(&mut app_server, thread_id)
        .await;

    assert!(!is_available);
    assert_eq!(app.agent_navigation.get(&thread_id), None);
    assert!(!app.thread_event_channels.contains_key(&thread_id));
    Ok(())
}

#[tokio::test]
async fn open_agent_picker_prompts_to_enable_multi_agent_when_disabled() -> Result<()> {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let mut app_server = crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
        .await
        .expect("embedded app server");
    let _ = app.config.features.disable(Feature::Collab);

    app.open_agent_picker(&mut app_server).await;
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_matches!(
        app_event_rx.try_recv(),
        Ok(AppEvent::UpdateFeatureFlags { updates }) if updates == vec![(Feature::Collab, true)]
    );
    let cell = match app_event_rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => cell,
        other => panic!("expected InsertHistoryCell event, got {other:?}"),
    };
    let rendered = cell
        .display_lines(/*width*/ 120)
        .into_iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(rendered.contains("Subagents will be enabled in the next session."));
    Ok(())
}

#[tokio::test]
async fn update_memory_settings_persists_and_updates_widget_config() -> Result<()> {
    let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let codex_home = tempdir()?;
    app.config.codex_home = codex_home.path().to_path_buf().abs();
    let mut app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;

    app.update_memory_settings_with_app_server(
        &mut app_server,
        /*use_memories*/ false,
        /*generate_memories*/ false,
    )
    .await;

    assert!(!app.config.memories.use_memories);
    assert!(!app.config.memories.generate_memories);
    assert!(!app.chat_widget.config_ref().memories.use_memories);
    assert!(!app.chat_widget.config_ref().memories.generate_memories);

    let config = std::fs::read_to_string(codex_home.path().join("config.toml"))?;
    let config_value = toml::from_str::<TomlValue>(&config)?;
    let memories = config_value
        .as_table()
        .and_then(|table| table.get("memories"))
        .and_then(TomlValue::as_table)
        .expect("memories table should exist");
    assert_eq!(
        memories.get("use_memories"),
        Some(&TomlValue::Boolean(false))
    );
    assert_eq!(
        memories.get("generate_memories"),
        Some(&TomlValue::Boolean(false))
    );
    assert!(
        !memories.contains_key("disable_on_external_context")
            && !memories.contains_key("no_memories_if_mcp_or_web_search"),
        "the TUI menu should not write the external-context memory setting"
    );
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn update_memory_settings_updates_current_thread_memory_mode() -> Result<()> {
    let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let codex_home = tempdir()?;
    app.config.codex_home = codex_home.path().to_path_buf().abs();
    app.config.sqlite_home = codex_home.path().to_path_buf();
    // Seed the previous setting so this test exercises the thread-mode update path.
    app.config.memories.generate_memories = true;

    let mut app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;
    let started = app_server.start_thread(&app.config).await?;
    let thread_id = started.session.thread_id;
    app.active_thread_id = Some(thread_id);

    app.update_memory_settings_with_app_server(
        &mut app_server,
        /*use_memories*/ true,
        /*generate_memories*/ false,
    )
    .await;

    let state_db = codex_state::StateRuntime::init(
        codex_home.path().to_path_buf(),
        app.config.model_provider_id.clone(),
    )
    .await
    .expect("state db should initialize");
    let memory_mode = state_db
        .get_thread_memory_mode(thread_id)
        .await
        .expect("thread memory mode should be readable");
    assert_eq!(memory_mode.as_deref(), Some("disabled"));

    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn reset_memories_clears_local_memory_directories() -> Result<()> {
    let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let codex_home = tempdir()?;
    app.config.codex_home = codex_home.path().to_path_buf().abs();
    app.config.sqlite_home = codex_home.path().to_path_buf();

    let memory_root = codex_home.path().join("memories");
    let extensions_root = codex_home.path().join("memories_extensions");
    std::fs::create_dir_all(memory_root.join("rollout_summaries"))?;
    std::fs::create_dir_all(&extensions_root)?;
    std::fs::write(memory_root.join("MEMORY.md"), "stale memory\n")?;
    std::fs::write(
        memory_root.join("rollout_summaries").join("stale.md"),
        "stale summary\n",
    )?;
    std::fs::write(extensions_root.join("stale.txt"), "stale extension\n")?;

    let mut app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;

    app.reset_memories_with_app_server(&mut app_server).await;

    assert_eq!(std::fs::read_dir(&memory_root)?.count(), 0);
    assert_eq!(std::fs::read_dir(&extensions_root)?.count(), 0);

    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn update_feature_flags_enabling_guardian_selects_auto_review() -> Result<()> {
    let (mut app, mut app_event_rx, mut op_rx) = make_test_app_with_channels().await;
    let codex_home = tempdir()?;
    app.config.codex_home = codex_home.path().to_path_buf().abs();
    let auto_review = auto_review_mode();

    app.update_feature_flags(vec![(Feature::GuardianApproval, true)])
        .await;

    assert!(app.config.features.enabled(Feature::GuardianApproval));
    assert!(
        app.chat_widget
            .config_ref()
            .features
            .enabled(Feature::GuardianApproval)
    );
    assert_eq!(
        app.config.approvals_reviewer,
        auto_review.approvals_reviewer
    );
    assert_eq!(
        app.config.permissions.approval_policy.value(),
        auto_review.approval_policy
    );
    assert_eq!(
        app.chat_widget
            .config_ref()
            .permissions
            .approval_policy
            .value(),
        auto_review.approval_policy
    );
    assert_eq!(
        app.chat_widget
            .config_ref()
            .permissions
            .sandbox_policy
            .get(),
        &auto_review.sandbox_policy
    );
    assert_eq!(
        app.chat_widget.config_ref().approvals_reviewer,
        auto_review.approvals_reviewer
    );
    assert_eq!(app.runtime_approval_policy_override, None);
    assert_eq!(app.runtime_sandbox_policy_override, None);
    assert_eq!(
        op_rx.try_recv(),
        Ok(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: Some(auto_review.approval_policy),
            approvals_reviewer: Some(auto_review.approvals_reviewer),
            sandbox_policy: Some(auto_review.sandbox_policy.clone()),
            permission_profile: None,
            windows_sandbox_level: None,
            model: None,
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
    );
    let cell = match app_event_rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => cell,
        other => panic!("expected InsertHistoryCell event, got {other:?}"),
    };
    let rendered = cell
        .display_lines(/*width*/ 120)
        .into_iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(rendered.contains("Permissions updated to Auto-review"));

    let config = std::fs::read_to_string(codex_home.path().join("config.toml"))?;
    assert!(config.contains("guardian_approval = true"));
    assert!(config.contains("approvals_reviewer = \"guardian_subagent\""));
    assert!(config.contains("approval_policy = \"on-request\""));
    assert!(config.contains("sandbox_mode = \"workspace-write\""));
    Ok(())
}

#[tokio::test]
async fn update_feature_flags_disabling_guardian_clears_review_policy_and_restores_default()
-> Result<()> {
    let (mut app, mut app_event_rx, mut op_rx) = make_test_app_with_channels().await;
    let codex_home = tempdir()?;
    app.config.codex_home = codex_home.path().to_path_buf().abs();
    let config_toml_path = codex_home.path().join("config.toml").abs();
    let config_toml = "approvals_reviewer = \"guardian_subagent\"\napproval_policy = \"on-request\"\nsandbox_mode = \"workspace-write\"\n\n[features]\nguardian_approval = true\n";
    std::fs::write(config_toml_path.as_path(), config_toml)?;
    let user_config = toml::from_str::<TomlValue>(config_toml)?;
    app.config.config_layer_stack = app
        .config
        .config_layer_stack
        .with_user_config(&config_toml_path, user_config);
    app.config
        .features
        .set_enabled(Feature::GuardianApproval, /*enabled*/ true)?;
    app.chat_widget
        .set_feature_enabled(Feature::GuardianApproval, /*enabled*/ true);
    app.config.approvals_reviewer = ApprovalsReviewer::AutoReview;
    app.chat_widget
        .set_approvals_reviewer(ApprovalsReviewer::AutoReview);
    app.config
        .permissions
        .approval_policy
        .set(AskForApproval::OnRequest)?;
    app.config
        .permissions
        .sandbox_policy
        .set(SandboxPolicy::new_workspace_write_policy())?;
    app.chat_widget
        .set_approval_policy(AskForApproval::OnRequest);
    app.chat_widget
        .set_sandbox_policy(SandboxPolicy::new_workspace_write_policy())?;

    app.update_feature_flags(vec![(Feature::GuardianApproval, false)])
        .await;

    assert!(!app.config.features.enabled(Feature::GuardianApproval));
    assert!(
        !app.chat_widget
            .config_ref()
            .features
            .enabled(Feature::GuardianApproval)
    );
    assert_eq!(app.config.approvals_reviewer, ApprovalsReviewer::User);
    assert_eq!(
        app.config.permissions.approval_policy.value(),
        AskForApproval::OnRequest
    );
    assert_eq!(
        app.chat_widget.config_ref().approvals_reviewer,
        ApprovalsReviewer::User
    );
    assert_eq!(app.runtime_approval_policy_override, None);
    assert_eq!(
        op_rx.try_recv(),
        Ok(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: None,
            approvals_reviewer: Some(ApprovalsReviewer::User),
            sandbox_policy: None,
            permission_profile: None,
            windows_sandbox_level: None,
            model: None,
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
    );
    let cell = match app_event_rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => cell,
        other => panic!("expected InsertHistoryCell event, got {other:?}"),
    };
    let rendered = cell
        .display_lines(/*width*/ 120)
        .into_iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(rendered.contains("Permissions updated to Default"));

    let config = std::fs::read_to_string(codex_home.path().join("config.toml"))?;
    assert!(!config.contains("guardian_approval = true"));
    assert!(!config.contains("approvals_reviewer ="));
    assert!(config.contains("approval_policy = \"on-request\""));
    assert!(config.contains("sandbox_mode = \"workspace-write\""));
    Ok(())
}

#[tokio::test]
async fn update_feature_flags_enabling_guardian_overrides_explicit_manual_review_policy()
-> Result<()> {
    let (mut app, _app_event_rx, mut op_rx) = make_test_app_with_channels().await;
    let codex_home = tempdir()?;
    app.config.codex_home = codex_home.path().to_path_buf().abs();
    let auto_review = auto_review_mode();
    let config_toml_path = codex_home.path().join("config.toml").abs();
    let config_toml = "approvals_reviewer = \"user\"\n";
    std::fs::write(config_toml_path.as_path(), config_toml)?;
    let user_config = toml::from_str::<TomlValue>(config_toml)?;
    app.config.config_layer_stack = app
        .config
        .config_layer_stack
        .with_user_config(&config_toml_path, user_config);
    app.config.approvals_reviewer = ApprovalsReviewer::User;
    app.chat_widget
        .set_approvals_reviewer(ApprovalsReviewer::User);

    app.update_feature_flags(vec![(Feature::GuardianApproval, true)])
        .await;

    assert!(app.config.features.enabled(Feature::GuardianApproval));
    assert_eq!(
        app.config.approvals_reviewer,
        auto_review.approvals_reviewer
    );
    assert_eq!(
        app.chat_widget.config_ref().approvals_reviewer,
        auto_review.approvals_reviewer
    );
    assert_eq!(
        app.config.permissions.approval_policy.value(),
        auto_review.approval_policy
    );
    assert_eq!(
        app.chat_widget
            .config_ref()
            .permissions
            .sandbox_policy
            .get(),
        &auto_review.sandbox_policy
    );
    assert_eq!(
        op_rx.try_recv(),
        Ok(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: Some(auto_review.approval_policy),
            approvals_reviewer: Some(auto_review.approvals_reviewer),
            sandbox_policy: Some(auto_review.sandbox_policy.clone()),
            permission_profile: None,
            windows_sandbox_level: None,
            model: None,
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
    );

    let config = std::fs::read_to_string(codex_home.path().join("config.toml"))?;
    assert!(config.contains("approvals_reviewer = \"guardian_subagent\""));
    assert!(config.contains("guardian_approval = true"));
    assert!(config.contains("approval_policy = \"on-request\""));
    assert!(config.contains("sandbox_mode = \"workspace-write\""));
    Ok(())
}

#[tokio::test]
async fn update_feature_flags_disabling_guardian_clears_manual_review_policy_without_history()
-> Result<()> {
    let (mut app, mut app_event_rx, mut op_rx) = make_test_app_with_channels().await;
    let codex_home = tempdir()?;
    app.config.codex_home = codex_home.path().to_path_buf().abs();
    let config_toml_path = codex_home.path().join("config.toml").abs();
    let config_toml = "approvals_reviewer = \"user\"\napproval_policy = \"on-request\"\nsandbox_mode = \"workspace-write\"\n\n[features]\nguardian_approval = true\n";
    std::fs::write(config_toml_path.as_path(), config_toml)?;
    let user_config = toml::from_str::<TomlValue>(config_toml)?;
    app.config.config_layer_stack = app
        .config
        .config_layer_stack
        .with_user_config(&config_toml_path, user_config);
    app.config
        .features
        .set_enabled(Feature::GuardianApproval, /*enabled*/ true)?;
    app.chat_widget
        .set_feature_enabled(Feature::GuardianApproval, /*enabled*/ true);
    app.config.approvals_reviewer = ApprovalsReviewer::User;
    app.chat_widget
        .set_approvals_reviewer(ApprovalsReviewer::User);

    app.update_feature_flags(vec![(Feature::GuardianApproval, false)])
        .await;

    assert!(!app.config.features.enabled(Feature::GuardianApproval));
    assert_eq!(app.config.approvals_reviewer, ApprovalsReviewer::User);
    assert_eq!(
        app.chat_widget.config_ref().approvals_reviewer,
        ApprovalsReviewer::User
    );
    assert_eq!(
        op_rx.try_recv(),
        Ok(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: None,
            approvals_reviewer: Some(ApprovalsReviewer::User),
            sandbox_policy: None,
            permission_profile: None,
            windows_sandbox_level: None,
            model: None,
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
    );
    assert!(
        app_event_rx.try_recv().is_err(),
        "manual review should not emit a permissions history update when the effective state stays default"
    );

    let config = std::fs::read_to_string(codex_home.path().join("config.toml"))?;
    assert!(!config.contains("guardian_approval = true"));
    assert!(!config.contains("approvals_reviewer ="));
    Ok(())
}

#[tokio::test]
async fn update_feature_flags_enabling_guardian_in_profile_sets_profile_auto_review_policy()
-> Result<()> {
    let (mut app, _app_event_rx, mut op_rx) = make_test_app_with_channels().await;
    let codex_home = tempdir()?;
    app.config.codex_home = codex_home.path().to_path_buf().abs();
    let auto_review = auto_review_mode();
    app.active_profile = Some("guardian".to_string());
    let config_toml_path = codex_home.path().join("config.toml").abs();
    let config_toml = "profile = \"guardian\"\napprovals_reviewer = \"user\"\n";
    std::fs::write(config_toml_path.as_path(), config_toml)?;
    let user_config = toml::from_str::<TomlValue>(config_toml)?;
    app.config.config_layer_stack = app
        .config
        .config_layer_stack
        .with_user_config(&config_toml_path, user_config);
    app.config.approvals_reviewer = ApprovalsReviewer::User;
    app.chat_widget
        .set_approvals_reviewer(ApprovalsReviewer::User);

    app.update_feature_flags(vec![(Feature::GuardianApproval, true)])
        .await;

    assert!(app.config.features.enabled(Feature::GuardianApproval));
    assert_eq!(
        app.config.approvals_reviewer,
        auto_review.approvals_reviewer
    );
    assert_eq!(
        app.chat_widget.config_ref().approvals_reviewer,
        auto_review.approvals_reviewer
    );
    assert_eq!(
        op_rx.try_recv(),
        Ok(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: Some(auto_review.approval_policy),
            approvals_reviewer: Some(auto_review.approvals_reviewer),
            sandbox_policy: Some(auto_review.sandbox_policy.clone()),
            permission_profile: None,
            windows_sandbox_level: None,
            model: None,
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
    );

    let config = std::fs::read_to_string(codex_home.path().join("config.toml"))?;
    let config_value = toml::from_str::<TomlValue>(&config)?;
    let profile_config = config_value
        .as_table()
        .and_then(|table| table.get("profiles"))
        .and_then(TomlValue::as_table)
        .and_then(|profiles| profiles.get("guardian"))
        .and_then(TomlValue::as_table)
        .expect("guardian profile should exist");
    assert_eq!(
        config_value
            .as_table()
            .and_then(|table| table.get("approvals_reviewer")),
        Some(&TomlValue::String("user".to_string()))
    );
    assert_eq!(
        profile_config.get("approvals_reviewer"),
        Some(&TomlValue::String("guardian_subagent".to_string()))
    );
    Ok(())
}

#[tokio::test]
async fn update_feature_flags_disabling_guardian_in_profile_allows_inherited_user_reviewer()
-> Result<()> {
    let (mut app, mut app_event_rx, mut op_rx) = make_test_app_with_channels().await;
    let codex_home = tempdir()?;
    app.config.codex_home = codex_home.path().to_path_buf().abs();
    app.active_profile = Some("guardian".to_string());
    let config_toml_path = codex_home.path().join("config.toml").abs();
    let config_toml = r#"
profile = "guardian"
approvals_reviewer = "user"

[profiles.guardian]
approvals_reviewer = "guardian_subagent"

[profiles.guardian.features]
guardian_approval = true
"#;
    std::fs::write(config_toml_path.as_path(), config_toml)?;
    let user_config = toml::from_str::<TomlValue>(config_toml)?;
    app.config.config_layer_stack = app
        .config
        .config_layer_stack
        .with_user_config(&config_toml_path, user_config);
    app.config
        .features
        .set_enabled(Feature::GuardianApproval, /*enabled*/ true)?;
    app.chat_widget
        .set_feature_enabled(Feature::GuardianApproval, /*enabled*/ true);
    app.config.approvals_reviewer = ApprovalsReviewer::AutoReview;
    app.chat_widget
        .set_approvals_reviewer(ApprovalsReviewer::AutoReview);

    app.update_feature_flags(vec![(Feature::GuardianApproval, false)])
        .await;

    assert!(!app.config.features.enabled(Feature::GuardianApproval));
    assert!(
        !app.chat_widget
            .config_ref()
            .features
            .enabled(Feature::GuardianApproval)
    );
    assert_eq!(app.config.approvals_reviewer, ApprovalsReviewer::User);
    assert_eq!(
        app.chat_widget.config_ref().approvals_reviewer,
        ApprovalsReviewer::User
    );
    assert_eq!(
        op_rx.try_recv(),
        Ok(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: None,
            approvals_reviewer: Some(ApprovalsReviewer::User),
            sandbox_policy: None,
            permission_profile: None,
            windows_sandbox_level: None,
            model: None,
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
    );
    let cell = match app_event_rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => cell,
        other => panic!("expected InsertHistoryCell event, got {other:?}"),
    };
    let rendered = cell
        .display_lines(/*width*/ 120)
        .into_iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(rendered.contains("Permissions updated to Default"));

    let config = std::fs::read_to_string(codex_home.path().join("config.toml"))?;
    assert!(!config.contains("guardian_approval = true"));
    assert!(!config.contains("guardian_subagent"));
    assert_eq!(
        toml::from_str::<TomlValue>(&config)?
            .as_table()
            .and_then(|table| table.get("approvals_reviewer")),
        Some(&TomlValue::String("user".to_string()))
    );
    Ok(())
}

#[tokio::test]
async fn update_feature_flags_disabling_guardian_in_profile_keeps_inherited_non_user_reviewer_enabled()
-> Result<()> {
    let (mut app, mut app_event_rx, mut op_rx) = make_test_app_with_channels().await;
    let codex_home = tempdir()?;
    app.config.codex_home = codex_home.path().to_path_buf().abs();
    app.active_profile = Some("guardian".to_string());
    let config_toml_path = codex_home.path().join("config.toml").abs();
    let config_toml = "profile = \"guardian\"\napprovals_reviewer = \"guardian_subagent\"\n\n[features]\nguardian_approval = true\n";
    std::fs::write(config_toml_path.as_path(), config_toml)?;
    let user_config = toml::from_str::<TomlValue>(config_toml)?;
    app.config.config_layer_stack = app
        .config
        .config_layer_stack
        .with_user_config(&config_toml_path, user_config);
    app.config
        .features
        .set_enabled(Feature::GuardianApproval, /*enabled*/ true)?;
    app.chat_widget
        .set_feature_enabled(Feature::GuardianApproval, /*enabled*/ true);
    app.config.approvals_reviewer = ApprovalsReviewer::AutoReview;
    app.chat_widget
        .set_approvals_reviewer(ApprovalsReviewer::AutoReview);

    app.update_feature_flags(vec![(Feature::GuardianApproval, false)])
        .await;

    assert!(app.config.features.enabled(Feature::GuardianApproval));
    assert!(
        app.chat_widget
            .config_ref()
            .features
            .enabled(Feature::GuardianApproval)
    );
    assert_eq!(app.config.approvals_reviewer, ApprovalsReviewer::AutoReview);
    assert_eq!(
        app.chat_widget.config_ref().approvals_reviewer,
        ApprovalsReviewer::AutoReview
    );
    assert!(
        op_rx.try_recv().is_err(),
        "disabling an inherited non-user reviewer should not patch the active session"
    );
    let app_events = std::iter::from_fn(|| app_event_rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(
        !app_events.iter().any(|event| match event {
            AppEvent::InsertHistoryCell(cell) => cell
                .display_lines(/*width*/ 120)
                .iter()
                .any(|line| line.to_string().contains("Permissions updated to")),
            _ => false,
        }),
        "blocking disable with inherited guardian review should not emit a permissions history update: {app_events:?}"
    );

    let config = std::fs::read_to_string(codex_home.path().join("config.toml"))?;
    assert!(config.contains("guardian_approval = true"));
    assert_eq!(
        toml::from_str::<TomlValue>(&config)?
            .as_table()
            .and_then(|table| table.get("approvals_reviewer")),
        Some(&TomlValue::String("guardian_subagent".to_string()))
    );
    Ok(())
}

#[tokio::test]
async fn open_agent_picker_allows_existing_agent_threads_when_feature_is_disabled() -> Result<()> {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let mut app_server = crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
        .await
        .expect("embedded app server");
    let thread_id = ThreadId::new();
    app.thread_event_channels
        .insert(thread_id, ThreadEventChannel::new(/*capacity*/ 1));

    app.open_agent_picker(&mut app_server).await;
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_matches!(
        app_event_rx.try_recv(),
        Ok(AppEvent::SelectAgentThread(selected_thread_id)) if selected_thread_id == thread_id
    );
    Ok(())
}

#[tokio::test]
async fn refresh_pending_thread_approvals_only_lists_inactive_threads() {
    let mut app = make_test_app().await;
    let main_thread_id =
        ThreadId::from_string("00000000-0000-0000-0000-000000000001").expect("valid thread");
    let agent_thread_id =
        ThreadId::from_string("00000000-0000-0000-0000-000000000002").expect("valid thread");

    app.primary_thread_id = Some(main_thread_id);
    app.active_thread_id = Some(main_thread_id);
    app.thread_event_channels
        .insert(main_thread_id, ThreadEventChannel::new(/*capacity*/ 1));

    let agent_channel = ThreadEventChannel::new(/*capacity*/ 1);
    {
        let mut store = agent_channel.store.lock().await;
        store.push_request(exec_approval_request(
            agent_thread_id,
            "turn-1",
            "call-1",
            /*approval_id*/ None,
        ));
    }
    app.thread_event_channels
        .insert(agent_thread_id, agent_channel);
    app.agent_navigation.upsert(
        agent_thread_id,
        Some("Robie".to_string()),
        Some("explorer".to_string()),
        /*is_closed*/ false,
    );

    app.refresh_pending_thread_approvals().await;
    assert_eq!(
        app.chat_widget.pending_thread_approvals(),
        &["Robie [explorer]".to_string()]
    );

    app.active_thread_id = Some(agent_thread_id);
    app.refresh_pending_thread_approvals().await;
    assert!(app.chat_widget.pending_thread_approvals().is_empty());
}

#[tokio::test]
async fn inactive_thread_approval_bubbles_into_active_view() -> Result<()> {
    let mut app = make_test_app().await;
    let main_thread_id =
        ThreadId::from_string("00000000-0000-0000-0000-000000000011").expect("valid thread");
    let agent_thread_id =
        ThreadId::from_string("00000000-0000-0000-0000-000000000022").expect("valid thread");

    app.primary_thread_id = Some(main_thread_id);
    app.active_thread_id = Some(main_thread_id);
    app.thread_event_channels
        .insert(main_thread_id, ThreadEventChannel::new(/*capacity*/ 1));
    app.thread_event_channels.insert(
        agent_thread_id,
        ThreadEventChannel::new_with_session(
            /*capacity*/ 1,
            ThreadSessionState {
                approval_policy: AskForApproval::OnRequest,
                sandbox_policy: SandboxPolicy::new_workspace_write_policy(),
                permission_profile: Some(PermissionProfile::from_legacy_sandbox_policy(
                    &SandboxPolicy::new_workspace_write_policy(),
                    std::path::Path::new("/tmp/agent"),
                )),
                rollout_path: Some(test_path_buf("/tmp/agent-rollout.jsonl")),
                ..test_thread_session(agent_thread_id, test_path_buf("/tmp/agent"))
            },
            Vec::new(),
        ),
    );
    app.agent_navigation.upsert(
        agent_thread_id,
        Some("Robie".to_string()),
        Some("explorer".to_string()),
        /*is_closed*/ false,
    );

    app.enqueue_thread_request(
        agent_thread_id,
        exec_approval_request(
            agent_thread_id,
            "turn-approval",
            "call-approval",
            /*approval_id*/ None,
        ),
    )
    .await?;

    assert_eq!(app.chat_widget.has_active_view(), true);
    assert_eq!(
        app.chat_widget.pending_thread_approvals(),
        &["Robie [explorer]".to_string()]
    );

    Ok(())
}

#[tokio::test]
async fn side_defers_parent_approval_overlay_until_parent_replay() -> Result<()> {
    let mut app = make_test_app().await;
    let parent_thread_id =
        ThreadId::from_string("00000000-0000-0000-0000-000000000011").expect("valid thread");
    let side_thread_id =
        ThreadId::from_string("00000000-0000-0000-0000-000000000022").expect("valid thread");

    app.primary_thread_id = Some(parent_thread_id);
    app.active_thread_id = Some(side_thread_id);
    app.side_threads
        .insert(side_thread_id, SideThreadState::new(parent_thread_id));
    app.thread_event_channels.insert(
        parent_thread_id,
        ThreadEventChannel::new_with_session(
            /*capacity*/ 4,
            test_thread_session(parent_thread_id, test_path_buf("/tmp/main")),
            Vec::new(),
        ),
    );

    app.enqueue_thread_request(
        parent_thread_id,
        exec_approval_request(
            parent_thread_id,
            "turn-approval",
            "call-approval",
            /*approval_id*/ None,
        ),
    )
    .await?;

    assert_eq!(app.chat_widget.has_active_view(), false);
    assert!(app.chat_widget.pending_thread_approvals().is_empty());
    assert_eq!(
        app.side_threads
            .get(&side_thread_id)
            .and_then(|state| state.parent_status),
        Some(SideParentStatus::NeedsApproval)
    );

    let snapshot = {
        let channel = app
            .thread_event_channels
            .get(&parent_thread_id)
            .expect("parent thread channel");
        let store = channel.store.lock().await;
        store.snapshot()
    };
    app.side_threads.remove(&side_thread_id);
    app.active_thread_id = Some(parent_thread_id);
    app.replay_thread_snapshot(snapshot, /*resume_restored_queue*/ false);

    assert_eq!(app.chat_widget.has_active_view(), true);

    Ok(())
}

#[tokio::test]
async fn replay_snapshot_with_pending_request_suppresses_replay_notices() {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let thread_id =
        ThreadId::from_string("00000000-0000-0000-0000-000000000011").expect("valid thread");
    let stale_warning = "stale startup warning that should not cover the approval";

    app.replay_thread_snapshot(
        ThreadEventSnapshot {
            session: Some(test_thread_session(thread_id, test_path_buf("/tmp/main"))),
            turns: Vec::new(),
            events: vec![
                ThreadBufferedEvent::Notification(ServerNotification::Warning(
                    WarningNotification {
                        thread_id: Some(thread_id.to_string()),
                        message: stale_warning.to_string(),
                    },
                )),
                ThreadBufferedEvent::Request(exec_approval_request(
                    thread_id,
                    "turn-approval",
                    "call-approval",
                    /*approval_id*/ None,
                )),
            ],
            input_state: None,
        },
        /*resume_restored_queue*/ false,
    );

    assert_eq!(app.chat_widget.has_active_view(), true);

    let mut replayed_history = String::new();
    while let Ok(event) = app_event_rx.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event {
            replayed_history.push_str(&lines_to_single_string(
                &cell.transcript_lines(/*width*/ 80),
            ));
        }
    }

    assert!(
        replayed_history.is_empty(),
        "expected pending approval replay to suppress session notices, got {replayed_history:?}"
    );
}

#[tokio::test]
async fn side_defers_subagent_approval_overlay_until_side_exits() -> Result<()> {
    let mut app = make_test_app().await;
    let main_thread_id =
        ThreadId::from_string("00000000-0000-0000-0000-000000000011").expect("valid thread");
    let side_thread_id =
        ThreadId::from_string("00000000-0000-0000-0000-000000000022").expect("valid thread");
    let agent_thread_id =
        ThreadId::from_string("00000000-0000-0000-0000-000000000033").expect("valid thread");

    app.primary_thread_id = Some(main_thread_id);
    app.active_thread_id = Some(side_thread_id);
    app.side_threads
        .insert(side_thread_id, SideThreadState::new(main_thread_id));
    app.thread_event_channels.insert(
        agent_thread_id,
        ThreadEventChannel::new_with_session(
            /*capacity*/ 4,
            ThreadSessionState {
                approval_policy: AskForApproval::OnRequest,
                sandbox_policy: SandboxPolicy::new_workspace_write_policy(),
                permission_profile: Some(PermissionProfile::from_legacy_sandbox_policy(
                    &SandboxPolicy::new_workspace_write_policy(),
                    std::path::Path::new("/tmp/agent"),
                )),
                rollout_path: Some(test_path_buf("/tmp/agent-rollout.jsonl")),
                ..test_thread_session(agent_thread_id, test_path_buf("/tmp/agent"))
            },
            Vec::new(),
        ),
    );
    app.agent_navigation.upsert(
        agent_thread_id,
        Some("Robie".to_string()),
        Some("explorer".to_string()),
        /*is_closed*/ false,
    );

    app.enqueue_thread_request(
        agent_thread_id,
        exec_approval_request(
            agent_thread_id,
            "turn-approval",
            "call-approval",
            /*approval_id*/ None,
        ),
    )
    .await?;

    assert_eq!(app.chat_widget.has_active_view(), false);
    assert_eq!(
        app.chat_widget.pending_thread_approvals(),
        &["Robie [explorer]".to_string()]
    );

    app.side_threads.remove(&side_thread_id);
    app.active_thread_id = Some(main_thread_id);
    app.surface_pending_inactive_thread_interactive_requests()
        .await;

    assert_eq!(app.chat_widget.has_active_view(), true);

    Ok(())
}

#[tokio::test]
async fn inactive_thread_exec_approval_preserves_context() {
    let app = make_test_app().await;
    let thread_id = ThreadId::new();
    let mut request = exec_approval_request(
        thread_id,
        "turn-approval",
        "call-approval",
        /*approval_id*/ None,
    );
    let ServerRequest::CommandExecutionRequestApproval { params, .. } = &mut request else {
        panic!("expected exec approval request");
    };
    params.network_approval_context = Some(AppServerNetworkApprovalContext {
        host: "example.com".to_string(),
        protocol: AppServerNetworkApprovalProtocol::Socks5Tcp,
    });
    params.additional_permissions = Some(AdditionalPermissionProfile {
        network: Some(AdditionalNetworkPermissions {
            enabled: Some(true),
        }),
        file_system: Some(AdditionalFileSystemPermissions {
            read: Some(vec![test_absolute_path("/tmp/read-only")]),
            write: Some(vec![test_absolute_path("/tmp/write")]),
            glob_scan_max_depth: None,
            entries: None,
        }),
    });
    params.proposed_network_policy_amendments = Some(vec![AppServerNetworkPolicyAmendment {
        host: "example.com".to_string(),
        action: AppServerNetworkPolicyRuleAction::Allow,
    }]);

    let Some(ThreadInteractiveRequest::Approval(ApprovalRequest::Exec {
        available_decisions,
        network_approval_context,
        additional_permissions,
        ..
    })) = app
        .interactive_request_for_thread_request(thread_id, &request)
        .await
    else {
        panic!("expected exec approval request");
    };

    assert_eq!(
        network_approval_context,
        Some(NetworkApprovalContext {
            host: "example.com".to_string(),
            protocol: NetworkApprovalProtocol::Socks5Tcp,
        })
    );
    assert_eq!(
        additional_permissions,
        Some(PermissionProfile {
            network: Some(NetworkPermissions {
                enabled: Some(true),
            }),
            file_system: Some(FileSystemPermissions::from_read_write_roots(
                Some(vec![test_absolute_path("/tmp/read-only")]),
                Some(vec![test_absolute_path("/tmp/write")]),
            )),
        })
    );
    assert_eq!(
        available_decisions,
        vec![
            codex_protocol::protocol::ReviewDecision::Approved,
            codex_protocol::protocol::ReviewDecision::ApprovedForSession,
            codex_protocol::protocol::ReviewDecision::NetworkPolicyAmendment {
                network_policy_amendment: codex_protocol::approvals::NetworkPolicyAmendment {
                    host: "example.com".to_string(),
                    action: codex_protocol::approvals::NetworkPolicyRuleAction::Allow,
                },
            },
            codex_protocol::protocol::ReviewDecision::Abort,
        ]
    );
}

#[tokio::test]
async fn inactive_thread_exec_approval_splits_shell_wrapped_command() {
    let app = make_test_app().await;
    let thread_id = ThreadId::new();
    let script = r#"python3 -c 'print("Hello, world!")'"#;
    let mut request = exec_approval_request(
        thread_id,
        "turn-approval",
        "call-approval",
        /*approval_id*/ None,
    );
    let ServerRequest::CommandExecutionRequestApproval { params, .. } = &mut request else {
        panic!("expected exec approval request");
    };
    params.command =
        Some(shlex::try_join(["/bin/zsh", "-lc", script]).expect("round-trippable shell wrapper"));

    let Some(ThreadInteractiveRequest::Approval(ApprovalRequest::Exec { command, .. })) = app
        .interactive_request_for_thread_request(thread_id, &request)
        .await
    else {
        panic!("expected exec approval request");
    };

    assert_eq!(
        command,
        vec![
            "/bin/zsh".to_string(),
            "-lc".to_string(),
            script.to_string(),
        ]
    );
}

#[tokio::test]
async fn inactive_thread_permissions_approval_preserves_file_system_permissions() {
    let app = make_test_app().await;
    let thread_id = ThreadId::new();
    let request = ServerRequest::PermissionsRequestApproval {
        request_id: AppServerRequestId::Integer(7),
        params: PermissionsRequestApprovalParams {
            thread_id: thread_id.to_string(),
            turn_id: "turn-approval".to_string(),
            item_id: "call-approval".to_string(),
            cwd: test_absolute_path("/tmp"),
            reason: Some("Need access to .git".to_string()),
            permissions: codex_app_server_protocol::RequestPermissionProfile {
                network: Some(AdditionalNetworkPermissions {
                    enabled: Some(true),
                }),
                file_system: Some(AdditionalFileSystemPermissions {
                    read: Some(vec![test_absolute_path("/tmp/read-only")]),
                    write: Some(vec![test_absolute_path("/tmp/write")]),
                    glob_scan_max_depth: None,
                    entries: None,
                }),
            },
        },
    };

    let Some(ThreadInteractiveRequest::Approval(ApprovalRequest::Permissions {
        permissions, ..
    })) = app
        .interactive_request_for_thread_request(thread_id, &request)
        .await
    else {
        panic!("expected permissions approval request");
    };

    assert_eq!(
        permissions,
        RequestPermissionProfile {
            network: Some(NetworkPermissions {
                enabled: Some(true),
            }),
            file_system: Some(FileSystemPermissions::from_read_write_roots(
                Some(vec![test_absolute_path("/tmp/read-only")]),
                Some(vec![test_absolute_path("/tmp/write")]),
            )),
        }
    );
}

#[tokio::test]
async fn inactive_thread_approval_badge_clears_after_turn_completion_notification() -> Result<()> {
    let mut app = make_test_app().await;
    let main_thread_id =
        ThreadId::from_string("00000000-0000-0000-0000-000000000101").expect("valid thread");
    let agent_thread_id =
        ThreadId::from_string("00000000-0000-0000-0000-000000000202").expect("valid thread");

    app.primary_thread_id = Some(main_thread_id);
    app.active_thread_id = Some(main_thread_id);
    app.thread_event_channels
        .insert(main_thread_id, ThreadEventChannel::new(/*capacity*/ 1));
    app.thread_event_channels.insert(
        agent_thread_id,
        ThreadEventChannel::new_with_session(
            /*capacity*/ 4,
            ThreadSessionState {
                approval_policy: AskForApproval::OnRequest,
                sandbox_policy: SandboxPolicy::new_workspace_write_policy(),
                permission_profile: Some(PermissionProfile::from_legacy_sandbox_policy(
                    &SandboxPolicy::new_workspace_write_policy(),
                    std::path::Path::new("/tmp/agent"),
                )),
                rollout_path: Some(test_path_buf("/tmp/agent-rollout.jsonl")),
                ..test_thread_session(agent_thread_id, test_path_buf("/tmp/agent"))
            },
            Vec::new(),
        ),
    );
    app.agent_navigation.upsert(
        agent_thread_id,
        Some("Robie".to_string()),
        Some("explorer".to_string()),
        /*is_closed*/ false,
    );

    app.enqueue_thread_request(
        agent_thread_id,
        exec_approval_request(
            agent_thread_id,
            "turn-approval",
            "call-approval",
            /*approval_id*/ None,
        ),
    )
    .await?;
    assert_eq!(
        app.chat_widget.pending_thread_approvals(),
        &["Robie [explorer]".to_string()]
    );

    app.enqueue_thread_notification(
        agent_thread_id,
        turn_completed_notification(agent_thread_id, "turn-approval", TurnStatus::Completed),
    )
    .await?;

    assert!(
        app.chat_widget.pending_thread_approvals().is_empty(),
        "turn completion should clear inactive-thread approval badge immediately"
    );

    Ok(())
}

#[tokio::test]
async fn inactive_thread_started_notification_initializes_replay_session() -> Result<()> {
    let mut app = make_test_app().await;
    let temp_dir = tempdir()?;
    let main_thread_id =
        ThreadId::from_string("00000000-0000-0000-0000-000000000101").expect("valid thread");
    let agent_thread_id =
        ThreadId::from_string("00000000-0000-0000-0000-000000000202").expect("valid thread");
    let primary_session = ThreadSessionState {
        approval_policy: AskForApproval::OnRequest,
        sandbox_policy: SandboxPolicy::new_workspace_write_policy(),
        permission_profile: Some(PermissionProfile::from_legacy_sandbox_policy(
            &SandboxPolicy::new_workspace_write_policy(),
            std::path::Path::new("/tmp/main"),
        )),
        ..test_thread_session(main_thread_id, test_path_buf("/tmp/main"))
    };

    app.primary_thread_id = Some(main_thread_id);
    app.active_thread_id = Some(main_thread_id);
    app.primary_session_configured = Some(primary_session.clone());
    app.thread_event_channels.insert(
        main_thread_id,
        ThreadEventChannel::new_with_session(
            /*capacity*/ 4,
            primary_session.clone(),
            Vec::new(),
        ),
    );

    let rollout_path = temp_dir.path().join("agent-rollout.jsonl");
    let turn_context = TurnContextItem {
        turn_id: None,
        trace_id: None,
        cwd: test_path_buf("/tmp/agent"),
        current_date: None,
        timezone: None,
        approval_policy: primary_session.approval_policy,
        sandbox_policy: primary_session.sandbox_policy.clone(),
        permission_profile: None,
        network: None,
        file_system_sandbox_policy: None,
        model: "gpt-agent".to_string(),
        personality: None,
        collaboration_mode: None,
        realtime_active: Some(false),
        effort: primary_session.reasoning_effort,
        summary: app.config.model_reasoning_summary.unwrap_or_default(),
        user_instructions: None,
        developer_instructions: None,
        final_output_json_schema: None,
        truncation_policy: None,
    };
    let rollout = RolloutLine {
        timestamp: "t0".to_string(),
        item: RolloutItem::TurnContext(turn_context),
    };
    std::fs::write(
        &rollout_path,
        format!("{}\n", serde_json::to_string(&rollout)?),
    )?;
    app.enqueue_thread_notification(
        agent_thread_id,
        ServerNotification::ThreadStarted(ThreadStartedNotification {
            thread: Thread {
                id: agent_thread_id.to_string(),
                forked_from_id: None,
                preview: "agent thread".to_string(),
                ephemeral: false,
                model_provider: "agent-provider".to_string(),
                created_at: 1,
                updated_at: 2,
                status: codex_app_server_protocol::ThreadStatus::Idle,
                path: Some(rollout_path.clone()),
                cwd: test_path_buf("/tmp/agent").abs(),
                cli_version: "0.0.0".to_string(),
                source: codex_app_server_protocol::SessionSource::Unknown,
                agent_nickname: Some("Robie".to_string()),
                agent_role: Some("explorer".to_string()),
                git_info: None,
                name: Some("agent thread".to_string()),
                turns: Vec::new(),
            },
        }),
    )
    .await?;

    let store = app
        .thread_event_channels
        .get(&agent_thread_id)
        .expect("agent thread channel")
        .store
        .lock()
        .await;
    let session = store.session.clone().expect("inferred session");
    drop(store);

    assert_eq!(session.thread_id, agent_thread_id);
    assert_eq!(session.thread_name, Some("agent thread".to_string()));
    assert_eq!(session.model, "gpt-agent");
    assert_eq!(session.model_provider_id, "agent-provider");
    assert_eq!(session.approval_policy, primary_session.approval_policy);
    assert_eq!(session.cwd.as_path(), test_path_buf("/tmp/agent").as_path());
    assert_eq!(session.rollout_path, Some(rollout_path));
    assert_eq!(
        app.agent_navigation.get(&agent_thread_id),
        Some(&AgentPickerThreadEntry {
            agent_nickname: Some("Robie".to_string()),
            agent_role: Some("explorer".to_string()),
            is_closed: false,
        })
    );

    Ok(())
}

#[tokio::test]
async fn inactive_thread_started_notification_preserves_primary_model_when_path_missing()
-> Result<()> {
    let mut app = make_test_app().await;
    let main_thread_id =
        ThreadId::from_string("00000000-0000-0000-0000-000000000301").expect("valid thread");
    let agent_thread_id =
        ThreadId::from_string("00000000-0000-0000-0000-000000000302").expect("valid thread");
    let primary_session = ThreadSessionState {
        approval_policy: AskForApproval::OnRequest,
        sandbox_policy: SandboxPolicy::new_workspace_write_policy(),
        permission_profile: Some(PermissionProfile::from_legacy_sandbox_policy(
            &SandboxPolicy::new_workspace_write_policy(),
            std::path::Path::new("/tmp/main"),
        )),
        ..test_thread_session(main_thread_id, test_path_buf("/tmp/main"))
    };

    app.primary_thread_id = Some(main_thread_id);
    app.active_thread_id = Some(main_thread_id);
    app.primary_session_configured = Some(primary_session.clone());
    app.thread_event_channels.insert(
        main_thread_id,
        ThreadEventChannel::new_with_session(
            /*capacity*/ 4,
            primary_session.clone(),
            Vec::new(),
        ),
    );

    app.enqueue_thread_notification(
        agent_thread_id,
        ServerNotification::ThreadStarted(ThreadStartedNotification {
            thread: Thread {
                id: agent_thread_id.to_string(),
                forked_from_id: None,
                preview: "agent thread".to_string(),
                ephemeral: false,
                model_provider: "agent-provider".to_string(),
                created_at: 1,
                updated_at: 2,
                status: codex_app_server_protocol::ThreadStatus::Idle,
                path: None,
                cwd: test_path_buf("/tmp/agent").abs(),
                cli_version: "0.0.0".to_string(),
                source: codex_app_server_protocol::SessionSource::Unknown,
                agent_nickname: Some("Robie".to_string()),
                agent_role: Some("explorer".to_string()),
                git_info: None,
                name: Some("agent thread".to_string()),
                turns: Vec::new(),
            },
        }),
    )
    .await?;

    let store = app
        .thread_event_channels
        .get(&agent_thread_id)
        .expect("agent thread channel")
        .store
        .lock()
        .await;
    let session = store.session.clone().expect("inferred session");

    assert_eq!(session.model, primary_session.model);

    Ok(())
}

/// `thread/read` is metadata/replay hydration and does not return an
/// authoritative runtime `PermissionProfile`, so it must not reuse the active
/// primary session profile after swapping in the read thread's cwd.
#[tokio::test]
async fn thread_read_session_state_does_not_reuse_primary_permission_profile() {
    let mut app = make_test_app().await;
    let main_thread_id =
        ThreadId::from_string("00000000-0000-0000-0000-000000000401").expect("valid thread");
    let read_thread_id =
        ThreadId::from_string("00000000-0000-0000-0000-000000000402").expect("valid thread");
    let primary_session = ThreadSessionState {
        approval_policy: AskForApproval::OnRequest,
        sandbox_policy: SandboxPolicy::new_workspace_write_policy(),
        permission_profile: Some(PermissionProfile::from_legacy_sandbox_policy(
            &SandboxPolicy::new_workspace_write_policy(),
            std::path::Path::new("/tmp/main"),
        )),
        ..test_thread_session(main_thread_id, test_path_buf("/tmp/main"))
    };
    app.primary_session_configured = Some(primary_session);

    let thread = Thread {
        id: read_thread_id.to_string(),
        forked_from_id: None,
        preview: "read thread".to_string(),
        ephemeral: false,
        model_provider: "read-provider".to_string(),
        created_at: 1,
        updated_at: 2,
        status: codex_app_server_protocol::ThreadStatus::Idle,
        path: None,
        cwd: test_path_buf("/tmp/read").abs(),
        cli_version: "0.0.0".to_string(),
        source: codex_app_server_protocol::SessionSource::Unknown,
        agent_nickname: None,
        agent_role: None,
        git_info: None,
        name: Some("read thread".to_string()),
        turns: Vec::new(),
    };

    let session = app
        .session_state_for_thread_read(read_thread_id, &thread)
        .await;

    assert_eq!(session.thread_id, read_thread_id);
    assert_eq!(session.cwd.as_path(), test_path_buf("/tmp/read").as_path());
    assert_eq!(
        session.permission_profile, None,
        "thread/read does not return an authoritative permission profile; reusing the primary \
         session profile would reinterpret cwd-bound entries against the read thread cwd"
    );
}

#[test]
fn agent_picker_item_name_snapshot() {
    let thread_id =
        ThreadId::from_string("00000000-0000-0000-0000-000000000123").expect("valid thread id");
    let snapshot = [
        format!(
            "{} | {}",
            format_agent_picker_item_name(
                Some("Robie"),
                Some("explorer"),
                /*is_primary*/ true
            ),
            thread_id
        ),
        format!(
            "{} | {}",
            format_agent_picker_item_name(
                Some("Robie"),
                Some("explorer"),
                /*is_primary*/ false
            ),
            thread_id
        ),
        format!(
            "{} | {}",
            format_agent_picker_item_name(
                Some("Robie"),
                /*agent_role*/ None,
                /*is_primary*/ false
            ),
            thread_id
        ),
        format!(
            "{} | {}",
            format_agent_picker_item_name(
                /*agent_nickname*/ None,
                Some("explorer"),
                /*is_primary*/ false
            ),
            thread_id
        ),
        format!(
            "{} | {}",
            format_agent_picker_item_name(
                /*agent_nickname*/ None, /*agent_role*/ None, /*is_primary*/ false
            ),
            thread_id
        ),
    ]
    .join("\n");
    assert_app_snapshot!("agent_picker_item_name", snapshot);
}

#[tokio::test]
async fn side_fork_config_is_ephemeral_and_appends_developer_guardrails() {
    let mut app = make_test_app().await;
    app.config.developer_instructions = Some("Existing developer policy.".to_string());
    let original_approval_policy = app.config.permissions.approval_policy.value();
    let original_sandbox_policy = app.config.permissions.sandbox_policy.get().clone();

    let fork_config = app.side_fork_config();

    assert!(fork_config.ephemeral);
    assert_eq!(
        fork_config.permissions.approval_policy.value(),
        original_approval_policy
    );
    assert_eq!(
        fork_config.permissions.sandbox_policy.get(),
        &original_sandbox_policy
    );
    let developer_instructions = fork_config
        .developer_instructions
        .as_deref()
        .expect("side developer instructions");
    assert!(developer_instructions.contains("Existing developer policy."));
    assert!(
        developer_instructions.contains("You are in a side conversation, not the main thread.")
    );
    assert!(
        developer_instructions
            .contains("inherited fork history is provided only as reference context")
    );
    assert!(
        developer_instructions.contains(
            "Only instructions submitted after the side-conversation boundary are active"
        )
    );
    assert!(developer_instructions.contains("Do not continue, execute, or complete any task"));
    assert!(
        developer_instructions
            .contains("External tools may be available according to this thread's current")
    );
    assert!(
        developer_instructions
            .contains("Any MCP or external tool calls or outputs visible in the inherited")
    );
    assert!(developer_instructions.contains("non-mutating inspection"));
    assert!(developer_instructions.contains("Do not modify files"));
    assert!(developer_instructions.contains("Do not request escalated permissions"));
    assert!(app.transcript_cells.is_empty());
}

#[tokio::test]
async fn side_start_block_message_tracks_open_side_conversation() {
    let mut app = make_test_app().await;
    assert_eq!(
        app.side_start_block_message(),
        Some("'/side' is unavailable until the main thread is ready.")
    );

    app.primary_thread_id = Some(ThreadId::new());
    assert_eq!(app.side_start_block_message(), None);

    let parent_thread_id = ThreadId::new();
    let side_thread_id = ThreadId::new();
    app.side_threads
        .insert(side_thread_id, SideThreadState::new(parent_thread_id));

    assert_eq!(
        app.side_start_block_message(),
        Some("A side conversation is already open. Press Esc to return before starting another.")
    );

    app.side_threads.remove(&side_thread_id);
    assert_eq!(app.side_start_block_message(), None);
}

#[tokio::test]
async fn side_parent_status_tracks_parent_turn_lifecycle() -> Result<()> {
    let mut app = make_test_app().await;
    let parent_thread_id = ThreadId::new();
    let side_thread_id = ThreadId::new();
    app.primary_thread_id = Some(parent_thread_id);
    app.active_thread_id = Some(side_thread_id);
    app.side_threads
        .insert(side_thread_id, SideThreadState::new(parent_thread_id));

    app.enqueue_thread_notification(
        parent_thread_id,
        turn_completed_notification(parent_thread_id, "turn-1", TurnStatus::Completed),
    )
    .await?;
    assert_eq!(
        app.side_threads
            .get(&side_thread_id)
            .and_then(|state| state.parent_status),
        Some(SideParentStatus::Finished)
    );

    app.enqueue_thread_notification(
        parent_thread_id,
        turn_started_notification(parent_thread_id, "turn-2"),
    )
    .await?;
    assert_eq!(
        app.side_threads
            .get(&side_thread_id)
            .and_then(|state| state.parent_status),
        None
    );

    app.enqueue_thread_notification(
        parent_thread_id,
        turn_completed_notification(parent_thread_id, "turn-2", TurnStatus::Failed),
    )
    .await?;
    assert_eq!(
        app.side_threads
            .get(&side_thread_id)
            .and_then(|state| state.parent_status),
        Some(SideParentStatus::Failed)
    );

    Ok(())
}

#[tokio::test]
async fn side_parent_status_prioritizes_input_over_approval() -> Result<()> {
    let mut app = make_test_app().await;
    let parent_thread_id = ThreadId::new();
    let side_thread_id = ThreadId::new();
    app.primary_thread_id = Some(parent_thread_id);
    app.active_thread_id = Some(side_thread_id);
    app.side_threads
        .insert(side_thread_id, SideThreadState::new(parent_thread_id));

    app.enqueue_thread_request(
        parent_thread_id,
        exec_approval_request(
            parent_thread_id,
            "turn-approval",
            "call-approval",
            /*approval_id*/ None,
        ),
    )
    .await?;
    assert_eq!(
        app.side_threads
            .get(&side_thread_id)
            .and_then(|state| state.parent_status),
        Some(SideParentStatus::NeedsApproval)
    );

    app.enqueue_thread_request(
        parent_thread_id,
        request_user_input_request(parent_thread_id, "turn-input", "call-input"),
    )
    .await?;
    assert_eq!(
        app.side_threads
            .get(&side_thread_id)
            .and_then(|state| state.parent_status),
        Some(SideParentStatus::NeedsInput)
    );

    app.enqueue_thread_notification(
        parent_thread_id,
        ServerNotification::ServerRequestResolved(
            codex_app_server_protocol::ServerRequestResolvedNotification {
                thread_id: parent_thread_id.to_string(),
                request_id: AppServerRequestId::Integer(2),
            },
        ),
    )
    .await?;
    assert_eq!(
        app.side_threads
            .get(&side_thread_id)
            .and_then(|state| state.parent_status),
        Some(SideParentStatus::NeedsApproval)
    );

    app.enqueue_thread_notification(
        parent_thread_id,
        ServerNotification::ServerRequestResolved(
            codex_app_server_protocol::ServerRequestResolvedNotification {
                thread_id: parent_thread_id.to_string(),
                request_id: AppServerRequestId::Integer(1),
            },
        ),
    )
    .await?;
    assert_eq!(
        app.side_threads
            .get(&side_thread_id)
            .and_then(|state| state.parent_status),
        None
    );

    Ok(())
}

#[tokio::test]
async fn side_thread_snapshot_hides_forked_parent_transcript() {
    let parent_thread_id = ThreadId::new();
    let side_thread_id = ThreadId::new();
    let mut store = ThreadEventStore::new(/*capacity*/ 4);
    let session = ThreadSessionState {
        forked_from_id: Some(parent_thread_id),
        ..test_thread_session(side_thread_id, test_path_buf("/tmp/side"))
    };
    let parent_turn = test_turn(
        "parent-turn",
        TurnStatus::Completed,
        vec![ThreadItem::UserMessage {
            id: "parent-user".to_string(),
            content: vec![AppServerUserInput::Text {
                text: "parent prompt should stay hidden".to_string(),
                text_elements: Vec::new(),
            }],
        }],
    );

    App::install_side_thread_snapshot(&mut store, session, vec![parent_turn]);

    let stored_session = store.session.as_ref().expect("side session");
    assert_eq!(stored_session.thread_id, side_thread_id);
    assert_eq!(stored_session.forked_from_id, None);
    assert_eq!(store.turns, Vec::<Turn>::new());
    assert_eq!(store.active_turn_id(), None);
}

#[tokio::test]
async fn side_thread_snapshot_does_not_refresh_from_fork_history() {
    let mut app = make_test_app().await;
    let parent_thread_id = ThreadId::new();
    let side_thread_id = ThreadId::new();
    app.side_threads
        .insert(side_thread_id, SideThreadState::new(parent_thread_id));

    let snapshot = ThreadEventSnapshot {
        session: Some(ThreadSessionState {
            rollout_path: None,
            ..test_thread_session(side_thread_id, test_path_buf("/tmp/side"))
        }),
        turns: Vec::new(),
        events: Vec::new(),
        input_state: None,
    };

    assert!(!app.should_refresh_snapshot_session(
        side_thread_id,
        /*is_replay_only*/ false,
        &snapshot
    ));
}

#[tokio::test]
async fn side_thread_snapshot_skips_session_header_preamble() {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    while app_event_rx.try_recv().is_ok() {}

    let parent_thread_id = ThreadId::new();
    let side_thread_id = ThreadId::new();
    app.primary_thread_id = Some(parent_thread_id);
    app.side_threads
        .insert(side_thread_id, SideThreadState::new(parent_thread_id));

    let snapshot = ThreadEventSnapshot {
        session: Some(ThreadSessionState {
            forked_from_id: Some(parent_thread_id),
            ..test_thread_session(side_thread_id, test_path_buf("/tmp/side"))
        }),
        turns: Vec::new(),
        events: Vec::new(),
        input_state: None,
    };

    app.replay_thread_snapshot(snapshot, /*resume_restored_queue*/ false);

    let mut rendered_cells = Vec::new();
    while let Ok(event) = app_event_rx.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event {
            rendered_cells.push(lines_to_single_string(&cell.display_lines(/*width*/ 120)));
        }
    }
    assert_eq!(app.chat_widget.thread_id(), Some(side_thread_id));
    assert_eq!(rendered_cells, Vec::<String>::new());
    assert_eq!(
        app.chat_widget.active_cell_transcript_lines(/*width*/ 120),
        None
    );
}

#[tokio::test]
async fn side_thread_ignores_global_mcp_startup_notifications() {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    while app_event_rx.try_recv().is_ok() {}
    let app_server = crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
        .await
        .expect("embedded app server");
    let parent_thread_id = ThreadId::new();
    let side_thread_id = ThreadId::new();
    app.primary_thread_id = Some(parent_thread_id);
    app.active_thread_id = Some(side_thread_id);
    app.side_threads
        .insert(side_thread_id, SideThreadState::new(parent_thread_id));
    app.sync_side_thread_ui();

    app.handle_app_server_event(
        &app_server,
        codex_app_server_client::AppServerEvent::ServerNotification(
            ServerNotification::McpServerStatusUpdated(McpServerStatusUpdatedNotification {
                name: "sentry".to_string(),
                status: McpServerStartupState::Failed,
                error: Some("sentry is not logged in".to_string()),
            }),
        ),
    )
    .await;

    assert!(app_event_rx.try_recv().is_err());
}

#[tokio::test]
async fn side_restore_user_message_puts_inline_question_back_in_composer() {
    let mut app = make_test_app().await;
    let user_message = crate::chatwidget::UserMessage::from("side question");

    app.restore_side_user_message(Some(user_message));

    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "side question"
    );
}

#[tokio::test]
async fn side_discard_selection_keeps_current_side_thread() {
    let mut app = make_test_app().await;
    let parent_thread_id = ThreadId::new();
    let side_thread_id = ThreadId::new();
    app.active_thread_id = Some(side_thread_id);
    app.side_threads
        .insert(side_thread_id, SideThreadState::new(parent_thread_id));

    assert_eq!(
        app.side_thread_to_discard_after_switch(side_thread_id),
        None
    );
    assert_eq!(
        app.side_thread_to_discard_after_switch(parent_thread_id),
        Some(side_thread_id)
    );
}

#[tokio::test]
async fn discard_side_thread_removes_agent_navigation_entry() -> Result<()> {
    let mut app = make_test_app().await;
    let mut app_server =
        crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref()).await?;
    let mut side_config = app.chat_widget.config_ref().clone();
    side_config.ephemeral = true;
    let started = app_server.start_thread(&side_config).await?;
    let side_thread_id = started.session.thread_id;
    app.side_threads
        .insert(side_thread_id, SideThreadState::new(ThreadId::new()));
    app.agent_navigation.upsert(
        side_thread_id,
        Some("Side".to_string()),
        Some("side".to_string()),
        /*is_closed*/ false,
    );

    assert!(
        app.discard_side_thread(&mut app_server, side_thread_id)
            .await
    );

    assert_eq!(app.agent_navigation.get(&side_thread_id), None);
    assert!(!app.side_threads.contains_key(&side_thread_id));
    Ok(())
}

#[tokio::test]
async fn discard_side_thread_keeps_local_state_when_server_close_fails() -> Result<()> {
    let mut app = make_test_app().await;
    let mut app_server =
        crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref()).await?;
    let parent_thread_id = ThreadId::new();
    let side_thread_id = ThreadId::new();
    app.active_thread_id = Some(side_thread_id);
    app.side_threads
        .insert(side_thread_id, SideThreadState::new(parent_thread_id));
    app.agent_navigation.upsert(
        side_thread_id,
        Some("Side".to_string()),
        Some("side".to_string()),
        /*is_closed*/ false,
    );

    assert!(
        !app.discard_side_thread(&mut app_server, side_thread_id)
            .await
    );

    assert_eq!(app.active_thread_id, Some(side_thread_id));
    assert_eq!(
        app.side_threads
            .get(&side_thread_id)
            .map(|state| state.parent_thread_id),
        Some(parent_thread_id)
    );
    assert!(app.agent_navigation.get(&side_thread_id).is_some());
    Ok(())
}

#[tokio::test]
async fn discard_closed_side_thread_removes_local_state_without_server_rpc() {
    let mut app = make_test_app().await;
    let parent_thread_id = ThreadId::new();
    let side_thread_id = ThreadId::new();
    app.active_thread_id = Some(side_thread_id);
    app.side_threads
        .insert(side_thread_id, SideThreadState::new(parent_thread_id));
    app.thread_event_channels
        .insert(side_thread_id, ThreadEventChannel::new(/*capacity*/ 4));
    app.agent_navigation.upsert(
        side_thread_id,
        Some("Side".to_string()),
        Some("side".to_string()),
        /*is_closed*/ false,
    );

    app.discard_closed_side_thread(side_thread_id).await;

    assert_eq!(app.active_thread_id, None);
    assert!(!app.side_threads.contains_key(&side_thread_id));
    assert!(!app.thread_event_channels.contains_key(&side_thread_id));
    assert_eq!(app.agent_navigation.get(&side_thread_id), None);
}

#[tokio::test]
async fn active_non_primary_shutdown_target_returns_none_for_non_shutdown_event() -> Result<()> {
    let mut app = make_test_app().await;
    app.active_thread_id = Some(ThreadId::new());
    app.primary_thread_id = Some(ThreadId::new());

    assert_eq!(
        app.active_non_primary_shutdown_target(&ServerNotification::SkillsChanged(
            codex_app_server_protocol::SkillsChangedNotification {},
        )),
        None
    );
    Ok(())
}

#[tokio::test]
async fn active_non_primary_shutdown_target_returns_none_for_primary_thread_shutdown() -> Result<()>
{
    let mut app = make_test_app().await;
    let thread_id = ThreadId::new();
    app.active_thread_id = Some(thread_id);
    app.primary_thread_id = Some(thread_id);

    assert_eq!(
        app.active_non_primary_shutdown_target(&thread_closed_notification(thread_id)),
        None
    );
    Ok(())
}

#[tokio::test]
async fn active_non_primary_shutdown_target_returns_ids_for_non_primary_shutdown() -> Result<()> {
    let mut app = make_test_app().await;
    let active_thread_id = ThreadId::new();
    let primary_thread_id = ThreadId::new();
    app.active_thread_id = Some(active_thread_id);
    app.primary_thread_id = Some(primary_thread_id);

    assert_eq!(
        app.active_non_primary_shutdown_target(&thread_closed_notification(active_thread_id)),
        Some((active_thread_id, primary_thread_id))
    );
    Ok(())
}

#[tokio::test]
async fn active_non_primary_shutdown_target_returns_none_when_shutdown_exit_is_pending()
-> Result<()> {
    let mut app = make_test_app().await;
    let active_thread_id = ThreadId::new();
    let primary_thread_id = ThreadId::new();
    app.active_thread_id = Some(active_thread_id);
    app.primary_thread_id = Some(primary_thread_id);
    app.pending_shutdown_exit_thread_id = Some(active_thread_id);

    assert_eq!(
        app.active_non_primary_shutdown_target(&thread_closed_notification(active_thread_id)),
        None
    );
    Ok(())
}

#[tokio::test]
async fn active_non_primary_shutdown_target_still_switches_for_other_pending_exit_thread()
-> Result<()> {
    let mut app = make_test_app().await;
    let active_thread_id = ThreadId::new();
    let primary_thread_id = ThreadId::new();
    app.active_thread_id = Some(active_thread_id);
    app.primary_thread_id = Some(primary_thread_id);
    app.pending_shutdown_exit_thread_id = Some(ThreadId::new());

    assert_eq!(
        app.active_non_primary_shutdown_target(&thread_closed_notification(active_thread_id)),
        Some((active_thread_id, primary_thread_id))
    );
    Ok(())
}

async fn render_clear_ui_header_after_long_transcript_for_snapshot() -> String {
    let mut app = make_test_app().await;
    app.config.cwd = test_path_buf("/tmp/project").abs();
    app.chat_widget.set_model("gpt-test");
    app.chat_widget
        .set_reasoning_effort(Some(ReasoningEffortConfig::High));
    let story_part_one = "In the cliffside town of Bracken Ferry, the lighthouse had been dark for \
            nineteen years, and the children were told it was because the sea no longer wanted a \
            guide. Mara, who repaired clocks for a living, found that hard to believe. Every dawn she \
            heard the gulls circling the empty tower, and every dusk she watched ships hesitate at the \
            mouth of the bay as if listening for a signal that never came. When an old brass key fell \
            out of a cracked parcel in her workshop, tagged only with the words 'for the lamp room,' \
            she decided to climb the hill and see what the town had forgotten.";
    let story_part_two = "Inside the lighthouse she found gears wrapped in oilcloth, logbooks filled \
            with weather notes, and a lens shrouded beneath salt-stiff canvas. The mechanism was not \
            broken, only unfinished. Someone had removed the governor spring and hidden it in a false \
            drawer, along with a letter from the last keeper admitting he had darkened the light on \
            purpose after smugglers threatened his family. Mara spent the night rebuilding the clockwork \
            from spare watch parts, her fingers blackened with soot and grease, while a storm gathered \
            over the water and the harbor bells began to ring.";
    let story_part_three = "At midnight the first squall hit, and the fishing boats returned early, \
            blind in sheets of rain. Mara wound the mechanism, set the teeth by hand, and watched the \
            great lens begin to turn in slow, certain arcs. The beam swept across the bay, caught the \
            whitecaps, and reached the boats just as they were drifting toward the rocks below the \
            eastern cliffs. In the morning the town square was crowded with wet sailors, angry elders, \
            and wide-eyed children, but when the oldest captain placed the keeper's log on the fountain \
            and thanked Mara for relighting the coast, nobody argued. By sunset, Bracken Ferry had a \
            lighthouse again, and Mara had more clocks to mend than ever because everyone wanted \
            something in town to keep better time.";

    let user_cell = |text: &str| -> Arc<dyn HistoryCell> {
        Arc::new(UserHistoryCell {
            message: text.to_string(),
            text_elements: Vec::new(),
            local_image_paths: Vec::new(),
            remote_image_urls: Vec::new(),
        }) as Arc<dyn HistoryCell>
    };
    let agent_cell = |text: &str| -> Arc<dyn HistoryCell> {
        Arc::new(AgentMessageCell::new(
            vec![Line::from(text.to_string())],
            /*is_first_line*/ true,
        )) as Arc<dyn HistoryCell>
    };
    let make_header = |is_first| -> Arc<dyn HistoryCell> {
        let event = SessionConfiguredEvent {
            session_id: ThreadId::new(),
            forked_from_id: None,
            thread_name: None,
            model: "gpt-test".to_string(),
            model_provider_id: "test-provider".to_string(),
            service_tier: None,
            approval_policy: AskForApproval::Never,
            approvals_reviewer: ApprovalsReviewer::User,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            permission_profile: None,
            cwd: test_path_buf("/tmp/project").abs(),
            reasoning_effort: Some(ReasoningEffortConfig::High),
            history_log_id: 0,
            history_entry_count: 0,
            initial_messages: None,
            network_proxy: None,
            rollout_path: Some(PathBuf::new()),
        };
        Arc::new(new_session_info(
            app.chat_widget.config_ref(),
            app.chat_widget.current_model(),
            event,
            is_first,
            /*tooltip_override*/ None,
            /*auth_plan*/ None,
            /*show_fast_status*/ false,
        )) as Arc<dyn HistoryCell>
    };

    app.transcript_cells = vec![
        make_header(true),
        Arc::new(crate::history_cell::new_info_event(
            "startup tip that used to replay".to_string(),
            /*hint*/ None,
        )) as Arc<dyn HistoryCell>,
        user_cell("Tell me a long story about a town with a dark lighthouse."),
        agent_cell(story_part_one),
        user_cell("Continue the story and reveal why the light went out."),
        agent_cell(story_part_two),
        user_cell("Finish the story with a storm and a resolution."),
        agent_cell(story_part_three),
    ];
    app.has_emitted_history_lines = true;

    let rendered = app
        .clear_ui_header_lines_with_version(/*width*/ 80, "<VERSION>")
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        !rendered.contains("startup tip that used to replay"),
        "clear header should not replay startup notices"
    );
    assert!(
        !rendered.contains("Bracken Ferry"),
        "clear header should not replay prior conversation turns"
    );
    rendered
}

#[tokio::test]
#[cfg_attr(
    target_os = "windows",
    ignore = "snapshot path rendering differs on Windows"
)]
async fn clear_ui_after_long_transcript_snapshots_fresh_header_only() {
    let rendered = render_clear_ui_header_after_long_transcript_for_snapshot().await;
    assert_app_snapshot!("clear_ui_after_long_transcript_fresh_header_only", rendered);
}

#[tokio::test]
#[cfg_attr(
    target_os = "windows",
    ignore = "snapshot path rendering differs on Windows"
)]
async fn ctrl_l_clear_ui_after_long_transcript_reuses_clear_header_snapshot() {
    let rendered = render_clear_ui_header_after_long_transcript_for_snapshot().await;
    assert_app_snapshot!("clear_ui_after_long_transcript_fresh_header_only", rendered);
}

#[tokio::test]
#[cfg_attr(
    target_os = "windows",
    ignore = "snapshot path rendering differs on Windows"
)]
async fn clear_ui_header_shows_fast_status_for_fast_capable_models() {
    let mut app = make_test_app().await;
    app.config.cwd = test_path_buf("/tmp/project").abs();
    app.chat_widget.set_model("gpt-5.4");
    set_fast_mode_test_catalog(&mut app.chat_widget);
    app.chat_widget
        .set_reasoning_effort(Some(ReasoningEffortConfig::XHigh));
    app.chat_widget
        .set_service_tier(Some(codex_protocol::config_types::ServiceTier::Fast));
    set_chatgpt_auth(&mut app.chat_widget);
    set_fast_mode_test_catalog(&mut app.chat_widget);

    let rendered = app
        .clear_ui_header_lines_with_version(/*width*/ 80, "<VERSION>")
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert_app_snapshot!("clear_ui_header_fast_status_fast_capable_models", rendered);
}

async fn make_test_app() -> App {
    let (chat_widget, app_event_tx, _rx, _op_rx) = make_chatwidget_manual_with_sender().await;
    let config = chat_widget.config_ref().clone();
    let file_search = FileSearchManager::new(config.cwd.to_path_buf(), app_event_tx.clone());
    let model = crate::legacy_core::test_support::get_model_offline(config.model.as_deref());
    let session_telemetry = test_session_telemetry(&config, model.as_str());

    App {
        model_catalog: chat_widget.model_catalog(),
        session_telemetry,
        app_event_tx,
        chat_widget,
        config,
        active_profile: None,
        cli_kv_overrides: Vec::new(),
        harness_overrides: ConfigOverrides::default(),
        runtime_approval_policy_override: None,
        runtime_sandbox_policy_override: None,
        file_search,
        transcript_cells: Vec::new(),
        overlay: None,
        deferred_history_lines: Vec::new(),
        has_emitted_history_lines: false,
        enhanced_keys_supported: false,
        commit_anim_running: Arc::new(AtomicBool::new(false)),
        status_line_invalid_items_warned: Arc::new(AtomicBool::new(false)),
        terminal_title_invalid_items_warned: Arc::new(AtomicBool::new(false)),
        backtrack: BacktrackState::default(),
        backtrack_render_pending: false,
        feedback: codex_feedback::CodexFeedback::new(),
        feedback_audience: FeedbackAudience::External,
        environment_manager: Arc::new(EnvironmentManager::default_for_tests()),
        remote_app_server_url: None,
        remote_app_server_auth_token: None,
        pending_update_action: None,
        pending_shutdown_exit_thread_id: None,
        windows_sandbox: WindowsSandboxState::default(),
        thread_event_channels: HashMap::new(),
        thread_event_listener_tasks: HashMap::new(),
        agent_navigation: AgentNavigationState::default(),
        side_threads: HashMap::new(),
        active_thread_id: None,
        active_thread_rx: None,
        primary_thread_id: None,
        last_subagent_backfill_attempt: None,
        primary_session_configured: None,
        pending_primary_events: VecDeque::new(),
        pending_app_server_requests: PendingAppServerRequests::default(),
        pending_plugin_enabled_writes: HashMap::new(),
    }
}

async fn make_test_app_with_channels() -> (
    App,
    tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
    tokio::sync::mpsc::UnboundedReceiver<Op>,
) {
    let (chat_widget, app_event_tx, rx, op_rx) = make_chatwidget_manual_with_sender().await;
    let config = chat_widget.config_ref().clone();
    let file_search = FileSearchManager::new(config.cwd.to_path_buf(), app_event_tx.clone());
    let model = crate::legacy_core::test_support::get_model_offline(config.model.as_deref());
    let session_telemetry = test_session_telemetry(&config, model.as_str());

    (
        App {
            model_catalog: chat_widget.model_catalog(),
            session_telemetry,
            app_event_tx,
            chat_widget,
            config,
            active_profile: None,
            cli_kv_overrides: Vec::new(),
            harness_overrides: ConfigOverrides::default(),
            runtime_approval_policy_override: None,
            runtime_sandbox_policy_override: None,
            file_search,
            transcript_cells: Vec::new(),
            overlay: None,
            deferred_history_lines: Vec::new(),
            has_emitted_history_lines: false,
            enhanced_keys_supported: false,
            commit_anim_running: Arc::new(AtomicBool::new(false)),
            status_line_invalid_items_warned: Arc::new(AtomicBool::new(false)),
            terminal_title_invalid_items_warned: Arc::new(AtomicBool::new(false)),
            backtrack: BacktrackState::default(),
            backtrack_render_pending: false,
            feedback: codex_feedback::CodexFeedback::new(),
            feedback_audience: FeedbackAudience::External,
            environment_manager: Arc::new(EnvironmentManager::default_for_tests()),
            remote_app_server_url: None,
            remote_app_server_auth_token: None,
            pending_update_action: None,
            pending_shutdown_exit_thread_id: None,
            windows_sandbox: WindowsSandboxState::default(),
            thread_event_channels: HashMap::new(),
            thread_event_listener_tasks: HashMap::new(),
            agent_navigation: AgentNavigationState::default(),
            side_threads: HashMap::new(),
            active_thread_id: None,
            active_thread_rx: None,
            primary_thread_id: None,
            last_subagent_backfill_attempt: None,
            primary_session_configured: None,
            pending_primary_events: VecDeque::new(),
            pending_app_server_requests: PendingAppServerRequests::default(),
            pending_plugin_enabled_writes: HashMap::new(),
        },
        rx,
        op_rx,
    )
}

fn test_thread_session(thread_id: ThreadId, cwd: PathBuf) -> ThreadSessionState {
    ThreadSessionState {
        thread_id,
        forked_from_id: None,
        fork_parent_title: None,
        thread_name: None,
        model: "gpt-test".to_string(),
        model_provider_id: "test-provider".to_string(),
        service_tier: None,
        approval_policy: AskForApproval::Never,
        approvals_reviewer: ApprovalsReviewer::User,
        sandbox_policy: SandboxPolicy::new_read_only_policy(),
        permission_profile: Some(PermissionProfile::from_legacy_sandbox_policy(
            &SandboxPolicy::new_read_only_policy(),
            cwd.as_path(),
        )),
        cwd: cwd.abs(),
        instruction_source_paths: Vec::new(),
        reasoning_effort: None,
        history_log_id: 0,
        history_entry_count: 0,
        network_proxy: None,
        rollout_path: Some(PathBuf::new()),
    }
}

fn test_turn(turn_id: &str, status: TurnStatus, items: Vec<ThreadItem>) -> Turn {
    Turn {
        id: turn_id.to_string(),
        items,
        status,
        error: None,
        started_at: None,
        completed_at: None,
        duration_ms: None,
    }
}

fn turn_started_notification(thread_id: ThreadId, turn_id: &str) -> ServerNotification {
    ServerNotification::TurnStarted(TurnStartedNotification {
        thread_id: thread_id.to_string(),
        turn: Turn {
            started_at: Some(0),
            ..test_turn(turn_id, TurnStatus::InProgress, Vec::new())
        },
    })
}

fn turn_completed_notification(
    thread_id: ThreadId,
    turn_id: &str,
    status: TurnStatus,
) -> ServerNotification {
    ServerNotification::TurnCompleted(TurnCompletedNotification {
        thread_id: thread_id.to_string(),
        turn: Turn {
            completed_at: Some(0),
            duration_ms: Some(1),
            ..test_turn(turn_id, status, Vec::new())
        },
    })
}

fn thread_closed_notification(thread_id: ThreadId) -> ServerNotification {
    ServerNotification::ThreadClosed(ThreadClosedNotification {
        thread_id: thread_id.to_string(),
    })
}

fn token_usage_notification(
    thread_id: ThreadId,
    turn_id: &str,
    model_context_window: Option<i64>,
) -> ServerNotification {
    ServerNotification::ThreadTokenUsageUpdated(ThreadTokenUsageUpdatedNotification {
        thread_id: thread_id.to_string(),
        turn_id: turn_id.to_string(),
        token_usage: ThreadTokenUsage {
            total: TokenUsageBreakdown {
                total_tokens: 10,
                input_tokens: 4,
                cached_input_tokens: 1,
                output_tokens: 5,
                reasoning_output_tokens: 0,
            },
            last: TokenUsageBreakdown {
                total_tokens: 10,
                input_tokens: 4,
                cached_input_tokens: 1,
                output_tokens: 5,
                reasoning_output_tokens: 0,
            },
            model_context_window,
        },
    })
}

fn agent_message_delta_notification(
    thread_id: ThreadId,
    turn_id: &str,
    item_id: &str,
    delta: &str,
) -> ServerNotification {
    ServerNotification::AgentMessageDelta(AgentMessageDeltaNotification {
        thread_id: thread_id.to_string(),
        turn_id: turn_id.to_string(),
        item_id: item_id.to_string(),
        delta: delta.to_string(),
    })
}

fn exec_approval_request(
    thread_id: ThreadId,
    turn_id: &str,
    item_id: &str,
    approval_id: Option<&str>,
) -> ServerRequest {
    ServerRequest::CommandExecutionRequestApproval {
        request_id: AppServerRequestId::Integer(1),
        params: CommandExecutionRequestApprovalParams {
            thread_id: thread_id.to_string(),
            turn_id: turn_id.to_string(),
            item_id: item_id.to_string(),
            approval_id: approval_id.map(str::to_string),
            reason: Some("needs approval".to_string()),
            network_approval_context: None,
            command: Some("echo hello".to_string()),
            cwd: Some(test_path_buf("/tmp/project").abs()),
            command_actions: None,
            additional_permissions: None,
            proposed_execpolicy_amendment: None,
            proposed_network_policy_amendments: None,
            available_decisions: None,
        },
    }
}

fn request_user_input_request(thread_id: ThreadId, turn_id: &str, item_id: &str) -> ServerRequest {
    ServerRequest::ToolRequestUserInput {
        request_id: AppServerRequestId::Integer(2),
        params: ToolRequestUserInputParams {
            thread_id: thread_id.to_string(),
            turn_id: turn_id.to_string(),
            item_id: item_id.to_string(),
            questions: Vec::new(),
        },
    }
}

#[tokio::test]
async fn feedback_submission_without_thread_emits_error_history_cell() {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;

    app.handle_feedback_submitted(
        /*origin_thread_id*/ None,
        FeedbackCategory::Bug,
        /*include_logs*/ true,
        Err("boom".to_string()),
    )
    .await;

    let cell = match app_event_rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => cell,
        other => panic!("expected feedback error history cell, saw {other:?}"),
    };
    assert_eq!(
        lines_to_single_string(&cell.display_lines(/*width*/ 120)),
        "■ Failed to upload feedback: boom"
    );
}

#[tokio::test]
async fn feedback_submission_for_inactive_thread_replays_into_origin_thread() {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let origin_thread_id = ThreadId::new();
    let active_thread_id = ThreadId::new();
    let origin_session = test_thread_session(origin_thread_id, test_path_buf("/tmp/origin"));
    let active_session = test_thread_session(active_thread_id, test_path_buf("/tmp/active"));
    app.thread_event_channels.insert(
        origin_thread_id,
        ThreadEventChannel::new_with_session(
            THREAD_EVENT_CHANNEL_CAPACITY,
            origin_session.clone(),
            Vec::new(),
        ),
    );
    app.thread_event_channels.insert(
        active_thread_id,
        ThreadEventChannel::new_with_session(
            THREAD_EVENT_CHANNEL_CAPACITY,
            active_session.clone(),
            Vec::new(),
        ),
    );
    app.activate_thread_channel(active_thread_id).await;
    app.chat_widget.handle_thread_session(active_session);
    while app_event_rx.try_recv().is_ok() {}

    app.handle_feedback_submitted(
        Some(origin_thread_id),
        FeedbackCategory::Bug,
        /*include_logs*/ true,
        Ok("uploaded-thread".to_string()),
    )
    .await;

    assert_matches!(
        app_event_rx.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty)
    );

    let snapshot = {
        let channel = app
            .thread_event_channels
            .get(&origin_thread_id)
            .expect("origin thread channel should exist");
        let store = channel.store.lock().await;
        assert!(matches!(
            store.buffer.back(),
            Some(ThreadBufferedEvent::FeedbackSubmission(_))
        ));
        store.snapshot()
    };

    app.replay_thread_snapshot(snapshot, /*resume_restored_queue*/ false);

    let mut rendered_cells = Vec::new();
    while let Ok(event) = app_event_rx.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event {
            rendered_cells.push(lines_to_single_string(&cell.display_lines(/*width*/ 120)));
        }
    }
    assert!(rendered_cells.iter().any(|cell| {
        cell.contains("• Feedback uploaded. Please open an issue using the following URL:")
            && cell.contains("uploaded-thread")
    }));
}

fn next_user_turn_op(op_rx: &mut tokio::sync::mpsc::UnboundedReceiver<Op>) -> Op {
    let mut seen = Vec::new();
    while let Ok(op) = op_rx.try_recv() {
        if matches!(op, Op::UserTurn { .. }) {
            return op;
        }
        seen.push(format!("{op:?}"));
    }
    panic!("expected UserTurn op, saw: {seen:?}");
}

fn lines_to_single_string(lines: &[Line<'_>]) -> String {
    lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn test_session_telemetry(config: &Config, model: &str) -> SessionTelemetry {
    let model_info = crate::legacy_core::test_support::construct_model_info_offline(model, config);
    SessionTelemetry::new(
        ThreadId::new(),
        model,
        model_info.slug.as_str(),
        /*account_id*/ None,
        /*account_email*/ None,
        /*auth_mode*/ None,
        "test_originator".to_string(),
        /*log_user_prompts*/ false,
        "test".to_string(),
        SessionSource::Cli,
    )
}

#[test]
fn active_turn_not_steerable_turn_error_extracts_structured_server_error() {
    let turn_error = AppServerTurnError {
        message: "cannot steer a review turn".to_string(),
        codex_error_info: Some(AppServerCodexErrorInfo::ActiveTurnNotSteerable {
            turn_kind: AppServerNonSteerableTurnKind::Review,
        }),
        additional_details: None,
    };
    let error = TypedRequestError::Server {
        method: "turn/steer".to_string(),
        source: JSONRPCErrorError {
            code: -32602,
            message: turn_error.message.clone(),
            data: Some(serde_json::to_value(&turn_error).expect("turn error should serialize")),
        },
    };

    assert_eq!(
        active_turn_not_steerable_turn_error(&error),
        Some(turn_error)
    );
}

#[test]
fn active_turn_steer_race_detects_missing_active_turn() {
    let error = TypedRequestError::Server {
        method: "turn/steer".to_string(),
        source: JSONRPCErrorError {
            code: -32602,
            message: "no active turn to steer".to_string(),
            data: None,
        },
    };

    assert_eq!(
        active_turn_steer_race(&error),
        Some(ActiveTurnSteerRace::Missing)
    );
    assert_eq!(active_turn_not_steerable_turn_error(&error), None);
}

#[test]
fn active_turn_steer_race_extracts_actual_turn_id_from_mismatch() {
    let error = TypedRequestError::Server {
        method: "turn/steer".to_string(),
        source: JSONRPCErrorError {
            code: -32602,
            message: "expected active turn id `turn-expected` but found `turn-actual`".to_string(),
            data: None,
        },
    };

    assert_eq!(
        active_turn_steer_race(&error),
        Some(ActiveTurnSteerRace::ExpectedTurnMismatch {
            actual_turn_id: "turn-actual".to_string(),
        })
    );
}

#[tokio::test]
async fn fresh_session_config_uses_current_service_tier() {
    let mut app = make_test_app().await;
    app.chat_widget
        .set_service_tier(Some(codex_protocol::config_types::ServiceTier::Fast));

    let config = app.fresh_session_config();

    assert_eq!(
        config.service_tier,
        Some(codex_protocol::config_types::ServiceTier::Fast)
    );
}

#[tokio::test]
async fn backtrack_selection_with_duplicate_history_targets_unique_turn() {
    let (mut app, _app_event_rx, mut op_rx) = make_test_app_with_channels().await;

    let user_cell = |text: &str,
                     text_elements: Vec<TextElement>,
                     local_image_paths: Vec<PathBuf>,
                     remote_image_urls: Vec<String>|
     -> Arc<dyn HistoryCell> {
        Arc::new(UserHistoryCell {
            message: text.to_string(),
            text_elements,
            local_image_paths,
            remote_image_urls,
        }) as Arc<dyn HistoryCell>
    };
    let agent_cell = |text: &str| -> Arc<dyn HistoryCell> {
        Arc::new(AgentMessageCell::new(
            vec![Line::from(text.to_string())],
            /*is_first_line*/ true,
        )) as Arc<dyn HistoryCell>
    };

    let make_header = |is_first| {
        let event = SessionConfiguredEvent {
            session_id: ThreadId::new(),
            forked_from_id: None,
            thread_name: None,
            model: "gpt-test".to_string(),
            model_provider_id: "test-provider".to_string(),
            service_tier: None,
            approval_policy: AskForApproval::Never,
            approvals_reviewer: ApprovalsReviewer::User,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            permission_profile: None,
            cwd: test_path_buf("/home/user/project").abs(),
            reasoning_effort: None,
            history_log_id: 0,
            history_entry_count: 0,
            initial_messages: None,
            network_proxy: None,
            rollout_path: Some(PathBuf::new()),
        };
        Arc::new(new_session_info(
            app.chat_widget.config_ref(),
            app.chat_widget.current_model(),
            event,
            is_first,
            /*tooltip_override*/ None,
            /*auth_plan*/ None,
            /*show_fast_status*/ false,
        )) as Arc<dyn HistoryCell>
    };

    let placeholder = "[Image #1]";
    let edited_text = format!("follow-up (edited) {placeholder}");
    let edited_range = edited_text.len().saturating_sub(placeholder.len())..edited_text.len();
    let edited_text_elements = vec![TextElement::new(
        edited_range.into(),
        /*placeholder*/ None,
    )];
    let edited_local_image_paths = vec![PathBuf::from("/tmp/fake-image.png")];

    // Simulate a transcript with duplicated history (e.g., from prior backtracks)
    // and an edited turn appended after a session header boundary.
    app.transcript_cells = vec![
        make_header(true),
        user_cell("first question", Vec::new(), Vec::new(), Vec::new()),
        agent_cell("answer first"),
        user_cell("follow-up", Vec::new(), Vec::new(), Vec::new()),
        agent_cell("answer follow-up"),
        make_header(false),
        user_cell("first question", Vec::new(), Vec::new(), Vec::new()),
        agent_cell("answer first"),
        user_cell(
            &edited_text,
            edited_text_elements.clone(),
            edited_local_image_paths.clone(),
            vec!["https://example.com/backtrack.png".to_string()],
        ),
        agent_cell("answer edited"),
    ];

    assert_eq!(user_count(&app.transcript_cells), 2);

    let base_id = ThreadId::new();
    app.chat_widget.handle_codex_event(Event {
        id: String::new(),
        msg: EventMsg::SessionConfigured(SessionConfiguredEvent {
            session_id: base_id,
            forked_from_id: None,
            thread_name: None,
            model: "gpt-test".to_string(),
            model_provider_id: "test-provider".to_string(),
            service_tier: None,
            approval_policy: AskForApproval::Never,
            approvals_reviewer: ApprovalsReviewer::User,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            permission_profile: None,
            cwd: test_path_buf("/home/user/project").abs(),
            reasoning_effort: None,
            history_log_id: 0,
            history_entry_count: 0,
            initial_messages: None,
            network_proxy: None,
            rollout_path: Some(PathBuf::new()),
        }),
    });

    app.backtrack.base_id = Some(base_id);
    app.backtrack.primed = true;
    app.backtrack.nth_user_message = user_count(&app.transcript_cells).saturating_sub(1);

    let selection = app
        .confirm_backtrack_from_main()
        .expect("backtrack selection");
    assert_eq!(selection.nth_user_message, 1);
    assert_eq!(selection.prefill, edited_text);
    assert_eq!(selection.text_elements, edited_text_elements);
    assert_eq!(selection.local_image_paths, edited_local_image_paths);
    assert_eq!(
        selection.remote_image_urls,
        vec!["https://example.com/backtrack.png".to_string()]
    );

    app.apply_backtrack_rollback(selection);
    assert_eq!(
        app.chat_widget.remote_image_urls(),
        vec!["https://example.com/backtrack.png".to_string()]
    );

    let mut rollback_turns = None;
    while let Ok(op) = op_rx.try_recv() {
        if let Op::ThreadRollback { num_turns } = op {
            rollback_turns = Some(num_turns);
        }
    }

    assert_eq!(rollback_turns, Some(1));
}

#[tokio::test]
async fn backtrack_remote_image_only_selection_clears_existing_composer_draft() {
    let (mut app, _app_event_rx, mut op_rx) = make_test_app_with_channels().await;

    app.transcript_cells = vec![Arc::new(UserHistoryCell {
        message: "original".to_string(),
        text_elements: Vec::new(),
        local_image_paths: Vec::new(),
        remote_image_urls: Vec::new(),
    }) as Arc<dyn HistoryCell>];
    app.chat_widget
        .set_composer_text("stale draft".to_string(), Vec::new(), Vec::new());

    let remote_image_url = "https://example.com/remote-only.png".to_string();
    app.apply_backtrack_rollback(BacktrackSelection {
        nth_user_message: 0,
        prefill: String::new(),
        text_elements: Vec::new(),
        local_image_paths: Vec::new(),
        remote_image_urls: vec![remote_image_url.clone()],
    });

    assert_eq!(app.chat_widget.composer_text_with_pending(), "");
    assert_eq!(app.chat_widget.remote_image_urls(), vec![remote_image_url]);

    let mut rollback_turns = None;
    while let Ok(op) = op_rx.try_recv() {
        if let Op::ThreadRollback { num_turns } = op {
            rollback_turns = Some(num_turns);
        }
    }
    assert_eq!(rollback_turns, Some(1));
}

#[tokio::test]
async fn backtrack_resubmit_preserves_data_image_urls_in_user_turn() {
    let (mut app, _app_event_rx, mut op_rx) = make_test_app_with_channels().await;

    let thread_id = ThreadId::new();
    app.chat_widget.handle_codex_event(Event {
        id: String::new(),
        msg: EventMsg::SessionConfigured(SessionConfiguredEvent {
            session_id: thread_id,
            forked_from_id: None,
            thread_name: None,
            model: "gpt-test".to_string(),
            model_provider_id: "test-provider".to_string(),
            service_tier: None,
            approval_policy: AskForApproval::Never,
            approvals_reviewer: ApprovalsReviewer::User,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            permission_profile: None,
            cwd: test_path_buf("/home/user/project").abs(),
            reasoning_effort: None,
            history_log_id: 0,
            history_entry_count: 0,
            initial_messages: None,
            network_proxy: None,
            rollout_path: Some(PathBuf::new()),
        }),
    });

    let data_image_url = "data:image/png;base64,abc123".to_string();
    app.transcript_cells = vec![Arc::new(UserHistoryCell {
        message: "please inspect this".to_string(),
        text_elements: Vec::new(),
        local_image_paths: Vec::new(),
        remote_image_urls: vec![data_image_url.clone()],
    }) as Arc<dyn HistoryCell>];

    app.apply_backtrack_rollback(BacktrackSelection {
        nth_user_message: 0,
        prefill: "please inspect this".to_string(),
        text_elements: Vec::new(),
        local_image_paths: Vec::new(),
        remote_image_urls: vec![data_image_url.clone()],
    });

    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    let mut saw_rollback = false;
    let mut submitted_items: Option<Vec<UserInput>> = None;
    while let Ok(op) = op_rx.try_recv() {
        match op {
            Op::ThreadRollback { .. } => saw_rollback = true,
            Op::UserTurn { items, .. } => submitted_items = Some(items),
            _ => {}
        }
    }

    assert!(saw_rollback);
    let items = submitted_items.expect("expected user turn after backtrack resubmit");
    assert!(items.iter().any(|item| {
        matches!(
            item,
            UserInput::Image { image_url } if image_url == &data_image_url
        )
    }));
}

#[tokio::test]
async fn replay_thread_snapshot_replays_turn_history_in_order() {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let thread_id = ThreadId::new();
    app.replay_thread_snapshot(
        ThreadEventSnapshot {
            session: Some(test_thread_session(
                thread_id,
                test_path_buf("/home/user/project"),
            )),
            turns: vec![
                Turn {
                    id: "turn-1".to_string(),
                    items: vec![ThreadItem::UserMessage {
                        id: "user-1".to_string(),
                        content: vec![AppServerUserInput::Text {
                            text: "first prompt".to_string(),
                            text_elements: Vec::new(),
                        }],
                    }],
                    status: TurnStatus::Completed,
                    error: None,
                    started_at: None,
                    completed_at: None,
                    duration_ms: None,
                },
                Turn {
                    id: "turn-2".to_string(),
                    items: vec![
                        ThreadItem::UserMessage {
                            id: "user-2".to_string(),
                            content: vec![AppServerUserInput::Text {
                                text: "third prompt".to_string(),
                                text_elements: Vec::new(),
                            }],
                        },
                        ThreadItem::AgentMessage {
                            id: "assistant-2".to_string(),
                            text: "done".to_string(),
                            phase: None,
                            memory_citation: None,
                        },
                    ],
                    status: TurnStatus::Completed,
                    error: None,
                    started_at: None,
                    completed_at: None,
                    duration_ms: None,
                },
            ],
            events: Vec::new(),
            input_state: None,
        },
        /*resume_restored_queue*/ false,
    );

    while let Ok(event) = app_event_rx.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event {
            let cell: Arc<dyn HistoryCell> = cell.into();
            app.transcript_cells.push(cell);
        }
    }

    let user_messages: Vec<String> = app
        .transcript_cells
        .iter()
        .filter_map(|cell| {
            cell.as_any()
                .downcast_ref::<UserHistoryCell>()
                .map(|cell| cell.message.clone())
        })
        .collect();
    assert_eq!(
        user_messages,
        vec!["first prompt".to_string(), "third prompt".to_string()]
    );
}

#[tokio::test]
async fn replace_chat_widget_reseeds_collab_agent_metadata_for_replay() {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let receiver_thread_id =
        ThreadId::from_string("019cff70-2599-75e2-af72-b958ce5dc1cc").expect("valid thread");
    app.agent_navigation.upsert(
        receiver_thread_id,
        Some("Robie".to_string()),
        Some("explorer".to_string()),
        /*is_closed*/ false,
    );

    let replacement = ChatWidget::new_with_app_event(ChatWidgetInit {
        config: app.config.clone(),
        frame_requester: crate::tui::FrameRequester::test_dummy(),
        app_event_tx: app.app_event_tx.clone(),
        initial_user_message: None,
        enhanced_keys_supported: app.enhanced_keys_supported,
        has_chatgpt_account: app.chat_widget.has_chatgpt_account(),
        model_catalog: app.model_catalog.clone(),
        feedback: app.feedback.clone(),
        is_first_run: false,
        status_account_display: app.chat_widget.status_account_display().cloned(),
        initial_plan_type: app.chat_widget.current_plan_type(),
        model: Some(app.chat_widget.current_model().to_string()),
        startup_tooltip_override: None,
        status_line_invalid_items_warned: app.status_line_invalid_items_warned.clone(),
        terminal_title_invalid_items_warned: app.terminal_title_invalid_items_warned.clone(),
        session_telemetry: app.session_telemetry.clone(),
    });
    app.replace_chat_widget(replacement);

    app.replay_thread_snapshot(
        ThreadEventSnapshot {
            session: None,
            turns: Vec::new(),
            events: vec![ThreadBufferedEvent::Notification(
                ServerNotification::ItemStarted(
                    codex_app_server_protocol::ItemStartedNotification {
                        thread_id: "thread-1".to_string(),
                        turn_id: "turn-1".to_string(),
                        item: ThreadItem::CollabAgentToolCall {
                            id: "wait-1".to_string(),
                            tool: codex_app_server_protocol::CollabAgentTool::Wait,
                            status:
                                codex_app_server_protocol::CollabAgentToolCallStatus::InProgress,
                            sender_thread_id: ThreadId::new().to_string(),
                            receiver_thread_ids: vec![receiver_thread_id.to_string()],
                            prompt: None,
                            model: None,
                            reasoning_effort: None,
                            agents_states: HashMap::new(),
                        },
                    },
                ),
            )],
            input_state: None,
        },
        /*resume_restored_queue*/ false,
    );

    let mut saw_named_wait = false;
    while let Ok(event) = app_event_rx.try_recv() {
        if let AppEvent::InsertHistoryCell(cell) = event {
            let transcript = lines_to_single_string(&cell.transcript_lines(/*width*/ 80));
            saw_named_wait |= transcript.contains("Robie [explorer]");
        }
    }

    assert!(
        saw_named_wait,
        "expected replayed wait item to keep agent name"
    );
}

#[tokio::test]
async fn refreshed_snapshot_session_persists_resumed_turns() {
    let mut app = make_test_app().await;
    let thread_id = ThreadId::new();
    let initial_session = test_thread_session(thread_id, test_path_buf("/tmp/original"));
    app.thread_event_channels.insert(
        thread_id,
        ThreadEventChannel::new_with_session(
            /*capacity*/ 4,
            initial_session.clone(),
            Vec::new(),
        ),
    );

    let resumed_turns = vec![test_turn(
        "turn-1",
        TurnStatus::Completed,
        vec![ThreadItem::UserMessage {
            id: "user-1".to_string(),
            content: vec![AppServerUserInput::Text {
                text: "restored prompt".to_string(),
                text_elements: Vec::new(),
            }],
        }],
    )];
    let resumed_session = ThreadSessionState {
        cwd: test_path_buf("/tmp/refreshed").abs(),
        ..initial_session.clone()
    };
    let mut snapshot = ThreadEventSnapshot {
        session: Some(initial_session),
        turns: Vec::new(),
        events: Vec::new(),
        input_state: None,
    };

    app.apply_refreshed_snapshot_thread(
        thread_id,
        AppServerStartedThread {
            session: resumed_session.clone(),
            turns: resumed_turns.clone(),
        },
        &mut snapshot,
    )
    .await;

    assert_eq!(snapshot.session, Some(resumed_session.clone()));
    assert_eq!(snapshot.turns, resumed_turns);

    let store = app
        .thread_event_channels
        .get(&thread_id)
        .expect("thread channel")
        .store
        .lock()
        .await;
    let store_snapshot = store.snapshot();
    assert_eq!(store_snapshot.session, Some(resumed_session));
    assert_eq!(store_snapshot.turns, snapshot.turns);
}

#[tokio::test]
async fn queued_rollback_syncs_overlay_and_clears_deferred_history() {
    let mut app = make_test_app().await;
    app.transcript_cells = vec![
        Arc::new(UserHistoryCell {
            message: "first".to_string(),
            text_elements: Vec::new(),
            local_image_paths: Vec::new(),
            remote_image_urls: Vec::new(),
        }) as Arc<dyn HistoryCell>,
        Arc::new(AgentMessageCell::new(
            vec![Line::from("after first")],
            /*is_first_line*/ false,
        )) as Arc<dyn HistoryCell>,
        Arc::new(UserHistoryCell {
            message: "second".to_string(),
            text_elements: Vec::new(),
            local_image_paths: Vec::new(),
            remote_image_urls: Vec::new(),
        }) as Arc<dyn HistoryCell>,
        Arc::new(AgentMessageCell::new(
            vec![Line::from("after second")],
            /*is_first_line*/ false,
        )) as Arc<dyn HistoryCell>,
    ];
    app.overlay = Some(Overlay::new_transcript(app.transcript_cells.clone()));
    app.deferred_history_lines = vec![Line::from("stale buffered line")];
    app.backtrack.overlay_preview_active = true;
    app.backtrack.nth_user_message = 1;

    let changed = app.apply_non_pending_thread_rollback(/*num_turns*/ 1);

    assert!(changed);
    assert!(app.backtrack_render_pending);
    assert!(app.deferred_history_lines.is_empty());
    assert_eq!(app.backtrack.nth_user_message, 0);
    let user_messages: Vec<String> = app
        .transcript_cells
        .iter()
        .filter_map(|cell| {
            cell.as_any()
                .downcast_ref::<UserHistoryCell>()
                .map(|cell| cell.message.clone())
        })
        .collect();
    assert_eq!(user_messages, vec!["first".to_string()]);
    let overlay_cell_count = match app.overlay.as_ref() {
        Some(Overlay::Transcript(t)) => t.committed_cell_count(),
        _ => panic!("expected transcript overlay"),
    };
    assert_eq!(overlay_cell_count, app.transcript_cells.len());
}

#[tokio::test]
async fn thread_rollback_response_discards_queued_active_thread_events() {
    let mut app = make_test_app().await;
    let thread_id = ThreadId::new();
    let (tx, rx) = mpsc::channel(8);
    app.active_thread_id = Some(thread_id);
    app.active_thread_rx = Some(rx);
    tx.send(ThreadBufferedEvent::Notification(
        ServerNotification::ConfigWarning(ConfigWarningNotification {
            summary: "stale warning".to_string(),
            details: None,
            path: None,
            range: None,
        }),
    ))
    .await
    .expect("event should queue");

    app.handle_thread_rollback_response(
        thread_id,
        /*num_turns*/ 1,
        &ThreadRollbackResponse {
            thread: Thread {
                id: thread_id.to_string(),
                forked_from_id: None,
                preview: String::new(),
                ephemeral: false,
                model_provider: "openai".to_string(),
                created_at: 0,
                updated_at: 0,
                status: codex_app_server_protocol::ThreadStatus::Idle,
                path: None,
                cwd: test_path_buf("/tmp/project").abs(),
                cli_version: "0.0.0".to_string(),
                source: SessionSource::Cli.into(),
                agent_nickname: None,
                agent_role: None,
                git_info: None,
                name: None,
                turns: Vec::new(),
            },
        },
    )
    .await;

    let rx = app
        .active_thread_rx
        .as_mut()
        .expect("active receiver should remain attached");
    assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));
}

#[tokio::test]
async fn new_session_requests_shutdown_for_previous_conversation() {
    let (mut app, mut app_event_rx, mut op_rx) = make_test_app_with_channels().await;

    let thread_id = ThreadId::new();
    let event = SessionConfiguredEvent {
        session_id: thread_id,
        forked_from_id: None,
        thread_name: None,
        model: "gpt-test".to_string(),
        model_provider_id: "test-provider".to_string(),
        service_tier: None,
        approval_policy: AskForApproval::Never,
        approvals_reviewer: ApprovalsReviewer::User,
        sandbox_policy: SandboxPolicy::new_read_only_policy(),
        permission_profile: None,
        cwd: test_path_buf("/home/user/project").abs(),
        reasoning_effort: None,
        history_log_id: 0,
        history_entry_count: 0,
        initial_messages: None,
        network_proxy: None,
        rollout_path: Some(PathBuf::new()),
    };

    app.chat_widget.handle_codex_event(Event {
        id: String::new(),
        msg: EventMsg::SessionConfigured(event),
    });

    while app_event_rx.try_recv().is_ok() {}
    while op_rx.try_recv().is_ok() {}

    let mut app_server = crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
        .await
        .expect("embedded app server");
    app.shutdown_current_thread(&mut app_server).await;

    assert!(
        op_rx.try_recv().is_err(),
        "shutdown should not submit Op::Shutdown"
    );
}

#[tokio::test]
async fn shutdown_first_exit_returns_immediate_exit_when_shutdown_submit_fails() {
    let mut app = make_test_app().await;
    let thread_id = ThreadId::new();
    app.active_thread_id = Some(thread_id);

    let mut app_server = crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
        .await
        .expect("embedded app server");
    let control = app
        .handle_exit_mode(&mut app_server, ExitMode::ShutdownFirst)
        .await;

    assert_eq!(app.pending_shutdown_exit_thread_id, None);
    assert!(matches!(
        control,
        AppRunControl::Exit(ExitReason::UserRequested)
    ));
}

#[tokio::test]
async fn shutdown_first_exit_uses_app_server_shutdown_without_submitting_op() {
    let (mut app, _app_event_rx, mut op_rx) = make_test_app_with_channels().await;
    let thread_id = ThreadId::new();
    app.active_thread_id = Some(thread_id);

    let mut app_server = crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
        .await
        .expect("embedded app server");
    let control = app
        .handle_exit_mode(&mut app_server, ExitMode::ShutdownFirst)
        .await;

    assert_eq!(app.pending_shutdown_exit_thread_id, None);
    assert!(matches!(
        control,
        AppRunControl::Exit(ExitReason::UserRequested)
    ));
    assert!(
        op_rx.try_recv().is_err(),
        "shutdown should not submit Op::Shutdown"
    );
}

#[tokio::test]
async fn interrupt_without_active_turn_is_treated_as_handled() {
    let mut app = make_test_app().await;
    let mut app_server = crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
        .await
        .expect("embedded app server");
    let started = app_server
        .start_thread(app.chat_widget.config_ref())
        .await
        .expect("thread/start should succeed");
    let thread_id = started.session.thread_id;
    app.enqueue_primary_thread_session(started.session, started.turns)
        .await
        .expect("primary thread should be registered");
    let op = AppCommand::interrupt();

    let handled = app
        .try_submit_active_thread_op_via_app_server(&mut app_server, thread_id, &op)
        .await
        .expect("interrupt submission should not fail");

    assert_eq!(handled, true);
}

#[tokio::test]
async fn clear_only_ui_reset_preserves_chat_session_state() {
    let mut app = make_test_app().await;
    let thread_id = ThreadId::new();
    app.chat_widget.handle_codex_event(Event {
        id: String::new(),
        msg: EventMsg::SessionConfigured(SessionConfiguredEvent {
            session_id: thread_id,
            forked_from_id: None,
            thread_name: Some("keep me".to_string()),
            model: "gpt-test".to_string(),
            model_provider_id: "test-provider".to_string(),
            service_tier: None,
            approval_policy: AskForApproval::Never,
            approvals_reviewer: ApprovalsReviewer::User,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            permission_profile: None,
            cwd: test_path_buf("/tmp/project").abs(),
            reasoning_effort: None,
            history_log_id: 0,
            history_entry_count: 0,
            initial_messages: None,
            network_proxy: None,
            rollout_path: Some(PathBuf::new()),
        }),
    });
    app.chat_widget
        .apply_external_edit("draft prompt".to_string());
    app.transcript_cells = vec![Arc::new(UserHistoryCell {
        message: "old message".to_string(),
        text_elements: Vec::new(),
        local_image_paths: Vec::new(),
        remote_image_urls: Vec::new(),
    }) as Arc<dyn HistoryCell>];
    app.overlay = Some(Overlay::new_transcript(app.transcript_cells.clone()));
    app.deferred_history_lines = vec![Line::from("stale buffered line")];
    app.has_emitted_history_lines = true;
    app.backtrack.primed = true;
    app.backtrack.overlay_preview_active = true;
    app.backtrack.nth_user_message = 0;
    app.backtrack_render_pending = true;

    app.reset_app_ui_state_after_clear();

    assert!(app.overlay.is_none());
    assert!(app.transcript_cells.is_empty());
    assert!(app.deferred_history_lines.is_empty());
    assert!(!app.has_emitted_history_lines);
    assert!(!app.backtrack.primed);
    assert!(!app.backtrack.overlay_preview_active);
    assert!(app.backtrack.pending_rollback.is_none());
    assert!(!app.backtrack_render_pending);
    assert_eq!(app.chat_widget.thread_id(), Some(thread_id));
    assert_eq!(app.chat_widget.composer_text_with_pending(), "draft prompt");
}

#[tokio::test]
async fn session_summary_skips_when_no_usage_or_resume_hint() {
    assert!(
        session_summary(
            TokenUsage::default(),
            /*thread_id*/ None,
            /*thread_name*/ None,
            /*rollout_path*/ None,
        )
        .is_none()
    );
}

#[tokio::test]
async fn session_summary_skips_resume_hint_until_rollout_exists() {
    let usage = TokenUsage::default();
    let conversation = ThreadId::from_string("123e4567-e89b-12d3-a456-426614174000").unwrap();
    let temp_dir = tempdir().expect("temp dir");
    let rollout_path = temp_dir.path().join("rollout.jsonl");

    assert!(
        session_summary(
            usage,
            Some(conversation),
            /*thread_name*/ None,
            Some(&rollout_path),
        )
        .is_none()
    );
}

#[tokio::test]
async fn session_summary_includes_resume_hint_for_persisted_rollout() {
    let usage = TokenUsage {
        input_tokens: 10,
        output_tokens: 2,
        total_tokens: 12,
        ..Default::default()
    };
    let conversation = ThreadId::from_string("123e4567-e89b-12d3-a456-426614174000").unwrap();
    let temp_dir = tempdir().expect("temp dir");
    let rollout_path = temp_dir.path().join("rollout.jsonl");
    std::fs::write(&rollout_path, "{}\n").expect("write rollout");

    let summary = session_summary(
        usage,
        Some(conversation),
        /*thread_name*/ None,
        Some(&rollout_path),
    )
    .expect("summary");
    assert_eq!(
        summary.usage_line,
        Some("Token usage: total=12 input=10 output=2".to_string())
    );
    assert_eq!(
        summary.resume_command,
        Some("codex resume 123e4567-e89b-12d3-a456-426614174000".to_string())
    );
}

#[tokio::test]
async fn session_summary_uses_id_even_when_thread_has_name() {
    let usage = TokenUsage {
        input_tokens: 10,
        output_tokens: 2,
        total_tokens: 12,
        ..Default::default()
    };
    let conversation = ThreadId::from_string("123e4567-e89b-12d3-a456-426614174000").unwrap();
    let temp_dir = tempdir().expect("temp dir");
    let rollout_path = temp_dir.path().join("rollout.jsonl");
    std::fs::write(&rollout_path, "{}\n").expect("write rollout");

    let summary = session_summary(
        usage,
        Some(conversation),
        Some("my-session".to_string()),
        Some(&rollout_path),
    )
    .expect("summary");
    assert_eq!(
        summary.resume_command,
        Some("codex resume 123e4567-e89b-12d3-a456-426614174000".to_string())
    );
}
