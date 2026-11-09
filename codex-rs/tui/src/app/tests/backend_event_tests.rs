//! Incoming events cannot use a colliding native thread ID to mutate another backend's state.

use super::*;
use crate::app_server_session::AppServerSessionEvent;
use crate::backend_id::BackendId;
use codex_app_server_client::AppServerEvent;
use codex_app_server_protocol::AccountRateLimitsUpdatedNotification;
use codex_app_server_protocol::AccountUpdatedNotification;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn foreign_backend_events_leave_thread_cache_and_connection_untouched() -> Result<()> {
    let (mut app, mut events, _) = make_test_app_with_channels().await;
    let (mut server, requests, proxy) =
        session_lifecycle_requests::start_recording_remote_app_server(&app.config).await?;
    let id = ThreadId::new();
    app.primary_thread_id = Some(id);
    set_test_initial_prompt(&mut app, String::new());
    app.chat_widget
        .handle_thread_session(test_thread_session(id, app.config.cwd.to_path_buf()));
    app.chat_widget
        .restore_user_message_to_composer("keep draft".into());
    app.ensure_thread_channel(id);
    app.activate_thread_channel(id).await;
    while events.try_recv().is_ok() {}
    requests.lock().unwrap().clear();
    let input = app.chat_widget.capture_thread_input_state();
    let hard_stop_generation = app.rate_limit_hard_stop_generation;
    let foreign = BackendId::new();
    let request = exec_approval_request(id, "foreign-turn", "command", /*approval_id*/ None);
    let loss: codex_app_server_protocol::ErrorNotification =
        serde_json::from_value(serde_json::json!({
            "threadId": id.to_string(), "turnId": "", "willRetry": false,
            "error": { "message": "Cloud stream ended. Reopen the conversation to reconnect." }
        }))?;
    for event in [
        AppServerEvent::ServerNotification(Box::new(turn_started_notification(id, "foreign-turn"))),
        AppServerEvent::ServerRequest(Box::new(request.clone())),
        AppServerEvent::ThreadStreamDisconnected(Box::new(loss.clone())),
        AppServerEvent::Lagged { skipped: 1 },
        AppServerEvent::Disconnected {
            message: "foreign disconnected".into(),
        },
    ] {
        app.handle_backend_event(
            &server,
            AppServerSessionEvent {
                backend_id: foreign,
                event,
            },
        )
        .await;
    }
    for event in [
        AppServerEvent::Lagged { skipped: 1 },
        AppServerEvent::Disconnected {
            message: "background disconnected".into(),
        },
        AppServerEvent::ServerNotification(Box::new(ServerNotification::AccountUpdated(
            AccountUpdatedNotification {
                auth_mode: None,
                plan_type: None,
            },
        ))),
        AppServerEvent::ServerNotification(Box::new(ServerNotification::AccountRateLimitsUpdated(
            AccountRateLimitsUpdatedNotification {
                rate_limits: serde_json::from_value(
                    serde_json::json!({ "spendControlReached": true }),
                )?,
            },
        ))),
    ] {
        app.handle_polled_backend_event(
            &server,
            foreign,
            AppServerSessionEvent {
                backend_id: server.backend_id(),
                event,
            },
        )
        .await;
    }
    assert!(!app.reconnect.offline);
    assert_eq!(app.rate_limit_hard_stop_generation, hard_stop_generation);
    assert!(
        !app.pending_app_server_requests
            .contains_server_request(&request)
    );
    assert_eq!(app.chat_widget.capture_thread_input_state(), input);
    assert!(requests.lock().unwrap().is_empty());
    assert!(events.try_recv().is_err());
    {
        let store = app.thread_event_channels[&id].store.lock().await;
        assert_eq!(store.active_turn_id(), None);
        assert!(store.pending_replay_requests().is_empty());
    }
    app.handle_backend_event(
        &server,
        AppServerSessionEvent {
            backend_id: server.backend_id(),
            event: AppServerEvent::ServerNotification(Box::new(turn_started_notification(
                id,
                "owned-turn",
            ))),
        },
    )
    .await;
    assert_eq!(
        app.thread_event_channels[&id]
            .store
            .lock()
            .await
            .active_turn_id(),
        Some("owned-turn")
    );
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.drain_active_thread_events(&mut tui).await?;
    app.handle_backend_event(
        &server,
        AppServerSessionEvent {
            backend_id: server.backend_id(),
            event: AppServerEvent::ThreadStreamDisconnected(Box::new(loss)),
        },
    )
    .await;
    app.drain_active_thread_events(&mut tui).await?;
    let history =
        super::active_reconnect::drain_history(&mut app, &mut tui, &mut server, &mut events)
            .await?;
    insta::assert_snapshot!(history, @"■ Cloud stream ended. Reopen the conversation to reconnect.");
    server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn transport_events_retain_the_session_backend() -> Result<()> {
    let app = make_test_app().await;
    let (mut server, _, proxy) =
        session_lifecycle_requests::start_recording_remote_app_server(&app.config).await?;
    let started = server.start_thread(&app.config).await?;
    server
        .thread_settings_update(codex_app_server_protocol::ThreadSettingsUpdateParams {
            thread_id: started.session.thread_id.to_string(),
            permissions: Some(":read-only".into()),
            ..Default::default()
        })
        .await?;
    let event = tokio::time::timeout(
        std::time::Duration::from_secs(/*secs*/ 5),
        server.next_event(),
    )
    .await?
    .expect("settings update emits an event");
    assert_eq!(event.backend_id, server.backend_id());
    server.shutdown().await?;
    proxy.await??;
    Ok(())
}
