use super::*;
use codex_protocol::ThreadId;
use codex_protocol::protocol::EventMsg;
use color_eyre::eyre::Result;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;
use std::path::PathBuf;

#[tokio::test]
async fn start_btw_forks_switches_and_esc_returns_to_parent() -> Result<()> {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let mut tui = make_test_tui();

    let parent_thread_id =
        setup_btw_parent_thread(&mut app, Some("what have you explored so far?")).await?;
    let child_thread_id = start_btw_thread(&mut app, &mut tui, parent_thread_id).await?;
    assert_ne!(child_thread_id, parent_thread_id);
    assert_eq!(app.active_btw_parent_thread_id(), Some(parent_thread_id));
    assert_eq!(app.agent_navigation.get(&child_thread_id), None);
    let child_channel = app
        .thread_event_channels
        .get(&child_thread_id)
        .expect("child thread channel");
    let child_store = child_channel.store.lock().await;
    let session_configured = child_store
        .session_configured
        .as_ref()
        .expect("child session configured");
    let EventMsg::SessionConfigured(session_configured) = &session_configured.msg else {
        panic!(
            "expected SessionConfigured, got {:?}",
            session_configured.msg
        );
    };
    assert!(
        session_configured
            .initial_messages
            .as_ref()
            .is_some_and(|messages| {
                messages.iter().any(|message| {
                    matches!(
                        message,
                        EventMsg::UserMessage(ev) if ev.message == "what have you explored so far?"
                    )
                })
            }),
        "expected child session to replay forked history into initial_messages"
    );
    drop(child_store);

    let mut rendered_cells = Vec::new();
    let found_user_prompt = loop {
        match app_event_rx.try_recv() {
            Ok(AppEvent::InsertHistoryCell(cell)) => {
                let rendered = cell
                    .display_lines(80)
                    .into_iter()
                    .map(|line| line.to_string())
                    .collect::<Vec<_>>()
                    .join("\n");
                let contains_user_prompt = rendered.contains("explore the codebase");
                rendered_cells.push(rendered);
                if contains_user_prompt {
                    break true;
                }
            }
            Ok(_) => continue,
            Err(_) => break false,
        }
    };
    assert!(
        found_user_prompt,
        "expected BTW user prompt cell, got {rendered_cells:?}"
    );
    let forked_idx = rendered_cells
        .iter()
        .position(|rendered| rendered.contains("Thread forked from"));
    let user_prompt_idx = rendered_cells
        .iter()
        .position(|rendered| rendered.contains("explore the codebase"));
    assert!(
        matches!((forked_idx, user_prompt_idx), (Some(forked), Some(prompt)) if forked < prompt),
        "expected fork banner before BTW prompt, got {rendered_cells:?}"
    );

    app.handle_key_event(&mut tui, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
        .await;
    assert_eq!(app.active_thread_id, Some(parent_thread_id));
    assert_eq!(app.active_btw_parent_thread_id(), None);
    assert!(!app.thread_event_channels.contains_key(&child_thread_id));
    Ok(())
}

#[tokio::test]
async fn start_btw_ctrl_c_returns_to_parent() -> Result<()> {
    let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let mut tui = make_test_tui();

    let parent_thread_id = setup_btw_parent_thread(&mut app, None).await?;
    let child_thread_id = start_btw_thread(&mut app, &mut tui, parent_thread_id).await?;
    assert_ne!(child_thread_id, parent_thread_id);

    app.handle_key_event(
        &mut tui,
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
    )
    .await;

    assert_eq!(app.active_thread_id, Some(parent_thread_id));
    assert_eq!(app.active_btw_parent_thread_id(), None);
    assert!(!app.thread_event_channels.contains_key(&child_thread_id));
    Ok(())
}

#[tokio::test]
async fn start_btw_uses_displayed_rollout_path_for_replay_only_parent() -> Result<()> {
    let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let mut tui = make_test_tui();

    let parent_thread_id =
        setup_btw_parent_thread(&mut app, Some("what have you explored so far?")).await?;
    let sibling = app.server.start_thread(app.config.clone()).await?;
    let sibling_thread_id = sibling.thread_id;
    app.handle_thread_created(sibling_thread_id).await?;
    app.select_agent_thread(&mut tui, sibling_thread_id).await?;

    app.server.remove_thread(&parent_thread_id).await;
    app.select_agent_thread(&mut tui, parent_thread_id).await?;

    let parent_rollout_path = app
        .chat_widget
        .rollout_path()
        .expect("displayed parent rollout path");
    assert!(app.server.get_thread(parent_thread_id).await.is_err());
    assert!(parent_rollout_path.exists());

    let child_thread_id = start_btw_thread(&mut app, &mut tui, parent_thread_id).await?;

    assert_ne!(child_thread_id, parent_thread_id);
    assert_eq!(app.active_btw_parent_thread_id(), Some(parent_thread_id));
    Ok(())
}

#[tokio::test]
async fn idle_main_thread_ctrl_c_requests_shutdown_exit() -> Result<()> {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let mut tui = make_test_tui();

    let parent_thread_id = setup_btw_parent_thread(&mut app, None).await?;
    while app_event_rx.try_recv().is_ok() {}

    app.handle_key_event(
        &mut tui,
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
    )
    .await;

    let mut exit_mode = None;
    while let Ok(app_event) = app_event_rx.try_recv() {
        if let AppEvent::Exit(mode) = app_event {
            exit_mode = Some(mode);
            break;
        }
    }
    assert_eq!(app.active_thread_id, Some(parent_thread_id));
    assert_eq!(app.active_btw_parent_thread_id(), None);
    assert_eq!(exit_mode, Some(ExitMode::ShutdownFirst));
    Ok(())
}

#[tokio::test]
async fn ctrl_c_after_returning_from_btw_requests_shutdown_exit() -> Result<()> {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let mut tui = make_test_tui();

    let parent_thread_id = setup_btw_parent_thread(&mut app, None).await?;
    let child_thread_id = start_btw_thread(&mut app, &mut tui, parent_thread_id).await?;
    while app_event_rx.try_recv().is_ok() {}

    app.handle_key_event(
        &mut tui,
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
    )
    .await;

    assert_eq!(app.active_thread_id, Some(parent_thread_id));
    assert_eq!(app.active_btw_parent_thread_id(), None);
    assert!(!app.thread_event_channels.contains_key(&child_thread_id));
    while app_event_rx.try_recv().is_ok() {}

    app.handle_key_event(
        &mut tui,
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
    )
    .await;

    let mut exit_mode = None;
    while let Ok(app_event) = app_event_rx.try_recv() {
        if let AppEvent::Exit(mode) = app_event {
            exit_mode = Some(mode);
            break;
        }
    }
    assert_eq!(exit_mode, Some(ExitMode::ShutdownFirst));
    Ok(())
}

#[tokio::test]
async fn nested_btw_preserves_parent_chain_and_esc_returns_one_level() -> Result<()> {
    let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let mut tui = make_test_tui();

    let parent_thread_id = setup_btw_parent_thread(&mut app, None).await?;
    let child_thread_id = start_btw_thread(&mut app, &mut tui, parent_thread_id).await?;
    let grandchild_thread_id = start_btw_thread(&mut app, &mut tui, child_thread_id).await?;

    assert_eq!(app.active_thread_id, Some(grandchild_thread_id));
    assert_eq!(
        app.btw_threads
            .get(&grandchild_thread_id)
            .map(|state| state.parent_thread_id),
        Some(child_thread_id)
    );
    assert!(app.thread_event_channels.contains_key(&child_thread_id));
    assert!(
        app.thread_event_channels
            .contains_key(&grandchild_thread_id)
    );

    app.handle_key_event(&mut tui, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
        .await;
    assert_eq!(app.active_thread_id, Some(child_thread_id));
    assert_eq!(app.active_btw_parent_thread_id(), Some(parent_thread_id));
    assert!(app.thread_event_channels.contains_key(&child_thread_id));
    assert!(
        !app.thread_event_channels
            .contains_key(&grandchild_thread_id)
    );
    Ok(())
}

#[tokio::test]
async fn switching_away_from_nested_btw_discards_full_hidden_chain() -> Result<()> {
    let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let mut tui = make_test_tui();

    let parent_thread_id = setup_btw_parent_thread(&mut app, None).await?;
    let child_thread_id = start_btw_thread(&mut app, &mut tui, parent_thread_id).await?;
    let grandchild_thread_id = start_btw_thread(&mut app, &mut tui, child_thread_id).await?;

    app.select_agent_thread(&mut tui, parent_thread_id).await?;

    assert_eq!(app.active_thread_id, Some(parent_thread_id));
    assert_eq!(app.active_btw_parent_thread_id(), None);
    assert!(!app.thread_event_channels.contains_key(&child_thread_id));
    assert!(
        !app.thread_event_channels
            .contains_key(&grandchild_thread_id)
    );
    assert!(app.server.get_thread(child_thread_id).await.is_err());
    assert!(app.server.get_thread(grandchild_thread_id).await.is_err());
    Ok(())
}

#[tokio::test]
async fn switching_away_from_nested_btw_clears_hidden_pending_approvals() -> Result<()> {
    let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let mut tui = make_test_tui();

    let parent_thread_id = setup_btw_parent_thread(&mut app, None).await?;
    let child_thread_id = start_btw_thread(&mut app, &mut tui, parent_thread_id).await?;
    let _grandchild_thread_id = start_btw_thread(&mut app, &mut tui, child_thread_id).await?;

    {
        let child_channel = app
            .thread_event_channels
            .get(&child_thread_id)
            .expect("child thread channel");
        let mut store = child_channel.store.lock().await;
        store.push_event(Event {
            id: "ev-approval".to_string(),
            msg: EventMsg::ExecApprovalRequest(
                codex_protocol::protocol::ExecApprovalRequestEvent {
                    call_id: "call-approval".to_string(),
                    approval_id: None,
                    turn_id: "turn-approval".to_string(),
                    command: vec!["echo".to_string(), "hi".to_string()],
                    cwd: PathBuf::from("/tmp"),
                    reason: None,
                    network_approval_context: None,
                    proposed_execpolicy_amendment: None,
                    proposed_network_policy_amendments: None,
                    additional_permissions: None,
                    skill_metadata: None,
                    available_decisions: None,
                    parsed_cmd: Vec::new(),
                },
            ),
        });
    }

    app.refresh_pending_thread_approvals().await;
    assert_eq!(
        app.chat_widget.pending_thread_approvals(),
        &[app.thread_label(child_thread_id)]
    );

    app.select_agent_thread(&mut tui, parent_thread_id).await?;

    assert!(app.chat_widget.pending_thread_approvals().is_empty());
    Ok(())
}

#[tokio::test]
async fn open_agent_picker_keeps_active_btw_thread_until_selection() -> Result<()> {
    let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let mut tui = make_test_tui();

    let parent_thread_id = setup_btw_parent_thread(&mut app, None).await?;
    let child_thread_id = start_btw_thread(&mut app, &mut tui, parent_thread_id).await?;

    let control = app
        .handle_event(&mut tui, AppEvent::OpenAgentPicker)
        .await?;
    assert!(matches!(control, AppRunControl::Continue));
    assert_eq!(app.active_thread_id, Some(child_thread_id));
    assert_eq!(app.active_btw_parent_thread_id(), Some(parent_thread_id));
    assert!(app.thread_event_channels.contains_key(&child_thread_id));
    assert_eq!(app.agent_navigation.get(&child_thread_id), None);

    let control = app
        .handle_event(&mut tui, AppEvent::SelectAgentThread(parent_thread_id))
        .await?;
    assert!(matches!(control, AppRunControl::Continue));
    assert_eq!(app.active_thread_id, Some(parent_thread_id));
    assert_eq!(app.active_btw_parent_thread_id(), None);
    assert!(!app.thread_event_channels.contains_key(&child_thread_id));
    Ok(())
}

#[tokio::test]
async fn failed_switch_from_btw_keeps_current_thread_and_parent_chain() -> Result<()> {
    let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let mut tui = make_test_tui();

    let parent_thread_id = setup_btw_parent_thread(&mut app, None).await?;
    let child_thread_id = start_btw_thread(&mut app, &mut tui, parent_thread_id).await?;
    let missing_thread_id = ThreadId::new();

    app.select_agent_thread(&mut tui, missing_thread_id).await?;

    assert_eq!(app.active_thread_id, Some(child_thread_id));
    assert_eq!(app.active_btw_parent_thread_id(), Some(parent_thread_id));
    assert!(app.thread_event_channels.contains_key(&child_thread_id));
    Ok(())
}

#[tokio::test]
async fn fork_current_session_discards_active_btw_chain() -> Result<()> {
    let (mut app, _app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let mut tui = make_test_tui();

    let parent_thread_id = setup_btw_parent_thread(&mut app, None).await?;
    let child_thread_id = start_btw_thread(&mut app, &mut tui, parent_thread_id).await?;
    let child_rollout_path = app
        .server
        .get_thread(child_thread_id)
        .await?
        .rollout_path()
        .expect("BTW child rollout path");
    assert!(
        child_rollout_path.exists(),
        "expected BTW child rollout path"
    );

    let control = app
        .handle_event(&mut tui, AppEvent::ForkCurrentSession)
        .await?;
    assert!(matches!(control, AppRunControl::Continue));

    assert_eq!(app.active_btw_parent_thread_id(), None);
    assert!(app.btw_threads.is_empty());
    assert!(app.server.get_thread(parent_thread_id).await.is_err());
    assert!(app.server.get_thread(child_thread_id).await.is_err());
    Ok(())
}

async fn setup_btw_parent_thread(app: &mut App, parent_message: Option<&str>) -> Result<ThreadId> {
    let parent = app.server.start_thread(app.config.clone()).await?;
    let parent_thread_id = parent.thread_id;
    let parent_rollout_path = parent
        .session_configured
        .rollout_path
        .clone()
        .expect("thread rollout path");
    if let Some(parent_dir) = parent_rollout_path.parent() {
        std::fs::create_dir_all(parent_dir)?;
    }

    let mut rollout_lines = vec![serde_json::to_string(
        &codex_protocol::protocol::RolloutLine {
            timestamp: "2026-03-12T00:00:00.000Z".to_string(),
            item: codex_protocol::protocol::RolloutItem::SessionMeta(
                codex_protocol::protocol::SessionMetaLine {
                    meta: codex_protocol::protocol::SessionMeta {
                        id: parent_thread_id,
                        forked_from_id: None,
                        timestamp: "2026-03-12T00:00:00.000Z".to_string(),
                        cwd: app.config.cwd.clone(),
                        originator: "test".to_string(),
                        cli_version: env!("CARGO_PKG_VERSION").to_string(),
                        source: SessionSource::Cli,
                        agent_nickname: None,
                        agent_role: None,
                        model_provider: Some(app.config.model_provider_id.clone()),
                        base_instructions: None,
                        dynamic_tools: None,
                        memory_mode: None,
                    },
                    git: None,
                },
            ),
        },
    )?];
    if let Some(parent_message) = parent_message {
        rollout_lines.push(serde_json::to_string(
            &codex_protocol::protocol::RolloutLine {
                timestamp: "2026-03-12T00:00:01.000Z".to_string(),
                item: codex_protocol::protocol::RolloutItem::EventMsg(EventMsg::UserMessage(
                    codex_protocol::protocol::UserMessageEvent {
                        message: parent_message.to_string(),
                        images: None,
                        local_images: Vec::new(),
                        text_elements: Vec::new(),
                    },
                )),
            },
        )?);
    }
    std::fs::write(
        &parent_rollout_path,
        format!("{}\n", rollout_lines.join("\n")),
    )?;

    app.handle_thread_created(parent_thread_id).await?;
    app.primary_thread_id = Some(parent_thread_id);
    app.activate_thread_channel(parent_thread_id).await;
    Ok(parent_thread_id)
}

async fn start_btw_thread(
    app: &mut App,
    tui: &mut crate::tui::Tui,
    parent_thread_id: ThreadId,
) -> Result<ThreadId> {
    let control = app
        .handle_event(
            tui,
            AppEvent::StartBtw {
                parent_thread_id,
                user_message: "explore the codebase".into(),
            },
        )
        .await?;
    assert!(matches!(control, AppRunControl::Continue));
    Ok(app
        .active_thread_id
        .expect("BTW child should be active after start"))
}
