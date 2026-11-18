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
