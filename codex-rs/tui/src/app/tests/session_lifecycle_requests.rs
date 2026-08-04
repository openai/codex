use super::*;
use app_test_support::create_fake_paginated_rollout;
use app_test_support::create_fake_parented_rollout_with_source;
use app_test_support::create_fake_rollout;
use app_test_support::rollout_path;
use codex_app_server_protocol::ClientNotification;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCRequest;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::SortDirection;
use codex_app_server_protocol::ThreadItemsListParams;
use codex_app_server_protocol::ThreadItemsListResponse;
use codex_app_server_protocol::ThreadStatus;
use codex_protocol::AgentPath;
use codex_protocol::items::AgentMessageContent;
use codex_protocol::items::AgentMessageItem;
use codex_protocol::items::EnteredReviewModeItem;
use codex_protocol::items::ExitedReviewModeItem;
use codex_protocol::items::TurnItem;
use codex_protocol::items::UserMessageItem;
use codex_protocol::protocol::ItemCompletedEvent;
use codex_protocol::protocol::ReviewTarget;
use codex_protocol::user_input::UserInput as CoreUserInput;
use codex_state::SqliteConfig;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use std::sync::Mutex;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

type RecordedRequests = Arc<Mutex<Vec<JSONRPCRequest>>>;
type RecordingAppServer = (AppServerSession, RecordedRequests, JoinHandle<Result<()>>);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HistoryCapabilities {
    Current,
    LegacyOnly,
    LegacyOnlyUnsupportedVariant,
}

/// Returns and resets `(thread/loaded/list, thread/read)` request counts.
fn take_backfill_counts(requests: &RecordedRequests) -> (usize, usize) {
    let requests = std::mem::take(&mut *requests.lock().expect("request recorder lock"));
    (
        requests
            .iter()
            .filter(|request| request.method == "thread/loaded/list")
            .count(),
        requests
            .iter()
            .filter(|request| request.method == "thread/read")
            .count(),
    )
}

/// Starts an embedded app server behind a loopback WebSocket proxy that records JSON-RPC methods.
async fn start_recording_app_server(
    config: &Config,
    blocked_thread_list: Option<(ThreadId, oneshot::Sender<()>, oneshot::Receiver<()>)>,
    failed_thread_name: Option<&'static str>,
) -> Result<RecordingAppServer> {
    start_recording_app_server_with_history(
        config,
        HistoryCapabilities::Current,
        blocked_thread_list,
        failed_thread_name,
    )
    .await
}

/// Proxies a real app server while optionally rejecting modern pagination like an older server.
async fn start_recording_app_server_with_history(
    config: &Config,
    history_capabilities: HistoryCapabilities,
    mut blocked_thread_list: Option<(ThreadId, oneshot::Sender<()>, oneshot::Receiver<()>)>,
    failed_thread_name: Option<&'static str>,
) -> Result<RecordingAppServer> {
    let state_db =
        crate::init_state_db_for_app_server_target(config, &crate::AppServerTarget::Embedded)
            .await?;
    let embedded = crate::start_embedded_app_server(
        codex_arg0::Arg0DispatchPaths::default(),
        config.clone(),
        Vec::new(),
        codex_config::LoaderOverrides::default(),
        /*strict_config*/ false,
        codex_config::CloudConfigBundleLoader::default(),
        codex_feedback::CodexFeedback::new(),
        /*log_db*/ None,
        state_db,
        Arc::new(codex_exec_server::EnvironmentManager::default_for_tests()),
    )
    .await?;
    let codex_home = config.codex_home.display().to_string();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let request_sink = Arc::clone(&requests);
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let websocket_url = format!("ws://{}", listener.local_addr()?);
    let proxy = tokio::spawn(async move {
        let (stream, _) = listener.accept().await?;
        let mut websocket = accept_async(stream).await?;
        while let Some(frame) = websocket.next().await {
            let Message::Text(text) = frame? else {
                continue;
            };
            let message = serde_json::from_str::<JSONRPCMessage>(&text)?;
            match message {
                JSONRPCMessage::Request(request) if request.method == "initialize" => {
                    websocket
                        .send(Message::Text(
                            serde_json::to_string(&JSONRPCMessage::Response(JSONRPCResponse {
                                id: request.id,
                                result: serde_json::json!({
                                    "userAgent": "codex-tui-test",
                                    "codexHome": codex_home,
                                }),
                            }))?
                            .into(),
                        ))
                        .await?;
                }
                JSONRPCMessage::Request(request) => {
                    request_sink
                        .lock()
                        .expect("request recorder lock")
                        .push(request.clone());
                    let request_id = request.id.clone();
                    let params = request.params.as_ref();
                    let requires_pagination = match request.method.as_str() {
                        "thread/start" => params
                            .and_then(|params| params.get("historyMode"))
                            .is_some_and(|mode| !mode.is_null()),
                        "thread/resume" | "thread/fork" => params
                            .and_then(|params| params.get("excludeTurns"))
                            .and_then(serde_json::Value::as_bool)
                            .unwrap_or(false),
                        "thread/turns/list" | "thread/items/list" => true,
                        _ => false,
                    };
                    let response = if matches!(
                        history_capabilities,
                        HistoryCapabilities::LegacyOnly
                            | HistoryCapabilities::LegacyOnlyUnsupportedVariant
                    ) && requires_pagination
                    {
                        let (code, message) = if history_capabilities
                            == HistoryCapabilities::LegacyOnlyUnsupportedVariant
                            && request.method == "thread/start"
                        {
                            (-32602, "unknown variant \"paginated\", expected \"legacy\"")
                        } else {
                            (-32601, "method not found")
                        };
                        JSONRPCMessage::Error(JSONRPCError {
                            id: request_id,
                            error: JSONRPCErrorError {
                                code,
                                data: None,
                                message: message.to_string(),
                            },
                        })
                    } else {
                        let request = serde_json::from_value::<ClientRequest>(
                            serde_json::to_value(request)?,
                        )?;
                        if let ClientRequest::ThreadList { params, .. } = &request
                            && let Some((root, started, release)) = blocked_thread_list.take()
                        {
                            assert_eq!(params.ancestor_thread_id, Some(root.to_string()));
                            assert_eq!(params.sort_direction, Some(SortDirection::Desc));
                            let _ = started.send(());
                            let _ = release.await;
                        }
                        let force_failure = matches!(
                            &request,
                            ClientRequest::ThreadSetName { params, .. }
                                if failed_thread_name == Some(params.name.as_str())
                        );
                        if force_failure {
                            JSONRPCMessage::Error(JSONRPCError {
                                id: request_id,
                                error: JSONRPCErrorError {
                                    code: -32603,
                                    message: "forced thread/name/set failure".to_string(),
                                    data: None,
                                },
                            })
                        } else {
                            match embedded.request(request).await? {
                                Ok(result) => JSONRPCMessage::Response(JSONRPCResponse {
                                    id: request_id,
                                    result,
                                }),
                                Err(error) => JSONRPCMessage::Error(JSONRPCError {
                                    id: request_id,
                                    error,
                                }),
                            }
                        }
                    };
                    websocket
                        .send(Message::Text(serde_json::to_string(&response)?.into()))
                        .await?;
                }
                JSONRPCMessage::Notification(notification)
                    if notification.method == "initialized" => {}
                JSONRPCMessage::Notification(notification) => {
                    embedded
                        .notify(serde_json::from_value::<ClientNotification>(
                            serde_json::to_value(notification)?,
                        )?)
                        .await?;
                }
                JSONRPCMessage::Response(_) | JSONRPCMessage::Error(_) => {}
            }
        }
        embedded.shutdown().await?;
        Ok(())
    });
    let app_server = crate::connect_remote_app_server(crate::RemoteAppServerEndpoint::WebSocket {
        websocket_url,
        auth_token: None,
    })
    .await?;

    Ok((
        AppServerSession::new(
            app_server,
            crate::app_server_session::ThreadParamsMode::Embedded,
        ),
        requests,
        proxy,
    ))
}

fn create_legacy_history_rollout(config: &Config) -> Result<ThreadId> {
    let thread_id = create_fake_rollout(
        config.codex_home.as_path(),
        "2026-01-02T00-00-00",
        "2026-01-02T00:00:00Z",
        "legacy history",
        Some(config.model_provider_id.as_str()),
        /*git_info*/ None,
    )
    .map_err(|err| color_eyre::eyre::eyre!("failed to create history rollout: {err}"))?;
    Ok(ThreadId::from_string(&thread_id)?)
}

fn recorded_params(requests: &RecordedRequests, method: &str) -> Vec<serde_json::Value> {
    requests
        .lock()
        .expect("request recorder lock")
        .iter()
        .filter(|request| request.method == method)
        .map(|request| request.params.clone().unwrap_or(serde_json::Value::Null))
        .collect()
}

async fn make_history_test_app() -> Result<(App, tempfile::TempDir)> {
    let mut app = make_test_app().await;
    let codex_home = tempdir()?;
    app.config.codex_home = codex_home.path().to_path_buf().abs();
    app.config.sqlite = SqliteConfig::new_for_testing(codex_home.path().abs());
    Ok((app, codex_home))
}

#[tokio::test]
async fn older_pagination_reconciles_review_prompts_across_page_boundaries() -> Result<()> {
    let (mut app, codex_home) = make_history_test_app().await?;
    app.config.terminal_resize_reflow.max_rows = TerminalResizeReflowMaxRows::Limit(100);
    let thread_id = create_fake_paginated_rollout(
        codex_home.path(),
        "2026-01-02T00-00-00",
        "2026-01-02T00:00:00Z",
        "older visible prompt",
        Some(app.config.model_provider_id.as_str()),
        /*git_info*/ None,
    )
    .map_err(|error| color_eyre::eyre::eyre!("failed to create paginated rollout: {error}"))?;
    let thread_id = ThreadId::from_string(&thread_id)?;
    let path = rollout_path(
        codex_home.path(),
        "2026-01-02T00-00-00",
        &thread_id.to_string(),
    );
    let mut records = std::fs::read_to_string(&path)?
        .lines()
        .map(serde_json::from_str::<serde_json::Value>)
        .collect::<Result<Vec<_>, _>>()?;
    let user_item = |id: &str, text: &str| {
        TurnItem::UserMessage(UserMessageItem {
            id: id.to_string(),
            client_id: None,
            content: vec![CoreUserInput::Text {
                text: text.to_string(),
                text_elements: Vec::new(),
            }],
        })
    };
    let mut items = vec![
        user_item("older-visible-prompt", "older visible prompt"),
        TurnItem::EnteredReviewMode(EnteredReviewModeItem {
            id: "cross-page-review-start".to_string(),
            target: ReviewTarget::UncommittedChanges,
            user_facing_hint: "review started".to_string(),
        }),
        user_item("hidden-review-prompt", "hidden cross-page review prompt"),
    ];
    items.extend((0..97).map(|index| {
        TurnItem::AgentMessage(AgentMessageItem {
            id: format!("review-output-{index}"),
            content: vec![AgentMessageContent::Text {
                text: format!("review output {index}"),
            }],
            phase: None,
            memory_citation: None,
        })
    }));
    items.extend([
        TurnItem::ExitedReviewMode(ExitedReviewModeItem {
            id: "cross-page-review-end".to_string(),
            review_output: None,
        }),
        user_item("newer-visible-prompt", "newer visible prompt"),
    ]);
    let events = std::iter::once(EventMsg::TurnStarted(TurnStartedEvent {
        turn_id: "cross-page-review-turn".to_string(),
        trace_id: None,
        started_at: None,
        model_context_window: None,
        collaboration_mode_kind: Default::default(),
    }))
    .chain(items.into_iter().map(|item| {
        EventMsg::ItemCompleted(ItemCompletedEvent {
            thread_id,
            turn_id: "cross-page-review-turn".to_string(),
            item,
            started_at_ms: None,
            completed_at_ms: 0,
        })
    }));
    for event in events {
        records.push(serde_json::json!({
            "timestamp": "2026-01-02T00:00:00Z",
            "ordinal": records.len(),
            "type": "event_msg",
            "payload": serde_json::to_value(event)?,
        }));
    }
    let records = records
        .into_iter()
        .map(|record| record.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(path, format!("{records}\n"))?;
    let (mut app_server, _requests, proxy) = start_recording_app_server(
        &app.config,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
    )
    .await?;
    let started = app_server
        .resume_thread(
            app.config.clone(),
            thread_id,
            crate::app_server_session::ResumeModelSettings::RestoreFromThread,
        )
        .await?;
    let initial_cells = crate::thread_transcript::thread_items_to_transcript_cells(
        Some(thread_id),
        &app.config.cwd,
        started.turns.iter().flat_map(|turn| turn.items.clone()),
        crate::thread_transcript::RawReasoningVisibility::Hidden,
        Some(app.config.codex_home.as_path()),
    );
    app.enqueue_primary_thread_session(started.session, started.turns)
        .await?;
    app.transcript_cells = initial_cells;
    assert_eq!(
        app.transcript_cells
            .iter()
            .filter_map(|cell| cell.as_any().downcast_ref::<UserHistoryCell>())
            .map(|user| user.message.as_str())
            .collect::<Vec<_>>(),
        vec!["hidden cross-page review prompt", "newer visible prompt"]
    );
    app.backtrack.overlay_preview_active = true;
    app.backtrack.nth_user_message = 1;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.open_transcript_overlay(&mut tui);
    app.apply_backtrack_selection_internal(app.backtrack.nth_user_message);
    let cursor = app_server
        .begin_older_history_page(thread_id)
        .expect("review-mode marker should remain on an older page");
    let request_id = app_server.next_request_id();
    let page: ThreadItemsListResponse = app_server
        .request_handle()
        .request_typed(ClientRequest::ThreadItemsList {
            request_id,
            params: ThreadItemsListParams {
                thread_id: thread_id.to_string(),
                turn_id: None,
                cursor: Some(cursor.clone()),
                limit: Some(crate::app_server_session::HISTORY_ITEM_PAGE_LIMIT),
                sort_direction: Some(SortDirection::Desc),
            },
        })
        .await?;
    app.handle_older_history_page(&mut tui, &mut app_server, thread_id, &cursor, Ok(page))
        .await?;

    let visible_user_messages = app
        .transcript_cells
        .iter()
        .filter_map(|cell| cell.as_any().downcast_ref::<UserHistoryCell>())
        .map(|user| user.message.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        visible_user_messages,
        vec!["older visible prompt", "newer visible prompt"]
    );
    assert_eq!(app.backtrack.nth_user_message, 1);
    let area = ratatui::layout::Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 12,
    );
    let mut buffer = Buffer::empty(area);
    let Some(Overlay::Transcript(overlay)) = app.overlay.as_mut() else {
        panic!("expected a transcript overlay");
    };
    overlay.render(area, &mut buffer);
    let highlighted_output = area
        .positions()
        .filter(|position| {
            buffer[*position]
                .style()
                .add_modifier
                .contains(ratatui::style::Modifier::REVERSED)
        })
        .map(|position| buffer[position].symbol())
        .collect::<String>();
    let highlighted_message = visible_user_messages
        .iter()
        .find(|message| highlighted_output.contains(message.as_str()))
        .expect("the selected user message should remain highlighted");
    insta::assert_snapshot!(
        format!(
            "{}\nhighlighted: {highlighted_message}",
            visible_user_messages.join("\n")
        ),
        @r"
    older visible prompt
    newer visible prompt
    highlighted: newer visible prompt
    ");

    app_server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn transcript_home_loads_every_older_history_page() -> Result<()> {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let codex_home = tempdir()?;
    app.config.codex_home = codex_home.path().to_path_buf().abs();
    app.config.sqlite = SqliteConfig::new_for_testing(codex_home.path().abs());
    app.config.terminal_resize_reflow.max_rows = TerminalResizeReflowMaxRows::Limit(2);
    let thread_id = create_fake_paginated_rollout(
        codex_home.path(),
        "2026-01-02T00-00-00",
        "2026-01-02T00:00:00Z",
        "multi-page transcript",
        Some(app.config.model_provider_id.as_str()),
        /*git_info*/ None,
    )
    .map_err(|error| color_eyre::eyre::eyre!("failed to create paginated rollout: {error}"))?;
    let thread_id = ThreadId::from_string(&thread_id)?;
    let path = rollout_path(
        codex_home.path(),
        "2026-01-02T00-00-00",
        &thread_id.to_string(),
    );
    let mut records = std::fs::read_to_string(&path)?
        .lines()
        .map(serde_json::from_str::<serde_json::Value>)
        .collect::<Result<Vec<_>, _>>()?;
    let events = std::iter::once(EventMsg::TurnStarted(TurnStartedEvent {
        turn_id: "multi-page-turn".to_string(),
        trace_id: None,
        started_at: None,
        model_context_window: None,
        collaboration_mode_kind: Default::default(),
    }))
    .chain((0..305).map(|index| {
        EventMsg::ItemCompleted(ItemCompletedEvent {
            thread_id,
            turn_id: "multi-page-turn".to_string(),
            item: TurnItem::AgentMessage(AgentMessageItem {
                id: format!("history-item-{index}"),
                content: vec![AgentMessageContent::Text {
                    text: format!("history output {index}"),
                }],
                phase: None,
                memory_citation: None,
            }),
            started_at_ms: None,
            completed_at_ms: 0,
        })
    }));
    for event in events {
        records.push(serde_json::json!({
            "timestamp": "2026-01-02T00:00:00Z",
            "ordinal": records.len(),
            "type": "event_msg",
            "payload": serde_json::to_value(event)?,
        }));
    }
    let records = records
        .into_iter()
        .map(|record| record.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(path, format!("{records}\n"))?;

    let (mut app_server, requests, proxy) = start_recording_app_server(
        &app.config,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
    )
    .await?;
    let started = app_server
        .resume_thread(
            app.config.clone(),
            thread_id,
            crate::app_server_session::ResumeModelSettings::RestoreFromThread,
        )
        .await?;
    let initial_cells = crate::thread_transcript::thread_items_to_transcript_cells(
        Some(thread_id),
        &app.config.cwd,
        started.turns.iter().flat_map(|turn| turn.items.clone()),
        crate::thread_transcript::RawReasoningVisibility::Hidden,
        Some(app.config.codex_home.as_path()),
    );
    app.enqueue_primary_thread_session(started.session, started.turns)
        .await?;
    app.transcript_cells = initial_cells;
    app.scrollback_has_older_history = app_server.has_older_history(thread_id);
    assert!(app.scrollback_has_older_history);
    while app_event_rx.try_recv().is_ok() {}
    let initial_page_requests = recorded_params(&requests, "thread/items/list").len();
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.open_transcript_overlay(&mut tui);

    app.handle_backtrack_overlay_event(
        &mut tui,
        &mut app_server,
        TuiEvent::Key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE)),
    )
    .await?;
    while app_server.has_older_history(thread_id) {
        let event = tokio::time::timeout(Duration::from_secs(5), app_event_rx.recv())
            .await?
            .ok_or_else(|| color_eyre::eyre::eyre!("history event channel closed"))?;
        if matches!(event, AppEvent::OlderThreadHistoryLoaded { .. }) {
            app.handle_event(&mut tui, &mut app_server, event).await?;
        }
    }

    assert!(recorded_params(&requests, "thread/items/list").len() >= initial_page_requests + 3);
    assert!(app.transcript_cells.iter().any(|cell| {
        cell.display_lines(/*width*/ 80)
            .iter()
            .any(|line| line.to_string().contains("history output 0"))
    }));
    let Some(Overlay::Transcript(overlay)) = app.overlay.as_mut() else {
        panic!("expected transcript overlay after Home navigation");
    };
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 12,
    );
    let mut buffer = Buffer::empty(area);
    overlay.render(area, &mut buffer);
    let visible = (area.y..area.bottom())
        .map(|y| {
            (area.x..area.right())
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(visible.contains("history output 0"), "{visible}");
    assert!(!visible.contains("history output 304"), "{visible}");
    app_server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn remote_legacy_history_start_negotiates_once_for_resume_and_fork() -> Result<()> {
    let (app, _codex_home) = make_history_test_app().await?;
    let legacy_thread_id = create_legacy_history_rollout(&app.config)?;
    let (mut app_server, requests, proxy) = start_recording_app_server_with_history(
        &app.config,
        HistoryCapabilities::LegacyOnly,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
    )
    .await?;

    let started = app_server.start_thread(&app.config).await?;
    let resumed = app_server
        .resume_thread(
            app.config.clone(),
            legacy_thread_id,
            crate::app_server_session::ResumeModelSettings::RestoreFromThread,
        )
        .await?;
    let forked = app_server
        .fork_thread(app.config.clone(), legacy_thread_id)
        .await?;

    assert_ne!(started.session.thread_id, legacy_thread_id);
    assert_eq!(resumed.session.thread_id, legacy_thread_id);
    assert_ne!(forked.session.thread_id, legacy_thread_id);
    let starts = recorded_params(&requests, "thread/start");
    assert_eq!(starts.len(), 2);
    assert_eq!(starts[0]["historyMode"], "paginated");
    assert_eq!(starts[1]["historyMode"], serde_json::Value::Null);

    for method in ["thread/resume", "thread/fork"] {
        let params = recorded_params(&requests, method);
        assert_eq!(params.len(), 1, "legacy {method} must not be reprobed");
        assert_ne!(params[0]["excludeTurns"], true);
    }
    assert!(recorded_params(&requests, "thread/turns/list").is_empty());
    assert!(recorded_params(&requests, "thread/items/list").is_empty());

    app_server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn remote_legacy_history_start_retries_unsupported_paginated_variant() -> Result<()> {
    let (app, _codex_home) = make_history_test_app().await?;
    let (mut app_server, requests, proxy) = start_recording_app_server_with_history(
        &app.config,
        HistoryCapabilities::LegacyOnlyUnsupportedVariant,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
    )
    .await?;

    app_server.start_thread(&app.config).await?;

    let starts = recorded_params(&requests, "thread/start");
    assert_eq!(starts.len(), 2);
    assert_eq!(starts[0]["historyMode"], "paginated");
    assert_eq!(starts[1]["historyMode"], serde_json::Value::Null);

    app_server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LegacyHistoryRequest {
    Resume,
    Fork,
}

async fn assert_remote_legacy_history_retry(request: LegacyHistoryRequest) -> Result<()> {
    let (app, _codex_home) = make_history_test_app().await?;
    let legacy_thread_id = create_legacy_history_rollout(&app.config)?;
    let (mut app_server, requests, proxy) = start_recording_app_server_with_history(
        &app.config,
        HistoryCapabilities::LegacyOnly,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
    )
    .await?;

    let method = match request {
        LegacyHistoryRequest::Resume => {
            let resumed = app_server
                .resume_thread(
                    app.config.clone(),
                    legacy_thread_id,
                    crate::app_server_session::ResumeModelSettings::RestoreFromThread,
                )
                .await?;
            assert_eq!(resumed.session.thread_id, legacy_thread_id);
            "thread/resume"
        }
        LegacyHistoryRequest::Fork => {
            let forked = app_server
                .fork_thread(app.config.clone(), legacy_thread_id)
                .await?;
            assert_ne!(forked.session.thread_id, legacy_thread_id);
            "thread/fork"
        }
    };
    let attempts = recorded_params(&requests, method);
    if request == LegacyHistoryRequest::Resume {
        assert_eq!(attempts.len(), 2);
        assert_eq!(attempts[0]["excludeTurns"], true);
    } else {
        assert_eq!(attempts.len(), 1);
    }
    assert_ne!(
        attempts.last().expect("history request")["excludeTurns"],
        true
    );
    assert!(recorded_params(&requests, "thread/turns/list").is_empty());
    assert!(recorded_params(&requests, "thread/items/list").is_empty());

    app_server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn remote_legacy_history_resume_retries_generic_method_not_found() -> Result<()> {
    assert_remote_legacy_history_retry(LegacyHistoryRequest::Resume).await
}

#[tokio::test]
async fn remote_legacy_history_fork_avoids_unsupported_fields() -> Result<()> {
    assert_remote_legacy_history_retry(LegacyHistoryRequest::Fork).await
}
#[test]
fn fresh_session_applies_requested_name() -> Result<()> {
    const TEST_STACK_SIZE_BYTES: usize = 8 * 1024 * 1024;

    std::thread::Builder::new()
        .name("tui-named-fresh-session".to_string())
        .stack_size(TEST_STACK_SIZE_BYTES)
        .spawn(|| {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            runtime.block_on(async {
                let mut app = make_test_app().await;
                let codex_home = tempdir()?;
                app.config.codex_home = codex_home.path().to_path_buf().abs();
                app.config.sqlite = SqliteConfig::new_for_testing(codex_home.path().abs());
                let (mut app_server, requests, proxy) = start_recording_app_server(
                    &app.config,
                    /*blocked_thread_list*/ None,
                    /*failed_thread_name*/ None,
                )
                .await?;
                let mut tui = crate::tui::test_support::make_test_tui()?;

                app.start_fresh_session_with_summary_hint(
                    &mut tui,
                    &mut app_server,
                    /*session_start_source*/ None,
                    /*initial_user_message*/ None,
                    /*new_thread_name*/ Some("Add User".to_string()),
                )
                .await;

                let thread_id = app
                    .chat_widget
                    .thread_id()
                    .expect("fresh session should have a thread id");
                assert_eq!(app.chat_widget.thread_name(), Some("Add User".to_string()));
                assert!(
                    requests
                        .lock()
                        .expect("request recorder lock")
                        .iter()
                        .any(|request| request.method == "thread/name/set"),
                    "fresh session should be named through the app server"
                );
                let thread = app_server
                    .thread_read(thread_id, /*include_turns*/ false)
                    .await?;
                assert_eq!(thread.name.as_deref(), Some("Add User"));

                app_server.shutdown().await?;
                proxy.await??;
                Ok(())
            })
        })?
        .join()
        .expect("named fresh session test thread")
}

#[test]
fn session_lifecycle_avoids_redundant_subagent_metadata_reads() -> Result<()> {
    const TEST_STACK_SIZE_BYTES: usize = 8 * 1024 * 1024;

    std::thread::Builder::new()
        .name("tui-session-lifecycle-requests".to_string())
        .stack_size(TEST_STACK_SIZE_BYTES)
        .spawn(|| {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            runtime.block_on(async {
                let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
                let codex_home = tempdir()?;
                app.config.codex_home = codex_home.path().to_path_buf().abs();
                app.config.sqlite =
                    codex_state::SqliteConfig::new_for_testing(codex_home.path().abs());
                let root_timestamp = "2026-01-01T00-00-00";
                let root_thread_id = ThreadId::from_string(
                    &create_fake_rollout(
                        codex_home.path(),
                        root_timestamp,
                        "2026-01-01T00:00:00Z",
                        "Saved user message",
                        Some(app.config.model_provider_id.as_str()),
                        /*git_info*/ None,
                    )
                    .expect("create root rollout"),
                )?;
                let child_thread_id = ThreadId::from_string(
                    &create_fake_parented_rollout_with_source(
                        codex_home.path(),
                        "2026-01-01T00-00-01",
                        "2026-01-01T00:00:01Z",
                        "Saved child message",
                        Some(app.config.model_provider_id.as_str()),
                        /*git_info*/ None,
                        RolloutSessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                            parent_thread_id: root_thread_id,
                            depth: 1,
                            agent_path: Some(
                                AgentPath::try_from("/root/worker").expect("valid agent path"),
                            ),
                            agent_nickname: Some("worker".to_string()),
                            agent_role: Some("worker".to_string()),
                        }),
                        root_thread_id.into(),
                        root_thread_id,
                    )
                    .expect("create child rollout"),
                )?;
                let root_rollout_path = rollout_path(
                    codex_home.path(),
                    root_timestamp,
                    &root_thread_id.to_string(),
                );
                let (started_tx, started_rx) = oneshot::channel();
                let (release_tx, release_rx) = oneshot::channel();
                let (mut app_server, requests, proxy) = start_recording_app_server(
                    &app.config,
                    Some((root_thread_id, started_tx, release_rx)),
                    Some("Failed Fork"),
                )
                .await?;
                let root = app_server
                    .resume_thread(
                        app.config.clone(),
                        root_thread_id,
                        app.resume_model_settings(),
                    )
                    .await?;
                app.enqueue_primary_thread_session(root.session, root.turns)
                    .await?;
                app_server
                    .resume_thread(
                        app.config.clone(),
                        child_thread_id,
                        app.resume_model_settings(),
                    )
                    .await?;
                let mut tui = crate::tui::test_support::make_test_tui()?;
                take_backfill_counts(&requests);

                let control = Box::pin(app.handle_event(
                    &mut tui,
                    &mut app_server,
                    AppEvent::ForkCurrentSession {
                        name: Some("Add User Fork".to_string()),
                    },
                ))
                .await?;

                assert!(matches!(control, AppRunControl::Continue));
                assert_ne!(app.chat_widget.thread_id(), Some(root_thread_id));
                let named_fork_id = app
                    .chat_widget
                    .thread_id()
                    .expect("named fork should have a thread id");
                assert_eq!(
                    app.chat_widget.thread_name(),
                    Some("Add User Fork".to_string())
                );
                // Forking may read the source metadata once when the response includes its parent
                // id. It must not scan or backfill loaded threads for the newly created fork.
                assert!(matches!(take_backfill_counts(&requests), (0, 0) | (0, 1)));
                let named_fork = app_server
                    .thread_read(named_fork_id, /*include_turns*/ false)
                    .await?;
                assert_eq!(named_fork.name.as_deref(), Some("Add User Fork"));
                take_backfill_counts(&requests);

                let control = Box::pin(app.handle_event(
                    &mut tui,
                    &mut app_server,
                    AppEvent::ForkCurrentSession {
                        name: Some("Failed Fork".to_string()),
                    },
                ))
                .await?;

                assert!(matches!(control, AppRunControl::Continue));
                assert_ne!(app.chat_widget.thread_id(), Some(named_fork_id));
                let name_error = std::iter::from_fn(|| app_event_rx.try_recv().ok())
                    .find_map(|event| match event {
                        AppEvent::InsertHistoryCell(cell) => {
                            let rendered =
                                lines_to_single_string(&cell.display_lines(/*width*/ 80));
                            rendered
                                .contains("Failed to name the forked session")
                                .then_some(rendered)
                        }
                        _ => None,
                    })
                    .expect("fork naming error history cell");
                insta::assert_snapshot!(
                    name_error,
                    @"■ Failed to name the forked session: thread/name/set failed in TUI"
                );
                assert!(matches!(take_backfill_counts(&requests), (0, 0) | (0, 1)));

                app.start_fresh_session_with_summary_hint(
                    &mut tui,
                    &mut app_server,
                    /*session_start_source*/ None,
                    /*initial_user_message*/ None,
                    /*new_thread_name*/ None,
                )
                .await;

                assert_ne!(app.chat_widget.thread_id(), Some(root_thread_id));
                assert_eq!(take_backfill_counts(&requests), (0, 0));

                let loaded_threads = app_server
                    .thread_loaded_list(ThreadLoadedListParams {
                        cursor: None,
                        limit: None,
                    })
                    .await?
                    .data;
                let expected_reads = loaded_threads
                    .iter()
                    .filter(|thread_id| *thread_id != &root_thread_id.to_string())
                    .count();
                assert!(loaded_threads.contains(&child_thread_id.to_string()));
                take_backfill_counts(&requests);
                app.harness_overrides.cwd = Some(app.config.cwd.to_path_buf());

                let control = app
                    .resume_target_session(
                        &mut tui,
                        &mut app_server,
                        crate::resume_picker::SessionTarget {
                            path: Some(root_rollout_path),
                            thread_id: root_thread_id,
                        },
                    )
                    .await?;

                assert!(matches!(control, AppRunControl::Continue));
                assert_eq!(app.chat_widget.thread_id(), Some(root_thread_id));
                assert_eq!(take_backfill_counts(&requests), (1, expected_reads));
                assert_eq!(
                    app.agent_navigation.get(&child_thread_id),
                    Some(&AgentPickerThreadEntry {
                        agent_nickname: Some("worker".to_string()),
                        agent_role: Some("worker".to_string()),
                        agent_path: Some("/root/worker".to_string()),
                        is_running: false,
                        is_closed: false,
                    })
                );

                let child_store = Arc::clone(
                    &app.thread_event_channels
                        .entry(child_thread_id)
                        .or_insert_with(|| ThreadEventChannel::new(/*capacity*/ 1))
                        .store,
                );
                let child_store_guard = child_store.lock().await;
                futures::FutureExt::now_or_never(app.open_agent_picker(&mut app_server))
                    .expect("opening the agent picker waited for the app server");
                drop(child_store_guard);
                insta::assert_snapshot!(
                    render_bottom_popup(&app.chat_widget, /*width*/ 80)
                        .replace(&root_thread_id.to_string(), "[root]")
                        .replace(&child_thread_id.to_string(), "[child]"),
                    @r###"
                      Subagents
                      Select an agent to watch. ⌥ + ← previous, ⌥ + → next.

                    › 1. • Main [default] (current)  [root]
                      2. • /root/worker              [child]

                      Press enter to confirm or esc to go back
                    "###
                );
                assert_eq!(take_backfill_counts(&requests), (0, 0));
                tokio::time::timeout(Duration::from_secs(5), started_rx).await??;
                app.chat_widget
                    .handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
                futures::FutureExt::now_or_never(app.open_agent_picker(&mut app_server))
                    .expect("reopening the agent picker waited for the app server");
                assert_eq!(
                    requests
                        .lock()
                        .expect("request recorder lock")
                        .iter()
                        .filter(|request| request.method == "thread/list")
                        .count(),
                    1
                );
                app.chat_widget
                    .handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
                app.chat_widget.handle_server_request(
                    exec_approval_request(
                        root_thread_id,
                        "turn",
                        "item",
                        /*approval_id*/ None,
                    ),
                    /*replay_kind*/ None,
                );
                app.agent_navigation.mark_stopped(child_thread_id);
                release_tx.send(()).expect("release blocked thread list");
                let discovered_thread_id = ThreadId::new();
                let mut completion = tokio::time::timeout(Duration::from_secs(5), async {
                    loop {
                        let event = app_event_rx.recv().await.expect("app event channel");
                        if matches!(event, AppEvent::AgentPickerThreadsLoaded { .. }) {
                            break event;
                        }
                    }
                })
                .await?;
                if let AppEvent::AgentPickerThreadsLoaded {
                    result: Ok(threads),
                    ..
                } = &mut completion
                {
                    let child = threads
                        .iter_mut()
                        .find(|thread| thread.id == child_thread_id.to_string())
                        .expect("root-scoped response includes the cached child");
                    let mut discovered = child.clone();
                    discovered.id = discovered_thread_id.to_string();
                    discovered.can_accept_direct_input = None;
                    child.status = ThreadStatus::Active {
                        active_flags: Vec::new(),
                    };
                    threads.push(discovered);
                }
                app.handle_event(&mut tui, &mut app_server, completion)
                    .await?;
                assert_eq!(
                    app.agent_navigation
                        .ordered_threads()
                        .last()
                        .map(|(thread_id, _)| *thread_id),
                    Some(discovered_thread_id)
                );
                assert!(!app.agent_navigation.is_parent_owned(discovered_thread_id));
                assert_eq!(
                    app.chat_widget.selected_index_for_present_view(
                        super::super::agent_picker::AGENT_PICKER_VIEW_ID
                    ),
                    Some(1)
                );
                assert!(render_bottom_popup(&app.chat_widget, /*width*/ 80).contains("echo hello"));
                assert!(
                    app.agent_navigation
                        .get(&child_thread_id)
                        .is_some_and(|entry| !entry.is_running)
                );
                app_server.shutdown().await?;
                proxy.await??;
                Ok(())
            })
        })?
        .join()
        .expect("session lifecycle request test thread")
}
