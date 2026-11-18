#![cfg(test)]

use super::*;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ThreadClosedNotification;
use codex_app_server_protocol::ThreadStatusChangedNotification;
use codex_app_server_protocol::TurnCompletedNotification;
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
    observe(
        &completed_item(ThreadItemPayload::CollabAgentToolCall {
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
                    status: CollabAgentStatus::Running,
                    message: None,
                },
            )]),
        }),
        &mut overview,
    );
    assert_eq!(overview.states, HashMap::from([(id, State::Completed)]));
    observe(
        &thread_status(ThreadStatus::Active {
            active_flags: Vec::new(),
        }),
        &mut overview,
    );
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
async fn active_discovery_supersedes_restored_completion() -> color_eyre::Result<()> {
    use crate::app::owned_transcript::tests::attach_thread;
    use crate::app::test_support::enable_side_retention;
    use crate::app::tests::make_test_app_with_channels;

    let (mut app, _events, _ops) = Box::pin(make_test_app_with_channels()).await;
    enable_side_retention(&mut app)?;
    let config = app.chat_widget.config_ref();
    let mut server = Box::pin(crate::start_embedded_app_server_for_picker(config)).await?;
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
    let outcomes = [child, completed]
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
    app.restore_sidebar_agent_outcomes(
        root,
        &[crate::app::tests::test_turn(
            "saved-turn",
            TurnStatus::Completed,
            outcomes,
        )],
        super::super::OutcomeReplay::Reconnect,
    );
    for id in [child, completed] {
        app.upsert_agent_picker_thread(
            id,
            Some("worker".into()),
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
            .map(|row| (row.id, row.status))
            .collect::<Vec<_>>()
    };
    assert!(rows(&app).iter().all(|(_, state)| *state == "Completed"));
    let request = app
        .agent_navigation
        .begin_picker_refresh(root)
        .expect("start discovery");
    // A completion arriving while the refresh is in flight wins over the active snapshot.
    app.observe_sidebar_agent_event(&ServerNotification::TurnCompleted(
        TurnCompletedNotification {
            thread_id: completed.to_string(),
            turn: crate::app::tests::test_turn("fresh-turn", TurnStatus::Completed, Vec::new()),
        },
    ));
    app.apply_agent_picker_thread_refresh(
        &server,
        root,
        request,
        Ok(crate::app_event::AgentPickerThreadRefresh {
            threads: vec![thread, completed_thread, failed_thread],
            archived_thread_ids: HashSet::new(),
        }),
    );
    assert_eq!(
        rows(&app),
        vec![
            (child, "Running"),
            (completed, "Completed"),
            (failed, "Failed")
        ]
    );
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn opens_agent_transcript_beside_parent_and_returns_to_chooser() -> color_eyre::Result<()> {
    use crate::app::side_focus::SidebarTarget;
    use crate::app::sidebar::PaneFocus;
    use crate::app::sidebar::SidebarContent;
    use crate::app::test_support::enable_side_retention;
    use crate::app::test_support::side_frame;
    use crate::app::tests::make_test_app_with_channels;
    use crate::sidebar_tab_strip::SidebarTab;
    use crate::sidebar_tab_strip::TabTarget;
    use crate::tui::TuiEvent;
    use codex_protocol::protocol::SessionSource;
    use codex_protocol::protocol::SubAgentSource;
    use crossterm::event::KeyCode;
    use crossterm::event::MouseButton;
    use crossterm::event::MouseEventKind;
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
    let child = ThreadId::from_string(
        &app_test_support::create_fake_parented_rollout_with_source(
            config.codex_home.as_path(),
            "2025-01-05T12-00-01",
            "2025-01-05T12:00:01Z",
            "Review result",
            Some(config.model_provider_id.as_str()),
            /*git_info*/ None,
            SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id: root,
                depth: 1,
                agent_path: Some("/root/reviewer".parse().expect("agent path")),
                agent_nickname: Some("Reviewer".into()),
                agent_role: None,
            }),
            root.into(),
            root,
        )
        .expect("seed test rollout"),
    )?;
    let path = app_test_support::rollout_path(
        config.codex_home.as_path(),
        "2025-01-05T12-00-01",
        &child.to_string(),
    );
    let rollout = std::fs::read_to_string(&path)?;
    let (metadata, messages) = rollout.split_once('\n').expect("seeded child history");
    let mut metadata: serde_json::Value = serde_json::from_str(metadata)?;
    metadata["payload"]["multi_agent_version"] =
        serde_json::to_value(codex_protocol::protocol::MultiAgentVersion::V2)?;
    std::fs::write(path, format!("{metadata}\n{messages}"))?;

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
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    // Wait for the real app-server to discover the saved child before opening its row.
    let discovery = tokio::time::timeout(std::time::Duration::from_secs(5), async {
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
    Box::pin(app.handle_event(&mut tui, &mut server, discovery)).await?;
    while let Ok(event) = rx.try_recv() {
        Box::pin(app.handle_event(&mut tui, &mut server, event)).await?;
    }
    app.sidebar_mut()
        .show(SidebarContent::Chooser, PaneFocus::Sidebar);
    let screen = side_frame(&mut app, &mut tui, Size::new(140, 20))?;
    assert!(screen.contains("Subagents  1"), "{screen}");
    let pane = app.sidebar_mut();
    pane.choice = pane
        .chat_tabs()
        .agent_index(child)
        .expect("child agent row");
    Box::pin(app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(KeyCode::Enter.into())))
        .await?;
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
    assert!(screen.contains("Subagents  1"), "{screen}");
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
    side_frame(&mut app, &mut tui, Size::new(140, 20))?;
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
