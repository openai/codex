//! Demand-driven discovery, cursor retry, and visible Show more behavior.

use super::*;
use crate::app_server_session::ThreadParamsMode;
use codex_app_server_protocol::JSONRPCMessage;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

#[tokio::test]
async fn overview_show_more_refills_archived_rows_and_retries_without_losing_rows() -> Result<()> {
    check_discovery(/*mixed_sources*/ false).await?;
    check_discovery(/*mixed_sources*/ true).await
}

async fn check_discovery(mixed_sources: bool) -> Result<()> {
    let mut app = make_test_app().await;
    let rows: Vec<_> = (0..24)
        .map(|index| {
            let mut thread = overview_thread(
                ThreadId::new(),
                /*parent_thread_id*/ None,
                &format!("Task {index}"),
                ThreadStatus::NotLoaded,
            );
            if mixed_sources && (index == 0 || index >= 21) {
                thread.source = SessionSource::Exec;
            }
            thread.updated_at += 24 - index;
            thread.recency_at = Some(thread.updated_at);
            thread
        })
        .collect();
    let ids: Vec<_> = rows
        .iter()
        .map(|row| ThreadId::from_string(&row.id).unwrap())
        .collect();
    let mut expected: HashMap<_, _> = rows
        .iter()
        .map(|thread| {
            (
                ThreadId::from_string(&thread.id).unwrap(),
                Some(thread.clone()),
            )
        })
        .collect();
    let expected_first_twenty: HashSet<_> = rows[1..21]
        .iter()
        .map(|row| ThreadId::from_string(&row.id).unwrap())
        .collect();
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = crate::resolve_remote_addr(&format!("ws://{}", listener.local_addr()?))?;
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await?;
        let mut socket = tokio_tungstenite::accept_async(stream).await?;
        let mut cursors = Vec::new();
        while let Some(Ok(Message::Text(text))) = socket.next().await {
            let JSONRPCMessage::Request(request) = serde_json::from_str(&text)? else {
                continue;
            };
            let params = request.params.unwrap_or_default();
            let result = match request.method.as_str() {
                "initialize" => json!({"userAgent": "overview-test/1.0"}),
                "thread/loaded/list" => json!({"data": [], "nextCursor": null}),
                "thread/list" => {
                    assert_eq!(params["sortKey"], "recency_at");
                    assert_eq!(params["limit"], 10);
                    {
                        let interactive = params["sourceKinds"] == json!([]);
                        let source_rows: Vec<_> = rows
                            .iter()
                            .filter(|row| matches!(row.source, SessionSource::Exec) != interactive)
                            .collect();
                        let cursor = params["cursor"].as_str().unwrap_or("0").parse::<usize>()?;
                        if interactive {
                            cursors.push(cursor);
                        }
                        if !mixed_sources && interactive && cursors == [0, 10] {
                            socket.send(Message::Text(json!({"id": request.id, "error": {"code": -32603, "message": "transient"}}).to_string().into())).await?;
                            continue;
                        }
                        let end = (cursor + 10).min(source_rows.len());
                        json!({"data": source_rows[cursor..end], "nextCursor": (end < source_rows.len()).then(|| end.to_string())})
                    }
                }
                "thread/read" => {
                    json!({"thread": rows.iter().find(|thread| thread.id == params["threadId"]).unwrap()})
                }
                "thread/turns/list" => json!({"data": [], "nextCursor": null}),
                method => panic!("unexpected request: {method}"),
            };
            socket
                .send(Message::Text(
                    json!({"id": request.id, "result": result})
                        .to_string()
                        .into(),
                ))
                .await?;
        }
        Ok::<_, color_eyre::Report>(cursors)
    });
    let session = AppServerSession::new(
        crate::connect_remote_app_server(endpoint).await?,
        ThreadParamsMode::Remote,
    );
    let view = app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
    app.chat_widget.show_bottom_pane_view(Box::new(view));
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    app.app_event_tx = AppEventSender::new(tx);
    app.refresh_agents_overview_threads(&session);
    finish_overview_refresh(&mut app, &session, &mut rx).await;
    assert_eq!(app.agents_overview.threads.len(), 10);
    assert!(app.agents_overview.discovery.has_more());
    app.track_agents_overview_notification(&ServerNotification::ThreadArchived(
        ThreadArchivedNotification {
            thread_id: ids[0].to_string(),
        },
    ));
    app.remove_agents_overview_thread(ids[0]); // Duplicate local completion must not refill twice.
    expected.remove(&ids[0]);
    app.refresh_agents_overview_threads(&session);
    finish_overview_refresh(&mut app, &session, &mut rx).await;
    if !mixed_sources {
        assert_eq!(app.agents_overview.threads.len(), 9);
        assert!(
            app.agents_overview
                .view_state
                .lock()
                .unwrap()
                .refresh_failed
        );
        app.show_more_agents_overview(&session);
        finish_overview_refresh(&mut app, &session, &mut rx).await;
    }
    assert_eq!(app.agents_overview.threads.len(), 10);
    assert!(app.agents_overview.threads.contains_key(&ids[10]));
    app.show_more_agents_overview(&session);
    finish_overview_refresh(&mut app, &session, &mut rx).await;
    assert_eq!(
        app.agents_overview
            .threads
            .keys()
            .copied()
            .collect::<HashSet<_>>(),
        expected_first_twenty
    );
    app.remove_agents_overview_thread(ids[1]);
    expected.remove(&ids[1]);
    app.refresh_agents_overview_threads(&session);
    finish_overview_refresh(&mut app, &session, &mut rx).await;
    assert_eq!(app.agents_overview.threads.len(), 20);
    assert!(app.agents_overview.threads.contains_key(&ids[21]));
    for &id in &ids[2..22] {
        app.remove_agents_overview_thread(id);
        expected.remove(&id);
    }
    assert!(app.agents_overview.threads.is_empty());
    app.refresh_agents_overview_threads(&session);
    finish_overview_refresh(&mut app, &session, &mut rx).await;
    assert_eq!(app.agents_overview.threads, expected);
    assert!(!app.agents_overview.discovery.has_more());
    session.shutdown().await?;
    assert_eq!(
        server.await??,
        if mixed_sources {
            vec![0, 10]
        } else {
            vec![0, 10, 10, 20]
        }
    );
    Ok(())
}
