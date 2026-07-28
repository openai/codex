use super::*;
use app_test_support::create_fake_parented_rollout_with_source;
use app_test_support::create_fake_rollout;
use app_test_support::rollout_path;
use codex_app_server_protocol::ClientNotification;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::SortDirection;
use codex_app_server_protocol::ThreadStatus;
use codex_protocol::AgentPath;
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

/// Returns and resets `(thread/loaded/list, thread/read)` request counts.
fn take_backfill_counts(requests: &Arc<Mutex<Vec<String>>>) -> (usize, usize) {
    let requests = std::mem::take(&mut *requests.lock().expect("request recorder lock"));
    (
        requests
            .iter()
            .filter(|method| *method == "thread/loaded/list")
            .count(),
        requests
            .iter()
            .filter(|method| *method == "thread/read")
            .count(),
    )
}

/// Starts an embedded app server behind a loopback WebSocket proxy that records JSON-RPC methods.
async fn start_recording_app_server(
    config: &Config,
    mut blocked_thread_list: Option<(ThreadId, oneshot::Sender<()>, oneshot::Receiver<()>)>,
) -> Result<(
    AppServerSession,
    Arc<Mutex<Vec<String>>>,
    JoinHandle<Result<()>>,
)> {
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
                        .push(request.method.clone());
                    let request_id = request.id.clone();
                    let request =
                        serde_json::from_value::<ClientRequest>(serde_json::to_value(request)?)?;
                    if let ClientRequest::ThreadList { params, .. } = &request
                        && let Some((root, started, release)) = blocked_thread_list.take()
                    {
                        assert_eq!(params.ancestor_thread_id, Some(root.to_string()));
                        assert_eq!(params.sort_direction, Some(SortDirection::Desc));
                        let _ = started.send(());
                        let _ = release.await;
                    }
                    let response = match embedded.request(request).await? {
                        Ok(result) => JSONRPCMessage::Response(JSONRPCResponse {
                            id: request_id,
                            result,
                        }),
                        Err(error) => JSONRPCMessage::Error(JSONRPCError {
                            id: request_id,
                            error,
                        }),
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
                let (mut app_server, requests, proxy) =
                    start_recording_app_server(&app.config, /*blocked_thread_list*/ None).await?;
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
                        .any(|method| method == "thread/name/set"),
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
                    AppEvent::ForkCurrentSession,
                ))
                .await?;

                assert!(matches!(control, AppRunControl::Continue));
                assert_ne!(app.chat_widget.thread_id(), Some(root_thread_id));
                // Forking may read the source metadata once when the response includes its parent
                // id. It must not scan or backfill loaded threads for the newly created fork.
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
                        .filter(|method| *method == "thread/list")
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
