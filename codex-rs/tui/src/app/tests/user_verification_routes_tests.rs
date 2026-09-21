use super::*;
use pretty_assertions::assert_eq;

fn user_verification_server_request(thread_id: ThreadId, request_id: i64) -> ServerRequest {
    ServerRequest::McpServerElicitationRequest {
        request_id: AppServerRequestId::Integer(request_id),
        params: McpServerElicitationRequestParams {
            thread_id: thread_id.to_string(),
            turn_id: Some("turn-verification".to_string()),
            server_name: "deployments".to_string(),
            request: McpServerElicitationRequest::UserVerification {
                meta: None,
                title: "Approve deployment?".to_string(),
                description: "Verify the production deployment.".to_string(),
                challenge: "AQID".to_string(),
            },
        },
    }
}

async fn next_submitted_thread_op(
    app_event_rx: &mut tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
) -> (ThreadId, Op) {
    time::timeout(std::time::Duration::from_secs(/*secs*/ 1), async {
        loop {
            match app_event_rx.recv().await {
                Some(AppEvent::SubmitThreadOp { thread_id, op }) => break (thread_id, op),
                Some(_) => {}
                None => panic!("app event channel closed before submitting an operation"),
            }
        }
    })
    .await
    .expect("user-verification operation should be submitted promptly")
}

#[tokio::test]
async fn inactive_thread_user_verification_replays_queued_request_once() -> Result<()> {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let primary_thread_id = ThreadId::new();
    let side_thread_id = ThreadId::new();
    let thread_id = ThreadId::new();
    let request = user_verification_server_request(thread_id, /*request_id*/ 11);

    app.primary_thread_id = Some(primary_thread_id);
    app.active_thread_id = Some(side_thread_id);
    app.side_threads
        .insert(side_thread_id, SideThreadState::new(primary_thread_id));
    app.agent_navigation.upsert(
        thread_id,
        Some("Verifier".to_string()),
        Some("worker".to_string()),
        /*is_closed*/ false,
    );
    app.enqueue_thread_request(thread_id, request.clone())
        .await?;

    assert!(!app.chat_widget.has_active_view());
    assert_eq!(
        app.pending_inactive_thread_requests().await,
        vec![(thread_id, request)]
    );

    app.side_threads.remove(&side_thread_id);
    app.active_thread_id = Some(primary_thread_id);
    app.surface_pending_inactive_thread_interactive_requests()
        .await?;
    app.surface_pending_inactive_thread_interactive_requests()
        .await?;
    insta::assert_snapshot!(
        "inactive_thread_user_verification_prompt",
        render_bottom_popup(&app.chat_widget, /*width*/ 80)
    );
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

    assert_eq!(
        next_submitted_thread_op(&mut app_event_rx).await,
        (
            thread_id,
            Op::ResolveUserVerification {
                server_name: "deployments".to_string(),
                request_id: AppServerRequestId::Integer(11),
                response: crate::app_command::UserVerificationResponse::Cancel,
            }
        )
    );
    assert!(!app.chat_widget.has_active_view());
    assert!(
        !std::iter::from_fn(|| app_event_rx.try_recv().ok())
            .any(|event| matches!(event, AppEvent::SubmitThreadOp { .. }))
    );
    Ok(())
}

#[tokio::test]
async fn side_toggle_surfaces_pending_verification_from_an_inactive_thread() -> Result<()> {
    Box::pin(async {
        let (mut app, _events, _ops) = make_test_app_with_channels().await;
        let mut app_server =
            crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref()).await?;
        let primary = app_server.start_thread(&app.config).await?;
        let primary_thread_id = primary.session.thread_id;
        app.enqueue_primary_thread_session(primary.session, primary.turns)
            .await?;

        let side_thread_id = ThreadId::new();
        app.side_threads
            .insert(side_thread_id, SideThreadState::new(primary_thread_id));
        app.thread_event_channels.insert(
            side_thread_id,
            ThreadEventChannel::new_with_session(
                /*capacity*/ 4,
                test_thread_session(side_thread_id, test_path_buf("/tmp/side")),
                Vec::new(),
            ),
        );
        let mut tui = crate::tui::test_support::make_test_tui()?;
        app.select_agent_thread(&mut tui, &mut app_server, side_thread_id)
            .await?;
        assert_eq!(app.active_thread_id, Some(side_thread_id));

        let agent_thread_id = ThreadId::new();
        app.agent_navigation.upsert(
            agent_thread_id,
            Some("Verifier".to_string()),
            Some("worker".to_string()),
            /*is_closed*/ false,
        );
        app.enqueue_thread_request(
            agent_thread_id,
            user_verification_server_request(agent_thread_id, /*request_id*/ 17),
        )
        .await?;
        assert!(!app.chat_widget.has_active_view());

        app.toggle_side_conversation(&mut tui, &mut app_server)
            .await?;
        assert_eq!(app.active_thread_id, Some(primary_thread_id));
        insta::assert_snapshot!(
            "side_toggle_surfaces_pending_user_verification",
            render_bottom_popup(&app.chat_widget, /*width*/ 80)
        );
        app_server.shutdown().await?;
        Ok(())
    })
    .await
}

#[tokio::test]
async fn discarding_side_cancels_verification_and_ignores_late_proof() -> Result<()> {
    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    let mut app_server =
        crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref()).await?;
    let primary_thread_id = ThreadId::new();
    let side_thread_id = ThreadId::new();
    let request = user_verification_server_request(side_thread_id, /*request_id*/ 19);
    app.active_thread_id = Some(primary_thread_id);
    app.side_threads
        .insert(side_thread_id, SideThreadState::new(primary_thread_id));
    app.thread_event_channels
        .insert(side_thread_id, ThreadEventChannel::new(/*capacity*/ 4));
    app.pending_app_server_requests
        .note_server_request(&request);
    let attempt = app
        .pending_app_server_requests
        .user_verification
        .begin("deployments", request.id())
        .expect("pending side-thread verification");

    app.discard_side_thread_in_background(&mut app_server, side_thread_id)
        .await;
    assert!(attempt.cancelled.is_cancelled());
    assert!(
        !app.pending_app_server_requests
            .contains_server_request(&request)
    );

    app.finish_user_verification(
        &mut app_server,
        side_thread_id,
        "deployments".to_string(),
        request.id().clone(),
        attempt.id,
        Ok(codex_app_server_protocol::UserVerificationProof {
            credential_id: "discarded-credential".to_string(),
            signature: "discarded-signature".to_string(),
        }),
    )
    .await?;
    assert!(
        !std::iter::from_fn(|| events.try_recv().ok())
            .any(|event| matches!(event, AppEvent::SubmitThreadOp { .. }))
    );
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn inactive_thread_user_verification_preserves_foreground_stream() -> Result<()> {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let foreground_thread_id = ThreadId::new();
    let thread_id = ThreadId::new();
    app.primary_thread_id = Some(foreground_thread_id);
    app.active_thread_id = Some(foreground_thread_id);
    app.chat_widget.handle_thread_session(test_thread_session(
        foreground_thread_id,
        test_path_buf("/tmp/project"),
    ));
    while app_event_rx.try_recv().is_ok() {}
    app.chat_widget.handle_server_notification(
        agent_message_delta_notification(
            foreground_thread_id,
            "turn-foreground",
            "message-foreground",
            "The foreground answer",
        ),
        /*replay_kind*/ None,
    );
    assert!(app.chat_widget.has_active_agent_stream());

    app.enqueue_thread_request(
        thread_id,
        user_verification_server_request(thread_id, /*request_id*/ 13),
    )
    .await?;
    assert!(app.chat_widget.has_active_view());
    assert!(app.chat_widget.has_active_agent_stream());
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    app.chat_widget.handle_server_notification(
        agent_message_delta_notification(
            foreground_thread_id,
            "turn-foreground",
            "message-foreground",
            " continues.",
        ),
        /*replay_kind*/ None,
    );
    app.chat_widget.handle_server_notification(
        ServerNotification::ItemCompleted(codex_app_server_protocol::ItemCompletedNotification {
            thread_id: foreground_thread_id.to_string(),
            turn_id: "turn-foreground".to_string(),
            completed_at_ms: 0,
            item: ThreadItem::AgentMessage {
                id: "message-foreground".to_string(),
                text: "The foreground answer continues.".to_string(),
                phase: None,
                memory_citation: None,
                delivery: None,
                questions: None,
            },
        }),
        /*replay_kind*/ None,
    );

    let mut approved = Vec::new();
    let mut completed_messages = Vec::new();
    let mut tui = crate::tui::test_support::make_test_tui()?;
    while let Ok(event) = app_event_rx.try_recv() {
        match event {
            AppEvent::UserVerificationApproved {
                thread_id,
                server_name,
                request_id,
            } => approved.push((thread_id, server_name, request_id)),
            AppEvent::InsertHistoryCell(cell) => app.insert_history_cell(&mut tui, cell),
            AppEvent::ConsolidateAgentMessage {
                source,
                cwd,
                inline_visualization_context,
                scrollback_reflow,
                deferred_history_cell,
            } => {
                completed_messages.push(source.clone());
                app.handle_consolidate_agent_message(
                    &mut tui,
                    source,
                    cwd,
                    inline_visualization_context,
                    scrollback_reflow,
                    deferred_history_cell,
                )?;
                app.chat_widget.note_stream_consolidation_completed();
            }
            _ => {}
        }
    }
    assert_eq!(
        approved,
        vec![(
            thread_id,
            "deployments".to_string(),
            AppServerRequestId::Integer(13)
        )]
    );
    assert_eq!(completed_messages, vec!["The foreground answer continues."]);
    assert!(!app.chat_widget.has_active_agent_stream());
    insta::assert_snapshot!(
        "inactive_thread_user_verification_foreground_history",
        app.render_transcript_lines_for_reflow(/*width*/ 80)
            .lines
            .iter()
            .map(rendered_line_text)
            .collect::<Vec<_>>()
            .join("\n")
    );
    Ok(())
}

#[tokio::test]
async fn active_thread_user_verification_cancels_through_original_request() {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let thread_id = ThreadId::new();
    let request = user_verification_server_request(thread_id, /*request_id*/ 12);

    let ServerRequest::McpServerElicitationRequest { request_id, params } = request else {
        panic!("elicitation request")
    };
    app.chat_widget
        .handle_elicitation_request_now(request_id, params);
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

    let (submitted_thread_id, op) = next_submitted_thread_op(&mut app_event_rx).await;
    assert_eq!(submitted_thread_id, thread_id);
    assert_eq!(
        op,
        Op::ResolveUserVerification {
            server_name: "deployments".to_string(),
            request_id: AppServerRequestId::Integer(12),
            response: crate::app_command::UserVerificationResponse::Cancel,
        }
    );
}
