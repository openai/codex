//! Active recovery keeps local input and policy while reusing ordinary thread resume.

use super::*;
use crate::app::reconnect::ReconnectPresentation;
use crate::app::reconnect::reconnect;
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio::net::TcpListener;

use super::disconnect::serve_reconnect_requests;

#[tokio::test]
async fn reconnect_restores_history_permissions_and_keeps_old_input_paused() -> Result<()> {
    for (recovered_queue, edit_offline, resume_error_code, deferred_notice, notice_enabled) in [
        (true, false, -32603, false, false),
        (true, false, -32603, false, true),
        (false, false, -32603, false, false),
        (true, true, -32603, false, false),
        (true, false, -32600, false, false),
        (false, false, -32600, false, false),
        (true, true, -32600, false, false),
        (true, false, -32603, true, true),
    ] {
        let pending_profile = !recovered_queue && resume_error_code == -32600;
        let (mut app, mut events, mut ops) = make_test_app_with_channels().await;
        app.local_settings.tui.show_server_version_notice = notice_enabled;
        let id = ThreadId::new();
        let cwd = app.config.cwd.clone();
        app.config.model = Some("gpt-test".into());
        // Avoid platform-specific path widths in the mode-preservation snapshot.
        app.local_settings.tui.status_line = Some(vec!["model-with-reasoning".into()]);
        app.config
            .permissions
            .set_permission_profile(PermissionProfile::read_only())?;
        app.runtime_approval_policy_override = Some(RuntimeApprovalPolicyOverride::Explicit(
            AskForApproval::OnRequest,
        ));
        app.runtime_permission_profile_override =
            Some(RuntimePermissionProfileOverride::from_config(&app.config));
        app.active_thread_id = Some(id);
        app.primary_thread_id = Some(id);
        let mut thread_session = test_thread_session(id, cwd.to_path_buf());
        thread_session.approval_policy = AskForApproval::OnRequest;
        thread_session.permission_profile = PermissionProfile::read_only();
        app.ensure_thread_channel(id)
            .store
            .lock()
            .await
            .set_session(thread_session.clone(), Vec::new());
        app.chat_widget.handle_thread_session(thread_session);
        app.chat_widget.set_collaboration_mask(
            crate::collaboration_modes::plan_mask(app.model_catalog.as_ref()).unwrap(),
        );
        let expected_mode = app.chat_widget.effective_collaboration_mode().with_updates(
            Some("gpt-test".into()),
            Some(None),
            /*developer_instructions*/ None,
        );
        let expected_submitted_mode = expected_mode.clone();
        assert!(!app.model_catalog.collaboration_modes.is_empty());
        if edit_offline {
            app.chat_widget
                .restore_user_message_to_composer("unacknowledged prompt".into());
            app.chat_widget
                .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
            next_user_turn_op(&mut ops);
        }
        if recovered_queue {
            app.chat_widget
                .set_queue_autosend_suppressed(/*suppressed*/ true);
            app.chat_widget
                .restore_user_message_to_composer("old queued input".into());
            app.chat_widget
                .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        }
        app.chat_widget
            .restore_user_message_to_composer("kept draft".into());
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = crate::resolve_remote_addr(&format!("ws://{}", listener.local_addr()?))?;
        app.app_server_target = AppServerTarget::Remote { endpoint };
        if notice_enabled {
            let (notice, key) = crate::status::remote_connection::pending_server_version_notice(
                &app.local_settings.tui,
                &app.app_server_target,
                /*server_home*/ None,
                "2.1.0",
                Some("2.0.0"),
                /*last_shown*/ None,
            )
            .expect("older server should have a pending notice");
            app.reconnect.seen_version_notice = Some(key);
            if deferred_notice {
                app.pending_server_version_notice = Some(notice);
                app.update_server_version_overview_notice("2.1.0", Some("2.0.0"));
            }
        }
        let thread = json!({
            "id": id, "sessionId": id, "preview": "only once", "ephemeral": false,
            "modelProvider": "test-provider", "createdAt": 1, "updatedAt": 2,
            "status": {"type": "idle"}, "cwd": cwd, "cliVersion": "0.0.0", "source": "cli",
            "turns": [test_turn("accepted", TurnStatus::Completed, vec![ThreadItem::UserMessage {
                id: "input-1".into(), client_id: None, content: vec![UserInput::Text { text: "only once".into(), text_elements: Vec::new() }],
            }])]
        });
        let server = tokio::spawn(async move {
            let mut methods = Vec::new();
            for attempt in 0..2 {
                let (stream, _) = listener.accept().await?;
                methods.extend(serve_reconnect_requests(tokio_tungstenite::accept_async(stream).await?, |request| std::future::ready(match request.method.as_str() {
                    "thread/resume" if attempt == 0 => Some(json!({"error": {"code": resume_error_code, "message":
                        if resume_error_code == -32600 {
                            format!("thread {id} is closing; retry thread/resume after the thread is closed")
                        } else {
                            "temporarily unavailable".into()
                        }
                    }})),
                    "thread/resume" => {
                        let params = request.params.as_ref().unwrap();
                        assert_eq!(params["threadId"], id.to_string());
                        assert!(params["model"].is_null());
                        Some(json!({"result": {"thread": thread, "model": "gpt-test", "modelProvider": "test-provider", "cwd": cwd,
                            "approvalPolicy": "never", "approvalsReviewer": "user", "sandbox": {"type": "dangerFullAccess"}, "reasoningEffort": null}}))
                    }
                    "thread/read" => Some(json!({"result": {"thread": thread}})),
                    "thread/list" | "thread/loaded/list" => Some(json!({"result": {"data": [], "nextCursor": null}})),
                    "thread/goal/get" => Some(json!({"result": {"goal": null}})),
                    "turn/start" => {
                        assert!(!recovered_queue);
                        let params = request.params.as_ref().unwrap();
                        assert_eq!(params["input"][0]["text"], "fresh follow-up");
                        assert_eq!(params["approvalPolicy"], if pending_profile { "never" } else { "on-request" });
                        assert_eq!(params["sandboxPolicy"]["type"], if pending_profile { json!(null) } else { json!("readOnly") });
                        assert_eq!(params["collaborationMode"], json!(expected_submitted_mode));
                        Some(json!({"result": {"turn": {"id": "fresh", "items": [], "status": "inProgress"}}}))
                    }
                    method => panic!("unexpected reconnect request: {method}"),
                })).await?);
            }
            Ok::<_, color_eyre::Report>(methods)
        });
        let mut session = crate::start_embedded_app_server_for_picker(&app.config).await?;
        std::fs::write(
            app.config
                .codex_home
                .join("tui-thread-reference-capabilities"),
            "not a directory",
        )?;
        session.remember_task_tool_thread(id);
        let transport = crate::dynamic_tools_mcp::ThreadToolTransport::Mcp(Arc::new(
            crate::dynamic_tools_mcp::DynamicToolMcpServer::start(
                session.request_handle(),
                codex_app_server_protocol::ThreadStartParams::default(),
                app.app_event_tx.clone(),
                app.dynamic_tool_status_updates.clone(),
                /*managed_requirement*/ None,
            )
            .await?,
        ));
        let mut overrides = None;
        transport.configure_mcp(&mut overrides);
        let mcp = overrides.unwrap().remove("mcp_servers.codex_tui").unwrap();
        let client = codex_http_client::HttpClientBuilder::new().build_direct()?;
        let call_mcp = || {
            client
                .post(mcp["url"].as_str().unwrap())
                .header(
                    "Authorization",
                    mcp["http_headers"]["Authorization"].as_str().unwrap(),
                )
                .header("Accept", "application/json, text/event-stream")
                .header("MCP-Protocol-Version", "2026-07-28")
                .header("MCP-Method", "tools/call")
                .header("MCP-Name", "list_threads")
                .json(
                    &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {
                        "name": "list_threads", "arguments": {}, "_meta": {"threadId": id,
                        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                        "io.modelcontextprotocol/clientCapabilities": {}}
                    }}),
                )
        };

        app.chat_widget.remote_connection =
            crate::status::remote_connection::remote_connection_status_value(
                &app.app_server_target,
                Some("1.0.0"),
            );
        let mut tui = crate::tui::test_support::make_test_tui()?;
        if pending_profile {
            app.pending_server_profiles.insert(
                id,
                PermissionProfileSelection {
                    profile_id: "server-only".into(),
                    approval_policy: None,
                    approvals_reviewer: None,
                    display_label: "server-only".into(),
                },
            );
        }
        app.begin_reconnect();
        if deferred_notice {
            assert_eq!(
                app.agents_overview
                    .view_state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .server_version_notice,
                None
            );
        }
        if edit_offline {
            app.handle_tui_event(
                &mut tui,
                &mut session,
                TuiEvent::Key(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT)),
            )
            .await?;
            assert!(!app.chat_widget.has_queued_follow_up_messages());
            assert!(
                !app.chat_widget
                    .capture_thread_input_state()
                    .unwrap()
                    .recovered_queue
            );
        }
        let stale_picker_refresh = app.agent_navigation.begin_picker_refresh(id).unwrap();
        app.last_subagent_backfill_attempt = Some(id);
        app.rate_limit_refresh_state
            .start(
                RateLimitRefreshOrigin::Recovery,
                &mut app.rate_limit_hard_stop_generation,
            )
            .unwrap();
        let before_disconnect = Instant::now();
        app.recap.note_focus_lost(before_disconnect);
        for _ in 0..3 {
            app.recap
                .note_turn_finished(&TurnStatus::Completed, before_disconnect);
        }
        app.schedule_recap_check(id, Instant::now());
        let old_sender = app.app_event_tx.clone();
        let connected = reconnect(
            app.app_server_target.clone(),
            app.config.clone(),
            app.local_settings.clone(),
            Some(id),
            /*remote_cwd*/ None,
            transport,
            ReconnectPresentation::Conversation,
        )
        .await?;
        let paused = call_mcp().send().await?.text().await?;
        assert!(
            paused.contains("TUI is reconnecting; tool was not sent"),
            "{paused}"
        );
        app.finish_reconnect(&mut tui, &mut session, &mut events, connected, "2.1.0")
            .await?;
        assert!(app.pending_server_profiles.is_empty());
        assert!(!app.reconnect.offline);
        assert!(!app.thread_unavailable(id));
        assert_eq!(app.last_subagent_backfill_attempt, None);
        assert!(
            app.rate_limit_refresh_state
                .start(
                    RateLimitRefreshOrigin::Recovery,
                    &mut app.rate_limit_hard_stop_generation
                )
                .is_some()
        );
        assert!(app.agent_navigation.begin_picker_refresh(id).is_some());
        assert!(
            !app.agent_navigation
                .finish_picker_refresh(id, stale_picker_refresh)
        );
        // Let the rebound timer become due without depending on machine uptime.
        tokio::time::pause();
        tokio::time::advance(recap::RECAP_DELAY).await;
        tokio::time::resume();
        let mut deferred = Vec::new();
        tokio::time::timeout(Duration::from_secs(/*secs*/ 5), async {
            loop {
                let event = events.recv().await.expect("rebound recap event");
                if matches!(event, AppEvent::CheckRecap { thread_id } if thread_id == id) {
                    break;
                }
                deferred.push(event);
            }
        })
        .await?;
        for event in deferred {
            app.app_event_tx.send(event);
        }
        let ready = call_mcp().send().await?.text().await?;
        assert!(ready.contains("threads"), "{ready}");
        assert_eq!(
            app.chat_widget.remote_connection,
            crate::status::remote_connection::remote_connection_status_value(
                &app.app_server_target,
                Some("2.0.0")
            )
        );
        assert!(old_sender.app_event_tx.is_closed());
        assert!(session.task_tools_available(id));
        assert_eq!(app.current_displayed_thread_id(), Some(id));
        assert_eq!(
            app.chat_widget.effective_collaboration_mode(),
            expected_mode
        );
        assert!(app.model_catalog.collaboration_modes.is_empty());
        assert!(
            app.chat_widget
                .model_catalog()
                .collaboration_modes
                .is_empty()
        );
        assert_eq!(
            app.chat_widget.composer_text_with_pending(),
            if edit_offline {
                "old queued input"
            } else {
                "kept draft"
            }
        );
        let history = drain_history(&mut app, &mut tui, &mut session, &mut events).await?;
        let notices = history
            .lines()
            .filter(|line| {
                line.contains("Reconnected.") || line.contains("background Codex service")
            })
            .collect::<Vec<_>>()
            .join("\n");
        if deferred_notice {
            assert_snapshot!(notices, @r###"
• Reconnected. No input was resent. Review uncertain submissions before retrying; recovered queues remain paused.
⚠ A background Codex service is running v2.0.0, older than your Codex CLI
"###);
        } else {
            insta::allow_duplicates! {
                assert_snapshot!(notices, @"• Reconnected. No input was resent. Review uncertain submissions before retrying; recovered queues remain paused.");
            }
        }
        assert_eq!(history.matches("only once").count(), 1);
        if recovered_queue {
            assert!(app.chat_widget.has_queued_follow_up_messages());
            assert!(!app.chat_widget.maybe_send_next_queued_input());
            if edit_offline {
                app.handle_tui_event(
                    &mut tui,
                    &mut session,
                    TuiEvent::Key(KeyEvent::new(KeyCode::Up, KeyModifiers::ALT)),
                )
                .await?;
                assert_eq!(
                    app.chat_widget.composer_text_with_pending(),
                    "unacknowledged prompt"
                );
                assert!(ops.try_recv().is_err());
            } else {
                assert_snapshot!(
                    "restored_conversation",
                    render_bottom_popup(&app.chat_widget, /*width*/ 80)
                );
            }
        } else {
            app.chat_widget
                .handle_key_event(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
            app.chat_widget
                .restore_user_message_to_composer("fresh follow-up".into());
            app.chat_widget
                .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
            while let Ok(event) = events.try_recv() {
                app.handle_event(&mut tui, &mut session, event).await?;
            }
        }
        session.shutdown().await?;
        let methods = server.await??;
        assert_eq!(
            methods
                .iter()
                .filter(|method| *method == "thread/resume")
                .count(),
            2
        );
        assert_eq!(
            methods
                .iter()
                .filter(|method| *method == "turn/start")
                .count(),
            usize::from(!recovered_queue)
        );
    }
    Ok(())
}

#[tokio::test]
async fn reconnect_reconciles_offscreen_pending_profile_before_restoring_permissions() -> Result<()>
{
    let (mut app, mut events, _) = make_test_app_with_channels().await;
    let primary = ThreadId::new();
    let displayed = ThreadId::new();
    let cwd = app.config.cwd.to_path_buf();
    app.config
        .permissions
        .set_permission_profile(PermissionProfile::Disabled)?;
    app.runtime_permission_profile_override =
        Some(RuntimePermissionProfileOverride::from_config(&app.config));
    app.primary_thread_id = Some(primary);
    app.active_thread_id = Some(displayed);
    for id in [primary, displayed] {
        let mut session = test_thread_session(id, cwd.clone());
        session.permission_profile = PermissionProfile::Disabled;
        app.ensure_thread_channel(id)
            .store
            .lock()
            .await
            .set_session(session.clone(), Vec::new());
        if id == primary {
            app.primary_session_configured = Some(session);
            app.upsert_agent_picker_thread(
                id, /*agent_nickname*/ None, /*agent_role*/ None,
                /*is_closed*/ false,
            );
        } else {
            app.chat_widget.handle_thread_session(session);
        }
    }
    app.pending_server_profiles.insert(
        primary,
        PermissionProfileSelection {
            profile_id: "server-only".into(),
            approval_policy: None,
            approvals_reviewer: None,
            display_label: "server-only".into(),
        },
    );

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    app.app_server_target = AppServerTarget::Remote {
        endpoint: crate::resolve_remote_addr(&format!("ws://{}", listener.local_addr()?))?,
    };
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await?;
        serve_reconnect_requests(tokio_tungstenite::accept_async(stream).await?, |request| {
            let id = request.params.as_ref().and_then(|params| params["threadId"].as_str());
            let thread = |id: ThreadId| json!({
                "id": id, "sessionId": id, "preview": "task", "ephemeral": false,
                "modelProvider": "test-provider", "createdAt": 1, "updatedAt": 2,
                "status": {"type": "idle"}, "cwd": cwd, "cliVersion": "0.0.0",
                "source": "cli", "turns": []
            });
            std::future::ready(Some(match request.method.as_str() {
                "thread/resume" => {
                    let pending = id == Some(primary.to_string().as_str());
                    json!({"result": {"thread": thread(if pending { primary } else { displayed }),
                        "model": "gpt-test", "modelProvider": "test-provider", "cwd": cwd,
                        "approvalPolicy": if pending { "on-request" } else { "never" },
                        "approvalsReviewer": "user",
                        "sandbox": {"type": if pending { "readOnly" } else { "dangerFullAccess" }},
                        "activePermissionProfile": if pending { Some(json!({"id": "server-only"})) } else { None },
                        "reasoningEffort": null}})
                }
                "thread/read" => json!({"result": {"thread": thread(primary)}}),
                "turn/start" => {
                    let params = request.params.as_ref().unwrap();
                    assert_eq!(params["permissions"], "server-only");
                    assert_eq!(params["approvalPolicy"], "on-request");
                    assert_eq!(params["sandboxPolicy"], json!(null));
                    json!({"result": {"turn": {"id": "fresh", "items": [], "status": "inProgress"}}})
                }
                "thread/list" | "thread/loaded/list" =>
                    json!({"result": {"data": [], "nextCursor": null}}),
                "thread/goal/get" => json!({"result": {"goal": null}}),
                method => panic!("unexpected reconnect request: {method}"),
            }))
        })
        .await
    });
    let mut session = crate::start_embedded_app_server_for_picker(&app.config).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.begin_reconnect();
    let connected = reconnect(
        app.app_server_target.clone(),
        app.config.clone(),
        app.local_settings.clone(),
        Some(displayed),
        /*remote_cwd*/ None,
        session.thread_tool_transport(),
        ReconnectPresentation::Conversation,
    )
    .await?;
    app.finish_reconnect(
        &mut tui,
        &mut session,
        &mut events,
        connected,
        CODEX_CLI_VERSION,
    )
    .await?;
    assert!(app.pending_server_profiles.contains_key(&primary));

    app.select_agent_thread(&mut tui, &mut session, primary)
        .await?;
    assert!(!app.thread_unavailable(primary));
    assert!(!app.pending_server_profiles.contains_key(&primary));
    assert_eq!(
        app.chat_widget
            .config_ref()
            .permissions
            .permission_profile(),
        &PermissionProfile::read_only()
    );
    assert_eq!(
        app.chat_widget
            .config_ref()
            .permissions
            .active_permission_profile()
            .as_ref()
            .map(|profile| profile.id.as_str()),
        Some("server-only")
    );
    app.chat_widget
        .restore_user_message_to_composer("safe follow-up".into());
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    while let Ok(event) = events.try_recv() {
        if matches!(event, AppEvent::CodexOp(AppCommand::UserTurn { .. })) {
            app.handle_event(&mut tui, &mut session, event).await?;
        }
    }
    session.shutdown().await?;
    let methods = server.await??;
    assert_eq!(
        methods
            .iter()
            .filter(|method| *method == "turn/start")
            .count(),
        1
    );
    Ok(())
}

#[tokio::test]
async fn reconnect_exhaustion_and_unknown_initial_thread_stay_offline() -> Result<()> {
    let (mut app, _, _) = make_test_app_with_channels().await;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    app.app_server_target = AppServerTarget::Remote {
        endpoint: crate::resolve_remote_addr(&format!("ws://{}", listener.local_addr()?))?,
    };
    drop(listener);
    tokio::time::pause();
    let start = tokio::time::Instant::now();
    for id in [Some(ThreadId::new()), None] {
        assert!(
            reconnect(
                app.app_server_target.clone(),
                app.config.clone(),
                app.local_settings.clone(),
                id,
                /*remote_cwd*/ None,
                crate::dynamic_tools_mcp::ThreadToolTransport::Dynamic,
                ReconnectPresentation::Conversation
            )
            .await
            .is_err()
        );
    }
    assert!((15..=65).contains(&start.elapsed().as_secs()));
    app.begin_reconnect();
    app.chat_widget.reconnect_failed();
    assert_snapshot!(
        "reconnect_failed",
        render_bottom_popup(&app.chat_widget, /*width*/ 80)
    );
    tokio::time::resume();
    Ok(())
}

#[tokio::test]
async fn reconnect_allows_slow_hydration_but_bounds_a_stalled_server() -> Result<()> {
    for delay in [15, 150] {
        let (mut app, mut events, _) = make_test_app_with_channels().await;
        app.config.model = Some("gpt-test".into());
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = crate::RemoteAppServerEndpoint::WebSocket {
            websocket_url: format!("ws://{}", listener.local_addr()?),
            auth_token: None,
        };
        let id = ThreadId::new();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await?;
            serve_reconnect_requests(
                tokio_tungstenite::accept_async(stream).await?,
                move |request| async move {
                    assert_eq!(request.method, "thread/resume");
                    tokio::time::pause();
                    tokio::time::sleep(Duration::from_secs(delay)).await;
                    tokio::time::resume();
                    Some(json!({"error": {"code": -32600, "message": "thread no longer exists"}}))
                },
            )
            .await
        });
        let start = tokio::time::Instant::now();
        let result = reconnect(
            AppServerTarget::Remote {
                endpoint: endpoint.clone(),
            },
            app.config.clone(),
            app.local_settings.clone(),
            Some(id),
            /*remote_cwd*/ None,
            crate::dynamic_tools_mcp::ThreadToolTransport::Dynamic,
            ReconnectPresentation::Conversation,
        )
        .await;
        let elapsed = start.elapsed().as_secs();
        if delay == 15 {
            app.app_server_target = AppServerTarget::Remote { endpoint };
            app.active_thread_id = Some(id);
            app.primary_thread_id = Some(id);
            let cached = test_thread_session(id, app.config.cwd.to_path_buf());
            app.ensure_thread_channel(id)
                .store
                .lock()
                .await
                .set_session(cached, Vec::new());
            app.chat_widget
                .restore_user_message_to_composer("unavailable draft".into());
            app.begin_reconnect();
            let mut session = crate::start_embedded_app_server_for_picker(&app.config).await?;
            let mut tui = crate::tui::test_support::make_test_tui()?;
            app.finish_reconnect(
                &mut tui,
                &mut session,
                &mut events,
                result?,
                CODEX_CLI_VERSION,
            )
            .await?;
            assert!(app.thread_unavailable(id));
            app.handle_tui_event(
                &mut tui,
                &mut session,
                TuiEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            )
            .await?;
            assert_eq!(
                app.chat_widget.composer_text_with_pending(),
                "unavailable draft"
            );
            let history = drain_history(&mut app, &mut tui, &mut session, &mut events).await?;
            let history = &history[history.find("• This conversation is unavailable").unwrap()..];
            assert_snapshot!(
                "unavailable_conversation",
                format!(
                    "{history}\n{}",
                    render_bottom_popup(&app.chat_widget, /*width*/ 80)
                )
                .replace(
                    &test_path_buf("/tmp/project").display().to_string(),
                    "/tmp/project"
                )
            );
            session.shutdown().await?;
            assert!((15..120).contains(&elapsed), "{elapsed}");
            let methods = server.await??;
            assert_eq!(
                methods
                    .iter()
                    .filter(|method| *method == "initialize")
                    .count(),
                1
            );
        } else {
            tokio::time::resume();
            assert!(result.is_err());
            assert_eq!(elapsed, 120);
            server.abort();
        }
    }
    Ok(())
}

pub(super) async fn drain_history(
    app: &mut App,
    tui: &mut tui::Tui,
    session: &mut AppServerSession,
    events: &mut mpsc::UnboundedReceiver<AppEvent>,
) -> Result<String> {
    while let Ok(event) = events.try_recv() {
        assert!(!matches!(
            event,
            AppEvent::CodexOp(AppCommand::UserTurn { .. })
        ));
        if matches!(
            event,
            AppEvent::InsertHistoryCell(_)
                | AppEvent::BeginThreadSwitchHistoryReplayBuffer
                | AppEvent::EndInitialHistoryReplayBuffer
        ) {
            app.handle_event(tui, session, event).await?;
        }
    }
    Ok(lines_to_single_string(
        &app.transcript_cells
            .iter()
            .flat_map(|cell| cell.display_lines(/*width*/ 80))
            .collect::<Vec<_>>(),
    ))
}
