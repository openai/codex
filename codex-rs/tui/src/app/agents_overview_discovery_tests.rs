//! Demand-driven discovery, cursor retry, and visible Show more behavior.

use super::*;
use crate::app_event::AgentsOverviewAction;
use crate::app_server_session::ThreadParamsMode;
use codex_app_server_protocol::JSONRPCMessage;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
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
            if index == 23 {
                thread.name = None;
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
    let reopened_pin = overview_thread(
        ThreadId::new(),
        /*parent_thread_id*/ None,
        "Pinned after reopen",
        ThreadStatus::NotLoaded,
    );
    let reopened_pin_id = ThreadId::from_string(&reopened_pin.id).unwrap();
    let show_reopened_pin = Arc::new(AtomicBool::new(false));
    let expected_first_twenty: HashSet<_> = rows[1..21]
        .iter()
        .map(|row| ThreadId::from_string(&row.id).unwrap())
        .collect();
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = crate::resolve_remote_addr(&format!("ws://{}", listener.local_addr()?))?;
    let server_show_reopened_pin = Arc::clone(&show_reopened_pin);
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
                    if params["sortKey"] == "section_position" {
                        assert_eq!(params["sectionId"], codex_state::PINNED_THREAD_SECTION_ID);
                        let interactive = params["sourceKinds"] == json!([]);
                        let cursor = params["cursor"].as_str();
                        let reopened = server_show_reopened_pin.load(Ordering::SeqCst);
                        let data = if reopened && interactive && cursor.is_none() {
                            vec![reopened_pin.clone()]
                        } else if interactive && cursor == Some("pinned-page-2") {
                            vec![rows[1].clone()]
                        } else {
                            Vec::new()
                        };
                        socket
                            .send(Message::Text(
                                json!({
                                    "id": request.id,
                                    "result": {
                                        "data": data,
                                        "nextCursor": (!reopened && interactive && cursor.is_none())
                                            .then_some("pinned-page-2"),
                                    }
                                })
                                .to_string()
                                .into(),
                            ))
                            .await?;
                        continue;
                    }
                    assert_eq!(params["sortKey"], "recency_at");
                    let limit = params["limit"].as_u64().unwrap() as usize;
                    assert!(params["searchTerm"].is_null());
                    {
                        let interactive = params["sourceKinds"] == json!([]);
                        let mut source_rows: Vec<_> = rows
                            .iter()
                            .filter(|row| matches!(row.source, SessionSource::Exec) != interactive)
                            .collect();
                        if interactive && server_show_reopened_pin.load(Ordering::SeqCst) {
                            source_rows.insert(0, &reopened_pin);
                        }
                        let cursor = params["cursor"].as_str().unwrap_or("0").parse::<usize>()?;
                        if interactive && limit == 10 {
                            cursors.push(cursor);
                        }
                        if limit == 10 && !mixed_sources && interactive && cursors == [0, 10] {
                            socket.send(Message::Text(json!({"id": request.id, "error": {"code": -32603, "message": "transient"}}).to_string().into())).await?;
                            continue;
                        }
                        let end = (cursor + limit).min(source_rows.len());
                        json!({"data": source_rows[cursor..end], "nextCursor": (end < source_rows.len()).then(|| end.to_string())})
                    }
                }
                "thread/read" => {
                    let thread = rows
                        .iter()
                        .find(|thread| thread.id == params["threadId"])
                        .unwrap();
                    json!({"thread": thread, "readState": {"revision": "read", "firstUnread": null}})
                }
                "thread/turns/list" if params["threadId"] == rows[23].id => json!({"data": [{
                    "id":"latest", "status":"completed", "items":[{"type":"userMessage","id":"prompt",
                    "content":[{"type":"text","text":"A later prompt","textElements":[]}]}]
                }], "nextCursor":null}),
                "thread/turns/list" => json!({"data": [], "nextCursor": null}),
                "thread/archive" | "thread/delete" => {
                    socket.send(Message::Text(json!({"id": request.id, "error":{"code":-32603,"message":"try again"}}).to_string().into())).await?;
                    continue;
                }
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
    let mut session = AppServerSession::new(
        crate::backend_id::BackendId::new(),
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
    assert_eq!(app.agents_overview.pinned_thread_ids, Some(vec![ids[1]]));
    assert!(app.agents_overview.discovery.has_more());
    let browse = app.agents_overview.threads.clone();
    let browse_rows = || browse.values().flatten().cloned().collect();
    let mut view = app.agents_overview_view(browse_rows(), Some(ids[4]));
    view.handle_key_event(KeyCode::Char('/').into());
    view.handle_paste("TASK 23".into());
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.pause_events();
    finish_history_search(&mut app, &mut tui, &mut session, &mut rx).await?;
    assert_eq!(app.agents_overview.search_matches.len(), 1);
    view = app.agents_overview_view(browse_rows(), /*selected_thread_id*/ None);
    assert_eq!(view.thread_ids()[view.selected_index().unwrap()], ids[23]);
    let action = if mixed_sources {
        AgentsOverviewAction::Archive
    } else {
        AgentsOverviewAction::Delete
    };
    Box::pin(app.run_agents_overview_action(&mut tui, &mut session, ids[23], action)).await?;
    finish_overview_refresh(&mut app, &session, &mut rx).await;
    app.chat_widget.handle_key_event(KeyCode::Enter.into());
    assert!(render_bottom_popup(&app.chat_widget, /*width*/ 100).contains("Task 23"));
    view.handle_key_event(KeyCode::Esc.into());
    assert_eq!(view.thread_ids()[view.selected_index().unwrap()], ids[4]);
    assert_eq!(app.agents_overview.threads, browse);
    view.handle_key_event(KeyCode::Char('/').into());
    view.handle_paste("TASK 22".into());
    assert!(super::command_center::screen(&view, /*width*/ 100, /*height*/ 16).contains("All 10 "));
    finish_history_search(&mut app, &mut tui, &mut session, &mut rx).await?;
    view = app.agents_overview_view(browse_rows(), /*selected_thread_id*/ None);
    let rendered = super::command_center::screen(&view, /*width*/ 100, /*height*/ 16);
    assert!(rendered.contains("All 11 "));
    assert!(rendered.contains("1 of 10"), "{rendered}"); // The pinned task has its own group.
    view.handle_key_event(KeyCode::Esc.into());

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
    // Another client read this older, unloaded task while the center was closed.
    app.agents_overview.unread.insert(ids[19]);
    app.remove_agents_overview_thread(ids[1]);
    expected.remove(&ids[1]);
    app.refresh_agents_overview_threads(&session);
    finish_overview_refresh(&mut app, &session, &mut rx).await;
    assert!(!app.agents_overview.unread.contains(&ids[19]));
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
    let old = app.agents_overview.threads[&ids[23]].as_ref().unwrap();
    assert_eq!(old.preview, "Task 23");
    assert_eq!(
        app.agents_overview_details(old, &HashMap::new())
            .latest_prompt
            .as_deref(),
        Some("A later prompt")
    );
    // Another CLI inserts a matching session ahead of the exhausted browse cursor.
    show_reopened_pin.store(true, Ordering::SeqCst);
    view.handle_key_event(KeyCode::Char('/').into());
    view.handle_paste("Pinned after reopen".into());
    finish_history_search(&mut app, &mut tui, &mut session, &mut rx).await?;
    assert!(
        app.agents_overview
            .search_matches
            .contains_key(&reopened_pin_id)
    );
    view.handle_key_event(KeyCode::Esc.into());
    app.agents_overview.threads.clear();
    app.agents_overview.pinned_thread_ids = Some(Vec::new());
    app.agents_overview.refill_count = 0;
    app.refresh_agents_overview_threads(&session);
    finish_overview_refresh(&mut app, &session, &mut rx).await;
    assert_eq!(
        app.agents_overview.pinned_thread_ids,
        Some(vec![reopened_pin_id])
    );
    assert!(app.agents_overview.threads.contains_key(&reopened_pin_id));
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

async fn finish_history_search(
    app: &mut App,
    tui: &mut crate::tui::Tui,
    session: &mut AppServerSession,
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
) -> Result<()> {
    while app
        .agents_overview
        .view_state
        .lock()
        .unwrap()
        .history
        .as_ref()
        .is_some_and(|s| s.loading)
    {
        let event = tokio::time::timeout(Duration::from_secs(10), rx.recv())
            .await?
            .expect("search event");
        app.handle_event(tui, session, event).await?;
    }
    Ok(())
}
