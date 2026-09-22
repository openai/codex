//! Voice startup rejection coverage without an audio device.

use super::*;
use crate::app::tests::session_lifecycle_requests::recorded_params;
use crate::app::tests::session_lifecycle_requests::start_recording_remote_app_server;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn rejected_voice_start_does_not_leave_local_capture_active() -> Result<()> {
    for replay_only in [false, true] {
        let (mut app, mut events, _ops) = make_test_app_with_channels().await;
        let (mut app_server, requests, proxy) =
            start_recording_remote_app_server(&app.config).await?;
        let thread_id = ThreadId::new();
        app.chat_widget
            .handle_thread_session_quiet(test_thread_session(
                thread_id,
                app.config.cwd.to_path_buf(),
            ));
        crate::chatwidget::activate_voice_for_thread(&mut app.chat_widget, thread_id);
        assert!(app.chat_widget.may_receive_realtime_transcripts());
        if replay_only {
            app.app_server_target = AppServerTarget::Remote {
                endpoint: crate::resolve_remote_addr("ws://127.0.0.1:9")?,
            };
            app.active_thread_id = Some(thread_id);
            app.ensure_thread_channel(thread_id).mark_replay_only();
            assert!(app.thread_unavailable(thread_id));
        }
        let mut tui = crate::tui::test_support::make_test_tui()?;
        Box::pin(app.handle_event(
            &mut tui,
            &mut app_server,
            AppEvent::CodexOp(Op::RealtimeConversationStart {
                thread_id,
                offer_sdp: String::from("v=0\r\n").into(),
            }),
        ))
        .await?;

        assert!(!app.chat_widget.may_receive_realtime_transcripts());
        assert!(recorded_params(&requests, "thread/realtime/start").is_empty());
        let rendered = std::iter::from_fn(|| events.try_recv().ok())
            .filter_map(|event| match event {
                AppEvent::InsertHistoryCell(cell) => Some(
                    cell.display_lines(/*width*/ 80)
                        .into_iter()
                        .map(|line| line.to_string())
                        .collect::<Vec<_>>()
                        .join("\n"),
                ),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        if replay_only {
            insta::assert_snapshot!("rejected_voice_start_replay_only", rendered);
        } else {
            insta::assert_snapshot!("rejected_voice_start_no_active_thread", rendered);
        }
        app_server.shutdown().await?;
        proxy.await??;
    }
    Ok(())
}

#[tokio::test]
async fn rejected_voice_stop_clears_the_owned_session() -> Result<()> {
    for replay_only in [false, true] {
        let (mut app, _events, _ops) = make_test_app_with_channels().await;
        let (mut app_server, requests, proxy) =
            start_recording_remote_app_server(&app.config).await?;
        let thread_id = ThreadId::new();
        app.chat_widget
            .handle_thread_session_quiet(test_thread_session(
                thread_id,
                app.config.cwd.to_path_buf(),
            ));
        crate::chatwidget::activate_voice_for_thread(&mut app.chat_widget, thread_id);
        if replay_only {
            app.app_server_target = AppServerTarget::Remote {
                endpoint: crate::resolve_remote_addr("ws://127.0.0.1:9")?,
            };
            app.active_thread_id = Some(thread_id);
            app.ensure_thread_channel(thread_id).mark_replay_only();
        }
        let mut tui = crate::tui::test_support::make_test_tui()?;
        Box::pin(app.handle_event(
            &mut tui,
            &mut app_server,
            AppEvent::CodexOp(Op::RealtimeConversationStop { thread_id }),
        ))
        .await?;

        assert!(!app.chat_widget.may_receive_realtime_transcripts());
        assert!(recorded_params(&requests, "thread/realtime/stop").is_empty());
        app_server.shutdown().await?;
        proxy.await??;
    }
    Ok(())
}

#[tokio::test]
async fn background_voice_closes_with_its_owner_not_the_visible_thread() -> Result<()> {
    for owner_closed in [false, true] {
        let (mut app, _events, mut ops) = make_test_app_with_channels().await;
        let owner = ThreadId::new();
        let visible = ThreadId::new();
        crate::chatwidget::activate_voice_for_thread(&mut app.chat_widget, owner);
        let (replacement, _, _, _) =
            crate::chatwidget::tests::make_chatwidget_manual_with_sender().await;
        app.replace_chat_widget(replacement);
        app.chat_widget
            .handle_thread_session_quiet(test_thread_session(
                visible,
                app.config.cwd.to_path_buf(),
            ));
        app.active_thread_id = Some(visible);
        let closed = if owner_closed { owner } else { visible };
        app.enqueue_thread_notification(
            closed,
            ServerNotification::ThreadClosed(codex_app_server_protocol::ThreadClosedNotification {
                thread_id: closed.to_string(),
            }),
        )
        .await?;
        assert_eq!(
            app.voice_owner_thread_id(),
            (!owner_closed).then_some(owner)
        );
        assert!(ops.try_recv().is_err());
    }
    Ok(())
}

#[tokio::test]
async fn lifecycle_actions_stop_parked_voice_before_removing_owner() -> Result<()> {
    use crate::app_event::AgentsOverviewAction;
    for (action, slash_command) in [
        (AgentsOverviewAction::Archive, false),
        (AgentsOverviewAction::Delete, false),
        (AgentsOverviewAction::Archive, true),
        (AgentsOverviewAction::Delete, true),
    ] {
        let (mut app, _, _) = make_test_app_with_channels().await;
        let (mut server, requests, proxy) = start_recording_remote_app_server(&app.config).await?;
        let started = server.start_thread(&app.config).await?;
        let root = started.session.thread_id;
        let owner = if slash_command {
            ThreadId::from_string(
                &app_test_support::create_fake_parented_rollout_with_source(
                    &app.config.codex_home,
                    "2026-09-22T12-01-00",
                    "2026-09-22T12:01:00Z",
                    "Voice child",
                    Some(&app.config.model_provider_id),
                    /*git_info*/ None,
                    codex_protocol::protocol::SessionSource::SubAgent(
                        codex_protocol::protocol::SubAgentSource::ThreadSpawn {
                            parent_thread_id: root,
                            depth: 1,
                            agent_path: None,
                            agent_nickname: None,
                            agent_role: None,
                        },
                    ),
                    codex_protocol::SessionId::from(root),
                    root,
                )
                .expect("materialize child session"),
            )?
        } else {
            root
        };
        crate::chatwidget::activate_voice_for_thread(&mut app.chat_widget, owner);
        let (replacement, _, _, _) =
            crate::chatwidget::tests::make_chatwidget_manual_with_sender().await;
        app.replace_chat_widget(replacement);
        let mut tui = crate::tui::test_support::make_test_tui()?;
        tui.pause_events();
        if slash_command {
            Box::pin(server.resume_thread(
                &app.local_settings,
                app.config.clone(),
                owner,
                crate::app_server_session::ResumeModelSettings::PreserveExistingThread,
            ))
            .await?;
            app.active_thread_id = Some(root);
            app.app_server_target = AppServerTarget::Remote {
                endpoint: crate::resolve_remote_addr("ws://127.0.0.1:9")?,
            };
            match action {
                AgentsOverviewAction::Archive => {
                    Box::pin(app.archive_current_thread(&mut tui, &mut server)).await?;
                }
                AgentsOverviewAction::Delete => {
                    Box::pin(app.delete_current_thread(&mut tui, &mut server)).await?;
                }
            }
        } else {
            Box::pin(app.run_agents_overview_action(&mut tui, &mut server, owner, action)).await?;
        }
        assert_eq!(app.voice_owner_thread_id(), None);
        assert_eq!(
            recorded_params(&requests, "thread/realtime/stop"),
            vec![serde_json::json!({"threadId": owner.to_string()})]
        );
        server.shutdown().await?;
        proxy.await??;
    }
    Ok(())
}

#[tokio::test]
async fn voice_mute_shortcut_reaches_the_active_widget() -> Result<()> {
    let (mut app, mut events, _) = make_test_app_with_channels().await;
    crate::chatwidget::activate_voice_for_thread(&mut app.chat_widget, ThreadId::new());
    let mut server = crate::start_embedded_app_server_for_picker(&app.config).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.handle_tui_event(
        &mut tui,
        &mut server,
        tui::TuiEvent::Key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL)),
    )
    .await?;
    // The fixture has no microphone handle, so reaching mute reports that limitation.
    assert!(std::iter::from_fn(|| events.try_recv().ok()).any(|event| {
        matches!(event, AppEvent::InsertHistoryCell(cell) if cell.display_lines(/*width*/ 80)
            .iter().any(|line| line.to_string().contains("Start voice mode before muting")))
    }));
    server.shutdown().await?;
    Ok(())
}
