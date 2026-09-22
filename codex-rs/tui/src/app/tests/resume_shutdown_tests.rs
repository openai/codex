//! A delayed close must not shut down a thread that has already been reloaded.

use super::disconnect::serve_reconnect_requests;
use super::*;
use crate::app_server_session::ThreadParamsMode;
use codex_app_server_client::AppServerEvent;
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio::net::TcpListener;

#[tokio::test]
async fn thread_close_exits_only_when_displayed_thread_is_unloaded() -> Result<()> {
    let (mut app, mut events, _commands) = make_test_app_with_channels().await;
    app.config.thread_unload_delay = Duration::ZERO;
    let mut server = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let started = server.start_thread(&app.config).await?;
    let thread_id = started.session.thread_id;
    app.enqueue_primary_thread_session(started.session, started.turns)
        .await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.drain_active_thread_events(&mut tui).await?;

    // The old runtime's notification can arrive after this thread ID is loaded again.
    app.handle_app_server_event(
        &server,
        AppServerEvent::ServerNotification(Box::new(thread_closed_notification(thread_id))),
    )
    .await;
    app.drain_active_thread_events(&mut tui).await?;
    assert!(
        !std::iter::from_fn(|| events.try_recv().ok())
            .any(|event| matches!(event, AppEvent::Exit(_)))
    );

    // A genuine close must still exit.
    server.thread_unsubscribe(thread_id).await?;
    let closed = tokio::time::timeout(Duration::from_secs(/*secs*/ 5), async {
        while let Some(event) = server.next_event().await {
            if matches!(&event,
                AppServerEvent::ServerNotification(notification)
                    if matches!(notification.as_ref(), ServerNotification::ThreadClosed(closed)
                        if closed.thread_id == thread_id.to_string()))
            {
                return event;
            }
        }
        panic!("server event stream ended before the thread closed");
    })
    .await?;
    app.handle_app_server_event(&server, closed).await;
    app.drain_active_thread_events(&mut tui).await?;
    assert!(
        std::iter::from_fn(|| events.try_recv().ok())
            .any(|event| matches!(event, AppEvent::Exit(ExitMode::Immediate)))
    );
    server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn failed_close_status_read_reconnects_remote_or_closes_embedded() -> Result<()> {
    for (failure, expected_reads, offline) in [
        ("disconnect", 1, true),
        ("timeout", 1, true),
        ("transient", 2, false),
        ("persistent", 2, true),
        ("embedded", 2, false),
        ("unsupported", 1, false),
    ] {
        let (mut app, mut events, _commands) = make_test_app_with_channels().await;
        let id = ThreadId::new();
        app.enqueue_primary_thread_session(
            test_thread_session(id, app.config.cwd.to_path_buf()),
            Vec::new(),
        )
        .await?;
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = crate::resolve_remote_addr(&format!("ws://{}", listener.local_addr()?))?;
        if failure != "embedded" {
            app.app_server_target = AppServerTarget::Remote {
                endpoint: endpoint.clone(),
            };
        }
        let reads = Arc::new(AtomicUsize::new(/*v*/ 0));
        let server_reads = reads.clone();
        let cwd = app.config.cwd.clone();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await?;
            serve_reconnect_requests(
                tokio_tungstenite::accept_async(stream).await?,
                move |request| {
                    assert_eq!(request.method, "thread/read");
                    let attempt = server_reads.fetch_add(/*val*/ 1, Ordering::SeqCst);
                    let response = match failure {
                        "disconnect" | "timeout" => None,
                        "unsupported" => {
                            Some(json!({"error": {"code": -32601, "message": "method not found"}}))
                        }
                        "transient" if attempt == 1 => Some(json!({"result": {"thread": {
                            "id": id, "sessionId": id, "preview": "", "ephemeral": false,
                            "modelProvider": "test-provider", "createdAt": 1, "updatedAt": 2,
                            "status": {"type": "idle"}, "cwd": cwd, "cliVersion": "0.0.0",
                            "source": "cli", "turns": []
                        }}})),
                        _ => {
                            Some(json!({"error": {"code": -32603, "message": "temporary failure"}}))
                        }
                    };
                    async move {
                        if failure == "timeout" {
                            std::future::pending::<()>().await;
                        }
                        response
                    }
                },
            )
            .await
        });
        let session = AppServerSession::new(
            crate::connect_remote_app_server(endpoint).await?,
            ThreadParamsMode::Remote,
        );
        tokio::time::timeout(
            Duration::from_secs(/*secs*/ 10),
            app.handle_app_server_event(
                &session,
                AppServerEvent::ServerNotification(Box::new(thread_closed_notification(id))),
            ),
        )
        .await?;
        let mut tui = crate::tui::test_support::make_test_tui()?;
        app.drain_active_thread_events(&mut tui).await?;
        assert_eq!(
            (reads.load(Ordering::SeqCst), app.reconnect.offline),
            (expected_reads, offline),
            "{failure}"
        );
        assert_eq!(
            std::iter::from_fn(|| events.try_recv().ok())
                .any(|event| matches!(event, AppEvent::Exit(_) | AppEvent::FatalExitRequest(_))),
            matches!(failure, "embedded" | "unsupported"),
            "{failure}"
        );
        server.abort();
        let _ = server.await;
        session.shutdown().await?;
    }
    Ok(())
}
