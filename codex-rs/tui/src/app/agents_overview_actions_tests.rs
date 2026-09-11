use super::*;
use crate::app_event::AgentsOverviewAction;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadLoadedListParams;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_model_provider_info::ModelProviderInfo;
use core_test_support::responses;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn archive_confirmation_number_keys_act_immediately() {
    for key in ['1', '2'] {
        let (mut app, mut rx, _op_rx) = crate::app::tests::make_test_app_with_channels().await;
        let id = ThreadId::new();
        app.agents_overview.threads.insert(
            id,
            Some(overview_thread(
                id,
                /*parent_thread_id*/ None,
                "Current task",
                ThreadStatus::Idle,
            )),
        );
        app.confirm_agents_overview_action(id, AgentsOverviewAction::Archive);
        insta::assert_snapshot!(
            "archive_task_confirmation",
            render_bottom_popup(&app.chat_widget, /*width*/ 72)
        );

        app.chat_widget.handle_key_event(KeyCode::Char(key).into());

        assert!(!app.chat_widget.has_active_view());
        let actions = std::iter::from_fn(|| rx.try_recv().ok())
            .filter_map(|event| match event {
                AppEvent::RunAgentsOverviewAction { thread_id, action } => {
                    Some((thread_id, action))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            actions,
            if key == '2' {
                vec![(id, AgentsOverviewAction::Archive)]
            } else {
                Vec::new()
            }
        );
    }
}

#[tokio::test]
async fn lifecycle_shortcuts_target_filtered_task_in_any_state() {
    let mut app = make_test_app().await;
    let mut keymap = TuiKeymap::default();
    keymap.agents.archive = Some(KeybindingsSpec::One(KeybindingSpec("f5".into())));
    keymap.agents.delete = Some(KeybindingsSpec::One(KeybindingSpec("f6".into())));
    keymap.agents.hide = Some(KeybindingsSpec::One(KeybindingSpec("f7".into())));
    app.keymap = RuntimeKeymap::from_config(&keymap).unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    app.app_event_tx = AppEventSender::new(tx);
    for status in [
        ThreadStatus::Idle,
        ThreadStatus::NotLoaded,
        ThreadStatus::Active {
            active_flags: Vec::new(),
        },
    ] {
        app.agents_overview
            .view_state
            .lock()
            .unwrap()
            .focus_composer();
        let target = ThreadId::new();
        let mut view = app.agents_overview_view(
            vec![
                overview_thread(
                    ThreadId::new(),
                    /*parent_thread_id*/ None,
                    "Other",
                    ThreadStatus::Idle,
                ),
                overview_thread(target, /*parent_thread_id*/ None, "Target", status),
            ],
            Some(target),
        );
        view.handle_key_event(KeyCode::Esc.into());
        view.handle_key_event(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL));
        for character in "Target".chars() {
            view.handle_key_event(KeyCode::Char(character).into());
        }
        for (key, expected) in [
            (5, AgentsOverviewAction::Archive),
            (6, AgentsOverviewAction::Delete),
        ] {
            view.handle_key_event(KeyCode::F(key).into());
            let event = rx.try_recv();
            assert!(
                matches!(event, Ok(AppEvent::ConfirmAgentsOverviewAction { thread_id, action }) if thread_id == target && action == expected),
                "unexpected event: {event:?}"
            );
        }
        view.handle_key_event(KeyCode::F(7).into());
        assert!(
            matches!(rx.try_recv(), Ok(AppEvent::HideAgentsOverviewThread { thread_id }) if thread_id == target)
        );
        assert!(rx.try_recv().is_err());
        view.handle_key_event(KeyCode::Esc.into());
    }
}

#[tokio::test]
async fn hidden_task_stays_hidden_through_activity_and_seed_until_explicit_resume() -> Result<()> {
    let (mut app, mut rx, _op_rx) = crate::app::tests::make_test_app_with_channels().await;
    let mut app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;
    let started = app_server.start_thread(&app.config).await?;
    let id = started.session.thread_id;
    app.enqueue_primary_thread_session(started.session, started.turns)
        .await?;
    let thread = overview_thread(
        id,
        /*parent_thread_id*/ None,
        "Hidden task",
        ThreadStatus::Idle,
    );
    app.agents_overview.threads.insert(id, Some(thread.clone()));
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let view = app.agents_overview_view(vec![thread.clone()], Some(id));
    app.chat_widget.show_bottom_pane_view(Box::new(view));
    app.chat_widget.handle_key_event(KeyCode::Esc.into());
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL));
    let hide = std::iter::from_fn(|| rx.try_recv().ok())
        .find(|event| matches!(event, AppEvent::HideAgentsOverviewThread { .. }))
        .expect("shortcut requests hiding the task");
    Box::pin(app.handle_event(&mut tui, &mut app_server, hide)).await?;
    app.track_agents_overview_notification(&ServerNotification::ThreadStarted(
        ThreadStartedNotification {
            thread: thread.clone(),
        },
    ));
    app.track_agents_overview_notification(&ServerNotification::ThreadStatusChanged(
        codex_app_server_protocol::ThreadStatusChangedNotification {
            thread_id: id.to_string(),
            status: ThreadStatus::Active {
                active_flags: Vec::new(),
            },
        },
    ));
    app.track_agents_overview_notification(&ServerNotification::ThreadClosed(
        ThreadClosedNotification {
            thread_id: id.to_string(),
        },
    ));
    let request_id = Uuid::new_v4();
    app.agents_overview.initialized = false;
    app.agents_overview.request_id = Some(request_id);
    app.apply_agents_overview_thread_refresh(
        &app_server,
        request_id,
        Ok(AgentsOverviewThreadRefresh {
            threads: HashMap::from([(id, Some(thread.clone()))]),
            last_messages: HashMap::new(),
            recent_seed_complete: true,
        }),
    );
    assert_eq!(
        app.agents_overview_view(vec![thread.clone()], /*selected_thread_id*/ None)
            .thread_ids(),
        Vec::<ThreadId>::new()
    );
    assert_eq!(app.primary_thread_id, Some(id));
    app.resume_target_session(
        &mut tui,
        &mut app_server,
        SessionTarget {
            thread_id: id,
            path: None,
            cwd: None,
            history_mode: None,
        },
    )
    .await?;
    assert_eq!(
        app.agents_overview_view(vec![thread], /*selected_thread_id*/ None)
            .thread_ids(),
        vec![id]
    );
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn rejected_delete_preserves_a_live_attachment_and_draft() -> Result<()> {
    let (mut app, _rx, _op_rx) = crate::app::tests::make_test_app_with_channels().await;
    app.config.ephemeral = true;
    let mut app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;
    let started = app_server.start_thread(&app.config).await?;
    let id = started.session.thread_id;
    app.enqueue_primary_thread_session(started.session, started.turns)
        .await?;
    app.chat_widget
        .apply_external_edit("Keep this draft".into());
    let draft = app.chat_widget.capture_thread_input_state();
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.pause_events();
    // Ephemeral tasks provide a deterministic rejection before server teardown.
    Box::pin(app.run_agents_overview_action(
        &mut tui,
        &mut app_server,
        id,
        AgentsOverviewAction::Delete,
    ))
    .await?;
    assert_eq!(
        (app.primary_thread_id, app.chat_widget.thread_id()),
        (Some(id), Some(id))
    );
    assert_eq!(app.chat_widget.capture_thread_input_state(), draft);
    assert!(render_bottom_popup(&app.chat_widget, /*width*/ 80).contains("Could not delete task"));
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn lifecycle_removes_background_and_current_tasks_without_losing_the_dashboard() -> Result<()>
{
    for (action, snapshot, attach_child) in [
        (AgentsOverviewAction::Archive, "archive_task", false),
        (AgentsOverviewAction::Delete, "delete_task", false),
        (AgentsOverviewAction::Archive, "archive_task", true),
        (AgentsOverviewAction::Delete, "delete_task", true),
    ] {
        let key = match action {
            AgentsOverviewAction::Archive => {
                KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL)
            }
            AgentsOverviewAction::Delete => KeyCode::Delete.into(),
        };
        let (mut app, mut rx, _op_rx) =
            Box::pin(crate::app::tests::make_test_app_with_channels()).await;
        let (release, gate) = tokio::sync::oneshot::channel();
        let (server, _completions) = start_streaming_sse_server(vec![vec![
            StreamingSseChunk {
                gate: None,
                body: responses::sse(vec![responses::ev_response_created("running")]),
            },
            StreamingSseChunk {
                gate: Some(gate),
                body: responses::sse(vec![responses::ev_completed("running")]),
            },
        ]])
        .await;
        app.config.model = Some("gpt-5.2".into());
        app.config.model_provider_id = "lifecycle-test".into();
        app.config.model_provider = ModelProviderInfo {
            name: "Lifecycle test".into(),
            base_url: Some(format!("{}/v1", server.uri())),
            request_max_retries: Some(0),
            stream_max_retries: Some(0),
            ..ModelProviderInfo::default()
        };
        app_test_support::MockResponsesConfig::new(server.uri())
            .with_model("gpt-5.2")
            .with_model_provider("lifecycle-test")
            .with_provider_name("Lifecycle test")
            .write(app.config.codex_home.as_path())?;
        let mut app_server =
            Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
        let id = ThreadId::from_string(
            &app_test_support::create_fake_rollout(
                &app.config.codex_home,
                "2025-01-05T12-00-00",
                "2025-01-05T12:00:00Z",
                "Current task",
                Some(&app.config.model_provider_id),
                /*git_info*/ None,
            )
            .expect("materialize session"),
        )?;
        let primary = if attach_child {
            let child = ThreadId::from_string(
                &app_test_support::create_fake_parented_rollout_with_source(
                    &app.config.codex_home,
                    "2025-01-05T12-01-00",
                    "2025-01-05T12:01:00Z",
                    "Current child",
                    Some(&app.config.model_provider_id),
                    /*git_info*/ None,
                    codex_protocol::protocol::SessionSource::SubAgent(
                        SubAgentSource::ThreadSpawn {
                            parent_thread_id: id,
                            depth: 1,
                            agent_path: None,
                            agent_nickname: None,
                            agent_role: None,
                        },
                    ),
                    codex_protocol::SessionId::from(id),
                    id,
                )
                .expect("materialize child session"),
            )?;
            let state_db = codex_state::StateRuntime::init(
                codex_state::SqliteConfig::new_for_testing(app.config.codex_home.clone()),
                app.config.model_provider_id.clone(),
            )
            .await
            .expect("initialize spawn state");
            state_db
                .upsert_thread_spawn_edge(
                    id,
                    child,
                    codex_state::DirectionalThreadSpawnEdgeStatus::Open,
                )
                .await
                .expect("persist spawn edge");
            app.agents_overview.threads.insert(
                child,
                Some(overview_thread(
                    child,
                    Some(id),
                    "Current child",
                    ThreadStatus::Idle,
                )),
            );
            child
        } else {
            id
        };
        let resumed = Box::pin(app_server.resume_thread(
            &app.local_settings,
            app.config.clone(),
            primary,
            crate::app_server_session::ResumeModelSettings::PreserveExistingThread,
        ))
        .await?;
        app.enqueue_primary_thread_session(resumed.session, resumed.turns)
            .await?;
        app.agents_overview.threads.insert(
            id,
            Some(overview_thread(
                id,
                /*parent_thread_id*/ None,
                "Current task",
                ThreadStatus::Idle,
            )),
        );
        app.app_server_target = AppServerTarget::LocalDaemon {
            endpoint: crate::RemoteAppServerEndpoint::UnixSocket {
                socket_path: test_path_buf("/tmp/unused.sock").abs(),
            },
        };
        let mut tui = crate::tui::test_support::make_test_tui()?;
        tui.pause_events();
        app.open_agents_overview(&app_server, AgentsOverviewFocus::List);
        if action == AgentsOverviewAction::Archive {
            let rollout = app_server
                .thread_read(id, /*include_turns*/ false)
                .await?
                .path
                .expect("saved root history");
            let blocked_archive = app
                .config
                .codex_home
                .join("archived_sessions")
                .join(rollout.file_name().unwrap());
            std::fs::create_dir_all(&blocked_archive)?;
            Box::pin(app.run_agents_overview_action(&mut tui, &mut app_server, id, action)).await?;
            assert_eq!(
                (app.primary_thread_id, app.chat_widget.thread_id()),
                (None, None)
            );
            assert_eq!(app.agents_overview.visible_thread_ids, vec![id]);
            assert_eq!(
                app_server
                    .thread_read(primary, /*include_turns*/ false)
                    .await?
                    .status,
                ThreadStatus::NotLoaded
            );
            let error = render_bottom_popup(&app.chat_widget, /*width*/ 40);
            assert!(error.contains("Could not archive task"));
            assert!(error.contains("Work may have stopped."));
            app.chat_widget.handle_key_event(KeyCode::Enter.into());
            assert!(
                app.chat_widget
                    .selected_index_for_present_view(AGENTS_OVERVIEW_VIEW_ID)
                    .is_some()
            );
            std::fs::remove_dir(&blocked_archive)?;
            let resumed = Box::pin(app_server.resume_thread(
                &app.local_settings,
                app.config.clone(),
                primary,
                crate::app_server_session::ResumeModelSettings::PreserveExistingThread,
            ))
            .await?;
            app.enqueue_primary_thread_session(resumed.session, resumed.turns)
                .await?;
            app.open_agents_overview(&app_server, AgentsOverviewFocus::List);
        }
        let background = ThreadId::from_string(
            &app_test_support::create_fake_rollout(
                &app.config.codex_home,
                "2025-01-05T13-00-00",
                "2025-01-05T13:00:00Z",
                "Background task",
                Some(&app.config.model_provider_id),
                /*git_info*/ None,
            )
            .expect("materialize background session"),
        )?;
        let child = ThreadId::new();
        let grandchild = ThreadId::new();
        for (target, parent) in [
            (background, None),
            (child, Some(background)),
            (grandchild, Some(child)),
        ] {
            app.agents_overview.threads.insert(
                target,
                Some(overview_thread(
                    target,
                    parent,
                    "Background task",
                    ThreadStatus::NotLoaded,
                )),
            );
        }
        // Cached details and approval state are invalidated as a group after success.
        let request_id = Uuid::new_v4();
        app.agents_overview.request_id = Some(request_id);
        let stale_threads = app.agents_overview.threads.clone();
        Box::pin(app.run_agents_overview_action(&mut tui, &mut app_server, background, action))
            .await?;
        app.apply_agents_overview_thread_refresh(
            &app_server,
            request_id,
            Ok(AgentsOverviewThreadRefresh {
                threads: stale_threads,
                last_messages: HashMap::new(),
                recent_seed_complete: true,
            }),
        );
        assert_eq!(
            (app.primary_thread_id, app.chat_widget.thread_id()),
            (Some(primary), Some(primary))
        );
        assert_eq!(app.agents_overview.visible_thread_ids, vec![id]);
        assert_eq!(
            app.agents_overview
                .threads
                .keys()
                .copied()
                .collect::<std::collections::HashSet<_>>(),
            std::collections::HashSet::from([id, primary])
        );
        Box::pin(app.run_agents_overview_action(
            &mut tui,
            &mut app_server,
            ThreadId::from_string("00000000-0000-0000-0000-000000000001")?,
            action,
        ))
        .await?;
        assert_eq!(
            (app.primary_thread_id, app.chat_widget.thread_id()),
            (Some(primary), Some(primary))
        );
        insta::assert_snapshot!(
            format!("{snapshot}_failure"),
            render_bottom_popup(&app.chat_widget, /*width*/ 80)
        );
        app.chat_widget.handle_key_event(KeyCode::Enter.into());
        app_server
            .request_handle()
            .request_typed::<TurnStartResponse>(ClientRequest::TurnStart {
                request_id: RequestId::String(Uuid::new_v4().to_string()),
                params: TurnStartParams {
                    thread_id: primary.to_string(),
                    input: vec![codex_app_server_protocol::UserInput::Text {
                        text: "Keep working".into(),
                        text_elements: Vec::new(),
                    }],
                    ..Default::default()
                },
            })
            .await?;
        tokio::time::timeout(
            std::time::Duration::from_secs(/*secs*/ 5),
            server.wait_for_request_count(/*count*/ 1),
        )
        .await?;
        assert!(matches!(
            app_server
                .thread_read(primary, /*include_turns*/ false)
                .await?
                .status,
            ThreadStatus::Active { .. }
        ));
        app.chat_widget.handle_key_event(key);
        let confirmation = std::iter::from_fn(|| rx.try_recv().ok())
            .find(|event| matches!(event, AppEvent::ConfirmAgentsOverviewAction { .. }))
            .expect("shortcut requests confirmation");
        Box::pin(app.handle_event(&mut tui, &mut app_server, confirmation)).await?;
        insta::assert_snapshot!(
            format!("{snapshot}_confirmation"),
            render_bottom_popup(&app.chat_widget, /*width*/ 72)
        );
        app.chat_widget.handle_key_event(KeyCode::Enter.into());
        assert!(
            !std::iter::from_fn(|| rx.try_recv().ok())
                .any(|event| matches!(event, AppEvent::RunAgentsOverviewAction { .. }))
        );
        app.chat_widget.handle_key_event(key);
        let confirmation = std::iter::from_fn(|| rx.try_recv().ok())
            .find(|event| matches!(event, AppEvent::ConfirmAgentsOverviewAction { .. }))
            .expect("shortcut requests confirmation again");
        Box::pin(app.handle_event(&mut tui, &mut app_server, confirmation)).await?;
        app.chat_widget.handle_key_event(KeyCode::Down.into());
        app.chat_widget.handle_key_event(KeyCode::Enter.into());
        let confirmed = std::iter::from_fn(|| rx.try_recv().ok())
            .find(|event| matches!(event, AppEvent::RunAgentsOverviewAction { .. }))
            .expect("confirmation requests lifecycle action");
        if attach_child {
            // Removal must not depend on the overview having the primary or its ancestors cached.
            app.agents_overview.threads.remove(&primary);
        }
        Box::pin(app.handle_event(&mut tui, &mut app_server, confirmed)).await?;
        assert_eq!(
            (
                app.primary_thread_id,
                app.current_displayed_thread_id(),
                app.chat_widget.thread_id()
            ),
            (None, None, None)
        );
        assert_eq!(
            app.agents_overview.visible_thread_ids,
            Vec::<ThreadId>::new()
        );
        assert_eq!(
            app_server
                .thread_loaded_list(ThreadLoadedListParams::default())
                .await?
                .data,
            Vec::<String>::new()
        );
        assert!(
            app.chat_widget
                .selected_index_for_present_view(AGENTS_OVERVIEW_VIEW_ID)
                .is_some()
        );
        match action {
            AgentsOverviewAction::Archive => {
                app_server.thread_unarchive(id).await?;
            }
            AgentsOverviewAction::Delete => {
                assert!(
                    app_server
                        .thread_read(id, /*include_turns*/ false)
                        .await
                        .is_err()
                );
            }
        }
        app_server.shutdown().await?;
        drop(release);
        server.shutdown().await;
    }
    Ok(())
}

#[tokio::test]
async fn disabled_footer_shortcuts_stay_bold_when_wrapped() {
    let app = make_test_app().await;
    let mut view = app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
    view.handle_key_event(KeyCode::Esc.into());
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 84, /*height*/ 24,
    );
    let mut buffer = ratatui::buffer::Buffer::empty(area);
    view.render(area, &mut buffer);
    let delete_key = crate::key_hint::plain(KeyCode::Delete).display_label();
    for key in ["ctrl+x", "ctrl+w", "ctrl+e", delete_key.as_str()] {
        let cells = buffer
            .content()
            .windows(key.len())
            .find(|cells| {
                cells
                    .iter()
                    .map(ratatui::buffer::Cell::symbol)
                    .collect::<String>()
                    == key
            })
            .expect("footer shortcut");
        assert_eq!(
            cells.iter().map(|cell| cell.modifier).collect::<Vec<_>>(),
            vec![ratatui::style::Modifier::BOLD | ratatui::style::Modifier::DIM; key.len()]
        );
    }
}

#[tokio::test]
async fn lifecycle_footer_keeps_custom_chords_with_labels() {
    let mut app = make_test_app().await;
    app.keymap = RuntimeKeymap::from_config(
        &serde_json::from_value(serde_json::json!({
            "agents": { "archive": "f5 f6", "delete": "f5 f7", "hide": "f5 f8" }
        }))
        .unwrap(),
    )
    .unwrap();
    let mut view = app.agents_overview_view(
        vec![overview_thread(
            ThreadId::new(),
            /*parent_thread_id*/ None,
            "Task",
            ThreadStatus::Idle,
        )],
        /*selected_thread_id*/ None,
    );
    view.handle_key_event(KeyCode::Esc.into());
    for width in [36, 48, 80] {
        let area = Rect::new(/*x*/ 0, /*y*/ 0, width + 4, /*height*/ 24);
        let mut buffer = ratatui::buffer::Buffer::empty(area);
        view.render(area, &mut buffer);
        let lines = buffer
            .content()
            .chunks(usize::from(area.width))
            .map(|row| {
                row.iter()
                    .map(ratatui::buffer::Cell::symbol)
                    .collect::<String>()
                    .trim()
                    .to_string()
            })
            .collect::<Vec<_>>();
        for label in ["archive", "delete", "hide"] {
            assert!(
                lines
                    .iter()
                    .any(|line| line.contains(label) && line.contains("f5"))
            );
        }
        assert!(
            lines
                .iter()
                .all(|line| unicode_width::UnicodeWidthStr::width(line.as_str())
                    <= usize::from(width))
        );
    }
    app.chat_widget.show_bottom_pane_view(Box::new(view));
    insta::assert_snapshot!(
        "agents_custom_lifecycle_chords",
        render_bottom_popup(&app.chat_widget, /*width*/ 48)
            .replace(&test_path_display("/tmp/project"), "/tmp/project")
    );
}
