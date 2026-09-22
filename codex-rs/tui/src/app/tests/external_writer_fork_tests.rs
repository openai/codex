//! Locked-thread fork shortcuts reuse the normal fork flow without taking the source lease.

use super::session_lifecycle_requests::recorded_params;
use super::session_lifecycle_requests::start_recording_app_server;
use super::*;
use app_test_support::create_fake_rollout;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn external_writer_fork_shortcut_respects_input_ownership() -> Result<()> {
    let (mut app, mut events, _operations) = make_test_app_with_channels().await;
    app.enqueue_primary_thread_session(
        test_thread_session(ThreadId::new(), app.config.cwd.to_path_buf()),
        Vec::new(),
    )
    .await?;
    let mut server = crate::start_embedded_app_server_for_picker(&app.config).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let fork_key = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE);
    app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(fork_key))
        .await?;
    // A navigation key flushes the composer's pending single-character paste burst.
    app.handle_tui_event(
        &mut tui,
        &mut server,
        TuiEvent::Key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)),
    )
    .await?;
    assert_eq!(app.chat_widget.composer_text_with_pending(), "f");
    assert!(
        !std::iter::from_fn(|| events.try_recv().ok())
            .any(|event| matches!(event, AppEvent::ForkCurrentSession { .. }))
    );

    app.chat_widget.show_external_writer_thread();
    for (key, expected) in [
        (fork_key, true),
        (KeyEvent::new(KeyCode::Char('F'), KeyModifiers::SHIFT), true),
        (KeyEvent::new(KeyCode::Char('F'), KeyModifiers::NONE), true),
        (KeyEvent::new(KeyCode::Char('f'), KeyModifiers::ALT), false),
        (
            KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL),
            false,
        ),
        (
            KeyEvent::new(KeyCode::Char('f'), KeyModifiers::SUPER),
            false,
        ),
        (
            KeyEvent::new_with_kind(KeyCode::Char('f'), KeyModifiers::NONE, KeyEventKind::Repeat),
            false,
        ),
        (
            KeyEvent::new_with_kind(
                KeyCode::Char('f'),
                KeyModifiers::NONE,
                KeyEventKind::Release,
            ),
            false,
        ),
    ] {
        while events.try_recv().is_ok() {}
        app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(key))
            .await?;
        let forks = std::iter::from_fn(|| events.try_recv().ok())
            .filter(|event| matches!(event, AppEvent::ForkCurrentSession { name: None }))
            .count();
        assert_eq!(forks, usize::from(expected), "{key:?}");
        assert_eq!(app.chat_widget.composer_text_with_pending(), "f");
        if expected {
            for code in [KeyCode::Char('f'), KeyCode::Char('r')] {
                app.handle_tui_event(
                    &mut tui,
                    &mut server,
                    TuiEvent::Key(KeyEvent::new(code, KeyModifiers::NONE)),
                )
                .await?;
            }
            assert!(events.try_recv().is_err());
            // The event handler clears the guard when it consumes the fork request.
            app.chat_widget.fork_in_progress = false;
        }
    }

    app.handle_tui_event(
        &mut tui,
        &mut server,
        TuiEvent::Key(KeyEvent::new(KeyCode::F(3), KeyModifiers::NONE)),
    )
    .await?;
    assert!(app.overlay.is_some());
    while events.try_recv().is_ok() {}
    app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(fork_key))
        .await?;
    assert!(
        !std::iter::from_fn(|| events.try_recv().ok())
            .any(|event| matches!(event, AppEvent::ForkCurrentSession { .. }))
    );

    app.overlay = None;
    app.chat_widget.show_selection_view(SelectionViewParams {
        items: vec![SelectionItem {
            name: "Keep this popup open".into(),
            ..Default::default()
        }],
        ..SelectionViewParams::picker()
    });
    app.handle_tui_event(&mut tui, &mut server, TuiEvent::Key(fork_key))
        .await?;
    assert!(
        !std::iter::from_fn(|| events.try_recv().ok())
            .any(|event| matches!(event, AppEvent::ForkCurrentSession { .. }))
    );
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn external_writer_fork_opens_editable_thread_without_taking_source_lease() -> Result<()> {
    let (mut app, mut events, _operations) = make_test_app_with_channels().await;
    let codex_home = tempdir()?;
    app.config.codex_home = codex_home.path().to_path_buf().abs();
    app.config.sqlite = codex_state::SqliteConfig::new_for_testing(codex_home.path().abs());
    let thread_id = ThreadId::from_string(
        &create_fake_rollout(
            codex_home.path(),
            "2026-01-01T00-00-00",
            "2026-01-01T00:00:00Z",
            "Saved user message",
            Some(app.config.model_provider_id.as_str()),
            /*git_info*/ None,
        )
        .expect("create source rollout"),
    )?;
    let mut owner = crate::start_embedded_app_server_for_picker(&app.config).await?;
    owner
        .resume_thread(
            &app.local_settings,
            app.config.clone(),
            thread_id,
            app.resume_model_settings(),
        )
        .await?;
    let (mut server, requests, proxy) = start_recording_app_server(
        &app.config,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
    )
    .await?;
    let (view, _notice) = server
        .read_thread_for_viewing(&app.config, &app.local_settings, thread_id)
        .await?;
    app.enqueue_primary_thread_session(view.session, view.turns)
        .await?;
    app.ensure_thread_channel(thread_id).mark_external_writer();
    app.chat_widget
        .set_queue_autosend_suppressed(/*suppressed*/ true);
    app.chat_widget.insert_str("Retained queued prompt");
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(
        app.chat_widget.queued_user_message_texts(),
        vec!["Retained queued prompt".to_string()]
    );
    app.chat_widget.insert_str("Retained draft");
    app.chat_widget.show_external_writer_thread();
    let retained_input = app.chat_widget.capture_thread_input_state();
    while events.try_recv().is_ok() {}
    requests.lock().expect("request recorder lock").clear();
    let mut tui = crate::tui::test_support::make_test_tui()?;

    app.handle_tui_event(
        &mut tui,
        &mut server,
        TuiEvent::Key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE)),
    )
    .await?;
    let event = events.try_recv()?;
    assert!(matches!(event, AppEvent::ForkCurrentSession { name: None }));
    Box::pin(app.handle_event(&mut tui, &mut server, event)).await?;

    assert_eq!(app.chat_widget.capture_thread_input_state(), retained_input);
    assert_ne!(app.chat_widget.thread_id(), Some(thread_id));
    assert!(!app.chat_widget.is_external_writer_view());
    assert!(!app.chat_widget.fork_in_progress);
    let messages = std::iter::from_fn(|| events.try_recv().ok())
        .filter_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => {
                let text = lines_to_single_string(&cell.display_lines(/*width*/ 100));
                text.contains("Fork created.").then_some(text)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    insta::assert_snapshot!("fork_completion", messages.join("\n"));
    assert_eq!(
        recorded_params(&requests, "thread/fork")
            .into_iter()
            .map(|params| params["threadId"].clone())
            .collect::<Vec<_>>(),
        vec![serde_json::json!(thread_id.to_string())]
    );
    for method in ["thread/resume", "turn/start", "turn/interrupt"] {
        assert!(recorded_params(&requests, method).is_empty(), "{method}");
    }
    app.handle_tui_event(
        &mut tui,
        &mut server,
        TuiEvent::Paste("Editable fork".into()),
    )
    .await?;
    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "Retained draftEditable fork"
    );
    let error = server
        .resume_thread(
            &app.local_settings,
            app.config.clone(),
            thread_id,
            app.resume_model_settings(),
        )
        .await
        .expect_err("source still has its original writer");
    assert!(crate::app_server_session::is_active_writer_error(&error));
    owner.shutdown().await?;
    server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn external_writer_fork_failure_keeps_the_locked_view_and_draft() -> Result<()> {
    let (mut app, mut events, _operations) = make_test_app_with_channels().await;
    let thread_id = ThreadId::new();
    app.chat_widget
        .handle_thread_session(test_thread_session(thread_id, app.config.cwd.to_path_buf()));
    app.chat_widget.insert_str("Retained draft");
    app.chat_widget.show_external_writer_thread();
    let mut server = crate::start_embedded_app_server_for_picker(&app.config).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    while events.try_recv().is_ok() {}
    app.handle_tui_event(
        &mut tui,
        &mut server,
        TuiEvent::Key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE)),
    )
    .await?;
    let event = events.try_recv()?;
    assert!(matches!(event, AppEvent::ForkCurrentSession { name: None }));
    Box::pin(app.handle_event(&mut tui, &mut server, event)).await?;
    assert_eq!(app.chat_widget.thread_id(), Some(thread_id));
    assert!(app.chat_widget.is_external_writer_view());
    assert!(!app.chat_widget.fork_in_progress);
    // The input drain can consume a resize; the next draw must sample the backend again.
    tui.terminal.last_known_screen_size = Size::new(/*width*/ 120, /*height*/ 40);
    assert_eq!(
        tui.screen_size_for_event(&TuiEvent::Draw)?,
        tui.terminal.size()?,
    );
    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "Retained draft"
    );
    let messages = std::iter::from_fn(|| events.try_recv().ok())
        .filter_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => {
                Some(lines_to_single_string(&cell.display_lines(/*width*/ 100)))
            }
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(messages.contains("Failed to fork current session through the app server:"));
    server.shutdown().await?;
    Ok(())
}
