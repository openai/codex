use super::*;
use crate::app::test_support::make_test_app;
use crate::app_event::AgentsOverviewThreadRefresh;
use crate::bottom_pane::BottomPaneView;
use crate::bottom_pane::CancellationEvent;
use crate::chatwidget::tests::helpers::render_bottom_popup;
use crate::render::renderable::Renderable;
use crate::test_support::PathBufExt;
use crate::test_support::test_path_buf;
use crate::test_support::test_path_display;
use codex_app_server_client::AppServerEvent;
use codex_app_server_protocol::CurrentTimeReadParams;
use codex_app_server_protocol::ReasoningSummaryTextDeltaNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::SessionSource;
use codex_app_server_protocol::ThreadActiveFlag;
use codex_app_server_protocol::ThreadArchivedNotification;
use codex_app_server_protocol::ThreadClosedNotification;
use codex_app_server_protocol::ThreadDeletedNotification;
use codex_app_server_protocol::ThreadNameUpdatedNotification;
use codex_app_server_protocol::ThreadSource;
use codex_app_server_protocol::ThreadStartedNotification;
use codex_app_server_protocol::ThreadStatus;
use codex_app_server_protocol::ThreadUnsubscribeParams;
use codex_app_server_protocol::ThreadUnsubscribeResponse;
use codex_app_server_protocol::ThreadUnsubscribeStatus;
use codex_app_server_protocol::ToolRequestUserInputParams;
use codex_app_server_protocol::ToolRequestUserInputQuestion;
use codex_config::types::KeybindingSpec;
use codex_config::types::KeybindingsSpec;
use codex_config::types::TuiKeymap;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::SubAgentSource;
use crossterm::cursor::SetCursorStyle;
use pretty_assertions::assert_eq;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;

static OVERVIEW_TIMESTAMP: std::sync::LazyLock<i64> =
    std::sync::LazyLock::new(|| chrono::Utc::now().timestamp() - 120);

fn overview_draft(app: &App) -> (String, usize) {
    let state = app.agents_overview.view_state.lock().unwrap();
    let composer = state.composer.as_ref().unwrap();
    (composer.current_text_with_pending(), composer.cursor())
}

#[tokio::test]
async fn overview_composer_preserves_editing_and_routes_focus() {
    let mut app = make_test_app().await;
    app.config.disable_paste_burst = true;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    app.app_event_tx = AppEventSender::new(tx);
    let thread_id = ThreadId::new();
    let threads = vec![overview_thread(
        thread_id,
        /*parent_thread_id*/ None,
        "Existing task",
        ThreadStatus::Idle,
    )];
    let mut view = app.agents_overview_view(threads.clone(), Some(thread_id));
    view.handle_paste("    /first @file $skill\nthird \t".into());
    view.handle_key_event(KeyCode::Home.into());
    view.handle_paste("!second\n".into());
    let expected = overview_draft(&app);
    assert_eq!(
        expected,
        ("    /first @file $skill\n!second\nthird \t".into(), 32)
    );
    view = app.agents_overview_view(threads.clone(), Some(thread_id));
    assert_eq!(overview_draft(&app), expected);
    view.handle_key_event(KeyCode::Esc.into());
    view.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
    assert_eq!(view.on_ctrl_c(), CancellationEvent::Handled);
    view.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
    view.handle_paste(" renamed".into());
    view.handle_key_event(KeyCode::Enter.into());
    assert!(
        matches!(rx.try_recv(), Ok(AppEvent::RenameAgentsOverviewThread { name, .. }) if name == "Existing task renamed")
    );
    view = app.agents_overview_view(threads, Some(thread_id));
    view.handle_key_event(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL));
    view.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
    view.handle_key_event(KeyCode::Esc.into());
    view.handle_key_event(KeyCode::Tab.into());
    assert!(
        matches!(rx.try_recv(), Ok(AppEvent::DispatchAgentsOverviewTask { prompt, .. }) if prompt == expected.0)
    );
    view.handle_key_event(KeyCode::Up.into());
    assert_eq!(overview_draft(&app).0, expected.0);
    assert_eq!(view.on_ctrl_c(), CancellationEvent::Handled);
    view.handle_paste(" \t\n".into());
    view.handle_key_event(KeyCode::Enter.into());
    assert!(rx.try_recv().is_err());
    assert_eq!(view.on_ctrl_c(), CancellationEvent::Handled);
    let mut server = crate::start_embedded_app_server_for_picker(&app.config)
        .await
        .unwrap();
    app.cli_kv_overrides = vec![("model".into(), toml::Value::Integer(1))];
    app.dispatch_agents_overview_task(&mut server, "retry me".into(), Some(app.config.cwd.clone()))
        .await;
    view.handle_paste(" later".into());
    app.submit_agents_overview_prompt(&server, thread_id, "older failure".into())
        .await;
    assert_eq!(overview_draft(&app).0, "retry me later");
    server.shutdown().await.unwrap();
    while rx.try_recv().is_ok() {}
    view = app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
    view.handle_paste(format!(
        "x{}",
        " ".repeat(codex_protocol::user_input::MAX_USER_INPUT_TEXT_CHARS)
    ));
    view.handle_key_event(KeyCode::Enter.into());
    assert!(matches!(rx.try_recv(), Ok(AppEvent::InsertHistoryCell(_))));
    assert!(view.next_frame_delay().is_some());
    app.chat_widget.show_bottom_pane_view(Box::new(view));
    insta::assert_snapshot!(render_bottom_popup(&app.chat_widget, /*width*/ 80).lines().last().unwrap(), @"  Message too long; limit 1048576 characters");
}

#[tokio::test]
async fn overview_composer_preserves_pastes_and_editor_bindings() {
    let mut app = make_test_app().await;
    app.config.disable_paste_burst = true;
    app.chat_widget.toggle_vim_mode_and_notify();
    let mut keymap = TuiKeymap::default();
    keymap.composer.submit = Some(KeybindingsSpec::One(KeybindingSpec("f8".to_string())));
    keymap.editor.insert_newline = Some(KeybindingsSpec::One(KeybindingSpec("f7".to_string())));
    app.keymap = RuntimeKeymap::from_config(&keymap).unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    app.app_event_tx = AppEventSender::new(tx);
    let mut view = app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
    let pasted = "long paste ".repeat(/*n*/ 200);
    view.handle_paste(pasted.clone());
    view.handle_key_event(KeyCode::F(7).into());
    view.handle_paste("last line".into());
    view = app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 60, /*height*/ 24,
    );
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal
        .draw(|frame| view.render(frame.area(), frame.buffer_mut()))
        .unwrap();
    insta::assert_snapshot!(terminal.backend().to_string().lines().last().unwrap(), @r#""   f8 create task    f7 newline    esc esc tasks            ""#);
    assert_eq!(view.cursor_style(area), SetCursorStyle::SteadyBar);
    view.handle_key_event(KeyCode::Home.into());
    view.handle_key_event(KeyCode::Esc.into());
    assert_eq!(view.cursor_style(area), SetCursorStyle::DefaultUserShape);
    view.handle_key_event(KeyCode::Char('d').into());
    view.handle_key_event(KeyCode::Esc.into());
    assert!(view.cursor_pos(area).is_some());
    view.handle_key_event(KeyCode::Esc.into());
    assert_eq!(view.cursor_pos(area), None);
    view.handle_key_event(KeyCode::Char('x').into());
    assert_eq!(view.cursor_pos(area), None);
    view.handle_key_event(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL));
    assert_eq!(view.cursor_style(area), SetCursorStyle::SteadyBar);
    view.handle_key_event(KeyCode::Esc.into());
    view.handle_key_event(KeyCode::Esc.into());
    view.handle_paste("abc".into());
    view.handle_key_event(KeyCode::Char('x').into());
    view.handle_key_event(KeyCode::F(8).into());
    assert!(
        matches!(rx.try_recv(), Ok(AppEvent::DispatchAgentsOverviewTask { prompt, .. }) if prompt == format!("{pasted}\nabcxlast line"))
    );
}

#[tokio::test]
async fn overview_composer_flushes_pending_typing_after_refresh() {
    for disable_paste_burst in [false, true] {
        let mut app = make_test_app().await;
        app.config.disable_paste_burst = false;
        let mut view = app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
        view.handle_key_event(KeyCode::Char('x').into());
        app.config.disable_paste_burst = disable_paste_burst;
        if !disable_paste_burst {
            app.chat_widget.toggle_vim_mode_and_notify();
        }
        view = app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
        assert_eq!(overview_draft(&app).0, "x");
        view.handle_key_event(KeyCode::Esc.into());
        if !disable_paste_burst {
            view.handle_key_event(KeyCode::Esc.into());
        }
        assert_eq!(view.on_ctrl_c(), CancellationEvent::Handled);
        view.handle_key_event(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL));
        view.handle_key_event(KeyCode::Up.into());
        assert_eq!(overview_draft(&app).0, "x");
    }
}

#[tokio::test]
async fn offline_overview_preserves_unbracketed_paste_newlines() {
    let mut app = make_test_app().await;
    app.config.disable_paste_burst = false;
    let mut view = app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
    app.agents_overview
        .view_state
        .lock()
        .unwrap()
        .connection_notice = Some("Reconnecting");
    for ch in "a\nbc\nd".chars() {
        view.handle_key_event(
            if ch == '\n' {
                KeyCode::Enter
            } else {
                KeyCode::Char(ch)
            }
            .into(),
        );
    }
    view.handle_key_event(KeyCode::Tab.into());
    assert_eq!(view.on_ctrl_c(), CancellationEvent::Handled);
    view.handle_key_event(KeyCode::Up.into());
    view.handle_key_event(KeyCode::Enter.into());
    assert_eq!(overview_draft(&app).0, "a\nbc\nd");
    view.handle_key_event(KeyCode::Esc.into());
    app.chat_widget.show_bottom_pane_view(Box::new(view));
    insta::assert_snapshot!(render_bottom_popup(&app.chat_widget, /*width*/ 80).lines().last().unwrap(), @"  ctrl+c clear input, then quit · actions paused until the list is refreshed");
}

fn overview_thread(
    thread_id: ThreadId,
    parent_thread_id: Option<ThreadId>,
    name: &str,
    status: ThreadStatus,
) -> Thread {
    Thread {
        originator: None,
        environments: None,
        id: thread_id.to_string(),
        extra: None,
        project_id: None,
        daybreak_enabled: None,
        session_id: parent_thread_id.unwrap_or(thread_id).to_string(),
        forked_from_id: None,
        parent_thread_id: None,
        preview: name.to_string(),
        ephemeral: false,
        section: None,
        section_entered_at: None,
        history_mode: Default::default(),
        model_provider: "openai".to_string(),
        model: None,
        reasoning_effort: None,
        created_at: *OVERVIEW_TIMESTAMP,
        updated_at: *OVERVIEW_TIMESTAMP,
        recency_at: Some(*OVERVIEW_TIMESTAMP),
        status,
        path: None,
        cwd: test_path_buf("/tmp/project").abs(),
        cli_version: "0.0.0".to_string(),
        source: parent_thread_id.map_or(SessionSource::Cli, |parent_thread_id| {
            SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id,
                depth: 1,
                agent_path: None,
                agent_nickname: None,
                agent_role: None,
            })
        }),
        can_accept_direct_input: Some(parent_thread_id.is_none()),
        thread_source: None,
        agent_nickname: None,
        agent_role: None,
        git_info: None,
        name: Some(name.to_string()),
        turns: Vec::new(),
    }
}

#[tokio::test]
async fn shared_overview_keeps_rows_and_replays_changes_over_stale_reads() -> Result<()> {
    let mut app = make_test_app().await;
    let app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;
    let [retained, archived, deleted, created] = std::array::from_fn(|_| ThreadId::new());
    let [
        mut retained_thread,
        archived_thread,
        deleted_thread,
        created_thread,
    ] = [
        (retained, "Old name"),
        (archived, "Archived"),
        (deleted, "Deleted"),
        (created, "Created in another client"),
    ]
    .map(|(id, name)| {
        overview_thread(id, /*parent_thread_id*/ None, name, ThreadStatus::Idle)
    });
    let request_id = Uuid::new_v4();
    app.agents_overview.request_id = Some(request_id);
    // The seed is still in flight and the command center has never been opened.
    for notification in [
        ServerNotification::ThreadStarted(ThreadStartedNotification {
            thread: created_thread.clone(),
        }),
        ServerNotification::ThreadNameUpdated(ThreadNameUpdatedNotification {
            thread_id: retained.to_string(),
            thread_name: Some("New name".to_string()),
        }),
        ServerNotification::ThreadClosed(ThreadClosedNotification {
            thread_id: retained.to_string(),
        }),
        ServerNotification::ThreadArchived(ThreadArchivedNotification {
            thread_id: archived.to_string(),
        }),
        ServerNotification::ThreadDeleted(ThreadDeletedNotification {
            thread_id: deleted.to_string(),
        }),
    ] {
        app.handle_app_server_event(
            &app_server,
            AppServerEvent::ServerNotification(Box::new(notification)),
        )
        .await;
    }
    app.apply_agents_overview_thread_refresh(
        &app_server,
        request_id,
        Ok(AgentsOverviewThreadRefresh {
            last_messages: HashMap::new(),
            threads: HashMap::from([
                (retained, Some(retained_thread.clone())),
                (archived, Some(archived_thread)),
                (deleted, Some(deleted_thread)),
            ]),
            recent_seed_complete: true,
        }),
    );
    retained_thread.name = Some("New name".to_string());
    retained_thread.status = ThreadStatus::NotLoaded;
    let expected = HashMap::from([
        (retained, Some(retained_thread)),
        (created, Some(created_thread)),
    ]);
    assert_eq!(app.agents_overview.threads, expected);

    let request_id = Uuid::new_v4();
    app.agents_overview.request_id = Some(request_id);
    app.apply_agents_overview_thread_refresh(
        &app_server,
        request_id,
        Ok(AgentsOverviewThreadRefresh {
            last_messages: HashMap::new(),
            threads: HashMap::from([(retained, None)]),
            recent_seed_complete: true,
        }),
    );
    assert_eq!(app.agents_overview.threads, expected);
    let request_id = Uuid::new_v4();
    app.agents_overview.request_id = Some(request_id);
    app.apply_agents_overview_thread_refresh(
        &app_server,
        request_id,
        Err("temporarily unavailable".to_string()),
    );
    assert_eq!(app.agents_overview.threads, expected);

    let view = app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
    app.chat_widget.show_bottom_pane_view(Box::new(view));
    let stale_messages = HashMap::from([(retained, "Removed answer".into())]);
    for event in [
        AppServerEvent::ServerNotification(Box::new(ServerNotification::ThreadReverted(
            codex_app_server_protocol::ThreadRevertedNotification {
                thread_id: retained.to_string(),
            },
        ))),
        AppServerEvent::Lagged { skipped: 1 },
    ] {
        let old_request = Uuid::new_v4();
        app.agents_overview.request_id = Some(old_request);
        app.agents_overview.activity.entry(retained).or_default();
        app.agents_overview.last_messages = stale_messages.clone();
        app.handle_app_server_event(&app_server, event).await;
        assert!(!app.agents_overview.activity.contains_key(&retained));
        assert!(app.agents_overview.last_messages.is_empty());
        // Fresh activity must survive an older read arriving after invalidation.
        app.track_agents_overview_notification(&reasoning_delta(
            retained,
            "new-reasoning",
            "**Checking the revised task**",
        ));
        let current_request = app.agents_overview.request_id.unwrap();
        for (request_id, last_messages) in [
            (old_request, stale_messages.clone()),
            (current_request, HashMap::new()),
        ] {
            app.apply_agents_overview_thread_refresh(
                &app_server,
                request_id,
                Ok(AgentsOverviewThreadRefresh {
                    threads: HashMap::new(),
                    last_messages,
                    recent_seed_complete: true,
                }),
            );
            assert!(app.agents_overview.last_messages.is_empty());
        }
        assert!(app.agents_overview.activity.contains_key(&retained));
        assert!(app.agents_overview.request_id.is_none());
    }
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shared_overview_seeds_once_and_retains_locally_resumed_history() -> Result<()> {
    let mut app = make_test_app().await;
    let mut ids = Vec::new();
    for day in 1..=22 {
        let source = match day {
            3 => codex_protocol::protocol::SessionSource::Custom("atlas".to_string()),
            4 => codex_protocol::protocol::SessionSource::Custom("chatgpt".to_string()),
            5 => codex_protocol::protocol::SessionSource::Exec,
            6 => codex_protocol::protocol::SessionSource::Mcp,
            _ => codex_protocol::protocol::SessionSource::Cli,
        };
        ids.push(ThreadId::from_string(
            &app_test_support::create_fake_rollout_with_source(
                &app.config.codex_home,
                &format!("2025-01-{day:02}T12-00-00"),
                &format!("2025-01-{day:02}T12:00:00Z"),
                &format!("Task {day}"),
                Some(if day == 10 {
                    "other-provider"
                } else {
                    &app.config.model_provider_id
                }),
                /*git_info*/ None,
                source,
            )
            .expect("materialize historical session"),
        )?);
    }
    let message_path = app_test_support::rollout_path(
        &app.config.codex_home,
        "2025-01-21T12-00-00",
        &ids[20].to_string(),
    );
    let mut history = std::fs::read_to_string(&message_path)?;
    history.push_str(
        &serde_json::json!({
            "timestamp": "2025-01-21T12:00:01Z",
            "type": "event_msg",
            "payload": { "type": "agent_message", "message": "Found the regression in the parser." }
        })
        .to_string(),
    );
    history.push('\n');
    std::fs::write(message_path, history)?;
    let config = app.config.clone();
    let mut app_server = Box::pin(crate::start_embedded_app_server_for_picker(&config)).await?;
    for thread_id in [ids[0], ids[21]] {
        app_server
            .resume_thread(
                &app.local_settings,
                config.clone(),
                thread_id,
                crate::app_server_session::ResumeModelSettings::PreserveExistingThread,
            )
            .await?;
    }
    // A newer rollout missing from the index must not trigger a startup filesystem scan.
    app_test_support::create_fake_rollout_with_source(
        &app.config.codex_home,
        "2025-01-23T12-00-00",
        "2025-01-23T12:00:00Z",
        "Unindexed task",
        Some(&app.config.model_provider_id),
        /*git_info*/ None,
        codex_protocol::protocol::SessionSource::Cli,
    )
    .expect("materialize unindexed session");
    app.app_server_target = AppServerTarget::LocalDaemon {
        endpoint: crate::RemoteAppServerEndpoint::UnixSocket {
            socket_path: test_path_buf("/tmp/unused.sock").abs(),
        },
    };
    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
    app.app_event_tx = crate::app_event_sender::AppEventSender::new(event_tx);
    app.refresh_agents_overview_threads(&app_server);
    finish_overview_refresh(&mut app, &app_server, &mut event_rx).await;
    let recent_ids: HashSet<_> = ids[2..].iter().copied().collect();
    let mut expected = recent_ids.clone();
    expected.insert(ids[0]);
    let retained: HashSet<_> = app.agents_overview.threads.keys().copied().collect();
    assert_eq!(retained, expected);
    assert_eq!(
        app.agents_overview.last_messages,
        HashMap::from([(ids[20], "Found the regression in the parser.".to_string())])
    );
    let thread = app.agents_overview.threads[&ids[20]].as_ref().unwrap();
    assert_eq!(thread.status, ThreadStatus::NotLoaded);

    let created = app_server.start_thread(&config).await?.session.thread_id;
    // Closing the view must not cancel a metadata refresh or forget unloaded entries.
    app.primary_thread_id = Some(ids[0]);
    app.open_agents_overview(&app_server);
    let visible: HashSet<_> = app
        .agents_overview
        .visible_thread_ids
        .iter()
        .copied()
        .collect();
    assert_eq!(visible, expected);
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    finish_overview_refresh(&mut app, &app_server, &mut event_rx).await;
    let retained: HashSet<_> = app.agents_overview.threads.keys().copied().collect();
    assert_eq!(retained, expected);

    // A creation notification adds a session without attaching a local view.
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let event = app_server.next_event().await.expect("server notification");
            let is_created = matches!(&event, AppServerEvent::ServerNotification(notification)
                if matches!(notification.as_ref(), ServerNotification::ThreadStarted(started) if started.thread.id == created.to_string()));
            app.handle_app_server_event(&app_server, event).await;
            if is_created { break; }
        }
    }).await.expect("creation notification received");
    expected.insert(created);
    let retained: HashSet<_> = app.agents_overview.threads.keys().copied().collect();
    assert_eq!(retained, expected);

    // Opening older history is a local addition, even without starting a turn.
    let resumed = app_server
        .resume_thread(
            &app.local_settings,
            config.clone(),
            ids[1],
            crate::app_server_session::ResumeModelSettings::PreserveExistingThread,
        )
        .await?;
    app.enqueue_primary_thread_session(resumed.session, resumed.turns)
        .await?;
    app.open_agents_overview(&app_server);
    finish_overview_refresh(&mut app, &app_server, &mut event_rx).await;
    expected.insert(ids[1]);
    let visible: HashSet<_> = app
        .agents_overview
        .visible_thread_ids
        .iter()
        .copied()
        .collect();
    assert_eq!(visible, expected);

    // An unloaded row can be selected through the same flow as a loaded row.
    std::fs::write(
        app.config.codex_home.join("config.toml"),
        "[tui]\nresume_cwd = \"session\"\n",
    )?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    Box::pin(app.select_agents_overview_thread(&mut tui, &mut app_server, ids[2])).await?;
    assert_eq!(app.primary_thread_id, Some(ids[2]));
    app_server.shutdown().await?;

    // A fresh TUI/server has no in-memory additions; read-only resumes did not promote history.
    let mut restarted = make_test_app().await;
    restarted.app_server_target = app.app_server_target.clone();
    let app_server = Box::pin(crate::start_embedded_app_server_for_picker(&config)).await?;
    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
    restarted.app_event_tx = crate::app_event_sender::AppEventSender::new(event_tx);
    restarted.refresh_agents_overview_threads(&app_server);
    finish_overview_refresh(&mut restarted, &app_server, &mut event_rx).await;
    let retained: HashSet<_> = restarted.agents_overview.threads.keys().copied().collect();
    assert_eq!(retained, recent_ids);
    app_server.shutdown().await?;
    Ok(())
}

async fn finish_overview_refresh(
    app: &mut App,
    app_server: &AppServerSession,
    events: &mut tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let AppEvent::AgentsOverviewThreadsLoaded { request_id, result } =
                events.recv().await.expect("overview event")
            {
                assert!(result.is_ok(), "{result:?}");
                app.apply_agents_overview_thread_refresh(app_server, request_id, result);
                return;
            }
        }
    })
    .await
    .expect("overview refresh completed");
}

#[tokio::test]
async fn hidden_system_thread_does_not_refresh_shared_overview() {
    let mut app = make_test_app().await;
    let app_server = crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
        .await
        .expect("embedded app server");
    let view = app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
    app.chat_widget.show_bottom_pane_view(Box::new(view));

    let request_id = uuid::Uuid::new_v4();
    app.agents_overview.request_id = Some(request_id);

    let parent_thread_id = ThreadId::new();
    app.primary_thread_id = Some(parent_thread_id);
    app.active_thread_id = Some(parent_thread_id);
    app.ensure_thread_channel(parent_thread_id);
    app.agents_overview
        .dispatched_requests
        .insert(parent_thread_id, Vec::new());

    let hidden_thread_id = ThreadId::new();
    let mut thread = overview_thread(
        hidden_thread_id,
        Some(parent_thread_id),
        "Generate thread title",
        ThreadStatus::Idle,
    );
    thread.ephemeral = true;
    thread.thread_source = Some(ThreadSource::Feature("system".to_string()));

    app.handle_app_server_event(
        &app_server,
        AppServerEvent::ServerNotification(Box::new(ServerNotification::ThreadStarted(
            ThreadStartedNotification { thread },
        ))),
    )
    .await;

    assert_eq!(
        (
            app.agents_overview.request_id,
            app.agents_overview.refresh_pending,
        ),
        (Some(request_id), false)
    );
    assert!(!app.thread_event_channels.contains_key(&hidden_thread_id));
    assert!(
        !app.agents_overview
            .dispatched_requests
            .contains_key(&hidden_thread_id)
    );

    let visible_thread_id = ThreadId::new();
    let mut thread = overview_thread(
        visible_thread_id,
        Some(parent_thread_id),
        "Persisted system thread",
        ThreadStatus::Idle,
    );
    thread.thread_source = Some(ThreadSource::Feature("system".to_string()));

    app.handle_app_server_event(
        &app_server,
        AppServerEvent::ServerNotification(Box::new(ServerNotification::ThreadStarted(
            ThreadStartedNotification { thread },
        ))),
    )
    .await;

    assert_eq!(
        (
            app.agents_overview.request_id,
            app.agents_overview.refresh_pending,
        ),
        (Some(request_id), true)
    );
    assert!(app.thread_event_channels.contains_key(&visible_thread_id));
    assert!(
        app.agents_overview
            .dispatched_requests
            .contains_key(&visible_thread_id)
    );

    app_server.shutdown().await.expect("shutdown app server");
}

#[tokio::test]
async fn agents_overview_details_show_available_attention_without_expanding_rows() -> Result<()> {
    let mut app = make_test_app().await;
    let app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;
    let [root, child, unloaded] = std::array::from_fn(|_| ThreadId::new());
    let waiting = ThreadStatus::Active {
        active_flags: vec![
            ThreadActiveFlag::WaitingOnApproval,
            ThreadActiveFlag::WaitingOnUserInput,
        ],
    };
    // The child question takes priority over the generic waiting parent.
    let mut threads = [
        (root, None, "Repair authentication", waiting.clone()),
        (child, Some(root), "Check dependencies", waiting),
        (
            unloaded,
            None,
            "Investigate parser",
            ThreadStatus::NotLoaded,
        ),
    ]
    .map(|(id, parent, title, status)| overview_thread(id, parent, title, status))
    .to_vec();
    threads[0].name = None;
    threads[2].name = None;
    threads[2].preview = "Investigate parser\nInclude edge cases\n".repeat(8);
    app.agents_overview.threads = threads
        .iter()
        .cloned()
        .map(|thread| (ThreadId::from_string(&thread.id).unwrap(), Some(thread)))
        .collect();
    app.agents_overview.last_messages.insert(
        unloaded,
        super::super::agents_overview_details::preview_text(
            &"Found the regression\nin the parser. ".repeat(20),
        ),
    );
    app.agents_overview.dispatched_requests.insert(
        child,
        vec![ServerRequest::ToolRequestUserInput {
            request_id: RequestId::Integer(42),
            params: ToolRequestUserInputParams {
                thread_id: child.to_string(),
                turn_id: "turn".into(),
                item_id: "question".into(),
                questions: vec![ToolRequestUserInputQuestion {
                    id: "version".into(),
                    header: "Version".into(),
                    question: "Which dependency version should I use?".into(),
                    is_other: false,
                    is_secret: false,
                    options: None,
                }],
                is_blocking: true,
                auto_resolution_ms: None,
            },
        }],
    );
    let view = app.agents_overview_view(threads.clone(), Some(root));
    app.agents_overview.visible_thread_ids = view.thread_ids();
    app.chat_widget.show_bottom_pane_view(Box::new(view));
    let project = test_path_display("/tmp/project");
    let normalized_group = format!(
        "/tmp/project  2{}",
        " ".repeat(project.len().saturating_sub("/tmp/project".len()))
    );
    insta::with_settings!({snapshot_path => "../snapshots"}, {
        insta::assert_snapshot!("agents_overview_attention", render_bottom_popup(&app.chat_widget, /*width*/ 96).replace(&format!("{project}  2"), &normalized_group).replace(&project, "/tmp/project"));
    });
    app.handle_app_server_event(
        &app_server,
        AppServerEvent::ServerNotification(Box::new(ServerNotification::ServerRequestResolved(
            codex_app_server_protocol::ServerRequestResolvedNotification {
                thread_id: child.to_string(),
                request_id: RequestId::Integer(42),
            },
        ))),
    )
    .await;
    let rendered = render_bottom_popup(&app.chat_widget, /*width*/ 96);
    assert!(!rendered.contains("Which dependency version"));
    assert!(rendered.contains("Waiting for approval."));
    assert!(app.thread_event_channels.is_empty());
    assert!(app.agents_overview.request_id.is_none());

    let view = app.agents_overview_view(threads, Some(unloaded));
    app.chat_widget.show_bottom_pane_view(Box::new(view));
    insta::with_settings!({snapshot_path => "../snapshots"}, {
        insta::assert_snapshot!("agents_overview_last_message", render_bottom_popup(&app.chat_widget, /*width*/ 96).replace(&format!("{project}  2"), &normalized_group).replace(&project, "/tmp/project"));
    });
    app.track_agents_overview_notification(&ServerNotification::ThreadReverted(
        codex_app_server_protocol::ThreadRevertedNotification {
            thread_id: unloaded.to_string(),
        },
    ));
    assert!(!render_bottom_popup(&app.chat_widget, /*width*/ 96).contains("Last message"));
    app_server.shutdown().await?;
    Ok(())
}

fn reasoning_delta(thread_id: ThreadId, item_id: &str, delta: &str) -> ServerNotification {
    ServerNotification::ReasoningSummaryTextDelta(ReasoningSummaryTextDeltaNotification {
        thread_id: thread_id.to_string(),
        turn_id: "turn".into(),
        item_id: item_id.into(),
        summary_index: 0,
        delta: delta.into(),
    })
}

#[tokio::test]
async fn agents_overview_reasoning_uses_existing_events_and_expires_with_attachment() {
    let mut app = make_test_app().await;
    let thread_id = ThreadId::new();
    let mut thread = overview_thread(
        thread_id,
        /*parent_thread_id*/ None,
        "Check startup performance",
        ThreadStatus::Active {
            active_flags: Vec::new(),
        },
    );
    thread.preview.clear();
    app.agents_overview
        .threads
        .insert(thread_id, Some(thread.clone()));
    app.thread_event_channels
        .insert(thread_id, ThreadEventChannel::new(/*capacity*/ 8));
    let mut view = app.agents_overview_view(vec![thread.clone()], Some(thread_id));
    view.handle_paste("Keep this draft".into());
    app.agents_overview.visible_thread_ids = view.thread_ids();
    app.chat_widget.show_bottom_pane_view(Box::new(view));
    for delta in ["**Checking", " cold-start regressions**\nFurther reasoning"] {
        app.track_agents_overview_notification(&reasoning_delta(thread_id, "reasoning", delta));
    }
    let project = test_path_display("/tmp/project");
    let normalized_group = format!(
        "/tmp/project  1{}",
        " ".repeat(project.len().saturating_sub("/tmp/project".len()))
    );
    insta::with_settings!({snapshot_path => "../snapshots"}, {
        insta::assert_snapshot!("agents_overview_live_activity", render_bottom_popup(&app.chat_widget, /*width*/ 96).replace(&format!("{project}  1"), &normalized_group).replace(&project, "/tmp/project"));
    });
    assert!(app.agents_overview.request_id.is_none());

    // A working child can stream through a server attachment without a local channel.
    let parent = overview_thread(
        ThreadId::new(),
        /*parent_thread_id*/ None,
        "Parent",
        thread.status.clone(),
    );
    let parent_id = ThreadId::from_string(&parent.id).unwrap();
    app.agents_overview
        .threads
        .insert(parent_id, Some(parent.clone()));
    app.track_agents_overview_notification(&ServerNotification::ItemCompleted(
        codex_app_server_protocol::ItemCompletedNotification {
            thread_id: parent.id.clone(),
            turn_id: "previous-turn".into(),
            completed_at_ms: 0,
            item: ThreadItem::AgentMessage {
                id: "answer".into(),
                text: "Previous answer".into(),
                phase: None,
                memory_citation: None,
                delivery: None,
                questions: None,
            },
        },
    ));
    let channel = app.thread_event_channels.remove(&thread_id).unwrap();
    let details = app.agents_overview_details(
        &parent,
        &HashMap::from([(parent.id.clone(), vec![&thread])]),
    );
    assert!(
        details
            .iter()
            .any(|line| line.to_string().contains("Checking cold-start regressions"))
    );
    app.thread_event_channels.insert(thread_id, channel);

    // A new, oversized header clears the previous one and cannot grow the parsing buffer indefinitely.
    for delta in [format!("**{}", "界".repeat(10_000)), "**".into()] {
        app.track_agents_overview_notification(&reasoning_delta(thread_id, "oversized", &delta));
    }
    assert_eq!(
        app.agents_overview_details(&thread, &HashMap::new()),
        Vec::<Line>::new()
    );
    app.track_agents_overview_notification(&reasoning_delta(
        thread_id,
        "next",
        "**Running tests**",
    ));
    app.thread_event_channels
        .get_mut(&thread_id)
        .unwrap()
        .mark_replay_only();
    assert_eq!(
        app.agents_overview_details(&thread, &HashMap::new()),
        Vec::<Line>::new()
    );
    app.track_agents_overview_notification(&ServerNotification::ThreadClosed(
        ThreadClosedNotification {
            thread_id: thread_id.to_string(),
        },
    ));
    assert!(!app.agents_overview.activity.contains_key(&thread_id));
}

#[tokio::test]
async fn shared_overview_shows_only_root_sessions() {
    assert_eq!(
        AgentsOverviewGroup::for_status(&ThreadStatus::SystemError),
        AgentsOverviewGroup::NeedsYou
    );
    let mut app = make_test_app().await;
    let first_root = ThreadId::from_string("00000000-0000-0000-0000-000000000101").unwrap();
    let unloaded_root = ThreadId::from_string("00000000-0000-0000-0000-000000000102").unwrap();
    let [child, second_root, side_thread] = std::array::from_fn(|_| ThreadId::new());
    app.primary_thread_id = Some(first_root);

    let mut threads = vec![
        overview_thread(
            first_root,
            /*parent_thread_id*/ None,
            "Build the dashboard",
            ThreadStatus::Idle,
        ),
        overview_thread(
            child,
            Some(first_root),
            "Inspect keyboard shortcuts",
            ThreadStatus::Active {
                active_flags: vec![ThreadActiveFlag::WaitingOnApproval],
            },
        ),
        overview_thread(
            second_root,
            /*parent_thread_id*/ None,
            "Repair authentication",
            ThreadStatus::Active {
                active_flags: Vec::new(),
            },
        ),
        overview_thread(
            unloaded_root,
            /*parent_thread_id*/ None,
            "Review yesterday's changes",
            ThreadStatus::NotLoaded,
        ),
    ];
    let mut side = overview_thread(
        side_thread,
        /*parent_thread_id*/ None,
        "Ephemeral side",
        ThreadStatus::Idle,
    );
    side.ephemeral = true;
    threads.push(side);
    let view = app.agents_overview_view(threads, /*selected_thread_id*/ None);

    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut action_view = AgentsOverviewView::new(
        view.rows.clone(),
        Some(first_root),
        /*exit_on_cancel*/ false,
        crate::app_event_sender::AppEventSender::new(event_tx),
        app.keymap.clone(),
        Arc::clone(&app.agents_overview.view_state),
    );
    let state = &app.agents_overview.view_state;
    assert!(!state.lock().unwrap().status_grouping);
    action_view.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    action_view.handle_key_event(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
    assert!(state.lock().unwrap().status_grouping);
    app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
    assert!(state.lock().unwrap().status_grouping);
    assert!(
        action_view.handle_paste("Use \u{1b}[31mthe\u{1b}[0m current project\u{7}".to_string())
    );
    action_view.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(
        event_rx.try_recv(),
        Ok(AppEvent::DispatchAgentsOverviewTask { prompt, cwd: None })
            if prompt == "Use the current project"
    ));
    action_view.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    action_view.handle_key_event(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
    assert!(action_view.handle_paste("Fix the flaky tests after all retries complete".to_string()));
    let area = ratatui::layout::Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 12,
    );
    let mut buffer = ratatui::buffer::Buffer::empty(area);
    action_view.render(area, &mut buffer);
    let prompt = buffer
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect::<String>();
    assert!(prompt.contains("complete"));
    action_view.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(
        event_rx.try_recv(),
        Ok(AppEvent::DispatchAgentsOverviewTask { prompt, cwd: Some(cwd) })
            if prompt == "Fix the flaky tests after all retries complete"
                && cwd == test_path_buf("/tmp/project").abs()
    ));
    assert!(action_view.handle_paste("   ".to_string()));
    action_view.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    action_view.handle_key_event(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL));
    assert!(action_view.handle_paste("Repair authentication".to_string()));
    action_view.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(
        event_rx.try_recv(),
        Ok(AppEvent::SelectAgentsOverviewThread { thread_id }) if thread_id == second_root
    ));
    assert!(action_view.handle_paste("Continue working".to_string()));
    action_view.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(
        event_rx.try_recv(),
        Ok(AppEvent::DispatchAgentsOverviewTask { prompt, .. })
            if prompt.trim() == "Continue working"
    ));
    action_view.handle_key_event(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL));
    action_view.handle_paste("First line\nSecond line".into());
    crate::chatwidget::tests::helpers::set_active_cell(
        &mut app.chat_widget,
        Box::new(crate::history_cell::PlainHistoryCell::new(vec![
            Line::from("Previous session status"),
        ])),
    );
    app.chat_widget.show_bottom_pane_view(Box::new(view));
    let project = test_path_display("/tmp/project");
    let normalized_project_group = format!(
        "/tmp/project  3{}",
        " ".repeat(project.len().saturating_sub("/tmp/project".len()))
    );
    insta::with_settings!({snapshot_path => "../snapshots"}, {
        insta::assert_snapshot!(
            "agents_overview",
            render_bottom_popup(&app.chat_widget, /*width*/ 96)
                .replace(&format!("{project}  3"), &normalized_project_group)
                .replace(&project, "/tmp/project")
        );
    });

    let threads = (0..20)
        .map(|index| {
            let thread_id = if index == 0 {
                first_root
            } else {
                ThreadId::new()
            };
            let status = if index == 0 {
                ThreadStatus::Active {
                    active_flags: vec![ThreadActiveFlag::WaitingOnApproval],
                }
            } else {
                ThreadStatus::Idle
            };
            let mut candidate = overview_thread(
                thread_id,
                /*parent_thread_id*/ None,
                &format!("Task {index}"),
                status,
            );
            if index == 0 {
                candidate.name = None;
                candidate.preview = "Inspect unnamed task".to_string();
            }
            candidate.updated_at = index;
            candidate.cwd = if index == 0 {
                test_path_buf("/tmp/project-selected").abs()
            } else {
                test_path_buf(&format!("/tmp/project-{}", index % 3)).abs()
            };
            candidate
        })
        .collect();
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    let view = app.agents_overview_view(threads, /*selected_thread_id*/ None);
    app.chat_widget.show_bottom_pane_view(Box::new(view));
    let rendered = render_bottom_popup(&app.chat_widget, /*width*/ 96);
    assert!(
        rendered
            .lines()
            .any(|line| line.contains("› ● Inspect unnamed task  current")
                && line.contains("Needs input"))
    );

    app.transcript_cells.push(std::sync::Arc::new(
        crate::history_cell::PlainHistoryCell::new(vec![ratatui::text::Line::from(
            "Previous conversation",
        )]),
    ));
    let mut tui = crate::tui::test_support::make_test_tui().expect("test terminal");
    let screen_size = tui.terminal.last_known_screen_size;
    app.render_chat_widget_frame(&mut tui, screen_size)
        .expect("render full-screen dashboard");
    assert_eq!(tui.terminal.viewport_area.height, screen_size.height);
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    app.render_chat_widget_frame(&mut tui, screen_size)
        .expect("restore conversation after closing dashboard");
    assert!(tui.terminal.viewport_area.height < screen_size.height);
    assert!(app.last_rendered_history_tail.is_some());
}

#[cfg(any(unix, windows))]
#[tokio::test]
async fn embedded_sessions_offer_to_start_a_background_server_without_migrating() {
    let mut app = make_test_app().await;
    let app_server = crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
        .await
        .expect("embedded app server");

    app.open_agents_overview(&app_server);

    insta::with_settings!({snapshot_path => "../snapshots"}, {
        insta::assert_snapshot!(
            "agents_overview_embedded",
            render_bottom_popup(&app.chat_widget, /*width*/ 96)
        );
    });
    app_server.shutdown().await.expect("shutdown app server");
}

#[cfg(any(unix, windows))]
#[tokio::test]
async fn daemon_start_result_snapshots() -> Result<()> {
    let (mut app, mut app_event_rx, _op_rx) =
        crate::app::tests::make_test_app_with_channels().await;
    let mut app_server =
        crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref()).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;

    for (name, result) in [
        ("agents_daemon_started", Ok(())),
        (
            "agents_daemon_start_failed",
            Err("The host does not allow detached processes".to_string()),
        ),
    ] {
        app.handle_event(
            &mut tui,
            &mut app_server,
            AppEvent::AgentsDaemonStarted { result },
        )
        .await?;
        let cell = match app_event_rx.try_recv()? {
            AppEvent::InsertHistoryCell(cell) => cell,
            other => panic!("expected daemon result history, got {other:?}"),
        };
        let rendered = cell
            .display_lines(/*width*/ 80)
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        insta::with_settings!({snapshot_path => "../snapshots"}, {
            insta::assert_snapshot!(name, rendered);
        });
    }

    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn filtered_dashboard_actions_use_configured_shortcuts() {
    let mut app = make_test_app().await;
    app.chat_widget.toggle_vim_mode_and_notify();
    let mut keymap = TuiKeymap::default();
    keymap.agents.search = Some(KeybindingsSpec::One(KeybindingSpec("f6".to_string())));
    keymap.agents.stop = Some(KeybindingsSpec::One(KeybindingSpec("f10".to_string())));
    keymap.agents.resume = Some(KeybindingsSpec::One(KeybindingSpec("f8".to_string())));
    keymap.list.move_down = Some(KeybindingsSpec::One(KeybindingSpec("v".into())));
    app.keymap = crate::keymap::RuntimeKeymap::from_config(&keymap).expect("runtime keymap");
    let first = ThreadId::new();
    let second = ThreadId::new();
    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut view = AgentsOverviewView::new(
        app.agents_overview_view(
            vec![
                overview_thread(
                    first,
                    /*parent_thread_id*/ None,
                    "First task",
                    ThreadStatus::Idle,
                ),
                overview_thread(
                    second,
                    /*parent_thread_id*/ None,
                    "Second task",
                    ThreadStatus::Active {
                        active_flags: Vec::new(),
                    },
                ),
            ],
            Some(first),
        )
        .rows,
        Some(first),
        /*exit_on_cancel*/ false,
        crate::app_event_sender::AppEventSender::new(event_tx),
        app.keymap.clone(),
        Arc::clone(&app.agents_overview.view_state),
    );

    view.handle_key_event(KeyEvent::new(KeyCode::F(10), KeyModifiers::NONE));
    assert!(event_rx.try_recv().is_err());
    assert!(view.handle_paste("Do not dispatch this draft".to_string()));
    view.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    view.handle_key_event(KeyCode::Esc.into());
    for (key, expected) in [('v', second), ('k', first), ('j', first)] {
        view.handle_key_event(KeyCode::Char(key).into());
        let selected = &view.rows[view.selected_index().unwrap()];
        assert_eq!(selected.thread_id, expected);
    }
    assert_eq!(overview_draft(&app).0, "Do not dispatch this draft");
    view.handle_key_event(KeyEvent::new(KeyCode::F(6), KeyModifiers::NONE));
    assert!(view.handle_paste("Second task".to_string()));
    view.handle_key_event(KeyEvent::new(KeyCode::F(8), KeyModifiers::NONE));
    assert!(matches!(
        event_rx.try_recv(),
        Ok(AppEvent::OpenResumePicker)
    ));
    assert!(!view.is_complete());
    view.handle_key_event(KeyEvent::new(KeyCode::F(10), KeyModifiers::NONE));
    assert!(matches!(
        event_rx.try_recv(),
        Ok(AppEvent::StopAgentsOverviewThread { thread_id }) if thread_id == second
    ));
    view.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(
        event_rx.try_recv(),
        Ok(AppEvent::SelectAgentsOverviewThread { thread_id }) if thread_id == second
    ));
    assert!(event_rx.try_recv().is_err());
}

#[tokio::test]
async fn failed_root_switch_keeps_background_requests_on_the_active_session() -> Result<()> {
    let mut app = make_test_app().await;
    let mut app_server =
        crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref()).await?;
    app.primary_thread_id = Some(ThreadId::new());
    app.ensure_thread_channel(ThreadId::new())
        .store
        .lock()
        .await
        .active_turn_id = Some("running-turn".to_string());
    let mut tui = crate::tui::test_support::make_test_tui()?;

    app.select_agents_overview_thread(&mut tui, &mut app_server, ThreadId::new())
        .await?;

    assert!(app.agents_overview.dispatched_requests.is_empty());
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn root_switch_loads_local_preferences_from_disk() -> Result<()> {
    let mut app = make_test_app().await;
    let mut app_server =
        crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref()).await?;
    let previous = app_server.start_thread(&app.config).await?;
    app.enqueue_primary_thread_session(previous.session, previous.turns)
        .await?;
    let target_thread_id = ThreadId::from_string(
        &app_test_support::create_fake_rollout(
            app.config.codex_home.as_path(),
            "2025-01-05T12-00-00",
            "2025-01-05T12:00:00Z",
            "Target task",
            Some(&app.config.model_provider_id),
            /*git_info*/ None,
        )
        .expect("materialize target rollout"),
    )?;
    std::fs::write(
        app.local_settings.user_config_path.as_path(),
        "[tui]\ntheme = \"dracula\"\nresume_cwd = \"session\"\n[history]\npersistence = \"none\"\n",
    )?;
    let config = app
        .rebuild_config_for_cwd(app.config.cwd.to_path_buf())
        .await?;
    let expected = crate::local_settings::LocalSettings::from(&config);
    let mut tui = crate::tui::test_support::make_test_tui()?;

    app.select_agents_overview_thread(&mut tui, &mut app_server, target_thread_id)
        .await?;

    assert_eq!(app.current_displayed_thread_id(), Some(target_thread_id));
    assert_eq!(app.local_settings, expected);
    assert_eq!(app.chat_widget.local_settings, expected);
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn root_switch_preserves_idle_root_with_running_subagent() -> Result<()> {
    let mut app = make_test_app().await;
    let mut app_server =
        crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref()).await?;
    let previous = app_server.start_thread(&app.config).await?;
    let previous_root_id = previous.session.thread_id;
    app.enqueue_primary_thread_session(previous.session, previous.turns)
        .await?;
    let target_thread_id = ThreadId::from_string(
        &app_test_support::create_fake_rollout(
            app.config.codex_home.as_path(),
            "2025-01-05T12-00-00",
            "2025-01-05T12:00:00Z",
            "Target task",
            Some(&app.config.model_provider_id),
            /*git_info*/ None,
        )
        .expect("materialize target rollout"),
    )?;
    let target = app_server
        .resume_thread(
            &app.local_settings,
            app.config.clone(),
            target_thread_id,
            crate::app_server_session::ResumeModelSettings::PreserveExistingThread,
        )
        .await?;
    let child_id = ThreadId::new();
    let idle_child_id = ThreadId::new();
    app.ensure_thread_channel(idle_child_id);
    app.upsert_agent_picker_thread(
        child_id, /*agent_nickname*/ None, /*agent_role*/ None, /*is_closed*/ false,
    );
    app.agent_navigation.mark_running(child_id);
    app.active_thread_id = Some(child_id);
    let mut tui = crate::tui::test_support::make_test_tui()?;

    app.select_agents_overview_thread(&mut tui, &mut app_server, target.session.thread_id)
        .await?;

    assert!(
        app.agents_overview
            .dispatched_requests
            .contains_key(&idle_child_id)
    );
    app.handle_app_server_event(
        &app_server,
        AppServerEvent::ServerRequest(Box::new(ServerRequest::CurrentTimeRead {
            request_id: RequestId::Integer(99),
            params: CurrentTimeReadParams {
                thread_id: idle_child_id.to_string(),
            },
        })),
    )
    .await;
    assert!(app.agents_overview.dispatched_requests[&idle_child_id].is_empty());
    let response: ThreadUnsubscribeResponse = app_server
        .request_handle()
        .request_typed(ClientRequest::ThreadUnsubscribe {
            request_id: RequestId::String("verify-root-subscription".to_string()),
            params: ThreadUnsubscribeParams {
                thread_id: previous_root_id.to_string(),
            },
        })
        .await?;
    assert_eq!(response.status, ThreadUnsubscribeStatus::Unsubscribed);
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn overview_selection_applies_user_permissions_only_to_unloaded_threads() -> Result<()> {
    let mut app = make_test_app().await;
    std::fs::write(
        app.config.codex_home.join("config.toml"),
        "[tui]\nresume_cwd = \"session\"\n",
    )?;
    let mut server_config = app.config.clone();
    server_config
        .permissions
        .set_permission_profile(PermissionProfile::workspace_write())?;
    server_config
        .permissions
        .approval_policy
        .set(codex_protocol::protocol::AskForApproval::Never)?;
    let mut thread_ids = Vec::new();
    for day in 1..=5 {
        thread_ids.push(ThreadId::from_string(
            &app_test_support::create_fake_rollout(
                &server_config.codex_home,
                &format!("2025-01-{day:02}T12-00-00"),
                &format!("2025-01-{day:02}T12:00:00Z"),
                "Historical session",
                Some(&server_config.model_provider_id),
                /*git_info*/ None,
            )
            .expect("create historical session"),
        )?);
    }
    let mut app_server = crate::start_embedded_app_server_for_picker(&server_config).await?;
    let loaded = app_server
        .resume_thread(
            &crate::local_settings::LocalSettings::from(&server_config),
            server_config.clone(),
            thread_ids[0],
            crate::app_server_session::ResumeModelSettings::RestoreFromThread,
        )
        .await?
        .session;
    app.harness_overrides.sandbox_mode = Some(codex_protocol::config_types::SandboxMode::ReadOnly);
    app.harness_overrides.approval_policy =
        Some(codex_protocol::protocol::AskForApproval::UnlessTrusted);
    app.config
        .permissions
        .set_permission_profile(PermissionProfile::read_only())?;
    app.runtime_permission_profile_override =
        Some(RuntimePermissionProfileOverride::from_config(&app.config));
    app.runtime_approval_policy_override = Some(RuntimeApprovalPolicyOverride::Explicit(
        AskForApproval::UnlessTrusted,
    ));
    let mut tui = crate::tui::test_support::make_test_tui()?;
    for (thread_id, expected_permissions, expected_approval) in [
        (
            loaded.thread_id,
            loaded.permission_profile,
            loaded.approval_policy,
        ),
        (
            thread_ids[1],
            PermissionProfile::read_only(),
            codex_app_server_protocol::AskForApproval::UnlessTrusted,
        ),
        (
            thread_ids[2],
            PermissionProfile::read_only(),
            codex_app_server_protocol::AskForApproval::UnlessTrusted,
        ),
        (
            thread_ids[3],
            PermissionProfile::read_only(),
            codex_app_server_protocol::AskForApproval::UnlessTrusted,
        ),
    ] {
        if thread_id == thread_ids[2] {
            // A session-only /permissions choice must survive subsequent cold resumes.
            std::fs::write(
                app.config.codex_home.join("config.toml"),
                "approvals_reviewer = \"auto_review\"\n[tui]\nresume_cwd = \"session\"\n",
            )?;
            app.harness_overrides.sandbox_mode =
                Some(codex_protocol::config_types::SandboxMode::WorkspaceWrite);
            app.harness_overrides.approval_policy =
                Some(codex_protocol::protocol::AskForApproval::Never);
            app.runtime_permission_profile_override =
                Some(RuntimePermissionProfileOverride::from_config(&app.config));
            app.runtime_approval_policy_override = Some(RuntimeApprovalPolicyOverride::Explicit(
                AskForApproval::UnlessTrusted,
            ));
        }
        app.select_agents_overview_thread(&mut tui, &mut app_server, thread_id)
            .await?;
        let observed = app_server
            .resume_thread(
                &app.local_settings,
                app.config.clone(),
                thread_id,
                crate::app_server_session::ResumeModelSettings::PreserveExistingThread,
            )
            .await?
            .session;
        assert_eq!(
            (
                app.primary_thread_id,
                observed.permission_profile,
                observed.approval_policy,
                observed.approvals_reviewer,
            ),
            (
                Some(thread_id),
                expected_permissions,
                expected_approval,
                ApprovalsReviewer::User
            ),
        );
    }
    let requirements = app.config.codex_home.join("requirements.toml");
    std::fs::write(
        &requirements,
        "allowed_approvals_reviewers = [\"auto_review\"]\n",
    )?;
    app.loader_overrides.system_requirements_path = Some(requirements.to_path_buf());
    app.select_agents_overview_thread(&mut tui, &mut app_server, thread_ids[4])
        .await?;
    assert_eq!(
        (
            app.primary_thread_id,
            app_server
                .thread_read(thread_ids[4], /*include_turns*/ false)
                .await?
                .status
        ),
        (
            Some(thread_ids[3]),
            codex_app_server_protocol::ThreadStatus::NotLoaded
        ),
    );
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn overview_cold_resume_honors_working_directory_selection() -> Result<()> {
    for (mode, cli_cwd, runtime_cwd) in [
        ("current", false, false),
        ("session", true, false),
        ("session", true, true),
        ("session", false, false),
    ] {
        let mut app = make_test_app().await;
        let chosen = app.config.codex_home.join("chosen");
        let overridden = app.config.codex_home.join("overridden");
        std::fs::create_dir(&chosen)?;
        std::fs::create_dir(&overridden)?;
        std::fs::write(
            app.config.codex_home.join("config.toml"),
            format!("[tui]\nresume_cwd = \"{mode}\"\n"),
        )?;
        crate::legacy_core::config::set_project_trust_level(
            app.config.codex_home.as_path(),
            &chosen,
            codex_protocol::config_types::TrustLevel::Trusted,
        )
        .map_err(|error| color_eyre::eyre::eyre!(error.to_string()))?;
        let thread_id = ThreadId::from_string(
            &app_test_support::create_fake_rollout(
                &app.config.codex_home,
                "2025-01-01T12-00-00",
                "2025-01-01T12:00:00Z",
                "Historical session",
                Some(&app.config.model_provider_id),
                /*git_info*/ None,
            )
            .expect("create historical session"),
        )?;
        let mut app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;
        app.launch_cwd = chosen.to_path_buf();
        app.harness_overrides.cwd = cli_cwd.then(|| {
            if runtime_cwd {
                overridden.to_path_buf()
            } else {
                chosen.to_path_buf()
            }
        });
        app.runtime_working_directory_override = runtime_cwd.then(|| chosen.to_path_buf());
        let expected_cwd = if mode == "current" || cli_cwd || runtime_cwd {
            chosen
        } else {
            test_path_buf("/").abs()
        };
        let mut tui = crate::tui::test_support::make_test_tui()?;
        app.select_agents_overview_thread(&mut tui, &mut app_server, thread_id)
            .await?;
        let observed = app_server
            .resume_thread(
                &app.local_settings,
                app.config.clone(),
                thread_id,
                crate::app_server_session::ResumeModelSettings::PreserveExistingThread,
            )
            .await?
            .session;
        assert_eq!(
            (app.config.cwd.clone(), observed.cwd),
            (expected_cwd.clone(), expected_cwd),
        );
        app_server.shutdown().await?;
    }
    Ok(())
}

#[tokio::test]
async fn restored_server_permission_profile_survives_cd_without_turn_override() -> Result<()> {
    let mut app = make_test_app().await;
    let destination = app.config.codex_home.join("destination");
    std::fs::create_dir(&destination)?;
    crate::legacy_core::config::set_project_trust_level(
        app.config.codex_home.as_path(),
        &destination,
        codex_protocol::config_types::TrustLevel::Trusted,
    )
    .map_err(|error| color_eyre::eyre::eyre!(error.to_string()))?;
    let mut app_server =
        crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref()).await?;
    let previous = app_server.start_thread(&app.config).await?;
    app.enqueue_primary_thread_session(previous.session, previous.turns)
        .await?;
    let target = app_server.start_thread(&app.config).await?;
    let target_thread_id = target.session.thread_id;
    let mut tui = crate::tui::test_support::make_test_tui()?;

    app.select_agents_overview_thread(&mut tui, &mut app_server, target_thread_id)
        .await?;
    app.config
        .permissions
        .set_permission_profile(PermissionProfile::read_only())?;
    app.chat_widget.set_permission_profile_with_active_profile(
        PermissionProfile::read_only(),
        /*active_permission_profile*/ None,
    )?;
    app.runtime_permission_profile_override = Some(
        RuntimePermissionProfileOverride::from_restored_config(app.chat_widget.config_ref()),
    );

    assert_eq!(
        app.chat_widget
            .config_ref()
            .permissions
            .effective_permission_profile(),
        PermissionProfile::read_only()
    );
    assert_eq!(
        app.runtime_permission_profile_override
            .as_ref()
            .map(|profile| profile.turn_override),
        Some(RuntimePermissionProfileTurnOverride::Preserve)
    );
    assert_eq!(
        App::turn_permissions_override_from_config(
            app.chat_widget.config_ref(),
            /*active_permission_profile*/ None,
            app.runtime_permission_profile_override
                .as_ref()
                .and_then(RuntimePermissionProfileOverride::turn_permission_profile),
        ),
        TurnPermissionsOverride::Preserve
    );

    app.change_working_directory(&mut tui, &mut app_server, destination.clone())
        .await;

    assert_eq!(app.chat_widget.config_ref().cwd, destination);
    assert_eq!(
        app.chat_widget
            .config_ref()
            .permissions
            .effective_permission_profile(),
        PermissionProfile::read_only()
    );

    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn cancelling_resume_picker_preserves_command_center_state() -> Result<()> {
    for primary_thread_id in [None, Some(ThreadId::new())] {
        let mut app = make_test_app().await;
        app.config.disable_paste_burst = true;
        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
        app.app_event_tx = crate::app_event_sender::AppEventSender::new(event_tx);
        app.primary_thread_id = primary_thread_id;
        let threads = ["First task", "Second task"].map(|name| {
            overview_thread(
                ThreadId::new(),
                /*parent_thread_id*/ None,
                name,
                ThreadStatus::Idle,
            )
        });
        let selected = ThreadId::from_string(&threads[1].id).unwrap();
        let view = app.agents_overview_view(threads.into(), Some(selected));
        app.chat_widget.show_bottom_pane_view(Box::new(view));
        for key in [
            KeyCode::Esc.into(),
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
            KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL),
            KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE),
            KeyCode::Esc.into(),
            KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL),
            KeyEvent::new(KeyCode::Char('S'), KeyModifiers::NONE),
        ] {
            app.chat_widget.handle_key_event(key);
        }
        let before = render_bottom_popup(&app.chat_widget, /*width*/ 96);
        let selection = app
            .chat_widget
            .selected_index_for_present_view(AGENTS_OVERVIEW_VIEW_ID);
        while event_rx.try_recv().is_ok() {}
        app.chat_widget
            .handle_key_event(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL));
        assert!(matches!(
            event_rx.try_recv(),
            Ok(AppEvent::OpenResumePicker)
        ));
        assert!(event_rx.try_recv().is_err());
        let mut tui = crate::tui::test_support::make_test_tui()?;
        let mut server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
        for selection in [SessionSelection::StartFresh, SessionSelection::Exit] {
            assert!(matches!(
                app.apply_resume_picker_selection(&mut tui, &mut server, selection)
                    .await?,
                AppRunControl::Continue
            ));
            app.chat_widget.pre_draw_tick();
            assert_eq!(render_bottom_popup(&app.chat_widget, /*width*/ 96), before);
        }
        server.shutdown().await?;
        assert_eq!(
            app.chat_widget
                .selected_index_for_present_view(AGENTS_OVERVIEW_VIEW_ID),
            selection
        );
        assert_eq!(overview_draft(&app).0, "d");
    }
    Ok(())
}

#[tokio::test]
async fn command_center_cursor_tracks_wrapped_footer() {
    let app = make_test_app().await;
    let mut view = app.agents_overview_view(
        vec![overview_thread(
            ThreadId::new(),
            /*parent_thread_id*/ None,
            "Task",
            ThreadStatus::Idle,
        )],
        /*selected_thread_id*/ None,
    );
    for (key, label) in [('n', "› Describe"), ('f', "Search ›"), ('r', "Rename ›")] {
        view.handle_key_event(KeyCode::Esc.into());
        view.handle_key_event(KeyEvent::new(KeyCode::Char(key), KeyModifiers::CONTROL));
        for width in [48, 96, 120] {
            let area =
                ratatui::layout::Rect::new(/*x*/ 0, /*y*/ 0, width, /*height*/ 24);
            let mut buffer = ratatui::buffer::Buffer::empty(area);
            view.render(area, &mut buffer);
            let prompt_row = buffer
                .content()
                .chunks(usize::from(width))
                .position(|row| {
                    row.iter()
                        .map(ratatui::buffer::Cell::symbol)
                        .collect::<String>()
                        .contains(label)
                })
                .expect("rendered prompt");
            assert_eq!(
                view.cursor_pos(area).map(|(_, y)| usize::from(y)),
                Some(prompt_row)
            );
            assert!(area.contains(view.cursor_pos(area).unwrap().into()));
        }
    }
}

#[tokio::test]
async fn empty_command_center_can_open_resume_picker() {
    let mut app = make_test_app().await;
    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
    app.app_event_tx = crate::app_event_sender::AppEventSender::new(event_tx);
    let view = app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
    app.chat_widget.show_bottom_pane_view(Box::new(view));
    app.chat_widget.handle_key_event(KeyCode::Esc.into());
    while event_rx.try_recv().is_ok() {}
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL));
    assert!(matches!(
        event_rx.try_recv(),
        Ok(AppEvent::OpenResumePicker)
    ));
    insta::with_settings!({snapshot_path => "../snapshots"}, {
        insta::assert_snapshot!("agents_overview_empty_narrow", render_bottom_popup(&app.chat_widget, /*width*/ 48));
    });
}

#[tokio::test]
async fn resuming_active_session_closes_command_center() -> Result<()> {
    let mut app = make_test_app().await;
    let thread_id = ThreadId::new();
    app.active_thread_id = Some(thread_id);
    app.primary_thread_id = Some(thread_id);
    let view = app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
    app.chat_widget.show_bottom_pane_view(Box::new(view));
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    assert!(matches!(
        app.apply_resume_picker_selection(
            &mut tui,
            &mut server,
            SessionSelection::Resume(SessionTarget {
                path: None,
                thread_id,
                history_mode: None,
            })
        )
        .await?,
        AppRunControl::Continue
    ));
    assert_eq!(
        app.chat_widget
            .selected_index_for_present_view(AGENTS_OVERVIEW_VIEW_ID),
        None
    );
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn resume_failure_keeps_command_center_available() {
    let mut app = make_test_app().await;
    let view = app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
    app.chat_widget.show_bottom_pane_view(Box::new(view));
    let before = render_bottom_popup(&app.chat_widget, /*width*/ 96);
    app.add_session_picker_error("The session is unavailable.".to_string());
    insta::with_settings!({snapshot_path => "../snapshots"}, {
        insta::assert_snapshot!("agents_overview_resume_error", render_bottom_popup(&app.chat_widget, /*width*/ 96));
    });
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(render_bottom_popup(&app.chat_widget, /*width*/ 96), before);
}

#[tokio::test]
async fn resume_picker_round_trip_preserves_each_threads_input() -> Result<()> {
    let mut app = make_test_app().await;
    std::fs::write(
        app.config.codex_home.join("config.toml"),
        "[tui]\nresume_cwd = \"current\"\n",
    )?;
    let mut targets = Vec::new();
    for (timestamp, name) in [
        ("2025-01-05T12-00-00", "First task"),
        ("2025-01-05T13-00-00", "Second task"),
    ] {
        let id = app_test_support::create_fake_rollout(
            app.config.codex_home.as_path(),
            timestamp,
            "2025-01-05T12:00:00Z",
            name,
            Some(&app.config.model_provider_id),
            /*git_info*/ None,
        )
        .expect("saved rollout");
        targets.push(SessionTarget {
            path: Some(app_test_support::rollout_path(
                app.config.codex_home.as_path(),
                timestamp,
                &id,
            )),
            thread_id: ThreadId::from_string(&id)?,
            history_mode: None,
        });
    }
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let mut expected_states = Vec::new();
    for target in targets.iter().chain(&targets) {
        let view = app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
        app.chat_widget.show_bottom_pane_view(Box::new(view));
        app.apply_resume_picker_selection(
            &mut tui,
            &mut server,
            SessionSelection::Resume(target.clone()),
        )
        .await?;
        assert_eq!(app.chat_widget.thread_id(), Some(target.thread_id));
        if expected_states.len() < targets.len() {
            assert_eq!(app.chat_widget.composer_text_with_pending(), "");
            assert!(app.chat_widget.queued_user_message_texts().is_empty());
            app.chat_widget.handle_server_notification(
                ServerNotification::TurnStarted(
                    codex_app_server_protocol::TurnStartedNotification {
                        thread_id: target.thread_id.to_string(),
                        turn: codex_app_server_protocol::Turn {
                            id: "turn-with-follow-up".to_string(),
                            items_view: codex_app_server_protocol::TurnItemsView::Full,
                            items: Vec::new(),
                            status: codex_app_server_protocol::TurnStatus::InProgress,
                            error: None,
                            started_at: None,
                            completed_at: None,
                            duration_ms: None,
                        },
                    },
                ),
                /*replay_kind*/ None,
            );
            let follow_up = format!("Follow-up for {}", target.thread_id);
            app.chat_widget.apply_external_edit(follow_up.clone());
            app.chat_widget
                .handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
            assert_eq!(app.chat_widget.queued_user_message_texts(), vec![follow_up]);
            app.chat_widget
                .apply_external_edit(format!("Draft for {}", target.thread_id));
            let mut input_state = app.chat_widget.capture_thread_input_state().unwrap();
            // Recovered follow-ups stay paused until the user explicitly submits them.
            input_state.recovered_queue = true;
            app.chat_widget.restore_thread_input_state(
                Some(input_state),
                crate::chatwidget::ThreadInputStateRestoreMode {
                    preserve_in_flight_turn: false,
                },
            );
            expected_states.push(app.chat_widget.capture_thread_input_state());
        } else {
            let index = targets
                .iter()
                .position(|t| t.thread_id == target.thread_id)
                .unwrap();
            assert_eq!(
                app.chat_widget.capture_thread_input_state(),
                expected_states[index]
            );
        }
    }
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn command_center_handles_resume_failure_and_success() -> Result<()> {
    let mut app = make_test_app().await;
    std::fs::write(
        app.config.codex_home.join("config.toml"),
        "[tui]\nresume_cwd = \"current\"\n",
    )?;
    let timestamp = "2025-01-05T12-00-00";
    let id = app_test_support::create_fake_rollout(
        app.config.codex_home.as_path(),
        timestamp,
        "2025-01-05T12:00:00Z",
        "Saved task",
        Some(&app.config.model_provider_id),
        /*git_info*/ None,
    )
    .expect("saved rollout");
    let thread_id = ThreadId::from_string(&id)?;
    let path = app_test_support::rollout_path(app.config.codex_home.as_path(), timestamp, &id);
    let view = app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
    app.chat_widget.show_bottom_pane_view(Box::new(view));
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let mut server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    assert!(matches!(
        app.apply_resume_picker_selection(
            &mut tui,
            &mut server,
            SessionSelection::Resume(SessionTarget {
                path: None,
                thread_id: ThreadId::new(),
                history_mode: None,
            })
        )
        .await?,
        AppRunControl::Continue
    ));
    assert!(
        app.chat_widget
            .selected_index_for_present_view(AGENTS_OVERVIEW_VIEW_ID)
            .is_some()
    );
    assert!(
        render_bottom_popup(&app.chat_widget, /*width*/ 96).contains("Unable to resume session")
    );
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert!(matches!(
        app.apply_resume_picker_selection(
            &mut tui,
            &mut server,
            SessionSelection::Resume(SessionTarget {
                path: Some(path),
                thread_id,
                history_mode: None,
            })
        )
        .await?,
        AppRunControl::Continue
    ));
    assert_eq!(app.chat_widget.thread_id(), Some(thread_id));
    assert_eq!(
        app.chat_widget
            .selected_index_for_present_view(AGENTS_OVERVIEW_VIEW_ID),
        None
    );
    server.shutdown().await?;
    Ok(())
}
