//! Incoming events cannot use a colliding native thread ID to mutate another backend's state.

use super::*;
use crate::app_server_session::AppServerSessionEvent;
use crate::backend_id::BackendId;
use codex_app_server_client::AppServerEvent;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn foreign_backend_events_leave_thread_cache_and_connection_untouched() -> Result<()> {
    let (mut app, mut events, _) = make_test_app_with_channels().await;
    let (server, requests, proxy) =
        session_lifecycle_requests::start_recording_remote_app_server(&app.config).await?;
    let id = ThreadId::new();
    app.primary_thread_id = Some(id);
    app.active_thread_id = Some(id);
    set_test_initial_prompt(&mut app, String::new());
    app.chat_widget
        .handle_thread_session(test_thread_session(id, app.config.cwd.to_path_buf()));
    app.chat_widget
        .restore_user_message_to_composer("keep draft".into());
    app.ensure_thread_channel(id);
    while events.try_recv().is_ok() {}
    requests.lock().unwrap().clear();
    let input = app.chat_widget.capture_thread_input_state();
    let foreign = BackendId::new();
    let request = exec_approval_request(id, "foreign-turn", "command", /*approval_id*/ None);
    for event in [
        AppServerEvent::ServerNotification(Box::new(turn_started_notification(id, "foreign-turn"))),
        AppServerEvent::ServerRequest(Box::new(request.clone())),
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
    assert!(!app.reconnect.offline);
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
