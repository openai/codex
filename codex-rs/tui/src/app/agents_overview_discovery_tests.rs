//! Demand-driven discovery, cursor retry, and visible Show more behavior.

use super::*;
use crate::app::agents_overview_discovery::AgentsOverviewDiscovery;
use crate::app::backend_sessions::BackendSessions;
use crate::app::tests::test_thread_session;
use crate::app_event::AgentsOverviewAction;
use crate::app_server_session::ThreadParamsMode;
use codex_app_server_client::CloudCommand;
use codex_app_server_client::CloudOperation;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::ThreadHistoryMode;
use codex_app_server_protocol::ThreadSortKey;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

#[tokio::test]
async fn overview_cloud_discovery_uses_only_interactive_pages() {
    let (client, mut commands, _events) = CloudClient::channel(/*capacity*/ 1);
    let handle = AppServerClient::Cloud(client).request_handle();
    let threads = ["First Cloud task", "Second Cloud task"].map(|name| {
        overview_thread(
            ThreadId::new(),
            /*parent_thread_id*/ None,
            name,
            ThreadStatus::NotLoaded,
        )
    });

    let serve_pages = async {
        for (thread, (cursor, next_cursor)) in threads
            .iter()
            .zip([(None, Some("next")), (Some("next"), None)])
        {
            let command = commands.recv().await.unwrap();
            let CloudOperation::Request(request) = command.operation else {
                panic!("expected a Cloud request");
            };
            let ClientRequest::ThreadList { params, .. } = *request else {
                panic!("expected thread/list");
            };
            assert_eq!(params.source_kinds, Some(Vec::new()));
            assert_eq!(params.cursor.as_deref(), cursor);
            command
                .response
                .send(Ok(Ok(json!({"data": [thread], "nextCursor": next_cursor}))))
                .unwrap();
        }
    };
    let discover = async {
        let mut discovery = AgentsOverviewDiscovery::default();
        for (thread, has_more) in threads.iter().zip([true, false]) {
            let page = discovery.next_batch(&handle, /*limit*/ 10).await;
            assert_eq!(page, (vec![(thread.clone(), false)], true));
            assert_eq!(discovery.has_more(), has_more);
        }
    };
    tokio::join!(serve_pages, discover);
}

#[tokio::test]
async fn overview_return_to_current_chat_cancels_the_cloud_list_and_allows_reopen() -> Result<()> {
    let (mut app, mut events, _) = make_test_app_with_channels().await;
    let thread_id = ThreadId::new();
    attach_thread(&mut app, thread_id);
    let owner = BackendId::new();
    let (mut cloud, mut commands, _events) = overview_session_stub(owner, ThreadParamsMode::Cloud);
    app.app_server_target = AppServerTarget::Cloud;
    app.app_server_backend_id = Some(owner);
    app.backend_registry.connect(&cloud);
    assert!(app.backend_registry.observe(owner, thread_id));
    let mut current = overview_thread(
        thread_id,
        /*parent_thread_id*/ None,
        "Current Cloud chat",
        ThreadStatus::Idle,
    );
    current.history_mode = ThreadHistoryMode::Paginated;
    app.agents_overview
        .threads
        .insert(thread_id, Some(current.clone()));

    let timeout = Duration::from_secs(/*secs*/ 10);
    let next_list = async |commands: &mut mpsc::Receiver<CloudCommand>| {
        reject_cloud_pinned_list(commands).await;
        let loaded = commands.recv().await.expect("Cloud loaded threads");
        assert!(matches!(
            loaded.operation,
            CloudOperation::Request(request)
                if matches!(request.as_ref(), ClientRequest::ThreadLoadedList { .. })
        ));
        loaded
            .response
            .send(Ok(Ok(json!({"data": [], "nextCursor": null}))))
            .unwrap();
        next_cloud_page(commands, /*cursor*/ None).await
    };
    let mut tui = crate::tui::test_support::make_test_tui()?;
    for key in [KeyCode::Esc, KeyCode::Enter] {
        app.open_agents_overview(&cloud);
        let held_list = tokio::time::timeout(timeout, next_list(&mut commands)).await?;
        Box::pin(app.handle_tui_event(&mut tui, &mut cloud, TuiEvent::Key(key.into()))).await?;
        if key == KeyCode::Enter {
            tokio::time::timeout(timeout, async {
                loop {
                    if let AppEvent::SelectAgentsOverviewThread {
                        thread_id: selected,
                    } = events.recv().await.expect("overview selection event")
                    {
                        assert_eq!(selected, thread_id);
                        break;
                    }
                }
                app.select_agents_overview_thread(&mut tui, &mut cloud, thread_id)
                    .await
            })
            .await??;
        }
        assert!(held_list.response.is_closed());
        assert!(!app.chat_widget.active_view_is(AGENTS_OVERVIEW_VIEW_ID));
    }

    app.open_agents_overview(&cloud);
    let reopened = tokio::time::timeout(timeout, next_list(&mut commands)).await?;
    reopened
        .response
        .send(Ok(Ok(json!({"data": [], "nextCursor": null}))))
        .unwrap();
    let read = tokio::time::timeout(timeout, commands.recv())
        .await?
        .expect("Cloud task details after reopen");
    assert!(matches!(
        read.operation,
        CloudOperation::Request(request)
            if matches!(request.as_ref(), ClientRequest::ThreadRead { params, .. }
                if params.thread_id == thread_id.to_string() && !params.include_turns)
    ));
    read.response
        .send(Ok(Ok(json!({"thread": current, "readState": null}))))
        .unwrap();
    let turns = tokio::time::timeout(timeout, commands.recv())
        .await?
        .expect("Cloud task preview after reopen");
    assert!(matches!(
        turns.operation,
        CloudOperation::Request(request)
            if matches!(request.as_ref(), ClientRequest::ThreadTurnsList { params, .. }
                if params.thread_id == thread_id.to_string() && params.limit == Some(1))
    ));
    turns
        .response
        .send(Ok(Ok(
            json!({"data": [], "nextCursor": null, "backwardsCursor": null}),
        )))
        .unwrap();
    finish_overview_refresh(&mut app, &cloud, &mut events).await;
    assert!(app.chat_widget.active_view_is(AGENTS_OVERVIEW_VIEW_ID));
    assert!(!app.agents_overview.refresh_failed_backends.contains(&owner));
    Ok(())
}

#[tokio::test]
async fn resume_picker_cancels_a_held_cloud_show_more_page() -> Result<()> {
    let (mut app, _events, _) = make_test_app_with_channels().await;
    let owner = BackendId::new();
    let (cloud, mut commands, _server_events) =
        overview_session_stub(owner, ThreadParamsMode::Cloud);
    app.app_server_target = AppServerTarget::Cloud;
    app.app_server_backend_id = Some(owner);
    app.backend_registry.connect(&cloud);
    let row = overview_thread(
        ThreadId::new(),
        /*parent_thread_id*/ None,
        "Cloud task",
        ThreadStatus::Idle,
    );
    let mut discovery = AgentsOverviewDiscovery::default();
    let handle = cloud.request_handle();
    let (page, ()) = tokio::join!(discovery.next_batch(&handle, /*limit*/ 10), async {
        let command = next_cloud_page(&mut commands, /*cursor*/ None).await;
        command
            .response
            .send(Ok(Ok(json!({"data": [&row], "nextCursor": "next"}))))
            .unwrap();
    });
    assert_eq!(page, (vec![(row.clone(), false)], true));
    app.agents_overview.initialized.insert(owner);
    let view = app.agents_overview_view(vec![row], /*selected_thread_id*/ None);
    app.chat_widget.show_bottom_pane_view(Box::new(view));
    app.agents_overview.discovery.insert(owner, discovery);
    app.show_more_agents_overview(&cloud);
    reject_cloud_pinned_list(&mut commands).await;
    let held_page = next_cloud_page(&mut commands, Some("next")).await;
    let mut backends = BackendSessions::new(cloud);
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_alt_screen_enabled(/*enabled*/ false);
    tui.pause_events();
    let mut picker = Box::pin(app.open_resume_picker_for_backends(&mut tui, &mut backends));
    let list = tokio::time::timeout(Duration::from_secs(/*secs*/ 10), async {
        tokio::select! {
            result = &mut picker => panic!("picker exited before listing Cloud tasks: {result:?}"),
            command = commands.recv() => command.expect("picker Cloud list"),
        }
    })
    .await?;
    assert!(held_page.response.is_closed());
    assert!(
        matches!(list.operation, CloudOperation::Request(request) if matches!(request.as_ref(), ClientRequest::ThreadList { .. }))
    );
    drop(picker);
    Ok(())
}

#[tokio::test]
async fn navigation_without_an_unsubscribe_keeps_the_cloud_page() -> Result<()> {
    let mut app = make_test_app().await;
    let owner = BackendId::new();
    let (mut cloud, _commands, _events) = overview_session_stub(owner, ThreadParamsMode::Cloud);
    app.backend_registry.connect(&cloud);
    let request = Uuid::new_v4();
    app.agents_overview.active_refresh = Some((request, owner));
    let (guard, mut dropped) = tokio::sync::oneshot::channel::<()>();
    app.agents_overview.refresh_task = Some(tokio::spawn(async move {
        let _guard = guard;
        std::future::pending::<()>().await;
    }));

    app.detach_current_thread_for_navigation(&mut cloud, Some(ThreadId::new()))
        .await;

    assert_eq!(app.agents_overview.active_refresh, Some((request, owner)));
    assert!(matches!(
        dropped.try_recv(),
        Err(tokio::sync::oneshot::error::TryRecvError::Empty)
    ));
    app.agents_overview.refresh_task.take().unwrap().abort();
    Ok(())
}

#[tokio::test]
async fn cycling_from_a_blank_cloud_task_releases_its_page_before_unsubscribe() -> Result<()> {
    let (mut app, _events, _) = make_test_app_with_channels().await;
    let local_settings = app.backend_settings();
    let local = crate::start_embedded_app_server_for_picker(&app.config).await?;
    let local_owner = local.backend_id();
    let owner = BackendId::new();
    let id = ThreadId::new();
    let (cloud, mut commands, _server_events) =
        overview_session_stub(owner, ThreadParamsMode::Cloud);
    app.app_server_target = AppServerTarget::Cloud;
    app.app_server_backend_id = Some(owner);
    app.backend_registry.connect(&cloud);
    assert!(app.backend_registry.observe(owner, id));
    let session = test_thread_session(id, app.config.cwd.to_path_buf());
    app.enqueue_primary_thread_session(session.clone(), Vec::new())
        .await?;
    app.agents_overview.blank_sessions.insert(
        id,
        AppServerStartedThread {
            session,
            turns: Vec::new(),
            blocks_direct_input: false,
            task_tools_available: false,
        },
    );
    app.refresh_agents_overview_threads(&cloud);
    reject_cloud_pinned_list(&mut commands).await;
    let loaded = commands.recv().await.expect("Cloud loaded threads");
    assert!(matches!(loaded.operation, CloudOperation::Request(request)
        if matches!(request.as_ref(), ClientRequest::ThreadLoadedList { .. })));
    loaded
        .response
        .send(Ok(Ok(json!({"data": [], "nextCursor": null}))))
        .unwrap();
    let mut held_page = next_cloud_page(&mut commands, /*cursor*/ None).await;
    let mut backends = BackendSessions::from_initial(cloud, vec![local]);
    backends.settings.insert(local_owner, local_settings);
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let (switched, ()) = tokio::time::timeout(Duration::from_secs(/*secs*/ 10), async {
        tokio::join!(
            Box::pin(app.start_new_session_on_other_backend(
                &mut tui,
                &mut backends,
                local_owner,
                crate::app::backend_selection::NewBackendSession::CarryDraft,
            )),
            async {
                held_page.response.closed().await;
                let command = commands.recv().await.expect("blank Cloud unsubscribe");
                assert!(matches!(command.operation, CloudOperation::Request(request)
                    if matches!(request.as_ref(), ClientRequest::ThreadUnsubscribe { params, .. }
                        if params.thread_id == id.to_string())));
                command
                    .response
                    .send(Ok(Ok(json!({"status": "unsubscribed"}))))
                    .unwrap();
            }
        )
    })
    .await?;
    switched.expect("switch backend");
    assert_eq!(backends.displayed_owner(), local_owner);
    let (shutdown, ()) = tokio::join!(backends.shutdown(), async {
        let command = commands.recv().await.expect("Cloud shutdown");
        assert!(matches!(command.operation, CloudOperation::Shutdown));
        command
            .response
            .send(Ok(Ok(serde_json::Value::Null)))
            .unwrap();
    });
    shutdown?;
    Ok(())
}

#[tokio::test]
async fn overview_local_enter_drops_the_displayed_cloud_list_before_detach() -> Result<()> {
    let (mut app, mut events, _) = make_test_app_with_channels().await;
    let mut local = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let local_owner = local.backend_id();
    let local_session = local.start_thread(&app.config).await?.session;
    let local_id = local_session.thread_id;
    let saved_item = serde_json::from_str(
        r#"{"type":"message","role":"assistant","content":[{"type":"output_text","text":"saved locally"}]}"#,
    )?;
    local
        .thread_inject_items(local_id, vec![saved_item])
        .await?;
    let local_settings = app.backend_settings();
    let mut local_row = overview_thread(
        local_id,
        /*parent_thread_id*/ None,
        "Local saved chat",
        ThreadStatus::Idle,
    );
    local_row.cwd = local_session.cwd;

    let cloud_owner = BackendId::new();
    let cloud_id = ThreadId::new();
    let (cloud, mut commands, _events) =
        overview_session_stub(cloud_owner, ThreadParamsMode::Cloud);
    let cloud_row = overview_thread(
        cloud_id,
        /*parent_thread_id*/ None,
        "Current Cloud chat",
        ThreadStatus::Idle,
    );
    app.backend_registry.connect(&cloud);
    for (owner, id, row) in [
        (local_owner, local_id, local_row.clone()),
        (cloud_owner, cloud_id, cloud_row.clone()),
    ] {
        assert!(app.backend_registry.observe(owner, id));
        app.agents_overview.threads.insert(id, Some(row));
        app.agents_overview.initialized.insert(owner);
    }
    app.app_server_target = AppServerTarget::Cloud;
    app.app_server_backend_id = Some(cloud_owner);
    app.enqueue_primary_thread_session(
        test_thread_session(cloud_id, app.config.cwd.to_path_buf()),
        Vec::new(),
    )
    .await?;

    let timeout = Duration::from_secs(/*secs*/ 10);
    let mut discovery = AgentsOverviewDiscovery::default();
    let handle = cloud.request_handle();
    let (first_page, ()) = tokio::time::timeout(timeout, async {
        tokio::join!(discovery.next_batch(&handle, /*limit*/ 10), async {
            let command = next_cloud_page(&mut commands, /*cursor*/ None).await;
            command
                .response
                .send(Ok(Ok(
                    json!({"data": [&cloud_row], "nextCursor": "cloud-next"}),
                )))
                .unwrap();
        })
    })
    .await?;
    assert_eq!(first_page, (vec![(cloud_row.clone(), false)], true));
    app.agents_overview.discovery.insert(cloud_owner, discovery);
    let view = app.agents_overview_view(vec![cloud_row.clone(), local_row.clone()], Some(local_id));
    app.agents_overview.visible_thread_ids = view.thread_ids();
    app.chat_widget.show_bottom_pane_view(Box::new(view));
    app.show_more_agents_overview(&cloud);
    let mut held_list = tokio::time::timeout(timeout, async {
        reject_cloud_pinned_list(&mut commands).await;
        next_cloud_page(&mut commands, Some("cloud-next")).await
    })
    .await?;

    let mut backends = BackendSessions::from_initial(cloud, vec![local]);
    backends.settings.insert(local_owner, local_settings);
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let target =
        enter_overview_thread(&mut app, &mut tui, &mut backends, &mut events, local_id).await?;
    assert_eq!(
        (target.backend_id, target.thread_id),
        (local_owner, local_id)
    );

    let detach = async {
        // Model the native worker's serial dispatch while the page caller is alive.
        held_list.response.closed().await;
        let command = commands.recv().await.expect("outgoing Cloud detach");
        assert!(matches!(
            command.operation,
            CloudOperation::Request(request)
                if matches!(request.as_ref(), ClientRequest::ThreadUnsubscribe { params, .. }
                    if params.thread_id == cloud_id.to_string())
        ));
        command
            .response
            .send(Ok(Ok(json!({"status": "unsubscribed"}))))
            .unwrap();
    };
    let (resumed, ()) = tokio::time::timeout(timeout, async {
        tokio::join!(
            app.resume_backend_session(&mut tui, &mut backends, target),
            detach
        )
    })
    .await?;
    assert!(matches!(resumed?, AppRunControl::Continue));
    assert_eq!(backends.displayed_owner(), local_owner);
    assert_eq!(app.current_displayed_thread_id(), Some(local_id));
    assert!(
        app.agents_overview
            .show_more_backends
            .contains(&cloud_owner)
    );

    let (shutdown, ()) = tokio::time::timeout(timeout, async {
        tokio::join!(backends.shutdown(), async {
            let command = commands.recv().await.expect("Cloud shutdown");
            assert!(matches!(command.operation, CloudOperation::Shutdown));
            command
                .response
                .send(Ok(Ok(serde_json::Value::Null)))
                .unwrap();
        })
    })
    .await?;
    shutdown?;
    Ok(())
}

async fn enter_overview_thread(
    app: &mut App,
    tui: &mut crate::tui::Tui,
    backends: &mut BackendSessions,
    events: &mut mpsc::UnboundedReceiver<AppEvent>,
    expected: ThreadId,
) -> Result<SessionTarget> {
    let index = app
        .agents_overview
        .visible_thread_ids
        .iter()
        .position(|id| *id == expected)
        .unwrap();
    for _ in 0..app.agents_overview.visible_thread_ids.len() {
        if app
            .chat_widget
            .selected_index_for_present_view(AGENTS_OVERVIEW_VIEW_ID)
            == Some(index)
        {
            break;
        }
        app.chat_widget.handle_key_event(KeyCode::Down.into());
    }
    app.chat_widget.handle_key_event(KeyCode::Enter.into());
    tokio::time::timeout(Duration::from_secs(/*secs*/ 10), async {
        loop {
            if let AppEvent::SelectAgentsOverviewThread { thread_id } =
                events.recv().await.expect("overview selection event")
            {
                assert_eq!(thread_id, expected);
                break;
            }
        }
        let displayed = backends.displayed;
        app.select_agents_overview_thread(tui, backends.session_at_mut(displayed), expected)
            .await?;
        loop {
            if let AppEvent::ResumeBackendSession(target) =
                events.recv().await.expect("owner-routed resume event")
            {
                return Ok(target);
            }
        }
    })
    .await?
}

async fn reject_cloud_pinned_list(commands: &mut mpsc::Receiver<CloudCommand>) {
    let command = commands.recv().await.expect("Cloud pinned threads");
    assert!(matches!(
        command.operation,
        CloudOperation::Request(request)
            if matches!(request.as_ref(), ClientRequest::ThreadList { params, .. }
                if params.sort_key == Some(ThreadSortKey::SectionPosition))
    ));
    command
        .response
        .send(Ok(Err(JSONRPCErrorError {
            code: -32000,
            message: "These cloud conversation filters are not supported yet.".to_string(),
            data: None,
        })))
        .unwrap();
}

async fn next_cloud_page(
    commands: &mut mpsc::Receiver<CloudCommand>,
    cursor: Option<&str>,
) -> CloudCommand {
    let command = commands.recv().await.expect("Cloud inventory page");
    let CloudOperation::Request(request) = &command.operation else {
        panic!("expected a Cloud request");
    };
    let ClientRequest::ThreadList { params, .. } = request.as_ref() else {
        panic!("expected thread/list");
    };
    assert_eq!(params.cursor.as_deref(), cursor);
    assert_eq!(params.source_kinds, Some(Vec::new()));
    command
}

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
                thread.history_mode = ThreadHistoryMode::Paginated;
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
    let reopened_pin_id =
        ThreadId::from_u128(/*value*/ 0xda7aba5e_0000_4000_8000_000000000001);
    let reopened_pin = overview_thread(
        reopened_pin_id,
        /*parent_thread_id*/ None,
        "Pinned after reopen",
        ThreadStatus::NotLoaded,
    );
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
                    assert_ne!(params["includeTurns"], true);
                    let thread = rows
                        .iter()
                        .find(|thread| thread.id == params["threadId"])
                        .unwrap();
                    json!({"thread": thread, "readState": {"revision": "read", "firstUnread": null}})
                }
                "thread/turns/list" if params["threadId"] == rows[23].id => json!({"data": [{
                    "id":"latest", "status":"completed", "items":[{"type":"userMessage","id":"prompt",
                    "content":[{"type":"text","text":"A later prompt","textElements":[]}]},
                    {"type":"agentMessage","id":"reply","text":"A saved reply"}]
                }], "nextCursor":null}),
                "thread/turns/list" => panic!("overview requested legacy transcript: {params}"),
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
    let owner = session.backend_id();
    app.app_server_backend_id = Some(owner);
    app.backend_registry.connect(&session);
    let view = app.agents_overview_view(Vec::new(), /*selected_thread_id*/ None);
    app.chat_widget.show_bottom_pane_view(Box::new(view));
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    app.app_event_tx = AppEventSender::new(tx);
    app.refresh_agents_overview_threads(&session);
    finish_overview_refresh(&mut app, &session, &mut rx).await;
    assert_eq!(app.agents_overview.threads.len(), 10);
    assert_eq!(
        app.agents_overview.pins_by_backend[&session.backend_id()],
        Some(vec![ids[1]])
    );
    assert!(
        app.agents_overview
            .has_more_for(&app.backend_registry, owner)
    );
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

    if mixed_sources {
        // Queue Show more behind a metadata refresh before the archive creates a vacancy.
        app.refresh_agents_overview_threads(&session);
        app.show_more_agents_overview(&session);
    }
    app.track_agents_overview_notification(&ServerNotification::ThreadArchived(
        ThreadArchivedNotification {
            thread_id: ids[0].to_string(),
        },
    ));
    app.remove_agents_overview_thread(ids[0]); // Duplicate local completion must not refill twice.
    expected.remove(&ids[0]);
    app.refresh_agents_overview_threads(&session);
    finish_overview_refresh(&mut app, &session, &mut rx).await;
    if mixed_sources {
        finish_overview_refresh(&mut app, &session, &mut rx).await;
    } else {
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
    let other = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
    let other_owner = other.backend_id();
    if mixed_sources {
        let archived = ids[22];
        app.backend_registry.observe(owner, archived);
        app.track_agents_overview_notification(&ServerNotification::ThreadArchived(
            ThreadArchivedNotification {
                thread_id: archived.to_string(),
            },
        ));
        expected.remove(&archived);
    } else {
        app.show_more_agents_overview(&session);
    }
    assert!(
        app.agents_overview.active_refresh.is_some(),
        "Show more should load a page after the vacancy is filled"
    );
    assert_eq!(
        app.agents_overview.active_refresh.map(|(_, owner)| owner),
        Some(owner)
    );
    // The page may finish while another backend is displayed, or after returning.
    app.app_server_backend_id = Some(other_owner);
    app.backend_registry.connect(&other);
    assert!(app.backend_registry.request_handle(owner).is_some());
    assert!(
        !app.agents_overview
            .has_more_for(&app.backend_registry, other_owner)
    );
    app.show_more_agents_overview(&other);
    assert!(app.agents_overview.pending_refresh_backends.is_empty());
    if mixed_sources {
        app.refresh_agents_overview_threads(&other);
    }
    finish_overview_refresh(&mut app, &other, &mut rx).await;
    if mixed_sources {
        finish_overview_refresh(&mut app, &other, &mut rx).await;
        assert!(app.agents_overview.initialized.contains(&other_owner));
    }
    app.app_server_backend_id = Some(owner);
    assert!(
        app.agents_overview
            .has_more_for(&app.backend_registry, owner)
    );
    app.backend_registry.disconnect(other_owner);
    other.shutdown().await?;
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
    assert!(
        !app.agents_overview
            .has_more_for(&app.backend_registry, owner)
    );
    let old = app.agents_overview.threads[&ids[23]].as_ref().unwrap();
    assert_eq!(old.preview, "Task 23");
    assert_eq!(
        app.agents_overview_details(old, &HashMap::new())
            .latest_prompt
            .as_deref(),
        Some("A later prompt")
    );
    assert_eq!(
        app.agents_overview
            .last_messages
            .get(&ids[23])
            .map(String::as_str),
        Some("A saved reply")
    );
    // Another CLI inserts a matching session ahead of the exhausted browse cursor.
    show_reopened_pin.store(true, Ordering::SeqCst);
    view.handle_key_event(KeyCode::Char('/').into());
    view.handle_paste("DA7ABA5E-0000-4000".into());
    finish_history_search(&mut app, &mut tui, &mut session, &mut rx).await?;
    assert_eq!(
        app.agents_overview
            .search_matches
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        [reopened_pin_id],
    );
    view.handle_key_event(KeyCode::Esc.into());
    app.agents_overview.threads.clear();
    app.agents_overview.pins_by_backend = [(session.backend_id(), Some(Vec::new()))].into();
    app.agents_overview.refill_by_backend.clear();
    app.refresh_agents_overview_threads(&session);
    finish_overview_refresh(&mut app, &session, &mut rx).await;
    assert_eq!(
        app.agents_overview.pins_by_backend[&session.backend_id()],
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
