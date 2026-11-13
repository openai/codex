//! Real session streams remain independent through polling, closure and replacement.

use super::*;
use crate::app::backend_sessions::BackendSessions;
use crate::app_server_session::ThreadParamsMode;
use crate::backend_id::BackendId;
use codex_app_server_client::AppServerClient;
use codex_app_server_client::AppServerEvent;
use codex_app_server_client::CloudClient;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadSettingsUpdateResponse;
use futures::FutureExt;
use pretty_assertions::assert_eq;

fn approval_request(thread_id: ThreadId, request_id: i64) -> ServerRequest {
    ServerRequest::FileChangeRequestApproval {
        request_id: AppServerRequestId::Integer(request_id),
        params: FileChangeRequestApprovalParams {
            thread_id: thread_id.to_string(),
            turn_id: "turn-1".to_string(),
            item_id: "item-1".to_string(),
            started_at_ms: 0,
            reason: None,
            grant_root: None,
        },
    }
}

#[tokio::test]
async fn backend_sessions_parked_request_does_not_hide_disconnect() -> Result<()> {
    let [
        (displayed, _displayed_events),
        (background, background_events),
    ] = std::array::from_fn(|_| {
        let (client, _, events) = CloudClient::channel(/*capacity*/ 1);
        (
            AppServerSession::new(
                BackendId::new(),
                AppServerClient::Cloud(client),
                ThreadParamsMode::Cloud,
            ),
            events,
        )
    });
    let background_id = background.backend_id();
    let displayed_id = displayed.backend_id();
    let mut backends = BackendSessions::new(displayed);
    assert_eq!(backends.next_listening_backend(displayed_id), None);
    backends.insert(background);
    backends.set_preference_key(background_id, "remote.work");
    let initial = backends.displayed;
    let thread_id = ThreadId::new();

    // A startup failure that arrives before a fork is registered stays on its owning backend.
    let mut app = make_test_app().await;
    app.sidebar_mut().creation = Some(sidebar::SideCreation {
        owner: background_id,
        state: sidebar::CreationState::Waiting {
            request: uuid::Uuid::new_v4(),
            started: Instant::now(),
        },
        message: None,
    });
    let failure = ServerNotification::McpServerStatusUpdated(
        codex_app_server_protocol::McpServerStatusUpdatedNotification {
            environment_id: None,
            thread_id: Some(thread_id.to_string()),
            name: "tools".into(),
            status: McpServerStartupState::Failed,
            error: Some("startup failed".into()),
            failure_reason: None,
        },
    );
    let event = crate::app_server_session::AppServerSessionEvent {
        backend_id: background_id,
        event: AppServerEvent::ServerNotification(Box::new(failure.clone())),
    };
    assert!(app.defer_side_startup_notification(&event, displayed_id));
    backends.defer_side_startup(event);
    backends.replay_ready_side_startup(&app);
    assert!(
        backends
            .next_event(/*paused_backend*/ None)
            .now_or_never()
            .is_none()
    );
    backends.displayed = backends.slot(background_id).unwrap();
    backends.replay_ready_side_startup(&app);
    assert!(
        backends
            .next_event(/*paused_backend*/ None)
            .now_or_never()
            .is_none()
    );
    app.ensure_thread_channel(thread_id);
    backends.replay_ready_side_startup(&app);
    let (owner, event) = backends.next_event(/*paused_backend*/ None).await;
    assert_eq!(owner, background_id);
    let AppServerEvent::ServerNotification(notification) = event.unwrap().event else {
        panic!("expected the parked MCP startup failure");
    };
    assert_eq!(
        serde_json::to_value(notification)?,
        serde_json::to_value(failure)?
    );
    backends.displayed = initial;

    background_events
        .send(AppServerEvent::ServerRequest(Box::new(approval_request(
            thread_id, /*request_id*/ 1,
        ))))
        .unwrap();
    assert!(
        backends
            .next_event(/*paused_backend*/ None)
            .now_or_never()
            .is_none()
    );
    backends.displayed = backends.slot(background_id).unwrap();
    let (owner, parked) = backends.next_event(/*paused_backend*/ None).await;
    assert_eq!(owner, background_id);
    assert!(matches!(
        parked.unwrap().event,
        AppServerEvent::ServerRequest(_)
    ));

    backends.displayed = initial;
    background_events
        .send(AppServerEvent::ServerRequest(Box::new(approval_request(
            thread_id, /*request_id*/ 2,
        ))))
        .unwrap();
    background_events
        .send(AppServerEvent::ServerRequest(Box::new(approval_request(
            thread_id, /*request_id*/ 3,
        ))))
        .unwrap();
    assert!(
        backends
            .next_event(/*paused_backend*/ None)
            .now_or_never()
            .is_none()
    );
    assert!(
        backends
            .next_event(/*paused_backend*/ None)
            .now_or_never()
            .is_none()
    );
    background_events
        .send(AppServerEvent::ServerNotification(Box::new(
            ServerNotification::ServerRequestResolved(
                codex_app_server_protocol::ServerRequestResolvedNotification {
                    thread_id: thread_id.to_string(),
                    request_id: AppServerRequestId::Integer(2),
                },
            ),
        )))
        .unwrap();
    let (owner, resolved) = backends.next_event(/*paused_backend*/ None).await;
    assert_eq!(owner, background_id);
    assert!(matches!(
        resolved.unwrap().event,
        AppServerEvent::ServerNotification(notification)
            if matches!(
                notification.as_ref(),
                ServerNotification::ServerRequestResolved(resolved)
                    if resolved.request_id == AppServerRequestId::Integer(2)
            )
    ));
    backends.displayed = backends.slot(background_id).unwrap();
    let (owner, unresolved) = backends.next_event(/*paused_backend*/ None).await;
    assert_eq!(owner, background_id);
    assert!(matches!(
        unresolved.unwrap().event,
        AppServerEvent::ServerRequest(request)
            if request.id() == &AppServerRequestId::Integer(3)
    ));
    assert!(
        backends
            .next_event(/*paused_backend*/ None)
            .now_or_never()
            .is_none()
    );

    backends.displayed = initial;
    background_events
        .send(AppServerEvent::ServerRequest(Box::new(approval_request(
            thread_id, /*request_id*/ 4,
        ))))
        .unwrap();
    assert!(
        backends
            .next_event(/*paused_backend*/ None)
            .now_or_never()
            .is_none()
    );
    background_events
        .send(AppServerEvent::Disconnected {
            message: "stream ended".to_string(),
        })
        .unwrap();
    drop(background_events);
    let (owner, disconnected) = backends.next_event(/*paused_backend*/ None).await;
    assert_eq!(owner, background_id);
    assert!(matches!(
        disconnected.unwrap().event,
        AppServerEvent::Disconnected { .. }
    ));
    backends.displayed = backends.slot(background_id).unwrap();
    let (owner, closed) = backends.next_event(/*paused_backend*/ None).await;
    assert_eq!(owner, background_id);
    assert!(closed.is_none());
    assert!(!backends.is_listening(background_id));
    assert_eq!(
        backends.preferred_listening_backend(Some("remote.work")),
        Some(displayed_id)
    );
    Ok(())
}

#[tokio::test]
async fn backend_sessions_poll_three_sources_and_isolate_closed_or_paused_streams() -> Result<()> {
    let mut app = make_test_app().await;
    app.config.model_reasoning_effort = Some(ReasoningEffortConfig::Low);
    let mut sessions = Vec::new();
    let mut proxies = Vec::new();
    let mut owners = Vec::new();
    let mut threads = Vec::new();
    for _ in 0..3 {
        let (mut session, _, proxy) =
            session_lifecycle_requests::start_recording_remote_app_server(&app.config).await?;
        let thread = session.start_thread(&app.config).await?.session.thread_id;
        assert!(
            session
                .thread_settings_update(ThreadSettingsUpdateParams {
                    thread_id: thread.to_string(),
                    effort: Some(ReasoningEffortConfig::High),
                    ..Default::default()
                })
                .await?
        );
        owners.push(session.backend_id());
        threads.push(thread);
        sessions.push(session);
        proxies.push(proxy);
    }
    let mut sessions = sessions.into_iter();
    let mut backends = BackendSessions::from_initial(sessions.next().unwrap(), sessions.collect());
    backends.set_preference_key(owners[2], "remote.work");
    backends.set_preference_key(owners[0], "local");
    backends.set_preference_key(owners[1], "cloud");
    assert_eq!(
        backends.preferred_listening_backend(Some("remote.work")),
        Some(owners[2])
    );
    assert_eq!(
        backends.preferred_listening_backend(Some("cloud")),
        Some(owners[1])
    );
    assert_eq!(
        backends.preferred_listening_backend(Some("missing")),
        Some(owners[0])
    );

    assert_eq!(
        owners
            .iter()
            .map(|owner| backends.next_listening_backend(*owner))
            .collect::<Vec<_>>(),
        [Some(owners[1]), Some(owners[2]), Some(owners[0])]
    );
    let mut unpaused = HashSet::new();
    for _ in 0..2 {
        let (owner, event) =
            time::timeout(Duration::from_secs(5), backends.next_event(Some(owners[0]))).await?;
        assert_ne!(owner, owners[0]);
        assert_eq!(event.unwrap().backend_id, owner);
        unpaused.insert(owner);
    }
    assert_eq!(unpaused, HashSet::from([owners[1], owners[2]]));
    let (owner, event) = time::timeout(
        Duration::from_secs(5),
        backends.next_event(/*paused_backend*/ None),
    )
    .await?;
    assert_eq!(owner, owners[0]);
    assert_eq!(event.unwrap().backend_id, owner);
    // Cancel a pending poll, then close only the initial transport.
    assert!(
        backends
            .next_event(/*paused_backend*/ None)
            .now_or_never()
            .is_none()
    );
    proxies[0].abort();
    loop {
        let (owner, event) = time::timeout(
            Duration::from_secs(5),
            backends.next_event(/*paused_backend*/ None),
        )
        .await?;
        assert_eq!(owner, owners[0]);
        if event.is_none() {
            break;
        }
    }
    assert!(!backends.is_listening(owners[0]));
    assert_eq!(backends.next_listening_backend(owners[2]), Some(owners[1]));
    assert_eq!(backends.next_listening_backend(owners[0]), Some(owners[1]));
    for index in 1..3 {
        assert!(backends.is_listening(owners[index]));
        let _: ThreadSettingsUpdateResponse = backends
            .session(owners[index])
            .unwrap()
            .request_handle()
            .request_typed(ClientRequest::ThreadSettingsUpdate {
                request_id: RequestId::Integer(900),
                params: ThreadSettingsUpdateParams {
                    thread_id: threads[index].to_string(),
                    effort: Some(ReasoningEffortConfig::Low),
                    ..Default::default()
                },
            })
            .await?;
    }
    let mut surviving = HashSet::new();
    for _ in 0..2 {
        let (owner, event) = time::timeout(
            Duration::from_secs(5),
            backends.next_event(/*paused_backend*/ None),
        )
        .await?;
        assert_eq!(event.unwrap().backend_id, owner);
        surviving.insert(owner);
    }
    assert_eq!(surviving, HashSet::from([owners[1], owners[2]]));
    // EOF is reported once, and a later cancelled poll does not spin on the closed source.
    assert!(
        backends
            .next_event(/*paused_backend*/ None)
            .now_or_never()
            .is_none()
    );

    let replacement = crate::start_app_server_for_picker(
        owners[0],
        &app.config,
        &AppServerTarget::Embedded,
        Vec::new(),
        LoaderOverrides::without_managed_config_for_tests(),
        /*state_db*/ None,
        Arc::clone(&app.environment_manager),
    )
    .await?;
    let reconnected_slot = backends.slot(owners[0]).unwrap();
    let previous = std::mem::replace(backends.session_at_mut(reconnected_slot), replacement);
    let _ = previous.shutdown().await;
    backends.resume_events(owners[0]);
    let thread = backends
        .session_at_mut(reconnected_slot)
        .start_thread(&app.config)
        .await?
        .session
        .thread_id;
    loop {
        let (owner, event) = time::timeout(
            Duration::from_secs(5),
            backends.next_event(/*paused_backend*/ None),
        )
        .await?;
        assert_eq!(owner, owners[0]);
        if let codex_app_server_client::AppServerEvent::ServerNotification(notification) =
            event.unwrap().event
            && let ServerNotification::ThreadStarted(started) = *notification
        {
            assert_eq!(started.thread.id, thread.to_string());
            break;
        }
    }
    backends.shutdown().await?;
    for (index, proxy) in proxies.into_iter().enumerate() {
        if index == 0 {
            assert!(proxy.await.unwrap_err().is_cancelled());
        } else {
            proxy.await??;
        }
    }
    Ok(())
}
