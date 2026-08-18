use super::*;
use crate::app::test_support::make_test_app;
use crate::bottom_pane::BottomPaneView;
use crate::chatwidget::tests::helpers::render_bottom_popup;
use crate::render::renderable::Renderable;
use crate::test_support::PathBufExt;
use crate::test_support::test_path_buf;
use crate::test_support::test_path_display;
use codex_app_server_client::AppServerEvent;
use codex_app_server_protocol::CurrentTimeReadParams;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::SessionSource;
use codex_app_server_protocol::ThreadActiveFlag;
use codex_app_server_protocol::ThreadUnsubscribeParams;
use codex_app_server_protocol::ThreadUnsubscribeResponse;
use codex_app_server_protocol::ThreadUnsubscribeStatus;
use codex_config::types::KeybindingSpec;
use codex_config::types::KeybindingsSpec;
use codex_config::types::TuiKeymap;
use codex_protocol::protocol::SubAgentSource;

static OVERVIEW_TIMESTAMP: std::sync::LazyLock<i64> =
    std::sync::LazyLock::new(|| chrono::Utc::now().timestamp() - 120);

fn overview_thread(
    thread_id: ThreadId,
    parent_thread_id: Option<ThreadId>,
    name: &str,
    status: ThreadStatus,
) -> Thread {
    Thread {
        id: thread_id.to_string(),
        extra: None,
        project_id: None,
        session_id: parent_thread_id.unwrap_or(thread_id).to_string(),
        forked_from_id: None,
        parent_thread_id: None,
        preview: name.to_string(),
        ephemeral: false,
        section: None,
        section_entered_at: None,
        history_mode: Default::default(),
        model_provider: "openai".to_string(),
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
async fn shared_overview_shows_only_root_sessions() {
    assert_eq!(
        AgentsOverviewGroup::for_status(&ThreadStatus::SystemError),
        AgentsOverviewGroup::NeedsYou
    );
    let mut app = make_test_app().await;
    let first_root = ThreadId::from_string("00000000-0000-0000-0000-000000000101").unwrap();
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
    action_view.handle_key_event(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
    assert!(state.lock().unwrap().status_grouping);
    app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
    assert!(state.lock().unwrap().status_grouping);
    assert!(
        action_view.handle_paste("Use \u{1b}[31mthe\u{1b}[0m current project\u{7}".to_string())
    );
    action_view.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
    action_view.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(
        event_rx.try_recv(),
        Ok(AppEvent::DispatchAgentsOverviewTask { prompt, cwd: None })
            if prompt == "Use the current project"
    ));
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
    crate::chatwidget::tests::helpers::set_active_cell(
        &mut app.chat_widget,
        Box::new(crate::history_cell::PlainHistoryCell::new(vec![
            Line::from("Previous session status"),
        ])),
    );
    app.chat_widget.show_bottom_pane_view(Box::new(view));
    let project = test_path_display("/tmp/project");
    let normalized_project_group = format!(
        "/tmp/project  2{}",
        " ".repeat(project.len().saturating_sub("/tmp/project".len()))
    );
    insta::with_settings!({snapshot_path => "../snapshots"}, {
        insta::assert_snapshot!(
            "agents_overview",
            render_bottom_popup(&app.chat_widget, /*width*/ 96)
                .replace(&format!("{project}  2"), &normalized_project_group)
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

#[cfg(unix)]
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

#[tokio::test]
async fn filtered_dashboard_actions_use_configured_shortcuts() {
    let mut app = make_test_app().await;
    let mut keymap = TuiKeymap::default();
    keymap.agents.search = Some(KeybindingsSpec::One(KeybindingSpec("f6".to_string())));
    keymap.agents.stop = Some(KeybindingsSpec::One(KeybindingSpec("f10".to_string())));
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
        .rows
        .clone(),
        Some(first),
        /*exit_on_cancel*/ false,
        crate::app_event_sender::AppEventSender::new(event_tx),
        app.keymap.clone(),
        Arc::default(),
    );

    view.handle_key_event(KeyEvent::new(KeyCode::F(10), KeyModifiers::NONE));
    assert!(event_rx.try_recv().is_err());
    assert!(view.handle_paste("Do not dispatch this draft".to_string()));
    view.handle_key_event(KeyEvent::new(KeyCode::F(6), KeyModifiers::NONE));
    assert!(view.handle_paste("Second task".to_string()));
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
