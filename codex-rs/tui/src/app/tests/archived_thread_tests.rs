#![cfg(test)]

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn archived_thread_notice_blocks_input_and_unarchives_with_draft() -> Result<()> {
    for recovery in [
        "unarchive",
        "already unarchived",
        "external unarchive",
        "archived writer",
    ] {
        let (mut app, _codex_home) = Box::pin(make_history_test_app()).await?;
        let (tx, mut events) = unbounded_channel();
        app.app_event_tx = AppEventSender::new(tx);
        let thread_id =
            create_history_rollout(&app.config, ThreadHistoryMode::Legacy, "Saved user message")?;
        let background_id =
            create_history_rollout(&app.config, ThreadHistoryMode::Legacy, "Background message")?;
        let mut app_server =
            Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
        let started = Box::pin(app_server.resume_thread(
            &app.local_settings,
            app.config.clone(),
            thread_id,
            crate::app_server_session::ResumeModelSettings::PreserveExistingThread,
        ))
        .await?;
        app.enqueue_primary_thread_session(started.session, started.turns)
            .await?;
        app.chat_widget.insert_str("Background draft");
        let background_input = app.chat_widget.capture_thread_input_state().unwrap();
        let channel = ThreadEventChannel::new_with_session(
            THREAD_EVENT_CHANNEL_CAPACITY,
            test_thread_session(background_id, app.config.cwd.to_path_buf()),
            Vec::new(),
        );
        channel.store.lock().await.input_state = Some(background_input.clone());
        app.thread_event_channels.insert(background_id, channel);
        app.chat_widget.apply_external_edit("Retained draft".into());
        while events.try_recv().is_ok() {}
        app_server.thread_archive(thread_id).await?;
        loop {
            let event =
                tokio::time::timeout(Duration::from_secs(/*secs*/ 5), app_server.next_event())
                    .await?
                    .expect("app-server event stream remains open");
            let archived = matches!(&event.event, AppServerEvent::ServerNotification(notification)
            if matches!(notification.as_ref(), ServerNotification::ThreadArchived(_)));
            app.handle_app_server_event(&app_server, event.event).await;
            if archived {
                break;
            }
        }
        assert!(app.chat_widget.is_archived_view());
        let mut tui = crate::tui::test_support::make_test_tui()?;
        for key in [
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
        ] {
            app.handle_key_event(&mut tui, &mut app_server, key).await;
        }
        app.chat_widget.handle_paste("unexpected paste".into());
        assert_eq!(
            app.chat_widget.composer_text_with_pending(),
            "Retained draft"
        );
        for width in [40, 100] {
            assert_snapshot!(
                format!("archived_thread_notice_{width}"),
                render_bottom_popup(&app.chat_widget, width)
            );
        }
        crate::legacy_core::config::set_project_trust_level(
            app.config.codex_home.as_path(),
            app.config.cwd.as_path(),
            codex_protocol::config_types::TrustLevel::Trusted,
        )
        .map_err(std::io::Error::other)?;
        app.harness_overrides.cwd = Some(app.config.cwd.to_path_buf());
        if recovery == "archived writer" {
            let writer = std::sync::Arc::new(codex_rollout::WriterLockCoordinator::new(
                app.config.codex_home.as_path(),
            ));
            let lock = writer.acquire(thread_id)?;
            let error = app_server.thread_unarchive(thread_id).await.unwrap_err();
            assert!(crate::app_server_session::is_active_writer_error(&error));
            crate::boxed_future!(app.handle_key_event(
                &mut tui,
                &mut app_server,
                KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE),
            ))
            .await;
            assert!(app.thread_event_channels[&thread_id].archived);
            assert!(app.chat_widget.is_archived_view());
            drop(lock);
        }
        if recovery != "unarchive" {
            app_server.thread_unarchive(thread_id).await?;
            if recovery == "already unarchived" {
                app_server
                    .resume_thread(
                        &app.local_settings,
                        app.config.clone(),
                        thread_id,
                        crate::app_server_session::ResumeModelSettings::PreserveExistingThread,
                    )
                    .await?;
            }
        }
        if matches!(recovery, "external unarchive" | "archived writer") {
            loop {
                let event =
                    tokio::time::timeout(Duration::from_secs(/*secs*/ 5), app_server.next_event())
                        .await?
                        .expect("app-server event stream remains open");
                let unarchived = matches!(&event.event, AppServerEvent::ServerNotification(notification)
                if matches!(notification.as_ref(), ServerNotification::ThreadUnarchived(_)));
                app.handle_app_server_event(&app_server, event.event).await;
                if unarchived {
                    break;
                }
            }
            let event = std::iter::from_fn(|| events.try_recv().ok())
                .find(|event| matches!(event, AppEvent::ResumeUnarchivedThread(id) if *id == thread_id))
                .expect("automatic recovery event");
            app.handle_event(&mut tui, &mut app_server, event).await?;
        } else {
            Box::pin(app.handle_key_event(
                &mut tui,
                &mut app_server,
                KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE),
            ))
            .await;
        }
        assert!(!app.chat_widget.is_read_only_view());
        assert_eq!(
            app.chat_widget.composer_text_with_pending(),
            "Retained draft"
        );
        Box::pin(app.handle_event(
            &mut tui,
            &mut app_server,
            AppEvent::ResumeSessionByIdOrName(background_id.to_string()),
        ))
        .await?;
        assert_eq!(
            app.chat_widget.capture_thread_input_state(),
            Some(background_input)
        );
    }
    Ok(())
}

#[tokio::test]
async fn archived_thread_notice_retains_draft_when_recovery_fails() -> Result<()> {
    let (mut app, _codex_home) = make_history_test_app().await?;
    let thread_id = ThreadId::new();
    app.enqueue_primary_thread_session(
        test_thread_session(thread_id, test_path_buf("/tmp/project")),
        Vec::new(),
    )
    .await?;
    app.chat_widget.insert_str("Retained draft");
    app.enqueue_thread_notification(
        thread_id,
        ServerNotification::ThreadArchived(ThreadArchivedNotification {
            thread_id: thread_id.to_string(),
        }),
    )
    .await?;
    let mut app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.handle_key_event(
        &mut tui,
        &mut app_server,
        KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE),
    )
    .await;
    assert!(app.chat_widget.is_archived_view());
    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "Retained draft"
    );
    app.enqueue_thread_notification(
        thread_id,
        ServerNotification::ThreadUnarchived(
            codex_app_server_protocol::ThreadUnarchivedNotification {
                thread_id: thread_id.to_string(),
            },
        ),
    )
    .await?;
    app.handle_event(
        &mut tui,
        &mut app_server,
        AppEvent::ResumeUnarchivedThread(thread_id),
    )
    .await?;
    assert!(app.chat_widget.is_archived_view());
    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "Retained draft"
    );
    Ok(())
}
