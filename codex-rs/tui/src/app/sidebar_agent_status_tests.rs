#![cfg(test)]

use super::*;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ThreadClosedNotification;
use codex_app_server_protocol::ThreadStatusChangedNotification;
use codex_app_server_protocol::TurnCompletedNotification;
use crossterm::event::MouseEventKind;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::collections::HashSet;

#[test]
fn idle_preserves_explicit_outcome_until_new_work() {
    let id = ThreadId::new();
    let mut overview = Overview {
        members: HashSet::from([id]),
        ..Default::default()
    };
    let thread_status = |status| {
        ServerNotification::ThreadStatusChanged(ThreadStatusChangedNotification {
            thread_id: id.to_string(),
            status,
        })
    };
    let completed = |status| {
        ServerNotification::TurnCompleted(TurnCompletedNotification {
            thread_id: id.to_string(),
            turn: crate::app::tests::test_turn("turn", status, Vec::new()),
        })
    };
    let completed_item = |item| {
        ServerNotification::ItemCompleted(ItemCompletedNotification::new(
            ThreadId::new().to_string(),
            "parent-turn".into(),
            item,
            /*started_at_ms*/ None,
            /*completed_at_ms*/ 0,
        ))
    };
    let activity = |kind| {
        completed_item(ThreadItemPayload::SubAgentActivity {
            model: None,
            reasoning_effort: None,
            id: "activity".into(),
            kind,
            agent_thread_id: id.to_string(),
            agent_path: "/root/worker".into(),
        })
    };
    observe(&thread_status(ThreadStatus::Idle), &mut overview);
    assert_eq!(overview.states, HashMap::from([(id, State::Idle)]));
    observe(&completed(TurnStatus::Completed), &mut overview);
    observe(&thread_status(ThreadStatus::Idle), &mut overview);
    observe(&activity(SubAgentActivityKind::Interacted), &mut overview);
    observe(&activity(SubAgentActivityKind::Started), &mut overview);
    let spawn = |status| {
        completed_item(ThreadItemPayload::CollabAgentToolCall {
            id: "spawn".into(),
            tool: CollabAgentTool::SpawnAgent,
            status: codex_app_server_protocol::CollabAgentToolCallStatus::Completed,
            sender_thread_id: ThreadId::new().to_string(),
            receiver_thread_ids: vec![id.to_string()],
            prompt: None,
            model: None,
            reasoning_effort: None,
            agents_states: HashMap::from([(
                id.to_string(),
                codex_app_server_protocol::CollabAgentState {
                    status,
                    message: None,
                },
            )]),
        })
    };
    observe(&spawn(CollabAgentStatus::Running), &mut overview);
    assert_eq!(overview.states, HashMap::from([(id, State::Completed)]));
    observe(
        &thread_status(ThreadStatus::Active {
            active_flags: Vec::new(),
        }),
        &mut overview,
    );
    // A delayed spawn notification must not disable a transcript that is already running.
    observe(&spawn(CollabAgentStatus::PendingInit), &mut overview);
    assert_eq!(overview.states, HashMap::from([(id, State::Running)]));
    observe(&completed(TurnStatus::Failed), &mut overview);
    observe(&thread_status(ThreadStatus::Idle), &mut overview);
    observe(&thread_status(ThreadStatus::NotLoaded), &mut overview);
    observe(
        &ServerNotification::ThreadClosed(ThreadClosedNotification {
            thread_id: id.to_string(),
        }),
        &mut overview,
    );
    assert_eq!(overview.states, HashMap::from([(id, State::Failed)]));
    // Parent activity may be our only evidence of a later successful turn.
    observe(&activity(SubAgentActivityKind::Completed), &mut overview);
    assert_eq!(overview.states, HashMap::from([(id, State::Completed)]));
}

#[tokio::test]
async fn chooser_lists_subagents_by_observed_outcome() -> color_eyre::Result<()> {
    use crate::app::owned_transcript::tests::attach_thread;
    use crate::app::sidebar::PaneFocus;
    use crate::app::sidebar::SidebarContent;
    use crate::app::test_support::enable_side_retention;
    use crate::app::test_support::side_frame;
    use crate::app::tests::make_test_app_with_channels;
    let (mut app, _events, _ops) = make_test_app_with_channels().await;
    enable_side_retention(&mut app)?;
    let root = ThreadId::new();
    attach_thread(&mut app, root);
    app.primary_thread_id = Some(root);
    app.local_settings.tui.animations = false;
    let first = ThreadId::new();
    let second = ThreadId::new();
    for (id, name) in [
        (first, "Source navigation reviewer with a long name"),
        (second, "Builder"),
    ] {
        assert!(app.register_sidebar_agent(root, id));
        app.upsert_agent_picker_thread(
            id,
            Some(name.into()),
            /*agent_role*/ None,
            /*is_closed*/ false,
        );
    }
    app.agent_navigation.mark_parent_owned(first);
    app.sidebar_mut()
        .agents
        .states
        .insert(first, State::Starting);
    assert!(app.confirm_sidebar_agent(root, second));
    app.observe_sidebar_agent_event(&ServerNotification::TurnCompleted(
        TurnCompletedNotification {
            thread_id: second.to_string(),
            turn: crate::app::tests::test_turn("done", TurnStatus::Completed, vec![]),
        },
    ));
    app.sidebar_mut()
        .show(SidebarContent::Chooser, PaneFocus::Sidebar);
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    for (name, width) in [("subagent_chooser", 210), ("subagent_chooser_narrow", 120)] {
        let screen = side_frame(&mut app, &mut tui, ratatui::layout::Size::new(width, 25))?;
        let rows = screen
            .lines()
            .filter_map(|line| line.split_once('│').map(|(_, row)| row.trim_end()))
            .filter(|row| !row.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        insta::assert_snapshot!(name, rows);
    }

    let pane = app.sidebar_mut();
    let old_index = pane.chat_tabs().agent_index(second).expect("second agent");
    pane.choice = old_index;
    app.observe_sidebar_agent_event(&ServerNotification::TurnCompleted(
        TurnCompletedNotification {
            thread_id: first.to_string(),
            turn: crate::app::tests::test_turn("done", TurnStatus::Completed, vec![]),
        },
    ));
    app.observe_sidebar_agent_event(&ServerNotification::ThreadStatusChanged(
        ThreadStatusChangedNotification {
            thread_id: second.to_string(),
            status: ThreadStatus::Active {
                active_flags: Vec::new(),
            },
        },
    ));
    side_frame(&mut app, &mut tui, ratatui::layout::Size::new(120, 25))?;
    let pane = app.sidebar().unwrap();
    assert_eq!(pane.choice, old_index - 1);
    assert_eq!(
        pane.chat_tabs().agent(pane.choice).map(|agent| agent.id),
        Some(second)
    );
    Ok(())
}

#[tokio::test]
async fn discovery_updates_provisional_status_without_overwriting_newer_outcomes()
-> color_eyre::Result<()> {
    use crate::app::owned_transcript::tests::attach_thread;
    use crate::app::test_support::enable_side_retention;
    use crate::app::test_support::side_frame;
    use crate::app::tests::make_test_app_with_channels;
    use crate::tui::TuiEvent;
    use crossterm::event::KeyCode;
    use ratatui::layout::Size;

    let (mut app, mut events, _ops) = Box::pin(make_test_app_with_channels()).await;
    enable_side_retention(&mut app)?;
    app.local_settings.tui.animations = false;
    let config = app.chat_widget.config_ref().clone();
    let mut server = Box::pin(crate::start_embedded_app_server_for_picker(&config)).await?;
    let root = ThreadId::new();
    let child = ThreadId::from_string(
        &app_test_support::create_fake_paginated_rollout(
            config.codex_home.as_path(),
            "2025-01-05T12-00-00",
            "2025-01-05T12:00:00Z",
            "Child conversation",
            Some(config.model_provider_id.as_str()),
            /*git_info*/ None,
        )
        .expect("seed child history"),
    )?;
    let mut thread = Box::pin(server.thread_read(child, /*include_turns*/ false)).await?;
    thread.status = ThreadStatus::Active {
        active_flags: Vec::new(),
    };

    let completed = ThreadId::new();
    let mut completed_thread = thread.clone();
    completed_thread.id = completed.to_string();
    attach_thread(&mut app, root);
    app.primary_thread_id = Some(root);
    let failed = ThreadId::new();
    let mut failed_thread = thread.clone();
    failed_thread.id = failed.to_string();
    failed_thread.status = ThreadStatus::SystemError;
    let idle = ThreadId::new();
    let mut idle_thread = thread.clone();
    idle_thread.id = idle.to_string();
    idle_thread.status = ThreadStatus::Idle;
    let saved = ThreadId::new();
    let mut saved_thread = idle_thread.clone();
    saved_thread.id = saved.to_string();
    saved_thread.status = ThreadStatus::NotLoaded;
    let unloaded = ThreadId::new();
    let mut unloaded_thread = saved_thread.clone();
    unloaded_thread.id = unloaded.to_string();
    let outcomes = [child, completed, saved]
        .into_iter()
        .flat_map(|id| {
            [
                SubAgentActivityKind::Completed,
                SubAgentActivityKind::Started,
            ]
            .into_iter()
            .map(move |kind| ThreadItemPayload::SubAgentActivity {
                id: format!("completed-{id}"),
                kind,
                agent_thread_id: id.to_string(),
                agent_path: "/root/worker".into(),
                model: None,
                reasoning_effort: None,
            })
        })
        .collect();
    app.register_sidebar_agent(root, child);
    app.observe_sidebar_agent_event(&ServerNotification::ThreadStatusChanged(
        ThreadStatusChangedNotification {
            thread_id: child.to_string(),
            status: thread.status.clone(),
        },
    ));
    assert!(
        std::iter::from_fn(|| events.try_recv().ok()).any(|event| {
            matches!(event, crate::app_event::AppEvent::DiscoverSidebarAgents(id) if id == root)
        }),
        "a live status update must start discovery even without chooser input"
    );
    let request = app
        .agent_navigation
        .begin_picker_refresh(root)
        .expect("start discovery");
    assert!(
        app.apply_agent_picker_thread_refresh(
            &server,
            root,
            request,
            Err("discovery failed".into()),
        )
        .is_none()
    );
    app.observe_sidebar_agent_event(&ServerNotification::ThreadStatusChanged(
        ThreadStatusChangedNotification {
            thread_id: child.to_string(),
            status: thread.status.clone(),
        },
    ));
    assert!(
        std::iter::from_fn(|| events.try_recv().ok()).any(|event| {
            matches!(event, crate::app_event::AppEvent::DiscoverSidebarAgents(id) if id == root)
        }),
        "a later active update must retry discovery for a running unconfirmed agent"
    );
    app.restore_sidebar_agent_outcomes(
        root,
        &[crate::app::tests::test_turn(
            "saved-turn",
            TurnStatus::Completed,
            outcomes,
        )],
        super::super::OutcomeReplay::Reconnect,
    );
    for id in [idle, unloaded] {
        assert!(app.register_sidebar_agent(root, id));
        app.sidebar_mut().agents.states.insert(id, State::Starting);
    }
    for (id, name) in [
        (child, "worker"),
        (completed, "worker"),
        (idle, "idle-worker"),
        (saved, "worker"),
        (unloaded, "saved-worker"),
    ] {
        app.upsert_agent_picker_thread(
            id,
            Some(name.into()),
            /*agent_role*/ None,
            /*is_closed*/ false,
        );
    }
    let rows = |app: &crate::app::App| {
        app.sidebar()
            .unwrap()
            .agents
            .chooser_rows(&app.agent_navigation)
            .into_iter()
            .map(|row| (row.id, (row.status, row.selectable)))
            .collect::<HashMap<_, _>>()
    };
    assert_eq!(
        rows(&app),
        HashMap::from([
            (child, ("Transcript unavailable", false)),
            (completed, ("Transcript unavailable", false)),
            (idle, ("Starting", false)),
            (saved, ("Transcript unavailable", false)),
            (unloaded, ("Starting", false)),
        ])
    );
    // No frame has drawn the hidden chooser since discovery failed.
    assert!(app.sidebar().unwrap().agents.rows.is_empty());
    app.keymap.app.focus_sidebar_tabs = vec![crate::key_hint::plain(KeyCode::F(6))];
    let size = Size::new(/*width*/ 210, /*height*/ 24);
    let mut tui = crate::tui::test_support::make_test_tui_with_size(size)?;
    tui.set_owned_screen(/*owned*/ true)?;
    Box::pin(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::F(6).into())))
        .await?;
    assert_eq!(
        app.chooser_focus(),
        Some(crate::app::sidebar::PaneFocus::Sidebar)
    );
    assert!(
        app.agent_navigation.picker_refresh.is_some(),
        "opening the hidden chooser retries discovery before the first draw"
    );
    let request = loop {
        if let crate::app_event::AppEvent::AgentPickerThreadsLoaded { request_id, .. } =
            events.recv().await.expect("app event channel")
        {
            break request_id;
        }
    };
    // A completion arriving while the refresh is in flight wins over the active snapshot.
    app.observe_sidebar_agent_event(&ServerNotification::TurnCompleted(
        TurnCompletedNotification {
            thread_id: completed.to_string(),
            turn: crate::app::tests::test_turn("fresh-turn", TurnStatus::Completed, Vec::new()),
        },
    ));
    assert!(
        app.apply_agent_picker_thread_refresh(
            &server,
            root,
            request,
            Ok(crate::app_event::AgentPickerThreadRefresh {
                threads: vec![
                    thread,
                    completed_thread,
                    failed_thread,
                    idle_thread,
                    saved_thread,
                    unloaded_thread,
                ],
                ..Default::default()
            }),
        )
        .is_none()
    );
    assert_eq!(
        rows(&app),
        HashMap::from([
            (child, ("Running", true)),
            (completed, ("Completed", true)),
            (failed, ("Failed", true)),
            (idle, ("Idle", true)),
            (saved, ("Completed", true)),
            (unloaded, ("Unavailable", true)),
        ])
    );
    app.restore_sidebar_agent_outcomes(
        root,
        &[crate::app::tests::test_turn(
            "reconnected-turn",
            TurnStatus::Completed,
            vec![ThreadItemPayload::SubAgentActivity {
                id: "completed-while-disconnected".into(),
                kind: SubAgentActivityKind::Completed,
                agent_thread_id: idle.to_string(),
                agent_path: "/root/idle-worker".into(),
                model: None,
                reasoning_effort: None,
            }],
        )],
        super::super::OutcomeReplay::Reconnect,
    );
    assert_eq!(rows(&app).get(&idle), Some(&("Completed", true)));
    let screen = side_frame(&mut app, &mut tui, size)?;
    let ready_rows = screen
        .lines()
        .filter_map(|line| line.split_once('│').map(|(_, row)| row))
        .filter(|row| row.contains("saved-worker") || row.contains("idle-worker"))
        .map(|row| row.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(ready_rows, @r"
    ○ saved-worker Unavailable
    ○ idle-worker Completed
    ");
    tui.set_owned_screen(/*owned*/ false)?;
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn opens_agent_transcript_beside_parent_and_returns_to_chooser() -> color_eyre::Result<()> {
    use crate::app::side_focus::SidebarTarget;
    use crate::app::test_support::enable_side_retention;
    use crate::app::test_support::side_frame;
    use crate::app::tests::make_test_app_with_channels;
    use crate::sidebar_tab_strip::SidebarTab;
    use crate::sidebar_tab_strip::TabTarget;
    use crate::tui::TuiEvent;
    use codex_protocol::protocol::SessionSource;
    use codex_protocol::protocol::SubAgentSource;
    use crossterm::event::KeyCode;
    use crossterm::event::KeyModifiers;
    use crossterm::event::MouseButton;
    use crossterm::event::MouseEvent;
    use ratatui::layout::Size;

    let (mut app, mut rx, _ops) = Box::pin(make_test_app_with_channels()).await;
    enable_side_retention(&mut app)?;
    app.local_settings.tui.animations = false;
    let config = app.chat_widget.config_ref().clone();
    let root = ThreadId::from_string(
        &app_test_support::create_fake_paginated_rollout(
            config.codex_home.as_path(),
            "2025-01-05T12-00-00",
            "2025-01-05T12:00:00Z",
            "Parent conversation",
            Some(config.model_provider_id.as_str()),
            /*git_info*/ None,
        )
        .expect("seed test rollout"),
    )?;
    let seed_agent =
        |parent, second: u8, path: &str, reply: &str| -> color_eyre::Result<ThreadId> {
            let timestamp = format!("2025-01-05T12-00-{second:02}");
            let id = app_test_support::create_fake_parented_rollout_with_source(
                config.codex_home.as_path(),
                &timestamp,
                &format!("2025-01-05T12:00:{second:02}Z"),
                reply,
                Some(config.model_provider_id.as_str()),
                /*git_info*/ None,
                SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                    parent_thread_id: parent,
                    depth: if parent == root { 1 } else { 2 },
                    agent_path: Some(path.parse().expect("agent path")),
                    agent_nickname: Some(path.rsplit('/').next().expect("agent name").into()),
                    agent_role: None,
                }),
                root.into(),
                parent,
            )
            .expect("seed agent rollout");
            let rollout_path =
                app_test_support::rollout_path(config.codex_home.as_path(), &timestamp, &id);
            let rollout = std::fs::read_to_string(&rollout_path)?;
            let (metadata, messages) = rollout.split_once('\n').expect("seeded child history");
            let mut metadata: serde_json::Value = serde_json::from_str(metadata)?;
            metadata["payload"]["multi_agent_version"] =
                serde_json::to_value(codex_protocol::protocol::MultiAgentVersion::V2)?;
            std::fs::write(rollout_path, format!("{metadata}\n{messages}"))?;
            Ok(ThreadId::from_string(&id)?)
        };
    let child = seed_agent(root, /*second*/ 1, "/root/reviewer", "Review result")?;
    let nested = seed_agent(
        child,
        /*second*/ 2,
        "/root/reviewer/helper",
        "Nested review result",
    )?;

    let mut server = Box::pin(crate::start_embedded_app_server_for_picker(&config)).await?;
    let started = Box::pin(server.resume_thread(
        &app.local_settings,
        config.clone(),
        root,
        crate::app_server_session::ResumeModelSettings::RestoreFromThread,
    ))
    .await?;
    app.enqueue_primary_thread_session(started.session, started.turns)
        .await?;
    let mut tui = crate::tui::test_support::make_test_tui_with_size(Size::new(
        /*width*/ 140, /*height*/ 20,
    ))?;
    tui.set_owned_screen(/*owned*/ true)?;
    // Hold the actual discovery response so the first click races it deterministically.
    let mut discovery = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let event = rx.recv().await.expect("app event channel");
            if matches!(
                event,
                crate::app_event::AppEvent::AgentPickerThreadsLoaded { .. }
            ) {
                break Ok::<_, color_eyre::Report>(event);
            }
            Box::pin(app.handle_event(&mut tui, &mut server, event)).await?;
        }
    })
    .await??;
    let crate::app_event::AppEvent::AgentPickerThreadsLoaded { result, .. } = &mut discovery else {
        unreachable!();
    };
    // Model the first request taking its snapshot before either agent was spawned.
    result.as_mut().expect("initial discovery").threads.clear();
    let activity = ThreadItemPayload::SubAgentActivity {
        id: "started".into(),
        agent_thread_id: child.to_string(),
        kind: SubAgentActivityKind::Started,
        agent_path: "/root/reviewer".into(),
        model: None,
        reasoning_effort: None,
    };
    // Parent notifications can expose a running row before an earlier lookup has finished.
    app.observe_sidebar_agent_event(&ServerNotification::ItemCompleted(
        ItemCompletedNotification::new(
            root.to_string(),
            "parent-turn".into(),
            activity.clone(),
            /*started_at_ms*/ None,
            /*completed_at_ms*/ 0,
        ),
    ));
    assert!(
        app.agent_navigation
            .picker_refresh
            .as_ref()
            .unwrap()
            .follow_up_requested,
        "a lookup that started before the spawn needs a follow-up"
    );
    use crate::app::sidebar::PaneFocus;
    use crate::app::sidebar::SidebarContent;
    app.sidebar_mut()
        .show(SidebarContent::Chooser, PaneFocus::Sidebar);
    let size = Size::new(/*width*/ 210, /*height*/ 24);
    let screen = side_frame(&mut app, &mut tui, size)?;
    let loading_row = screen
        .lines()
        .filter_map(|line| line.split_once('│').map(|(_, row)| row.trim()))
        .find(|row| row.contains("reviewer"))
        .expect("visible agent loading row")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    insta::assert_snapshot!(loading_row, @"● reviewer Loading transcript…");
    let (row, column) = screen
        .lines()
        .enumerate()
        .find_map(|(row, line)| {
            let offset = line.find("reviewer")?;
            Some((row as u16, line[..offset].chars().count() as u16))
        })
        .expect("visible disabled row");
    let status_column = screen.lines().nth(usize::from(row)).unwrap();
    let status_column = status_column[..status_column.find("Loading transcript…").unwrap()]
        .chars()
        .count() as u16;
    let buffer = tui.terminal.previous_buffer();
    for col in [column, status_column] {
        assert!(
            buffer[(col, row)]
                .modifier
                .contains(ratatui::style::Modifier::DIM),
            "the disabled name and status both use dim text"
        );
    }
    for kind in [
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::Up(MouseButton::Left),
    ] {
        let click = crate::app::owned_transcript::tests::mouse(kind, column, row);
        Box::pin(app.handle_tui_event(&mut tui, &mut server, click)).await?;
    }
    assert_eq!(
        app.sidebar().unwrap().choice,
        0,
        "click skips the disabled row"
    );
    assert_eq!(app.chooser_focus(), Some(PaneFocus::Sidebar));
    for key in [KeyCode::Down, KeyCode::End, KeyCode::PageDown] {
        Box::pin(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(key.into()))).await?;
        assert_eq!(
            app.sidebar().unwrap().choice,
            0,
            "{key:?} skips the disabled row"
        );
    }
    assert_eq!(app.docked_agent(), None);
    app.sidebar_mut()
        .show(SidebarContent::Chooser, PaneFocus::Main);
    let cell = crate::multi_agents::sub_agent_activity_history_cell(&activity).unwrap();
    use crate::history_cell::HistoryCell;
    let lines = cell.display_hyperlink_lines(/*width*/ 80);
    let link = &lines[0].hyperlinks[0];
    assert_eq!(link.terminal_destination(), None);
    let agent = link.agent_id().unwrap();
    let backend = server.backend_id();
    for (source, target) in [(ThreadId::new(), agent), (root, ThreadId::new())] {
        Box::pin(app.handle_event(
            &mut tui,
            &mut server,
            crate::app_event::AppEvent::OpenSidebarAgent {
                backend,
                source,
                agent: target,
            },
        ))
        .await?;
        assert_eq!(app.current_displayed_thread_id(), Some(root));
    }
    // A thread ID recovered from replay is a candidate, even if it was inherited from a fork.
    let inherited = ThreadId::new();
    assert!(app.register_sidebar_agent(root, inherited));
    Box::pin(app.handle_event(
        &mut tui,
        &mut server,
        crate::app_event::AppEvent::OpenSidebarAgent {
            backend,
            source: root,
            agent: inherited,
        },
    ))
    .await?;
    assert_eq!(app.current_displayed_thread_id(), Some(root));
    let pane = app.sidebar_mut();
    assert_eq!(
        pane.agents.pending_link.as_ref().map(|link| link.agent),
        Some(inherited)
    );
    pane.show(
        crate::app::sidebar::SidebarContent::Chooser,
        crate::app::sidebar::PaneFocus::Main,
    );
    assert!(pane.agents.pending_link.is_none());

    // Use the same activity cell as replay and click the link in the parent transcript.
    app.transcript_view.agent_links = Some(backend);
    app.insert_history_cell(&mut tui, Box::new(cell));
    let screen = side_frame(&mut app, &mut tui, Size::new(140, 20))?;
    let (row, column) = screen
        .lines()
        .enumerate()
        .find_map(|(row, line)| {
            let offset = line.find("/root/reviewer")?;
            Some((row as u16, line[..offset].chars().count() as u16))
        })
        .expect("visible subagent link");
    for kind in [
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::Up(MouseButton::Left),
    ] {
        let click = crate::app::owned_transcript::tests::mouse(kind, column, row);
        Box::pin(app.handle_tui_event(&mut tui, &mut server, click)).await?;
    }
    while let Ok(event) = rx.try_recv() {
        Box::pin(app.handle_event(&mut tui, &mut server, event)).await?;
    }
    assert_eq!(app.current_displayed_thread_id(), Some(root));
    Box::pin(app.handle_event(&mut tui, &mut server, discovery)).await?;
    assert_eq!(
        app.sidebar()
            .unwrap()
            .agents
            .pending_link
            .as_ref()
            .map(|link| link.agent),
        Some(child),
        "a click stays pending while the follow-up checks ancestry"
    );
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while app.current_displayed_thread_id() == Some(root) {
            let event = rx.recv().await.expect("app event channel");
            Box::pin(app.handle_event(&mut tui, &mut server, event)).await?;
        }
        Ok::<_, color_eyre::Report>(())
    })
    .await??;
    while let Ok(event) = rx.try_recv() {
        Box::pin(app.handle_event(&mut tui, &mut server, event)).await?;
    }
    assert_eq!(app.current_displayed_thread_id(), Some(child));
    assert_eq!(app.docked_agent(), Some((root, child)));
    let screen = side_frame(&mut app, &mut tui, Size::new(180, 20))?;
    assert!(app.sidebar_has_focus());
    insta::assert_snapshot!(
        "watch_only_subagent",
        crate::chatwidget::tests::helpers::normalize_agent_center_snapshot(
            screen.replace(&child.to_string(), "[agent id]")
        )
    );
    assert!(!screen.contains("direct input is disabled"), "{screen}");

    app.keymap.pager.jump_top = vec![crate::key_hint::plain(KeyCode::F(6))];
    Box::pin(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::F(6).into())))
        .await?;
    assert!(!app.transcript_view.is_following());
    Box::pin(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::End.into())))
        .await?;
    assert!(app.transcript_view.is_following());

    tui.screen_size_for_event(&TuiEvent::Resize(Size::new(180, 20)))?;
    app.begin_pane_resize(&mut tui)?;
    assert!(app.pane_resize.is_some());
    let resize = side_frame(&mut app, &mut tui, Size::new(180, 20))?;
    let footer = resize
        .lines()
        .find_map(|line| {
            line.split_once('│')
                .map(|(_, sidebar)| sidebar.trim())
                .filter(|row| row.starts_with("Resize"))
        })
        .expect("agent resize footer");
    insta::assert_snapshot!(footer, @"Resize · ←/h →/l · Shift+←/→ 10 cols · Enter/Esc done");
    Box::pin(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::Enter.into())))
        .await?;

    Box::pin(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::F(3).into())))
        .await?;
    let screen = side_frame(&mut app, &mut tui, Size::new(140, 20))?;
    assert!(screen.contains("Find:"), "{screen}");
    Box::pin(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::Esc.into())))
        .await?;
    assert_eq!(app.current_displayed_thread_id(), Some(child));
    Box::pin(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::Esc.into())))
        .await?;
    while let Ok(event) = rx.try_recv() {
        Box::pin(app.handle_event(&mut tui, &mut server, event)).await?;
    }
    assert_eq!(app.current_displayed_thread_id(), Some(root));
    assert_eq!(app.docked_agent(), None);
    let screen = side_frame(&mut app, &mut tui, Size::new(140, 20))?;
    assert!(screen.contains("Subagents  2"), "{screen}");
    let (row, column) = screen
        .lines()
        .enumerate()
        .find_map(|(row, line)| {
            let (main, sidebar) = line.split_once('│')?;
            let offset = sidebar.find("reviewer")?;
            Some((
                row as u16,
                (main.chars().count() + 1 + sidebar[..offset].chars().count()) as u16,
            ))
        })
        .expect("visible selectable row");
    assert!(
        !tui.terminal.previous_buffer()[(column, row)]
            .modifier
            .contains(ratatui::style::Modifier::DIM),
        "the name uses normal emphasis once the transcript is available"
    );
    let pane = app.sidebar().unwrap();
    assert!(
        !pane
            .chat_tabs()
            .ids
            .contains(&crate::sidebar_tab_strip::SidebarTab::Subagents)
    );
    assert_eq!(
        pane.chat_tabs().agent(pane.choice).map(|agent| agent.id),
        Some(child)
    );

    Box::pin(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::Enter.into())))
        .await?;
    assert_eq!(app.docked_agent(), Some((root, child)));
    while let Ok(event) = rx.try_recv() {
        Box::pin(app.handle_event(&mut tui, &mut server, event)).await?;
    }
    let cell = crate::multi_agents::sub_agent_activity_history_cell(
        &ThreadItemPayload::SubAgentActivity {
            id: "nested-started".into(),
            agent_thread_id: nested.to_string(),
            kind: SubAgentActivityKind::Started,
            agent_path: "/root/reviewer/helper".into(),
            model: None,
            reasoning_effort: None,
        },
    )
    .unwrap();
    app.insert_history_cell(&mut tui, Box::new(cell));
    Box::pin(app.select_agent_thread(&mut tui, &mut server, root)).await?;
    while let Ok(event) = rx.try_recv() {
        Box::pin(app.handle_event(&mut tui, &mut server, event)).await?;
    }
    let screen = side_frame(&mut app, &mut tui, Size::new(140, 20))?;
    let (row, column) = screen
        .lines()
        .enumerate()
        .find_map(|(row, line)| {
            let offset = line.find("/root/reviewer/helper")?;
            Some((row as u16, line[..offset].chars().count() as u16))
        })
        .expect("visible link in passive child");
    let click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::CONTROL,
    };
    assert_eq!(app.current_displayed_thread_id(), Some(root));
    assert_eq!(app.sidebar_agent_link_root(&tui, child), Some(root));
    assert_eq!(
        app.transcript_link_at(click),
        Some(crate::terminal_hyperlinks::ReaderLink::Agent(nested))
    );
    Box::pin(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Mouse(click))).await?;
    while let Ok(event) = rx.try_recv() {
        Box::pin(app.handle_event(&mut tui, &mut server, event)).await?;
    }
    assert_eq!(app.current_displayed_thread_id(), Some(nested));
    assert_eq!(app.docked_agent(), Some((root, nested)));
    let screen = side_frame(&mut app, &mut tui, Size::new(140, 20))?;
    assert!(screen.contains("Nested review result"), "{screen}");
    let close = app
        .conversation_frame
        .sidebar_targets
        .iter()
        .find_map(|(target, area)| {
            (*target == SidebarTarget::Tab(TabTarget::Close(SidebarTab::Subagents)))
                .then_some(*area)
        })
        .expect("docked agent close button");
    for kind in [
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::Up(MouseButton::Left),
    ] {
        let click = crate::app::owned_transcript::tests::mouse(kind, close.x, close.y);
        Box::pin(app.handle_tui_event(&mut tui, &mut server, click)).await?;
    }
    assert_eq!(app.current_displayed_thread_id(), Some(root));
    assert_eq!(app.docked_agent(), None);
    while let Ok(event) = rx.try_recv() {
        Box::pin(app.handle_event(&mut tui, &mut server, event)).await?;
    }
    assert!(!app.side_companions.contains_key(&child));

    // Closing a saved side chat can return to the last-used agent tab.
    app.open_sidebar_agent(&mut tui, &mut server, child).await?;
    while let Ok(event) = rx.try_recv() {
        Box::pin(app.handle_event(&mut tui, &mut server, event)).await?;
    }
    let started = server.start_thread(&config).await?;
    let side = started.session.thread_id;
    let channel = crate::app::ThreadEventChannel::new(/*capacity*/ 1);
    channel
        .store
        .lock()
        .await
        .set_session(started.session, started.turns);
    app.thread_event_channels.insert(side, channel);
    let mut state = crate::app::side::SideThreadState::new(root);
    state.storage = crate::app_server_session::SideChatStorage::Attached;
    app.side_threads.insert(side, state);
    app.activate_sidebar_tab(&mut tui, &mut server, SidebarTab::Chat(side))
        .await?;
    while let Ok(event) = rx.try_recv() {
        Box::pin(app.handle_event(&mut tui, &mut server, event)).await?;
    }
    assert_eq!(app.current_displayed_thread_id(), Some(side));
    app.close_sidebar_tab(&mut tui, &mut server, SidebarTab::Chat(side))
        .await?;
    assert_eq!(app.current_displayed_thread_id(), Some(child));
    assert!(app.side_threads[&side].close_request.is_some());
    while let Ok(event) = rx.try_recv() {
        Box::pin(app.handle_event(&mut tui, &mut server, event)).await?;
    }

    app.keymap.app.close_sidebar_tab = vec![crate::key_hint::plain(KeyCode::F(8))];
    Box::pin(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::F(8).into())))
        .await?;
    assert_eq!(app.current_displayed_thread_id(), Some(root));
    assert!(app.sidebar().unwrap().agents.detail.is_none());
    server.shutdown().await?;
    Ok(())
}
