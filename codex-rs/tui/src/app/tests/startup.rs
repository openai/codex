use super::*;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::BottomPane;
use crate::bottom_pane::BottomPaneParams;
use crate::tui::FrameRequester;
use app_test_support::create_fake_parented_rollout_with_source;
use codex_app_server_protocol::ToolRequestUserInputOption;
use codex_app_server_protocol::ToolRequestUserInputQuestion;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::unbounded_channel;

fn startup_bottom_pane() -> (BottomPane, UnboundedReceiver<AppEvent>) {
    let (app_event_tx, app_event_rx) = unbounded_channel();
    (
        BottomPane::new(BottomPaneParams {
            app_event_tx: AppEventSender::new(app_event_tx),
            frame_requester: FrameRequester::test_dummy(),
            has_input_focus: true,
            enhanced_keys_supported: false,
            placeholder_text: "Ask Codex to do anything".to_string(),
            disable_paste_burst: true,
            animations_enabled: true,
            skills: None,
        }),
        app_event_rx,
    )
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
                history_mode: None,
            }
        )),
        false
    );
    assert_eq!(
        App::should_wait_for_initial_session(&SessionSelection::Fork(
            crate::resume_picker::SessionTarget {
                path: Some(PathBuf::from("/tmp/fork")),
                thread_id: ThreadId::new(),
                history_mode: None,
            }
        )),
        false
    );
}

#[test]
fn startup_paused_goal_prompt_gate_is_only_for_quiet_resume() {
    let resume = SessionSelection::Resume(crate::resume_picker::SessionTarget {
        path: Some(PathBuf::from("/tmp/restore")),
        thread_id: ThreadId::new(),
        history_mode: None,
    });
    let fork = SessionSelection::Fork(crate::resume_picker::SessionTarget {
        path: Some(PathBuf::from("/tmp/fork")),
        thread_id: ThreadId::new(),
        history_mode: None,
    });
    let no_images: Vec<PathBuf> = Vec::new();
    let initial_images = vec![PathBuf::from("/tmp/image.png")];

    assert!(App::should_prompt_for_paused_goal_after_startup_resume(
        &resume, &None, &no_images
    ));
    assert!(!App::should_prompt_for_paused_goal_after_startup_resume(
        &resume,
        &Some("continue from here".to_string()),
        &no_images
    ));
    assert!(!App::should_prompt_for_paused_goal_after_startup_resume(
        &resume,
        &None,
        &initial_images
    ));
    assert!(!App::should_prompt_for_paused_goal_after_startup_resume(
        &SessionSelection::StartFresh,
        &None,
        &no_images
    ));
    assert!(!App::should_prompt_for_paused_goal_after_startup_resume(
        &fork, &None, &no_images
    ));
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
            history_mode: None,
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
            history_mode: None,
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
async fn queued_startup_requests_block_terminal_input_after_draft_handoff() -> Result<()> {
    for user_input in [false, true] {
        let (mut app, mut app_event_rx, mut op_rx) = make_test_app_with_channels().await;
        app.startup_protected_input_boundary = true;
        let thread_id = ThreadId::new();
        app.enqueue_primary_thread_session(
            test_thread_session(thread_id, test_path_buf("/tmp/project")),
            Vec::new(),
        )
        .await?;
        while app_event_rx.try_recv().is_ok() {}

        let (mut startup_pane, _startup_app_event_rx) = startup_bottom_pane();
        startup_pane.set_composer_text("draft".to_string(), Vec::new(), Vec::new());
        let mut pending_startup_draft = Some(startup_pane.composer_draft_snapshot());
        app.chat_widget
            .restore_startup_draft_when_ready(&mut pending_startup_draft);
        assert!(pending_startup_draft.is_none());
        assert_eq!(app.chat_widget.composer_text_with_pending(), "draft");
        assert!(!app.has_queued_startup_protected_request());

        let request = if user_input {
            request_user_input_request(thread_id, "turn-1", "call-1")
        } else {
            exec_approval_request(thread_id, "turn-1", "call-1", /*approval_id*/ None)
        };
        let _ = app
            .pending_app_server_requests
            .note_server_request(&request);
        app.enqueue_primary_thread_request(request).await?;

        assert!(
            app.active_thread_rx
                .as_ref()
                .is_some_and(|receiver| !receiver.is_empty())
        );
        assert!(
            app.has_queued_startup_protected_request(),
            "a queued protected startup request must be handled before terminal input"
        );
        assert_eq!(app.chat_widget.composer_text_with_pending(), "draft");
        while let Ok(op) = op_rx.try_recv() {
            assert!(
                !matches!(
                    op,
                    Op::UserTurn { .. } | Op::ExecApproval { .. } | Op::UserInputAnswer { .. }
                ),
                "terminal input must not submit or answer the queued protected request: {op:?}"
            );
        }

        let event = app
            .active_thread_rx
            .as_mut()
            .expect("primary thread receiver should be active")
            .try_recv()
            .expect("protected request should be queued on the active thread");
        app.handle_thread_event_now(event);

        assert!(
            !app.has_queued_startup_protected_request(),
            "terminal polling must resume after the protected event is drained"
        );
        assert!(app.startup_protected_input_boundary);
        assert_eq!(app.chat_widget.composer_text_with_pending(), "draft");

        app.startup_protected_input_boundary = false;
        app.enqueue_primary_thread_request(exec_approval_request(
            thread_id, "turn-2", "call-2", /*approval_id*/ None,
        ))
        .await?;
        assert!(
            !app.has_queued_startup_protected_request(),
            "ordinary post-startup protected requests must not block terminal input"
        );
    }

    Ok(())
}

#[tokio::test]
async fn queued_startup_requests_block_draft_restore_until_drained() -> Result<()> {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    app.startup_protected_input_boundary = true;
    let thread_id = ThreadId::new();
    let request = exec_approval_request(thread_id, "turn-1", "call-1", /*approval_id*/ None);
    let _ = app
        .pending_app_server_requests
        .note_server_request(&request);
    app.enqueue_primary_thread_request(request).await?;

    assert!(app.has_queued_startup_protected_request());

    app.enqueue_primary_thread_session(
        test_thread_session(thread_id, test_path_buf("/tmp/project")),
        Vec::new(),
    )
    .await?;
    while app_event_rx.try_recv().is_ok() {}
    assert!(app.has_queued_startup_protected_request());

    let (mut startup_pane, _startup_app_event_rx) = startup_bottom_pane();
    startup_pane.set_composer_text("draft".to_string(), Vec::new(), Vec::new());
    let mut pending_startup_draft = Some(startup_pane.composer_draft_snapshot());
    if app_event_rx.is_empty() && !app.has_queued_startup_protected_request() {
        app.chat_widget
            .restore_startup_draft_when_ready(&mut pending_startup_draft);
    }

    assert!(pending_startup_draft.is_some());
    assert!(app.chat_widget.composer_is_empty());

    let event = app
        .active_thread_rx
        .as_mut()
        .expect("primary thread receiver should be active")
        .try_recv()
        .expect("protected request should be queued on the active thread");
    app.handle_thread_event_now(event);

    assert!(!app.has_queued_startup_protected_request());
    assert!(app.chat_widget.has_active_view());
    app.chat_widget
        .restore_startup_draft_when_ready(&mut pending_startup_draft);
    assert!(pending_startup_draft.is_some());
    assert!(app.chat_widget.composer_is_empty());

    Ok(())
}

#[tokio::test]
async fn startup_draft_handoff_keeps_approval_shortcuts_in_recently_active_composer() -> Result<()>
{
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let thread_id = ThreadId::new();
    app.enqueue_primary_thread_session(
        test_thread_session(thread_id, test_path_buf("/tmp/project")),
        Vec::new(),
    )
    .await?;

    let (mut startup_pane, _startup_app_event_rx) = startup_bottom_pane();
    startup_pane.handle_key_event(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
    let mut pending_startup_draft = Some(startup_pane.composer_draft_snapshot());
    app.chat_widget
        .restore_startup_draft_when_ready(&mut pending_startup_draft);

    assert!(pending_startup_draft.is_none());
    assert_eq!(app.chat_widget.composer_text_with_pending(), "x");

    let approval_request =
        exec_approval_request(thread_id, "turn-1", "call-1", /*approval_id*/ None);
    let _ = app
        .pending_app_server_requests
        .note_server_request(&approval_request);
    app.enqueue_primary_thread_request(approval_request).await?;
    let approval_event = app
        .active_thread_rx
        .as_mut()
        .expect("primary thread receiver should be active")
        .try_recv()
        .expect("approval should be queued on the active thread");
    app.handle_thread_event_now(approval_event);

    assert!(!app.chat_widget.has_active_view());
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    assert_eq!(app.chat_widget.composer_text_with_pending(), "xy");

    while let Ok(event) = app_event_rx.try_recv() {
        assert!(
            !matches!(event, AppEvent::SubmitThreadOp { .. }),
            "startup typeahead should not approve a newly buffered request: {event:?}"
        );
    }

    Ok(())
}

#[tokio::test]
async fn startup_draft_delayed_approval_becomes_protected_on_redraw() -> Result<()> {
    let (mut app, mut app_event_rx, mut op_rx) = make_test_app_with_channels().await;
    app.startup_protected_input_boundary = true;
    let thread_id = ThreadId::new();
    app.enqueue_primary_thread_session(
        test_thread_session(thread_id, test_path_buf("/tmp/project")),
        Vec::new(),
    )
    .await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server =
        crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref()).await?;

    let (mut startup_pane, _startup_app_event_rx) = startup_bottom_pane();
    startup_pane.set_composer_text("draft".to_string(), Vec::new(), Vec::new());
    let mut draft = startup_pane.composer_draft_snapshot();
    draft.last_composer_activity_at = Some(Instant::now() - Duration::from_millis(/*millis*/ 950));
    let mut pending_startup_draft = Some(draft);
    app.chat_widget
        .restore_startup_draft_when_ready(&mut pending_startup_draft);

    let approval_request =
        exec_approval_request(thread_id, "turn-1", "call-1", /*approval_id*/ None);
    let _ = app
        .pending_app_server_requests
        .note_server_request(&approval_request);
    app.enqueue_primary_thread_request(approval_request).await?;
    let approval_event = app
        .active_thread_rx
        .as_mut()
        .expect("primary thread receiver should be active")
        .try_recv()
        .expect("approval should be queued on the active thread");
    app.handle_thread_event_now(approval_event);
    assert!(!app.chat_widget.has_active_view());
    assert!(app.startup_pending_protected_request);

    app.handle_tui_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
    )
    .await?;
    assert!(app.startup_protected_input_boundary);
    assert!(app.startup_pending_protected_request);

    tokio::time::sleep(Duration::from_millis(/*millis*/ 75)).await;
    let redraw_result = app
        .handle_tui_event(&mut tui, &mut app_server, TuiEvent::Draw)
        .await;

    assert!(app.chat_widget.has_active_view());
    assert!(!tui.terminal.viewport_area.is_empty());
    while let Ok(event) = app_event_rx.try_recv() {
        assert!(
            !matches!(event, AppEvent::SubmitThreadOp { .. }),
            "revealing a delayed startup approval must not submit a decision: {event:?}"
        );
    }
    while let Ok(op) = op_rx.try_recv() {
        assert!(
            !matches!(op, Op::ExecApproval { .. } | Op::UserInputAnswer { .. }),
            "revealing a delayed startup approval must not answer it: {op:?}"
        );
    }
    if let Err(error) = redraw_result {
        tracing::debug!(error = %error, "test terminal cannot quarantine interactive input");
    } else {
        assert!(!app.startup_pending_protected_request);
    }

    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn remotely_resolved_startup_approvals_release_the_draft_after_the_last_request() -> Result<()>
{
    let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
    app.startup_protected_input_boundary = true;
    let thread_id = ThreadId::new();
    app.enqueue_primary_thread_session(
        test_thread_session(thread_id, test_path_buf("/tmp/project")),
        Vec::new(),
    )
    .await?;
    let app_server =
        crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref()).await?;
    app.chat_widget.handle_paste("x".to_string());

    for (request_id, item_id) in [(1, "call-1"), (2, "call-2")] {
        let mut request =
            exec_approval_request(thread_id, "turn-1", item_id, /*approval_id*/ None);
        let ServerRequest::CommandExecutionRequestApproval {
            request_id: approval_request_id,
            ..
        } = &mut request
        else {
            unreachable!("exec approval helper should return an exec approval");
        };
        *approval_request_id = AppServerRequestId::Integer(request_id);
        let _ = app
            .pending_app_server_requests
            .note_server_request(&request);
        app.handle_thread_event_now(ThreadBufferedEvent::Request(Box::new(request)));
    }

    assert!(!app.chat_widget.has_active_view());
    assert!(app.startup_pending_protected_request);
    let (mut startup_pane, _startup_app_event_rx) = startup_bottom_pane();
    startup_pane.set_composer_text("startup draft".to_string(), Vec::new(), Vec::new());
    let mut pending_startup_draft = Some(startup_pane.composer_draft_snapshot());
    app.chat_widget
        .restore_startup_draft_when_ready(&mut pending_startup_draft);
    assert!(pending_startup_draft.is_some());

    for (request_id, still_pending) in [(1, true), (2, false)] {
        app.handle_app_server_event(
            &app_server,
            codex_app_server_client::AppServerEvent::ServerNotification(Box::new(
                ServerNotification::ServerRequestResolved(
                    codex_app_server_protocol::ServerRequestResolvedNotification {
                        thread_id: thread_id.to_string(),
                        request_id: AppServerRequestId::Integer(request_id),
                    },
                ),
            )),
        )
        .await;

        assert_eq!(app.startup_pending_protected_request, still_pending);
        app.chat_widget
            .restore_startup_draft_when_ready(&mut pending_startup_draft);
        assert_eq!(pending_startup_draft.is_some(), still_pending);
    }

    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "x\nstartup draft"
    );
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn later_thread_approval_preserves_input_after_startup_boundary_ends() -> Result<()> {
    let (mut app, mut app_event_rx, mut op_rx) = make_test_app_with_channels().await;
    let thread_id = ThreadId::new();
    app.enqueue_primary_thread_session(
        test_thread_session(thread_id, test_path_buf("/tmp/project")),
        Vec::new(),
    )
    .await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server =
        crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref()).await?;

    assert!(!app.startup_protected_input_boundary);
    app.chat_widget
        .apply_external_edit("keep this draft".to_string());
    let approval_request =
        exec_approval_request(thread_id, "turn-1", "call-1", /*approval_id*/ None);
    let _ = app
        .pending_app_server_requests
        .note_server_request(&approval_request);
    app.enqueue_primary_thread_request(approval_request).await?;
    let approval_event = app
        .active_thread_rx
        .as_mut()
        .expect("primary thread receiver should be active")
        .try_recv()
        .expect("approval should be queued on the active thread");
    assert!(!app.chat_widget.has_active_view());

    app.handle_active_thread_event(&mut tui, &mut app_server, approval_event)
        .await?;

    assert!(app.chat_widget.has_active_view());
    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "keep this draft"
    );
    assert!(tui.terminal.viewport_area.is_empty());
    while let Ok(event) = app_event_rx.try_recv() {
        assert!(
            !matches!(event, AppEvent::SubmitThreadOp { .. }),
            "a later protected request must not consume existing input: {event:?}"
        );
    }
    while let Ok(op) = op_rx.try_recv() {
        assert!(
            !matches!(op, Op::ExecApproval { .. } | Op::UserInputAnswer { .. }),
            "a later protected request must not submit an answer: {op:?}"
        );
    }

    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn auto_declined_mcp_elicitations_do_not_leave_startup_quarantine_armed() {
    for replay in [false, true] {
        for elicitation in [
            McpServerElicitationRequest::Url {
                meta: None,
                message: "Review the payment details to continue.".to_string(),
                url: "http://payments.example/checkout/123".to_string(),
                elicitation_id: "payment-123".to_string(),
            },
            McpServerElicitationRequest::OpenAiForm {
                meta: None,
                message: "Choose a report.".to_string(),
                requested_schema: serde_json::json!({}),
            },
        ] {
            let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
            app.startup_protected_input_boundary = true;
            let thread_id = ThreadId::new();
            let request = ServerRequest::McpServerElicitationRequest {
                request_id: AppServerRequestId::Integer(10),
                params: McpServerElicitationRequestParams {
                    thread_id: thread_id.to_string(),
                    turn_id: Some("turn-auth".to_string()),
                    server_name: "payments".to_string(),
                    request: elicitation,
                },
            };
            let _ = app
                .pending_app_server_requests
                .note_server_request(&request);
            let event = ThreadBufferedEvent::Request(Box::new(request));
            if replay {
                app.handle_thread_event_replay(event);
            } else {
                app.handle_thread_event_now(event);
            }

            assert!(!app.chat_widget.has_active_view());
            assert!(!app.startup_pending_protected_request);
            assert_matches!(
                app_event_rx.try_recv(),
                Ok(AppEvent::SubmitThreadOp {
                    thread_id: op_thread_id,
                    op: Op::ResolveElicitation {
                        server_name,
                        request_id: AppServerRequestId::Integer(10),
                        decision: codex_app_server_protocol::McpServerElicitationAction::Decline,
                        content: None,
                        meta: None,
                    },
                }) if op_thread_id == thread_id && server_name == "payments"
            );
        }
    }
}

#[tokio::test]
async fn startup_draft_handoff_recognizes_late_user_input_as_new_protected_view() -> Result<()> {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let thread_id = ThreadId::new();
    app.enqueue_primary_thread_session(
        test_thread_session(thread_id, test_path_buf("/tmp/project")),
        Vec::new(),
    )
    .await?;

    let (mut startup_pane, _startup_app_event_rx) = startup_bottom_pane();
    startup_pane.handle_key_event(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
    let mut pending_startup_draft = Some(startup_pane.composer_draft_snapshot());
    app.chat_widget
        .restore_startup_draft_when_ready(&mut pending_startup_draft);

    let mut request = request_user_input_request(thread_id, "turn-1", "call-1");
    let ServerRequest::ToolRequestUserInput { params, .. } = &mut request else {
        panic!("expected a user input request");
    };
    params.questions.push(ToolRequestUserInputQuestion {
        id: "choice".to_string(),
        header: "Pick one".to_string(),
        question: "Choose an option.".to_string(),
        is_other: false,
        is_secret: false,
        options: Some(vec![ToolRequestUserInputOption {
            label: "First".to_string(),
            description: "First option".to_string(),
        }]),
    });
    let _ = app
        .pending_app_server_requests
        .note_server_request(&request);
    app.enqueue_primary_thread_request(request).await?;
    let event = app
        .active_thread_rx
        .as_mut()
        .expect("primary thread receiver should be active")
        .try_recv()
        .expect("user input request should be queued on the active thread");

    assert!(!app.chat_widget.has_active_view());
    app.handle_thread_event_now(event);

    assert!(app.chat_widget.has_active_view());
    assert_eq!(app.chat_widget.composer_text_with_pending(), "x");
    while let Ok(event) = app_event_rx.try_recv() {
        assert!(
            !matches!(event, AppEvent::CodexOp(Op::UserInputAnswer { .. })),
            "showing the protected user input request must not submit an answer: {event:?}"
        );
    }

    Ok(())
}

#[tokio::test]
async fn fresh_startup_thread_drains_buffered_approval_before_draft_handoff() -> Result<()> {
    let (mut app, mut app_event_rx, mut op_rx) = make_test_app_with_channels().await;
    app.pending_startup_thread_start = true;
    let thread_id = ThreadId::new();
    let approval_request =
        exec_approval_request(thread_id, "turn-1", "call-1", /*approval_id*/ None);
    let _ = app
        .pending_app_server_requests
        .note_server_request(&approval_request);
    app.enqueue_primary_thread_request(approval_request).await?;

    let (mut startup_pane, _startup_app_event_rx) = startup_bottom_pane();
    startup_pane.set_composer_text("inspect @src".to_string(), Vec::new(), Vec::new());
    let mut pending_startup_draft = Some(startup_pane.composer_draft_snapshot());
    let mut waiting_for_initial_session_configured =
        App::should_wait_for_initial_session(&SessionSelection::StartFresh);
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server =
        crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref()).await?;

    let control = Box::pin(app.handle_event(
        &mut tui,
        &mut app_server,
        AppEvent::StartupThreadStarted {
            result: Ok(AppServerStartedThread {
                session: test_thread_session(thread_id, test_path_buf("/tmp/project")),
                turns: Vec::new(),
                blocks_direct_input: false,
            }),
        },
    ))
    .await?;

    assert!(matches!(control, AppRunControl::Continue));
    assert!(
        app.active_thread_rx
            .as_ref()
            .is_some_and(|receiver| !receiver.is_empty())
    );
    assert!(!app.chat_widget.has_active_view());

    if App::should_stop_waiting_for_initial_session(
        waiting_for_initial_session_configured,
        app.primary_thread_id,
    ) {
        waiting_for_initial_session_configured = false;
        app.drain_active_thread_events(&mut tui).await?;
    }

    assert!(!waiting_for_initial_session_configured);
    assert!(app.chat_widget.has_active_view());
    app.chat_widget
        .restore_startup_draft_when_ready(&mut pending_startup_draft);
    assert_eq!(
        pending_startup_draft
            .as_ref()
            .map(|draft| draft.text.as_str()),
        Some("inspect @src")
    );
    assert!(app.chat_widget.composer_is_empty());
    while let Ok(op) = op_rx.try_recv() {
        assert!(
            !matches!(op, Op::ExecApproval { .. } | Op::UserInputAnswer { .. }),
            "showing a protected startup approval must not answer it: {op:?}"
        );
    }

    while let Ok(event) = app_event_rx.try_recv() {
        assert!(
            !matches!(
                event,
                AppEvent::StartFileSearch(_) | AppEvent::SubmitThreadOp { .. }
            ),
            "fresh startup approval must own input before draft side effects: {event:?}"
        );
    }

    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn queued_startup_app_event_owns_protected_view_before_draft_restore() -> Result<()> {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    while app_event_rx.try_recv().is_ok() {}

    let (mut startup_pane, _startup_app_event_rx) = startup_bottom_pane();
    startup_pane.set_composer_text("inspect @src".to_string(), Vec::new(), Vec::new());
    let mut pending_startup_draft = Some(startup_pane.composer_draft_snapshot());
    app.app_event_tx.send(AppEvent::OpenApprovalsPopup);

    if app_event_rx.is_empty() {
        app.chat_widget
            .restore_startup_draft_when_ready(&mut pending_startup_draft);
    }
    assert!(pending_startup_draft.is_some());
    assert!(app.chat_widget.composer_is_empty());

    let event = app_event_rx
        .try_recv()
        .expect("protected startup app event should be queued");
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut app_server =
        crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref()).await?;
    assert!(!app.chat_widget.has_active_view());
    let control = Box::pin(app.handle_event(&mut tui, &mut app_server, event)).await?;

    assert!(matches!(control, AppRunControl::Continue));
    assert!(app.chat_widget.has_active_view());
    app.chat_widget
        .restore_startup_draft_when_ready(&mut pending_startup_draft);
    assert_eq!(
        pending_startup_draft
            .as_ref()
            .map(|draft| draft.text.as_str()),
        Some("inspect @src")
    );
    assert!(app.chat_widget.composer_is_empty());

    while let Ok(event) = app_event_rx.try_recv() {
        assert!(
            !matches!(
                event,
                AppEvent::StartFileSearch(_)
                    | AppEvent::UpdateWorldWritableWarningAcknowledged(_)
                    | AppEvent::PersistWorldWritableWarningAcknowledged
            ),
            "protected startup app event must own input before draft side effects: {event:?}"
        );
    }

    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn startup_thread_started_submits_queued_startup_input() {
    let (mut app, _app_event_rx, mut op_rx) = make_test_app_with_channels().await;
    app.pending_startup_thread_start = true;
    app.chat_widget
        .set_queue_submissions_until_session_configured(/*queue*/ true);
    app.chat_widget
        .apply_external_edit("queued before startup completes".to_string());
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_eq!(
        app.chat_widget.queued_user_message_texts(),
        vec!["queued before startup completes".to_string()]
    );

    let mut app_server = Box::pin(crate::start_embedded_app_server_for_picker(
        app.chat_widget.config_ref(),
    ))
    .await
    .expect("embedded app server");
    let thread_id = ThreadId::new();
    app.handle_startup_thread_started(
        &mut app_server,
        Ok(AppServerStartedThread {
            session: test_thread_session(thread_id, test_path_buf("/tmp/project")),
            turns: Vec::new(),
            blocks_direct_input: false,
        }),
    )
    .await
    .expect("startup thread should attach");

    match next_user_turn_op(&mut op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "queued before startup completes".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected queued startup input submission, got {other:?}"),
    }
}

#[tokio::test]
async fn startup_thread_started_discards_another_threads_buffered_events() {
    let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
    app.pending_startup_thread_start = true;
    let other_thread_id = ThreadId::new();
    app.enqueue_primary_thread_notification(ServerNotification::McpServerStatusUpdated(
        McpServerStatusUpdatedNotification {
            thread_id: Some(other_thread_id.to_string()),
            name: "other".to_string(),
            status: McpServerStartupState::Starting,
            error: None,
            failure_reason: None,
        },
    ))
    .await
    .expect("foreign notification should be buffered");

    let request = ServerRequest::CommandExecutionRequestApproval {
        request_id: AppServerRequestId::Integer(1),
        params: CommandExecutionRequestApprovalParams {
            thread_id: other_thread_id.to_string(),
            turn_id: "turn-1".to_string(),
            item_id: "item-1".to_string(),
            started_at_ms: 0,
            approval_id: None,
            environment_id: None,
            reason: None,
            network_approval_context: None,
            command: None,
            cwd: None,
            command_actions: None,
            additional_permissions: None,
            proposed_execpolicy_amendment: None,
            proposed_network_policy_amendments: None,
            available_decisions: None,
        },
    };
    app.pending_app_server_requests
        .note_server_request(&request);
    app.enqueue_primary_thread_request(request)
        .await
        .expect("foreign request should be buffered");

    let mut app_server = Box::pin(crate::start_embedded_app_server_for_picker(
        app.chat_widget.config_ref(),
    ))
    .await
    .expect("embedded app server");
    app.handle_app_server_event(
        &app_server,
        codex_app_server_client::AppServerEvent::ServerRequest(Box::new(
            ServerRequest::CurrentTimeRead {
                request_id: AppServerRequestId::Integer(2),
                params: codex_app_server_protocol::CurrentTimeReadParams {
                    thread_id: other_thread_id.to_string(),
                },
            },
        )),
    )
    .await;
    assert!(app.pending_primary_events.iter().any(|event| {
        matches!(event, ThreadBufferedEvent::Request(request)
            if matches!(request.as_ref(), ServerRequest::CurrentTimeRead { .. }))
    }));
    let thread_id = ThreadId::new();
    app.handle_startup_thread_started(
        &mut app_server,
        Ok(AppServerStartedThread {
            session: test_thread_session(thread_id, test_path_buf("/tmp/project")),
            turns: Vec::new(),
            blocks_direct_input: false,
        }),
    )
    .await
    .expect("startup thread should attach");

    assert!(app.pending_primary_events.is_empty());
    assert!(
        app.active_thread_rx
            .as_mut()
            .is_some_and(|rx| rx.try_recv().is_err())
    );
}

#[tokio::test]
async fn startup_thread_started_does_not_replay_resolved_approval() -> Result<()> {
    let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
    app.pending_startup_thread_start = true;
    let app_server =
        crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref()).await?;
    let thread_id = ThreadId::new();
    let request = exec_approval_request(thread_id, "turn-1", "item-1", /*approval_id*/ None);
    let request_id = request.id().clone();

    app.handle_app_server_event(
        &app_server,
        codex_app_server_client::AppServerEvent::ServerRequest(Box::new(request.clone())),
    )
    .await;
    app.handle_app_server_event(
        &app_server,
        codex_app_server_client::AppServerEvent::ServerNotification(Box::new(
            ServerNotification::ServerRequestResolved(
                codex_app_server_protocol::ServerRequestResolvedNotification {
                    thread_id: thread_id.to_string(),
                    request_id,
                },
            ),
        )),
    )
    .await;

    let mut app_server = app_server;
    app.handle_startup_thread_started(
        &mut app_server,
        Ok(AppServerStartedThread {
            session: test_thread_session(thread_id, test_path_buf("/tmp/project")),
            turns: Vec::new(),
            blocks_direct_input: false,
        }),
    )
    .await?;

    assert!(
        !app.pending_app_server_requests
            .contains_server_request(&request)
    );
    Ok(())
}

#[tokio::test]
async fn owned_subagent_approval_before_thread_started_is_preserved() -> Result<()> {
    let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let codex_home = tempdir()?;
    app.config.codex_home = codex_home.path().to_path_buf().abs();
    app.config.sqlite = codex_state::SqliteConfig::new_for_testing(codex_home.path().abs());
    let mut app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;
    let parent = app_server.start_thread(&app.config).await?;
    let parent_thread_id = parent.session.thread_id;
    app.enqueue_primary_thread_session(parent.session, parent.turns)
        .await?;
    let child_thread_id = ThreadId::from_string(
        &create_fake_parented_rollout_with_source(
            codex_home.path(),
            "2026-01-01T00-00-01",
            "2026-01-01T00:00:01Z",
            "child task",
            Some(app.config.model_provider_id.as_str()),
            /*git_info*/ None,
            RolloutSessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id,
                depth: 1,
                agent_path: None,
                agent_nickname: None,
                agent_role: None,
            }),
            parent_thread_id.into(),
            parent_thread_id,
        )
        .expect("create child rollout"),
    )?;
    app_server
        .resume_thread(
            app.config.clone(),
            child_thread_id,
            crate::app_server_session::ResumeModelSettings::RestoreFromThread,
        )
        .await?;
    let request = exec_approval_request(
        child_thread_id,
        "turn-1",
        "item-1",
        /*approval_id*/ None,
    );

    app.handle_app_server_event(
        &app_server,
        codex_app_server_client::AppServerEvent::ServerRequest(Box::new(request.clone())),
    )
    .await;

    assert!(
        app.pending_app_server_requests
            .contains_server_request(&request)
    );
    assert!(app.thread_event_channels.contains_key(&child_thread_id));
    Ok(())
}

#[tokio::test]
async fn startup_thread_start_failure_returns_error() {
    let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
    app.pending_startup_thread_start = true;

    let mut app_server = Box::pin(crate::start_embedded_app_server_for_picker(
        app.chat_widget.config_ref(),
    ))
    .await
    .expect("embedded app server");
    let err = app
        .handle_startup_thread_started(&mut app_server, Err("boom".to_string()))
        .await
        .expect_err("startup thread failure should exit instead of leaving chat unconfigured");

    assert!(
        err.to_string()
            .contains("Failed to start a fresh session through the app server: boom")
    );
    assert!(!app.pending_startup_thread_start);
    assert_eq!(app.primary_thread_id, None);
}

#[test]
fn stale_startup_thread_started_removes_local_routing_state() -> Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .thread_stack_size(8 * 1024 * 1024)
        .enable_all()
        .build()?
        .block_on(async {
            let mut app = make_test_app().await;
            let mut app_server =
                crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref()).await?;
            let primary_thread_id = ThreadId::new();
            let stale_thread_id = ThreadId::new();
            app.primary_thread_id = Some(primary_thread_id);
            app.thread_event_channels.insert(
                primary_thread_id,
                ThreadEventChannel::new(THREAD_EVENT_CHANNEL_CAPACITY),
            );
            app.activate_thread_channel(primary_thread_id).await;
            app.thread_event_channels.insert(
                stale_thread_id,
                ThreadEventChannel::new(THREAD_EVENT_CHANNEL_CAPACITY),
            );
            app.agent_navigation.upsert(
                stale_thread_id,
                /*agent_nickname*/ None,
                /*agent_role*/ None,
                /*is_closed*/ false,
            );
            assert!(app.thread_event_channels.contains_key(&stale_thread_id));
            assert!(app.agent_navigation.get(&stale_thread_id).is_some());

            app.handle_startup_thread_started(
                &mut app_server,
                Ok(AppServerStartedThread {
                    session: test_thread_session(stale_thread_id, test_path_buf("/tmp/project")),
                    turns: Vec::new(),
                    blocks_direct_input: false,
                }),
            )
            .await?;

            assert!(!app.thread_event_channels.contains_key(&stale_thread_id));
            assert_eq!(app.agent_navigation.get(&stale_thread_id), None);
            assert_eq!(app.active_thread_id, Some(primary_thread_id));
            Ok(())
        })
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
        history_mode: None,
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
        history_mode: None,
    });

    assert!(!ignored);
    assert!(app.transcript_cells.is_empty());
}
