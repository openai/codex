//! Command-center inventory and cached navigation recover independently of active work.

use super::active_reconnect::drain_history;
use super::disconnect::serve_reconnect_requests;
use super::*;
use crate::app::reconnect::ReconnectPresentation;
use crate::app::reconnect::reconnect;
use crate::app_event::AgentsOverviewThreadRefresh;
use crate::app_event::WindowsSandboxEnableMode;
use crate::app_server_session::ThreadParamsMode;
use codex_app_server_client::AppServerEvent;
use pretty_assertions::assert_eq;
use serde_json::json;

#[tokio::test]
async fn reconnect_daemon_command_center_after_socket_replacement_without_a_conversation()
-> Result<()> {
    use super::super::agents_overview::AGENTS_OVERVIEW_VIEW_ID;
    use codex_app_server_protocol::Thread;
    use codex_app_server_protocol::ThreadStatus;
    use tokio::net::UnixListener;

    // Losing an optional, previously opened thread must not strand the command center either.
    for (previous_thread, changed_child_permissions, overview_initialized) in [
        (None, false, false),
        (None, false, true),
        (Some(ThreadId::new()), false, true),
        (Some(ThreadId::new()), true, true),
    ] {
        let (mut app, mut events, _) = make_test_app_with_channels().await;
        app.keymap.app.open_agents = vec![crate::key_hint::alt(KeyCode::Char('a'))];
        let child = ThreadId::new();
        let child_recap = recap::RecapProgress {
            completed_turns: 3,
            last_recapped_turn_count: Some(3),
        };
        app.config.model = Some("gpt-test".into());
        app.active_thread_id = previous_thread;
        app.primary_thread_id = previous_thread;
        if let Some(id) = previous_thread {
            app.config
                .permissions
                .set_permission_profile(PermissionProfile::read_only())?;
            app.runtime_approval_policy_override = Some(RuntimeApprovalPolicyOverride::Explicit(
                AskForApproval::OnRequest,
            ));
            app.runtime_permission_profile_override =
                Some(RuntimePermissionProfileOverride::from_config(&app.config));
            let mut session = test_thread_session(id, app.config.cwd.to_path_buf());
            session.approval_policy = AskForApproval::OnRequest;
            session.permission_profile = PermissionProfile::read_only();
            app.primary_session_configured = Some(session.clone());
            app.chat_widget.handle_thread_session(session.clone());
            app.ensure_thread_channel(id)
                .store
                .lock()
                .await
                .set_session(
                    session,
                    vec![test_turn(
                        "cached",
                        TurnStatus::Completed,
                        vec![ThreadItem::Plan {
                            id: "cached-plan".into(),
                            text: "Cached previous conversation".into(),
                        }],
                    )],
                );
            app.chat_widget
                .restore_user_message_to_composer("old draft".into());
            app.store_active_thread_receiver().await;
            app.chat_widget
                .handle_key_event(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
            assert!(app.chat_widget.composer_is_empty());
            app.chat_widget
                .restore_user_message_to_composer("latest draft".into());
        }
        if previous_thread.is_some() {
            let child_session = test_thread_session(child, app.config.cwd.to_path_buf());
            app.ensure_thread_channel(child)
                .store
                .lock()
                .await
                .set_session(child_session, Vec::new());
            app.thread_event_channels[&child]
                .store
                .lock()
                .await
                .merge_recap_progress(child_recap);
            app.upsert_agent_picker_thread(
                child, /*agent_nickname*/ None, /*agent_role*/ None,
                /*is_closed*/ false,
            );
        }
        let selected = ThreadId::from_string("00000000-0000-0000-0000-000000000001")?;
        let vanished = ThreadId::new();
        let added = ThreadId::new();
        let make_thread = |id: ThreadId, name: &str, status: ThreadStatus| -> Result<Thread> {
            Ok(serde_json::from_value(json!({
                "id": id.to_string(), "sessionId": id.to_string(), "preview": name, "name": name,
                "ephemeral": false, "modelProvider": "test-provider", "createdAt": 1,
                "updatedAt": chrono::Utc::now().timestamp() - 120, "status": status,
                "cwd": app.config.cwd, "cliVersion": "0.0.0", "source": "cli", "turns": []
            }))?)
        };
        let child_thread = make_thread(child, "Cached child", ThreadStatus::NotLoaded)?;
        let stale = vec![
            make_thread(
                selected,
                "Selected task",
                ThreadStatus::Active {
                    active_flags: Vec::new(),
                },
            )?,
            make_thread(vanished, "Vanished task", ThreadStatus::Idle)?,
        ];
        let fresh = [
            make_thread(selected, "Selected task", ThreadStatus::Idle)?,
            make_thread(
                added,
                "New task",
                ThreadStatus::Active {
                    active_flags: Vec::new(),
                },
            )?,
        ];
        let stale_threads = stale
            .iter()
            .map(|thread| Ok((ThreadId::from_string(&thread.id)?, Some(thread.clone()))))
            .collect::<Result<HashMap<_, _>>>()?;
        app.agents_overview.threads = stale_threads.clone();
        app.agents_overview
            .last_messages
            .insert(selected, "Pre-disconnect answer".into());
        app.agents_overview.initialized = overview_initialized;
        let view = app.agents_overview_view(
            stale.clone(),
            Some(if previous_thread.is_some() {
                vanished
            } else {
                selected
            }),
        );
        app.agents_overview.visible_thread_ids = view.thread_ids();
        app.chat_widget.show_bottom_pane_view(Box::new(view));
        if previous_thread.is_some() {
            app.agents_overview.view_state.lock().unwrap().input = "Keep this task draft".into();
        } else {
            app.chat_widget.handle_paste("Keep this task draft".into());
        }
        app.agents_overview.view_state.lock().unwrap().renaming = previous_thread.is_some();
        let draft = |app: &App| {
            let state = app.agents_overview.view_state.lock().unwrap();
            if previous_thread.is_some() {
                state.input.clone()
            } else {
                state.composer.as_ref().unwrap().current_text_with_pending()
            }
        };
        let stale_request = Uuid::new_v4();
        app.agents_overview.request_id = Some(stale_request);
        app.agents_overview.refresh_pending = true;
        let refresh = tokio::spawn(std::future::pending::<()>());
        app.agents_overview.refresh_task = Some(refresh.abort_handle());
        let directory = tempfile::tempdir()?;
        let socket_path = directory.path().join("daemon.sock");
        let listener = UnixListener::bind(&socket_path)?;
        let endpoint = crate::RemoteAppServerEndpoint::UnixSocket {
            socket_path: codex_utils_absolute_path::AbsolutePathBuf::try_from(socket_path.clone())?,
        };
        app.app_server_target = AppServerTarget::LocalDaemon {
            endpoint: endpoint.clone(),
        };
        let interrupted_setup = previous_thread.is_none() && !overview_initialized;
        if interrupted_setup {
            let preset = codex_utils_approval_presets::builtin_approval_presets()
                .into_iter()
                .find(|preset| preset.id == "auto")
                .expect("auto preset");
            app.windows_sandbox.pending_setup =
                Some((WindowsSandboxEnableMode::Elevated, preset, None));
            app.windows_sandbox.setup_started_at = Some(Instant::now());
        } else if previous_thread.is_none() {
            app.chat_widget.windows_sandbox_elevated_setup_complete = true;
        }
        let available = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let server_available = Arc::clone(&available);
        let restored_previous = previous_thread
            .map(|id| make_thread(id, "Recovered task", ThreadStatus::NotLoaded))
            .transpose()?;
        let server = tokio::spawn(async move {
            let mut listener = listener;
            let mut methods = Vec::new();
            let mut child_opened = false;
            for connection in 0..(2 + usize::from(previous_thread.is_some())) {
                let (stream, _) = listener.accept().await?;
                let socket = tokio_tungstenite::accept_async(stream).await?;
                methods.extend(serve_reconnect_requests(socket, |request| std::future::ready({
                    match request.method.as_str() {
                        "thread/loaded/list" if connection == 0 => None,
                        "thread/resume" if connection == 1 && previous_thread.is_some() => None,
                        "thread/resume" if request.params.as_ref().unwrap()["threadId"] != child.to_string() && !server_available.load(std::sync::atomic::Ordering::SeqCst) => Some(json!({"error": {"code": -32600, "message": "thread no longer exists"}})),
                        "thread/resume" => {
                            let mut thread = if request.params.as_ref().unwrap()["threadId"] == child.to_string() {
                                child_opened = true;
                                child_thread.clone()
                            } else {
                                restored_previous.clone().unwrap()
                            };
                            if changed_child_permissions && thread.id != child.to_string() {
                                thread.turns = vec![test_turn("saved", TurnStatus::InProgress, Vec::new())];
                            }
                            let (approval, sandbox) = if changed_child_permissions && thread.id != child.to_string() {
                                ("on-request", "readOnly")
                            } else {
                                ("never", "dangerFullAccess")
                            };
                            Some(json!({"result": {"thread": thread, "model": "gpt-test", "modelProvider": "test-provider", "cwd": thread.cwd, "approvalPolicy": approval, "approvalsReviewer": "user", "sandbox": {"type": sandbox}, "reasoningEffort": null}}))
                        }
                        "turn/start" => {
                            let params = request.params.as_ref().unwrap();
                            assert_eq!(params["approvalPolicy"], "on-request");
                            assert_eq!(params["sandboxPolicy"]["type"], if changed_child_permissions { json!(null) } else { json!("readOnly") });
                            assert_eq!(params["input"][0]["text"], "latest draft!");
                            Some(json!({"result": {"turn": {"id": "fresh", "items": [], "status": "inProgress"}}}))
                        }
                        "thread/loaded/list" => Some(json!({"result": {"data": [selected, added], "nextCursor": null}})),
                        "thread/list" => Some(json!({"result": {"data": [], "nextCursor": null}})),
                        "thread/turns/list" => Some(json!({"result": {"data": [], "nextCursor": null}})),
                        "thread/goal/get" => Some(json!({"result": {"goal": null}})),
                        "thread/read" if request.params.as_ref().unwrap()["threadId"] == child.to_string() => Some(json!({"result": {"thread": child_thread}})),
                        "thread/read" if previous_thread.is_some_and(|id| request.params.as_ref().unwrap()["threadId"] == id.to_string()) => Some(if changed_child_permissions && child_opened {
                            let mut thread = restored_previous.clone().unwrap();
                            let status = if server_available.load(std::sync::atomic::Ordering::SeqCst) { TurnStatus::InProgress } else { TurnStatus::Completed };
                            thread.turns = vec![test_turn("saved", status, Vec::new())];
                            json!({"result": {"thread": thread}})
                        } else if server_available.load(std::sync::atomic::Ordering::SeqCst) { json!({"result": {"thread": restored_previous}}) } else { json!({"error": {"code": -32600, "message": "thread no longer exists"}}) }),
                        "thread/read" if request.params.as_ref().unwrap()["threadId"] == vanished.to_string() => Some(json!({"error": {"code": -32600, "message": "thread no longer exists"}})),
                        "thread/read" => Some(json!({"result": {"thread": fresh.iter().find(|thread| request.params.as_ref().unwrap()["threadId"] == thread.id).unwrap()}})),
                        method => panic!("unexpected daemon reconnect request: {method}"),
                    }
                })).await?);
                if connection == 0 {
                    drop(listener);
                    std::fs::remove_file(&socket_path)?;
                    listener = UnixListener::bind(&socket_path)?;
                }
            }
            Ok::<_, color_eyre::Report>(methods)
        });
        let mut session = AppServerSession::new(
            crate::connect_remote_app_server(endpoint).await?,
            ThreadParamsMode::Embedded,
        );
        assert!(
            session
                .thread_loaded_list(ThreadLoadedListParams {
                    cursor: None,
                    limit: Some(100)
                })
                .await
                .is_err()
        );
        let disconnected = session.next_event().await.unwrap();
        app.handle_app_server_event(&session, disconnected).await;
        assert!(app.reconnect.offline);
        assert!(app.agents_overview.last_messages.is_empty());
        assert!(refresh.await.unwrap_err().is_cancelled());
        assert_eq!(
            (
                app.agents_overview.request_id,
                app.agents_overview.refresh_pending
            ),
            (None, false)
        );
        app.apply_agents_overview_thread_refresh(
            &session,
            stale_request,
            Ok(AgentsOverviewThreadRefresh {
                last_messages: HashMap::new(),
                threads: HashMap::new(),
                recent_seed_complete: false,
            }),
        );
        assert_eq!(app.agents_overview.visible_thread_ids.len(), 2);
        let mut tui = crate::tui::test_support::make_test_tui()?;
        app.handle_tui_event(
            &mut tui,
            &mut session,
            TuiEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        )
        .await?;
        app.handle_tui_event(&mut tui, &mut session, TuiEvent::Paste("!".into()))
            .await?;
        assert_eq!(draft(&app), "Keep this task draft!");
        if previous_thread.is_none() {
            assert_snapshot!(
                "daemon_command_center_reconnecting",
                render_bottom_popup(&app.chat_widget, /*width*/ 100)
            );
            app.agents_overview
                .view_state
                .lock()
                .unwrap()
                .connection_notice =
                Some("Reconnect failed — agent list is stale; relaunch to retry");
            assert_snapshot!(
                "daemon_command_center_failed",
                render_bottom_popup(&app.chat_widget, /*width*/ 100)
            );
        }
        let connected = reconnect(
            app.app_server_target.clone(),
            app.config.clone(),
            app.local_settings.clone(),
            previous_thread,
            /*remote_cwd*/ None,
            session.thread_tool_transport(),
            ReconnectPresentation::Overview,
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
        if interrupted_setup {
            assert!(app.windows_sandbox.pending_setup.is_none());
            assert!(app.windows_sandbox.setup_started_at.is_none());
            let mut retained = Vec::new();
            let mut saw_warning = false;
            while let Ok(event) = events.try_recv() {
                if let AppEvent::InsertHistoryCell(cell) = &event {
                    let rendered = lines_to_single_string(&cell.display_lines(/*width*/ 100));
                    if rendered.contains("Windows sandbox setup was interrupted") {
                        insta::assert_snapshot!(rendered, @"■ Windows sandbox setup was interrupted. Restart Codex before using Agent mode.");
                        saw_warning = true;
                        continue;
                    }
                }
                retained.push(event);
            }
            assert!(saw_warning);
            for event in retained {
                app.app_event_tx.send(event);
            }
        } else if previous_thread.is_none() {
            assert!(app.chat_widget.windows_sandbox_elevated_setup_complete);
        }
        assert!(!app.reconnect.offline);
        assert_eq!(app.current_displayed_thread_id(), previous_thread);

        tokio::time::timeout(Duration::from_secs(/*secs*/ 5), async {
            let mut deferred = Vec::new();
            while app.agents_overview.request_id.is_some() {
                let event = events.recv().await.expect("inventory refresh event");
                if matches!(event, AppEvent::AgentsOverviewThreadsLoaded { .. }) {
                    app.handle_event(&mut tui, &mut session, event).await?;
                } else {
                    deferred.push(event);
                }
            }
            for event in deferred {
                app.app_event_tx.send(event);
            }
            Ok::<_, color_eyre::Report>(())
        })
        .await??;
        assert!(app.agents_overview.visible_thread_ids.contains(&vanished));

        // Missing metadata alone does not evict a retained row; an archive notification does.
        app.handle_app_server_event(
            &session,
            AppServerEvent::ServerNotification(Box::new(ServerNotification::ThreadArchived(
                ThreadArchivedNotification {
                    thread_id: vanished.to_string(),
                },
            ))),
        )
        .await;
        let index = app
            .chat_widget
            .selected_index_for_present_view(AGENTS_OVERVIEW_VIEW_ID)
            .unwrap();
        if previous_thread.is_none() {
            assert_eq!(app.agents_overview.visible_thread_ids[index], selected);
        }
        assert!(!app.agents_overview.visible_thread_ids.contains(&vanished));
        assert!(app.agents_overview.visible_thread_ids.contains(&added));
        assert_eq!(
            draft(&app),
            if previous_thread.is_some() {
                ""
            } else {
                "Keep this task draft!"
            }
        );
        if previous_thread.is_some() {
            let history = drain_history(&mut app, &mut tui, &mut session, &mut events).await?;
            assert!(history.contains("Unsubmitted title: Keep this task draft!"));
            assert_snapshot!(
                "daemon_command_center_vanished_rename",
                render_bottom_popup(&app.chat_widget, /*width*/ 100)
            );
            app.handle_tui_event(
                &mut tui,
                &mut session,
                TuiEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            )
            .await?;
            assert!(
                !std::iter::from_fn(|| events.try_recv().ok())
                    .any(|event| matches!(event, AppEvent::DispatchAgentsOverviewTask { .. }))
            );
        }
        assert!(
            app.agents_overview
                .view_state
                .lock()
                .unwrap()
                .connection_notice
                .is_none()
        );
        app.apply_agents_overview_thread_refresh(
            &session,
            stale_request,
            Ok(AgentsOverviewThreadRefresh {
                last_messages: HashMap::new(),
                threads: stale_threads,
                recent_seed_complete: true,
            }),
        );
        assert!(!app.agents_overview.visible_thread_ids.contains(&vanished));

        if let Some(id) = previous_thread {
            app.handle_tui_event(
                &mut tui,
                &mut session,
                TuiEvent::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            )
            .await?;

            assert!(app.chat_widget.has_active_view());
            app.handle_tui_event(
                &mut tui,
                &mut session,
                TuiEvent::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            )
            .await?;

            assert!(app.chat_widget.has_active_view());
            // Esc returns to the overview composer. Dismiss the retained view explicitly
            // to inspect the unavailable conversation and its cached draft below.
            app.agents_overview.view_state.lock().unwrap().completion =
                Some(crate::bottom_pane::ViewCompletion::Accepted);
            app.chat_widget.handle_key_event(KeyCode::Null.into());
            assert!(!app.chat_widget.has_active_view());
            assert_eq!(app.current_displayed_thread_id(), Some(id));
            let history = drain_history(&mut app, &mut tui, &mut session, &mut events).await?;
            assert!(history.contains("Cached previous conversation"));

            app.handle_tui_event(
                &mut tui,
                &mut session,
                TuiEvent::Key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL)),
            )
            .await?;
            let preserved_history =
                drain_history(&mut app, &mut tui, &mut session, &mut events).await?;
            assert_eq!(preserved_history, history);

            let content = &preserved_history[preserved_history
                .find("Cached previous conversation")
                .unwrap()..];
            assert_snapshot!(
                "reconnected_unavailable_conversation",
                format!(
                    "{content}\n{}",
                    render_bottom_popup(&app.chat_widget, /*width*/ 80)
                )
            );

            app.handle_tui_event(
                &mut tui,
                &mut session,
                TuiEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            )
            .await?;
            assert_eq!(app.chat_widget.composer_text_with_pending(), "latest draft");
            assert!(
                !std::iter::from_fn(|| events.try_recv().ok())
                    .any(|event| matches!(event, AppEvent::CodexOp(AppCommand::UserTurn { .. })))
            );
            app.handle_tui_event(&mut tui, &mut session, TuiEvent::Paste("!".into()))
                .await?;
            assert_eq!(
                app.chat_widget.composer_text_with_pending(),
                "latest draft!"
            );
            app.handle_event(&mut tui, &mut session, AppEvent::OpenAgentsOverview)
                .await?;
            assert!(app.chat_widget.has_active_view());
            app.handle_tui_event(
                &mut tui,
                &mut session,
                TuiEvent::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            )
            .await?;
            assert_eq!(
                app.chat_widget.composer_text_with_pending(),
                "latest draft!"
            );
            available.store(
                !changed_child_permissions,
                std::sync::atomic::Ordering::SeqCst,
            );
            app.select_agent_thread(&mut tui, &mut session, child)
                .await?;
            assert_eq!(app.recap.progress(), child_recap);
            assert_eq!(app.primary_thread_id, Some(id));
            assert_eq!(
                app.chat_widget
                    .config_ref()
                    .permissions
                    .permission_profile(),
                &PermissionProfile::Disabled
            );
            if changed_child_permissions {
                app.runtime_approval_policy_override = Some(
                    RuntimeApprovalPolicyOverride::Explicit(AskForApproval::Never),
                );
                app.runtime_permission_profile_override = Some(
                    RuntimePermissionProfileOverride::from_config(app.chat_widget.config_ref()),
                );
                app.select_agent_thread(&mut tui, &mut session, id).await?;
                assert!(app.thread_unavailable(id));
                assert_eq!(app.current_displayed_thread_id(), Some(id));
                assert_eq!(
                    app.chat_widget
                        .config_ref()
                        .permissions
                        .permission_profile(),
                    &PermissionProfile::read_only()
                );
            }
            available.store(true, std::sync::atomic::Ordering::SeqCst);
            app.handle_event(
                &mut tui,
                &mut session,
                AppEvent::SelectAgentsOverviewThread { thread_id: id },
            )
            .await?;
            assert!(!app.thread_unavailable(id));
            assert_eq!(
                app.primary_session_configured,
                app.thread_event_channels[&id].store.lock().await.session
            );
            assert!(!app.agent_navigation.get(&id).unwrap().is_closed);
            app.agent_navigation.mark_running(id);
            assert!(app.agent_navigation.get(&id).unwrap().is_running);
            assert_eq!(
                app.chat_widget.composer_text_with_pending(),
                "latest draft!"
            );
            if changed_child_permissions {
                assert!(app.chat_widget.is_task_running_for_test());
                // From an empty draft, daemon Ctrl+C offers the existing Cancel task action.
                for key in [
                    KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL),
                    KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
                    KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                ] {
                    app.handle_tui_event(&mut tui, &mut session, TuiEvent::Key(key))
                        .await?;
                }
                assert!(std::iter::from_fn(|| events.try_recv().ok())
                    .any(|event| matches!(event, AppEvent::RunningTaskExit { action: RunningTaskExitAction::CancelTask, thread_id } if thread_id == id)));
                app.handle_app_server_event(
                    &session,
                    codex_app_server_client::AppServerEvent::ServerNotification(Box::new(
                        turn_completed_notification(id, "saved", TurnStatus::Completed),
                    )),
                )
                .await;
                app.drain_active_thread_events(&mut tui).await?;
                app.chat_widget
                    .restore_user_message_to_composer("latest draft!".into());
            }
            app.handle_tui_event(
                &mut tui,
                &mut session,
                TuiEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            )
            .await?;
            while let Ok(event) = events.try_recv() {
                if matches!(event, AppEvent::CodexOp(AppCommand::UserTurn { .. })) {
                    app.handle_event(&mut tui, &mut session, event).await?;
                }
            }
            let channel = app.thread_event_channels.get(&id).unwrap();
            let event = ThreadBufferedEvent::Notification(Box::new(
                agent_message_delta_notification(id, "new-turn", "new-item", "fresh delivery"),
            ));
            channel.sender.send(event).await?;
            assert!(app.active_thread_rx.as_mut().unwrap().try_recv().is_ok());
        }
        session.shutdown().await?;
        let methods = server.await??;
        assert_eq!(
            methods
                .iter()
                .filter(|method| *method == "turn/start")
                .count(),
            usize::from(previous_thread.is_some())
        );
        assert_eq!(
            methods.iter().any(|method| method == "thread/resume"),
            previous_thread.is_some()
        );
        assert_eq!(
            methods
                .iter()
                .filter(|method| *method == "initialize")
                .count(),
            2 + usize::from(previous_thread.is_some())
        );
    }
    Ok(())
}
