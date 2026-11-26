#![cfg(test)]

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn docked_replay_finishes_while_skills_refresh_is_pending() -> Result<()> {
    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    let parent = ThreadId::new();
    let side = ThreadId::new();
    app.side_threads
        .insert(side, side::SideThreadState::new(parent));
    enable_side_retention(&mut app)?;
    let _ = app.config.features.disable(Feature::Plugins);
    app.chat_widget.sync_plugin_mentions_config(&app.config);
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    let mut session = test_thread_session(parent, app.config.cwd.to_path_buf());
    session.cwd = codex_utils_path_uri::PathUri::parse("file:///D:/Workspace")?;
    app.chat_widget.handle_thread_session(session);
    let cwd = codex_utils_path_uri::PathUri::parse("file:///D:/workspace")?;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = crate::resolve_remote_addr(&format!("ws://{}", listener.local_addr()?))?;
    let (pending_tx, mut pending_rx) = tokio::sync::mpsc::unbounded_channel();
    let proxy = tokio::spawn(async move {
        let (stream, _) = listener.accept().await?;
        let socket = tokio_tungstenite::accept_async(stream).await?;
        disconnect::serve_reconnect_requests(socket, |request| {
            assert_eq!(request.method, "skills/list");
            let (release, wait) = tokio::sync::oneshot::channel();
            pending_tx
                .send((request.params, release))
                .expect("test is running");
            async move {
                wait.await.expect("release the pending skills refresh");
                Some(serde_json::json!({"result": {"data": []}}))
            }
        })
        .await
    });
    let mut app_server = AppServerSession::new(
        crate::backend_id::BackendId::new(),
        crate::connect_remote_app_server(endpoint).await?,
        crate::app_server_session::ThreadParamsMode::Embedded,
    );
    let deadline = std::time::Duration::from_secs(/*secs*/ 5);
    while events.try_recv().is_ok() {}

    for thread_id in [side, parent] {
        app.active_thread_id = Some(thread_id);
        app.pending_thread_switch_resets = 1;
        app.app_event_tx
            .send(AppEvent::CodexOp(AppCommand::list_skills(
                cwd.clone(),
                /*force_reload*/ true,
            )));
        app.app_event_tx
            .send(AppEvent::SideReplayFinished(thread_id));

        time::timeout(
            deadline,
            drain_app_events(&mut app, &mut tui, &mut app_server, &mut events),
        )
        .await??;
        let (params, release) = time::timeout(deadline, pending_rx.recv())
            .await?
            .expect("skills/list should be requested");
        assert_eq!(
            params,
            Some(serde_json::json!({"cwds": [r"D:\workspace"], "forceReload": true}))
        );
        assert!(!app.awaiting_side_replay(&tui));

        release
            .send(())
            .expect("skills/list should still be pending");
        let event = time::timeout(deadline, events.recv())
            .await?
            .expect("receive the skills refresh");
        assert!(matches!(
            &event,
            AppEvent::SideReplaySkillsLoaded { thread_id: loaded_thread_id, result: Ok(_), .. }
                if *loaded_thread_id == thread_id
        ));
        boxed_future!(app.handle_event(&mut tui, &mut app_server, event)).await?;
        assert_eq!(app_server.pending_side_skills_refresh, None);
    }

    app_server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn sidebar_reopens_saved_side_chat_after_restart() -> Result<()> {
    boxed_future!(reopen_saved_side_chat(DuringOpen::Navigate)).await
}

#[tokio::test]
async fn closed_saved_chat_keeps_its_live_read_receipt_during_install() -> Result<()> {
    boxed_future!(reopen_saved_side_chat(DuringOpen::Close)).await
}

#[tokio::test]
async fn saved_chat_replays_approval_after_a_stale_runtime_close() -> Result<()> {
    boxed_future!(reopen_saved_side_chat(DuringOpen::Approval)).await
}

#[tokio::test]
async fn fork_releases_a_provisional_saved_chat_on_its_original_connection() -> Result<()> {
    boxed_future!(reopen_saved_side_chat(DuringOpen::Fork)).await
}

#[tokio::test]
async fn saved_chat_retry_and_supersession_with_pending_requests() -> Result<()> {
    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    let parent = ThreadId::new();
    let children = std::array::from_fn::<_, 4, _>(|_| ThreadId::new());
    app.chat_widget
        .handle_thread_session(test_thread_session(parent, app.config.cwd.to_path_buf()));
    app.chat_widget.replay_thread_turns(
        vec![test_turn("parent-turn", TurnStatus::Completed, Vec::new())],
        crate::chatwidget::ReplayKind::ResumeInitialMessages,
    );
    while events.try_recv().is_ok() {}
    app.active_thread_id = Some(parent);
    app.primary_thread_id = Some(parent);
    app.sidebar_mut().show(
        sidebar::SidebarContent::Chooser,
        sidebar::PaneFocus::Sidebar,
    );
    app.sidebar_mut()
        .update_saved_chats(children.map(Into::into).into());
    enable_side_retention(&mut app)?;
    app.local_settings.tui.animations = false;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = crate::resolve_remote_addr(&format!("ws://{}", listener.local_addr()?))?;
    let (requests, mut held) = tokio::sync::mpsc::unbounded_channel();
    let proxy = tokio::spawn(async move {
        let (stream, _) = listener.accept().await?;
        disconnect::serve_reconnect_requests(tokio_tungstenite::accept_async(stream).await?, |request| {
            assert_eq!(request.method, "thread/read");
            let (release, wait) = tokio::sync::oneshot::channel();
            requests.send((request.params.unwrap()["threadId"].clone(), release)).unwrap();
            async move {
                wait.await.expect("release the read");
                Some(serde_json::json!({"error":{"code":-32000,"message":"thread not loaded: offline"}}))
            }
        }).await
    });
    let mut server = AppServerSession::new(
        crate::backend_id::BackendId::new(),
        crate::connect_remote_app_server(endpoint).await?,
        crate::app_server_session::ThreadParamsMode::Remote,
    );
    let backend_id = server.backend_id();
    let target = |child| sidebar_loading::Target {
        backend_id,
        parent,
        child,
    };
    // The first two reads can run. Only the most recent extra destination runs afterwards.
    app.open_saved_chat(&mut tui, &mut server, target(children[0]))
        .await?;
    let mut first = Some(held.recv().await.expect("first held read"));
    for child in children {
        app.open_saved_chat(&mut tui, &mut server, target(child))
            .await?;
    }
    let size = Size::new(/*width*/ 208, /*height*/ 16);
    let sidebar_rows = |frame: String| {
        frame
            .lines()
            .filter_map(|line| line.split_once('│'))
            .map(|(_, sidebar)| sidebar.trim_end())
            .filter(|row| !row.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    };
    app.sidebar_mut().choice = 4;
    let waiting = sidebar_rows(side_frame(&mut app, &mut tui, size)?);
    let [first_child, second_child, _, latest] = children;
    for (index, expected) in [first_child, second_child, latest, latest, latest, latest]
        .into_iter()
        .enumerate()
    {
        let (id, release) = match first.take() {
            Some(first) => first,
            None => held.recv().await.expect("held request"),
        };
        assert_eq!(id, serde_json::json!(expected));
        if index == 3 {
            // A handoff revokes the intent even if the destination has the same native IDs.
            app.saved_openings.abandon();
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            app.app_event_tx = AppEventSender::new(tx);
            events = rx;
        } else if index == 4 {
            // Switching away and back cannot resurrect the original parent's intent.
            app.save_active_thread_state().await;
        } else if index == 5 {
            // An archive from another client wins over a read that is already in flight.
            boxed_future!(app.handle_event(
                &mut tui,
                &mut server,
                AppEvent::SideChatArchived {
                    backend_id,
                    target: expected,
                }
            ))
            .await?;
            assert!(
                app.sidebar()
                    .unwrap()
                    .chat_tabs()
                    .choice_index(expected)
                    .is_none()
            );
        }
        release.send(()).unwrap();
        loop {
            let event = app
                .saved_openings
                .next_event(&mut events)
                .await
                .expect("preparation result");
            let finished = matches!(&event, AppEvent::SavedChatPrepared { target, .. } if target.child == expected);
            boxed_future!(app.handle_event(&mut tui, &mut server, event)).await?;
            if finished {
                break;
            }
        }
        if index == 0 {
            let event = app
                .saved_openings
                .next_event(&mut events)
                .await
                .expect("queued destination");
            assert!(matches!(event, AppEvent::StartQueuedSavedChat));
            boxed_future!(app.handle_event(&mut tui, &mut server, event)).await?;
        }
        if index == 2 {
            let retry = sidebar_rows(side_frame(&mut app, &mut tui, size)?);
            insta::assert_snapshot!(
                "waiting_and_retry_rows",
                format!("Waiting\n{waiting}\n\nRetry\n{retry}")
            );
            app.sidebar_mut().choice = 4;
            boxed_future!(app.handle_tui_event(
                &mut tui,
                &mut server,
                TuiEvent::Key(KeyCode::Enter.into())
            ))
            .await?;
        }
        if matches!(index, 3 | 4) {
            assert!(app.sidebar().unwrap().opening.is_none());
            app.open_saved_chat(&mut tui, &mut server, target(expected))
                .await?;
        }
        assert_eq!(app.current_displayed_thread_id(), Some(parent));
    }
    assert!(app.sidebar().unwrap().opening.is_none());
    assert!(app.sidebar().unwrap().chat_tabs().ids.is_empty());
    server.shutdown().await?;
    proxy.await??;
    Ok(())
}
