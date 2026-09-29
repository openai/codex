//! Regression coverage for overlapping starts, replacement attempts, and thread-scoped results.
use super::*;
use codex_app_server_protocol::McpServerOauthLoginCompletedNotification;
use codex_app_server_protocol::McpServerOauthLoginResponse;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn overlapping_mcp_login_preserves_first_start() {
    let (mut app, mut events, _op_rx) = make_test_app_with_channels().await;
    while events.try_recv().is_ok() {}
    let mut server = crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
        .await
        .unwrap();
    let mut tui = crate::tui::test_support::make_test_tui().unwrap();
    let thread_id = ThreadId::new();
    app.pending_mcp_login_start = Some(PendingMcpLoginStart {
        request_id: "first".into(),
        name: "enterprise".into(),
        thread_id,
        completions: Vec::new(),
    });
    app.handle_event(
        &mut tui,
        &mut server,
        AppEvent::StartMcpLogin {
            name: "typo".into(),
            thread_id,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        app.pending_mcp_login_start.as_ref().unwrap().request_id,
        "first"
    );
    assert_snapshot!("overlapping_mcp_login", next_history_message(&mut events));
    app.handle_event(
        &mut tui,
        &mut server,
        AppEvent::McpLoginStarted {
            request_id: "first".into(),
            result: Err("first start failed".into()),
        },
    )
    .await
    .unwrap();
    let snapshot = app.thread_event_channels[&thread_id]
        .store
        .lock()
        .await
        .snapshot();
    for event in snapshot.events {
        app.handle_thread_event_replay(event);
    }
    assert!(next_history_message(&mut events).contains("first start failed"));
    assert!(app.pending_mcp_login_start.is_none());
    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn mcp_login_retry_suppresses_old_completion_and_late_browser_open() {
    let (mut app, mut events, _op_rx) = make_test_app_with_channels().await;
    while events.try_recv().is_ok() {}
    let mut server = crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
        .await
        .unwrap();
    let mut tui = crate::tui::test_support::make_test_tui().unwrap();
    let thread_id = ThreadId::new();
    app.primary_thread_id = Some(thread_id);
    app.active_thread_id = Some(thread_id);
    app.active_mcp_login_ids
        .insert("enterprise".into(), "old".into());
    app.active_mcp_login_ids
        .insert("other".into(), "other-login".into());
    app.pending_mcp_login_start = Some(PendingMcpLoginStart {
        request_id: "retry".into(),
        name: "enterprise".into(),
        thread_id,
        completions: Vec::new(),
    });
    for (login_id, success) in [("old", false), ("new", true)] {
        app.handle_app_server_event(
            &server,
            codex_app_server_client::AppServerEvent::ServerNotification(Box::new(
                ServerNotification::McpServerOauthLoginCompleted(
                    McpServerOauthLoginCompletedNotification {
                        name: "enterprise".into(),
                        thread_id: Some(thread_id.to_string()),
                        login_id: Some(login_id.into()),
                        success,
                        error: (!success).then(|| "Enterprise sign-in failed.".into()),
                    },
                ),
            )),
        )
        .await;
    }
    assert!(events.try_recv().is_err());
    app.handle_event(
        &mut tui,
        &mut server,
        AppEvent::McpLoginStarted {
            request_id: "retry".into(),
            result: Ok(McpServerOauthLoginResponse {
                authorization_url: String::new(),
                login_id: Some("new".into()),
            }),
        },
    )
    .await
    .unwrap();
    let snapshot = app.thread_event_channels[&thread_id]
        .store
        .lock()
        .await
        .snapshot();
    assert_eq!(snapshot.events.len(), 1);
    for event in snapshot.events {
        app.handle_thread_event_replay(event);
    }
    assert_snapshot!(
        "mcp_login_retry_completion",
        next_history_message(&mut events)
    );
    assert!(events.try_recv().is_err());
    // A cancellation delivered after the replacement response is stale as well.
    app.handle_app_server_event(
        &server,
        codex_app_server_client::AppServerEvent::ServerNotification(Box::new(
            ServerNotification::McpServerOauthLoginCompleted(
                McpServerOauthLoginCompletedNotification {
                    name: "enterprise".into(),
                    thread_id: Some(thread_id.to_string()),
                    login_id: Some("old".into()),
                    success: false,
                    error: Some("Enterprise sign-in failed.".into()),
                },
            ),
        )),
    )
    .await;
    assert_eq!(
        app.thread_event_channels[&thread_id]
            .store
            .lock()
            .await
            .snapshot()
            .events
            .len(),
        1
    );
    // Retrying one server must not discard a different server's outstanding OAuth result.
    app.handle_app_server_event(
        &server,
        codex_app_server_client::AppServerEvent::ServerNotification(Box::new(
            ServerNotification::McpServerOauthLoginCompleted(
                McpServerOauthLoginCompletedNotification {
                    name: "other".into(),
                    thread_id: Some(thread_id.to_string()),
                    login_id: Some("other-login".into()),
                    success: true,
                    error: None,
                },
            ),
        )),
    )
    .await;
    let snapshot = app.thread_event_channels[&thread_id]
        .store
        .lock()
        .await
        .snapshot();
    assert_eq!(snapshot.events.len(), 2);
    app.handle_thread_event_replay(snapshot.events.last().unwrap().clone());
    assert!(next_history_message(&mut events).contains("Signed in to other."));
    server.shutdown().await.unwrap();
}

#[tokio::test]
async fn mcp_login_results_survive_thread_switch_and_session_refresh() {
    for success in [true, false] {
        let (mut app, mut events, _op_rx) = make_test_app_with_channels().await;
        while events.try_recv().is_ok() {}
        let server = crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
            .await
            .unwrap();
        let origin = ThreadId::new();
        app.primary_thread_id = Some(origin);
        app.active_thread_id = Some(ThreadId::new());
        app.active_mcp_login_ids
            .insert("enterprise".into(), "login".into());
        app.handle_app_server_event(
            &server,
            codex_app_server_client::AppServerEvent::ServerNotification(Box::new(
                ServerNotification::McpServerOauthLoginCompleted(
                    McpServerOauthLoginCompletedNotification {
                        name: "enterprise".into(),
                        thread_id: Some(origin.to_string()),
                        login_id: Some("login".into()),
                        success,
                        error: (!success).then(|| "Enterprise sign-in failed.".into()),
                    },
                ),
            )),
        )
        .await;
        assert!(events.try_recv().is_err());
        let snapshot = {
            let mut store = app.thread_event_channels[&origin].store.lock().await;
            store.rebase_buffer_after_session_refresh();
            store.snapshot()
        };
        assert_eq!(snapshot.events.len(), 1);
        app.active_thread_id = Some(origin);
        for event in snapshot.events {
            app.handle_thread_event_replay(event);
        }
        assert_snapshot!(
            format!("mcp_login_switched_thread_{success}"),
            next_history_message(&mut events)
        );
        server.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn rejected_mcp_login_retry_preserves_previous_result() {
    let (mut app, mut events, _op_rx) = make_test_app_with_channels().await;
    while events.try_recv().is_ok() {}
    let mut server = crate::start_embedded_app_server_for_picker(app.chat_widget.config_ref())
        .await
        .unwrap();
    let mut tui = crate::tui::test_support::make_test_tui().unwrap();
    let thread_id = ThreadId::new();
    app.primary_thread_id = Some(thread_id);
    app.active_thread_id = Some(thread_id);
    app.active_mcp_login_ids
        .insert("enterprise".into(), "old".into());
    app.pending_mcp_login_start = Some(PendingMcpLoginStart {
        request_id: "typo".into(),
        name: "typo".into(),
        thread_id,
        completions: Vec::new(),
    });
    app.handle_app_server_event(
        &server,
        codex_app_server_client::AppServerEvent::ServerNotification(Box::new(
            ServerNotification::McpServerOauthLoginCompleted(
                McpServerOauthLoginCompletedNotification {
                    name: "enterprise".into(),
                    thread_id: Some(thread_id.to_string()),
                    login_id: Some("old".into()),
                    success: true,
                    error: None,
                },
            ),
        )),
    )
    .await;
    // The other server's result is already retained while this start is still pending.
    assert_eq!(
        app.thread_event_channels[&thread_id]
            .store
            .lock()
            .await
            .snapshot()
            .events
            .len(),
        1
    );
    app.handle_event(
        &mut tui,
        &mut server,
        AppEvent::McpLoginStarted {
            request_id: "typo".into(),
            result: Err("Unknown MCP server typo".into()),
        },
    )
    .await
    .unwrap();
    let snapshot = app.thread_event_channels[&thread_id]
        .store
        .lock()
        .await
        .snapshot();
    assert_eq!(snapshot.events.len(), 2);
    for event in snapshot.events {
        app.handle_thread_event_replay(event);
    }
    let messages = std::iter::from_fn(|| events.try_recv().ok())
        .filter_map(|event| {
            if let AppEvent::InsertHistoryCell(cell) = event {
                Some(lines_to_single_string(&cell.display_lines(/*width*/ 80)))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_snapshot!("rejected_mcp_login_retry", messages.join("\n"));
    server.shutdown().await.unwrap();
}
