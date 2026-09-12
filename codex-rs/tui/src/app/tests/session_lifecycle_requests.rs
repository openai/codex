use super::*;
use crate::app_event::TranscriptExportDestination;
use crate::bottom_pane::BottomPaneView;
use app_test_support::create_fake_paginated_rollout;
use app_test_support::create_fake_parented_rollout_with_source;
use app_test_support::create_fake_rollout;
use app_test_support::rollout_path;
use codex_app_server_client::AppServerEvent;
use codex_app_server_protocol::ClientNotification;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCRequest;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::SortDirection;
use codex_app_server_protocol::ThreadHistoryMode;
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

pub(super) type RecordedRequests = Arc<Mutex<Vec<JSONRPCRequest>>>;
pub(super) type RecordingAppServer = (AppServerSession, RecordedRequests, JoinHandle<Result<()>>);

async fn complete_managed_worktree_creation(
    app: &mut App,
    tui: &mut crate::tui::Tui,
    server: &mut AppServerSession,
    events: &mut tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
) -> Result<()> {
    drain_managed_worktree_start(app, server).await;
    assert!(
        app.pending_managed_worktree_creation,
        "checkout task started"
    );
    loop {
        let event = tokio::time::timeout(Duration::from_secs(15), events.recv())
            .await?
            .ok_or_else(|| color_eyre::eyre::eyre!("worktree creation event channel closed"))?;
        if matches!(event, AppEvent::ManagedWorktreeCreated(_)) {
            app.handle_event(tui, server, event).await?;
            if let Some(created) = app.pending_managed_worktree_created.take() {
                app.finish_managed_worktree(*created).await;
            }
            if let Some(transition) = app.pending_managed_worktree_transition.take() {
                assert!(app.pending_managed_worktree_attach.is_none());
                if let Err(error) = app
                    .switch_to_managed_worktree(tui, server, *transition)
                    .await
                {
                    app.chat_widget.add_error_message(error.to_string());
                }
                if let Some(attach) = app.pending_managed_worktree_attach.take() {
                    app.attach_working_directory(tui, server, *attach).await;
                }
            }
            return Ok(());
        }
    }
}

#[tokio::test]
async fn same_thread_retry_keeps_subscription_and_restores_draft() -> Result<()> {
    let (mut app, codex_home) = make_history_test_app().await?;
    let thread_id = ThreadId::from_string(
        &create_fake_rollout(
            codex_home.path(),
            "2026-01-01T00-00-00",
            "2026-01-01T00:00:00Z",
            "Saved user message",
            Some(app.config.model_provider_id.as_str()),
            /*git_info*/ None,
        )
        .expect("create rollout"),
    )?;
    let path = Some(rollout_path(
        codex_home.path(),
        "2026-01-01T00-00-00",
        &thread_id.to_string(),
    ));
    let (mut app_server, requests, proxy) = start_recording_app_server(
        &app.config,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
    )
    .await?;
    app.enqueue_primary_thread_session(
        test_thread_session(thread_id, test_path_buf("/tmp/project")),
        Vec::new(),
    )
    .await?;
    app.ensure_thread_channel(thread_id).mark_external_writer();
    app.chat_widget.insert_str("Retained draft");
    app.chat_widget.show_external_writer_thread();
    crate::legacy_core::config::set_project_trust_level(
        app.config.codex_home.as_path(),
        app.config.cwd.as_path(),
        codex_protocol::config_types::TrustLevel::Trusted,
    )
    .map_err(std::io::Error::other)?;
    app.harness_overrides.cwd = Some(app.config.cwd.to_path_buf());
    requests.lock().expect("request recorder lock").clear();
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.resume_target_session(
        &mut tui,
        &mut app_server,
        crate::resume_picker::SessionTarget {
            path,
            thread_id,
            cwd: None,
            history_mode: None,
        },
    )
    .await?;
    assert_eq!(recorded_params(&requests, "thread/resume").len(), 1);
    assert!(recorded_params(&requests, "thread/unsubscribe").is_empty());
    assert!(!app.chat_widget.is_external_writer_view());
    assert_eq!(
        app.chat_widget.composer_text_with_pending(),
        "Retained draft"
    );
    proxy.abort();
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HistoryCapabilities {
    Current,
    LegacyOnly,
    LegacyOnlyUnsupportedVariant,
    LegacyDynamicToolsAndHistory,
    ForkHydrationFails,
    ReadAfterResumeFails,
    ThreadListFails,
    ThreadStartFails,
    ConfigReadUnsupported(i64),
    ConfigReadFails,
    ConfigReadUnknownVoice,
    VoiceCatalogCustom,
    VoiceCatalogUnavailable,
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
pub(super) async fn start_recording_app_server(
    config: &Config,
    blocked_thread_list: Option<(ThreadId, oneshot::Sender<()>, oneshot::Receiver<()>)>,
    failed_thread_name: Option<&'static str>,
) -> Result<RecordingAppServer> {
    start_recording_app_server_with_history(
        config,
        HistoryCapabilities::Current,
        blocked_thread_list,
        failed_thread_name,
        crate::app_server_session::ThreadParamsMode::Embedded,
        LoaderOverrides::default(),
    )
    .await
}

pub(super) async fn start_recording_remote_app_server(
    config: &Config,
) -> Result<RecordingAppServer> {
    start_recording_app_server_with_history(
        config,
        HistoryCapabilities::Current,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
        crate::app_server_session::ThreadParamsMode::Remote,
        LoaderOverrides::default(),
    )
    .await
}

/// Proxies a real app server while optionally rejecting modern pagination like an older server.
pub(super) async fn start_recording_app_server_with_history(
    config: &Config,
    history_capabilities: HistoryCapabilities,
    blocked_thread_list: Option<(ThreadId, oneshot::Sender<()>, oneshot::Receiver<()>)>,
    failed_thread_name: Option<&'static str>,
    thread_params_mode: crate::app_server_session::ThreadParamsMode,
    loader_overrides: LoaderOverrides,
) -> Result<RecordingAppServer> {
    start_recording_app_server_with_realtime_speech(
        config,
        history_capabilities,
        blocked_thread_list,
        failed_thread_name,
        thread_params_mode,
        RealtimeRequestBehavior::Forward,
        loader_overrides,
    )
    .await
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum RealtimeRequestBehavior {
    Forward,
    AcceptStart,
    AcceptSpeech,
    AcceptSpeechAndStallStop,
}

pub(super) async fn start_recording_realtime_speech_app_server(
    config: &Config,
    realtime_behavior: RealtimeRequestBehavior,
) -> Result<RecordingAppServer> {
    start_recording_app_server_with_realtime_speech(
        config,
        HistoryCapabilities::Current,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
        crate::app_server_session::ThreadParamsMode::Embedded,
        realtime_behavior,
        LoaderOverrides::default(),
    )
    .await
}

pub(super) async fn start_recording_app_server_with_realtime_speech(
    config: &Config,
    history_capabilities: HistoryCapabilities,
    mut blocked_thread_list: Option<(ThreadId, oneshot::Sender<()>, oneshot::Receiver<()>)>,
    failed_thread_name: Option<&'static str>,
    thread_params_mode: crate::app_server_session::ThreadParamsMode,
    realtime_behavior: RealtimeRequestBehavior,
    loader_overrides: LoaderOverrides,
) -> Result<RecordingAppServer> {
    let state_db =
        crate::init_state_db_for_app_server_target(config, &crate::AppServerTarget::Embedded)
            .await?;
    let embedded = crate::start_embedded_app_server(
        codex_arg0::Arg0DispatchPaths::default(),
        config.clone(),
        Vec::new(),
        loader_overrides,
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
        let mut inventories = usize::from(failed_thread_name == Some("background"));
        let mut reject_detach = false;
        let mut reject_thread_list = history_capabilities == HistoryCapabilities::ThreadListFails;
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
                    if realtime_behavior == RealtimeRequestBehavior::AcceptSpeechAndStallStop
                        && request.method == "thread/realtime/stop"
                    {
                        continue;
                    }
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
                    let reject_fork_hydration = history_capabilities
                        == HistoryCapabilities::ForkHydrationFails
                        && request.method == "thread/items/list"
                        && request_sink
                            .lock()
                            .expect("request recorder lock")
                            .iter()
                            .any(|recorded| recorded.method == "thread/fork");
                    let reject_dynamic_tools = history_capabilities
                        == HistoryCapabilities::LegacyDynamicToolsAndHistory
                        && request.method == "thread/start"
                        && params
                            .and_then(|params| params.get("dynamicTools"))
                            .and_then(serde_json::Value::as_array)
                            .is_some_and(|tools| {
                                tools.iter().any(|tool| tool["type"] == "namespace")
                            });
                    let fake_realtime_response = (realtime_behavior
                        == RealtimeRequestBehavior::AcceptStart
                        && request.method == "thread/realtime/start")
                        || (matches!(
                            realtime_behavior,
                            RealtimeRequestBehavior::AcceptSpeech
                                | RealtimeRequestBehavior::AcceptSpeechAndStallStop
                        ) && request.method == "thread/realtime/appendSpeech");
                    let response = if fake_realtime_response {
                        JSONRPCMessage::Response(JSONRPCResponse {
                            id: request_id,
                            result: serde_json::json!({}),
                        })
                    } else if history_capabilities == HistoryCapabilities::ReadAfterResumeFails
                        && request.method == "thread/read"
                        && request_sink
                            .lock()
                            .expect("request recorder lock")
                            .iter()
                            .any(|recorded| recorded.method == "thread/resume")
                    {
                        JSONRPCMessage::Error(JSONRPCError {
                            id: request_id,
                            error: JSONRPCErrorError {
                                code: -32603,
                                data: None,
                                message: "saved history unavailable".to_string(),
                            },
                        })
                    } else if let HistoryCapabilities::ConfigReadUnsupported(code) =
                        history_capabilities
                        && request.method == "config/read"
                    {
                        JSONRPCMessage::Error(JSONRPCError {
                            id: request_id,
                            error: JSONRPCErrorError {
                                code,
                                data: None,
                                message: "unknown variant `config/read`".to_string(),
                            },
                        })
                    } else if history_capabilities == HistoryCapabilities::ConfigReadFails
                        && request.method == "config/read"
                    {
                        JSONRPCMessage::Error(JSONRPCError {
                            id: request_id,
                            error: JSONRPCErrorError {
                                code: -32603,
                                data: None,
                                message: "config temporarily unavailable".to_string(),
                            },
                        })
                    } else if history_capabilities == HistoryCapabilities::VoiceCatalogUnavailable
                        && request.method == "thread/realtime/listVoices"
                    {
                        JSONRPCMessage::Error(JSONRPCError {
                            id: request_id,
                            error: JSONRPCErrorError {
                                code: -32601,
                                data: None,
                                message: "method not found".to_string(),
                            },
                        })
                    } else if history_capabilities == HistoryCapabilities::ThreadStartFails
                        && request.method == "thread/start"
                    {
                        JSONRPCMessage::Error(JSONRPCError {
                            id: request_id,
                            error: JSONRPCErrorError {
                                code: -32603,
                                data: None,
                                message: "replacement unavailable".to_string(),
                            },
                        })
                    } else if request.method == "thread/list"
                        && std::mem::take(&mut reject_thread_list)
                    {
                        JSONRPCMessage::Error(JSONRPCError {
                            id: request_id,
                            error: JSONRPCErrorError {
                                code: -32603,
                                data: None,
                                message: "thread listing unavailable".to_string(),
                            },
                        })
                    } else if history_capabilities == HistoryCapabilities::LegacyOnly
                        && request.method == "thread/list"
                        && params.is_some_and(|params| params["sortKey"] == "recency_at")
                    {
                        JSONRPCMessage::Error(JSONRPCError {
                            id: request_id,
                            error: JSONRPCErrorError {
                                code: -32602,
                                data: None,
                                message: "unknown variant `recency_at`".to_string(),
                            },
                        })
                    } else if reject_dynamic_tools {
                        JSONRPCMessage::Error(JSONRPCError {
                            id: request_id,
                            error: JSONRPCErrorError {
                                code: -32602,
                                data: None,
                                message: "missing field `inputSchema`".to_string(),
                            },
                        })
                    } else if matches!(
                        history_capabilities,
                        HistoryCapabilities::LegacyOnly
                            | HistoryCapabilities::LegacyOnlyUnsupportedVariant
                            | HistoryCapabilities::LegacyDynamicToolsAndHistory
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
                    } else if reject_fork_hydration {
                        JSONRPCMessage::Error(JSONRPCError {
                            id: request_id,
                            error: JSONRPCErrorError {
                                code: -32603,
                                data: None,
                                message: "fork history hydration failed".to_string(),
                            },
                        })
                    } else {
                        let background = request.method == "thread/backgroundTerminals/list" && {
                            inventories += usize::from(inventories > 0);
                            matches!(inventories, 2 | 4)
                        };
                        let detach = request.method == "thread/unsubscribe";
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
                        ) || matches!(
                            &request,
                            ClientRequest::ThreadFork { params, .. }
                                if params.cwd.as_deref().is_some_and(|cwd| cwd.ends_with("failure"))
                                    && { reject_detach = true; true }
                        ) || (detach && std::mem::take(&mut reject_detach));
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
                            let unknown_voice = history_capabilities
                                == HistoryCapabilities::ConfigReadUnknownVoice
                                && matches!(&request, ClientRequest::ConfigRead { .. });
                            let custom_voice_catalog = history_capabilities
                                == HistoryCapabilities::VoiceCatalogCustom
                                && matches!(
                                    &request,
                                    ClientRequest::ThreadRealtimeListVoices { .. }
                                );
                            let mut result = embedded.request(request).await?;
                            if unknown_voice && let Ok(value) = &mut result {
                                value["config"]["realtime"]["voice"] =
                                    serde_json::json!("future_voice");
                            }
                            if custom_voice_catalog && let Ok(value) = &mut result {
                                value["voices"]["v1"] =
                                    serde_json::json!(["maple", "cove", "juniper"]);
                                value["voices"]["defaultV1"] = serde_json::json!("maple");
                            }
                            if background {
                                let terminal = r#"{"data":[{"itemId":"x","processId":"x","command":"x","cwd":"/"}],"nextCursor":null}"#;
                                result = Ok(serde_json::from_str(terminal)?);
                            }
                            match result {
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
                JSONRPCMessage::Response(response) => {
                    request_sink
                        .lock()
                        .expect("request recorder lock")
                        .push(JSONRPCRequest {
                            id: response.id,
                            method: "server/request/response".to_string(),
                            params: Some(response.result),
                            trace: None,
                        });
                }
                JSONRPCMessage::Error(_) => {}
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
        AppServerSession::new(app_server, thread_params_mode)
            .with_local_codex_home(&config.codex_home),
        requests,
        proxy,
    ))
}

fn create_history_rollout(
    config: &Config,
    history_mode: ThreadHistoryMode,
    preview: &str,
) -> Result<ThreadId> {
    let create_rollout = match history_mode {
        ThreadHistoryMode::Legacy => create_fake_rollout,
        ThreadHistoryMode::Paginated => create_fake_paginated_rollout,
    };
    let thread_id = create_rollout(
        config.codex_home.as_path(),
        "2026-01-02T00-00-00",
        "2026-01-02T00:00:00Z",
        preview,
        Some(config.model_provider_id.as_str()),
        /*git_info*/ None,
    )
    .map_err(|err| color_eyre::eyre::eyre!("failed to create history rollout: {err}"))?;
    Ok(ThreadId::from_string(&thread_id)?)
}

pub(super) fn recorded_params(requests: &RecordedRequests, method: &str) -> Vec<serde_json::Value> {
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
async fn delete_current_thread_navigates_only_after_success() -> Result<()> {
    let endpoint = crate::resolve_remote_addr("ws://127.0.0.1:4500")?;
    for target in [
        AppServerTarget::Embedded,
        AppServerTarget::LocalDaemon {
            endpoint: endpoint.clone(),
        },
        AppServerTarget::Remote { endpoint },
    ] {
        let (mut app, _codex_home) = make_history_test_app().await?;
        let thread_id =
            create_history_rollout(&app.config, ThreadHistoryMode::Legacy, "delete me")?;
        let (mut server, requests, proxy) = start_recording_app_server(
            &app.config,
            /*blocked_thread_list*/ None,
            /*failed_thread_name*/ None,
        )
        .await?;
        app.app_server_target = target;
        let mut tui = crate::tui::test_support::make_test_tui()?;
        let missing_id = ThreadId::new();
        app.active_thread_id = Some(missing_id);
        app.chat_widget.insert_str("Retained on failure");
        assert_matches!(
            app.delete_current_thread(&mut tui, &mut server).await?,
            AppRunControl::Continue
        );
        assert_eq!(app.active_thread_id, Some(missing_id));
        assert_eq!(
            app.chat_widget.composer_text_with_pending(),
            "Retained on failure"
        );
        assert!(!app.chat_widget.has_active_view());

        let resumed = server
            .resume_thread(
                &app.local_settings,
                app.config.clone(),
                thread_id,
                crate::app_server_session::ResumeModelSettings::RestoreFromThread,
            )
            .await?;
        app.active_thread_id = Some(thread_id);
        app.enqueue_primary_thread_session(resumed.session.clone(), resumed.turns)
            .await?;
        app.chat_widget.handle_thread_session(resumed.session);
        let mut side_config = app.config.clone();
        side_config.ephemeral = true;
        let side = server
            .fork_side_thread(&app.local_settings, side_config.clone(), thread_id)
            .await?;
        let side_id = side.session.thread_id;
        app.side_threads
            .insert(side_id, SideThreadState::new(thread_id));
        if !matches!(app.app_server_target, AppServerTarget::Embedded) {
            // The recording proxy rejects the next unsubscribe after a failed fork.
            side_config.cwd = side_config.cwd.join("failure");
            assert!(
                server
                    .fork_side_thread(&app.local_settings, side_config, thread_id)
                    .await
                    .is_err()
            );
            assert_matches!(
                app.delete_current_thread(&mut tui, &mut server).await?,
                AppRunControl::Continue
            );
            assert_eq!(app.active_thread_id, Some(thread_id));
            assert!(app.side_threads.contains_key(&side_id));
            assert!(!app.chat_widget.has_active_view());
            assert_eq!(recorded_params(&requests, "thread/delete").len(), 1);
        }
        let (tx, mut events) = tokio::sync::mpsc::unbounded_channel();
        app.app_event_tx = AppEventSender::new(tx);
        let control =
            Box::pin(app.handle_event(&mut tui, &mut server, AppEvent::DeleteCurrentThread))
                .await?;
        assert!(
            server
                .thread_read(thread_id, /*include_turns*/ false)
                .await
                .is_err()
        );
        if matches!(app.app_server_target, AppServerTarget::Embedded) {
            assert_matches!(control, AppRunControl::Exit(ExitReason::ThreadRemoved));
            assert!(recorded_params(&requests, "thread/unsubscribe").is_empty());
        } else {
            assert_matches!(control, AppRunControl::Continue);
            assert!(app.side_threads.is_empty());
            assert_eq!(
                recorded_params(&requests, "thread/unsubscribe"),
                vec![serde_json::json!({"threadId": side_id.to_string()}); 2]
            );
            loop {
                let event = tokio::time::timeout(Duration::from_secs(/*secs*/ 5), events.recv())
                    .await?
                    .expect("command center refresh");
                let refreshed = matches!(&event, AppEvent::AgentsOverviewThreadsLoaded { .. });
                Box::pin(app.handle_event(&mut tui, &mut server, event)).await?;
                if refreshed {
                    break;
                }
            }
            assert_eq!(
                (
                    app.active_thread_id,
                    app.primary_thread_id,
                    app.chat_widget.thread_id()
                ),
                (None, None, None)
            );
            assert!(app.chat_widget.composer_is_empty());
            assert!(app.chat_widget.has_active_view());
        }
        assert_eq!(
            recorded_params(&requests, "thread/delete"),
            vec![
                serde_json::json!({"threadId": missing_id.to_string()}),
                serde_json::json!({"threadId": thread_id.to_string()}),
            ]
        );
        server.shutdown().await?;
        proxy.await??;
    }
    Ok(())
}

fn spawn_approved_task_tool_call(
    app: &App,
    app_server: &AppServerSession,
    request_id: AppServerRequestId,
    params: codex_app_server_protocol::DynamicToolCallParams,
) {
    let request_handle = app_server.request_handle();
    let app_event_tx = app.app_event_tx.clone();
    let status_updates = app.dynamic_tool_status_updates.subscribe();
    let mut thread_start_params = crate::app_server_session::thread_start_params_from_config(
        &app.config,
        app_server.thread_params_mode(),
        app_server.remote_cwd_override(),
        /*session_start_source*/ None,
    );
    app_server
        .thread_tool_transport()
        .configure(&mut thread_start_params);
    tokio::spawn(async move {
        let response = crate::dynamic_tools::execute(
            request_handle,
            params,
            thread_start_params,
            status_updates,
            Some(&app_event_tx),
        )
        .await;
        app_event_tx.send(AppEvent::DynamicToolCallCompleted {
            request_id,
            response,
        });
    });
}

#[tokio::test]
async fn external_transport_registers_dynamic_tools_and_finds_task_mentions() -> Result<()> {
    let (app, _codex_home) = make_history_test_app().await?;
    let (mut app_server, requests, proxy) = start_recording_app_server(
        &app.config,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
    )
    .await?;

    let started = app_server.start_thread(&app.config).await?;
    assert!(started.task_tools_available);
    assert!(app_server.task_tools_available(started.session.thread_id));
    let startup = crate::app_server_session::start_thread_with_request_handle(
        app_server.request_handle(),
        &app.local_settings,
        app.config.clone(),
        crate::app_server_session::ThreadParamsMode::Embedded,
        /*remote_cwd_override*/ None,
        app_server.thread_tool_transport(),
    )
    .await?;
    assert!(startup.task_tools_available);

    let starts = recorded_params(&requests, "thread/start");
    assert_eq!(starts.len(), 2);
    for params in starts {
        assert_eq!(params["dynamicTools"][0]["type"], "namespace");
        assert_eq!(params["dynamicTools"][0]["name"], "codex_tui");
        assert_eq!(
            params["dynamicTools"][0]["tools"].as_array().map(Vec::len),
            Some(6)
        );
        assert!(
            params["dynamicTools"][0]["tools"]
                .as_array()
                .is_some_and(|tools| tools.iter().all(|tool| {
                    tool["deferLoading"] == true
                        && !crate::dynamic_tools::DELEGATION_TOOLS
                            .contains(&tool["name"].as_str().unwrap_or_default())
                }))
        );
    }
    let target_id = started.session.thread_id;
    app_server
        .thread_inject_items(target_id, vec![App::side_boundary_prompt_item()])
        .await?;
    crate::init_state_db_for_app_server_target(&app.config, &crate::AppServerTarget::Embedded)
        .await?
        .expect("state database")
        .set_thread_preview_if_empty(target_id, "Review database migration")
        .await
        .expect("seed searchable thread preview");
    app_server.shutdown().await?;
    proxy.await??;
    let (mut restarted_app_server, _restarted_requests, restarted_proxy) =
        start_recording_app_server(
            &app.config,
            /*blocked_thread_list*/ None,
            /*failed_thread_name*/ None,
        )
        .await?;
    let resumed = restarted_app_server
        .resume_thread(
            &app.local_settings,
            app.config.clone(),
            target_id,
            crate::app_server_session::ResumeModelSettings::RestoreFromThread,
        )
        .await?;
    assert!(resumed.task_tools_available);
    assert!(restarted_app_server.task_tools_available(target_id));
    let forked = restarted_app_server
        .fork_thread(&app.local_settings, app.config.clone(), target_id)
        .await?;
    assert!(forked.task_tools_available);
    assert!(restarted_app_server.task_tools_available(forked.session.thread_id));
    restarted_app_server
        .thread_set_name(target_id, "Bluebird".to_string())
        .await?;
    let (sender, mut events) = tokio::sync::mpsc::unbounded_channel();

    crate::task_mentions::spawn_search(
        restarted_app_server.request_handle(),
        "bbd".to_string(),
        startup.session.thread_id,
        app.config.cwd.to_path_buf(),
        restarted_app_server.task_search_generation(),
        crate::app_event_sender::AppEventSender::new(sender),
    );

    let event = tokio::time::timeout(Duration::from_secs(/*secs*/ 5), events.recv())
        .await?
        .expect("expected task search results");
    let AppEvent::TaskSearchResult { matches, .. } = event else {
        panic!("expected task search results");
    };
    assert!(
        matches
            .iter()
            .any(|task| task.thread_id == target_id.to_string() && task.title == "Bluebird"),
        "expected created task in {matches:?}"
    );

    restarted_app_server.shutdown().await?;
    restarted_proxy.await??;
    Ok(())
}

#[tokio::test]
async fn archive_current_thread_reports_success_only_after_archiving() -> Result<()> {
    let (mut app, _codex_home) = make_history_test_app().await?;
    let thread_id = ThreadId::from_string(
        &create_fake_rollout(
            &app.config.codex_home,
            "2026-08-25T01-00-00",
            "2026-08-25T01:00:00Z",
            "archive me",
            Some(&app.config.model_provider_id),
            /*git_info*/ None,
        )
        .expect("create rollout"),
    )?;
    let mut app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;

    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.active_thread_id = Some(ThreadId::new());
    assert_matches!(
        app.archive_current_thread(&mut tui, &mut app_server)
            .await?,
        AppRunControl::Continue
    );

    app.active_thread_id = Some(thread_id);
    assert_matches!(
        app.archive_current_thread(&mut tui, &mut app_server).await?,
        AppRunControl::Exit(ExitReason::Archived(archived_id)) if archived_id == thread_id
    );

    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn archive_current_thread_returns_shared_servers_to_agents() -> Result<()> {
    let endpoint = crate::resolve_remote_addr("ws://127.0.0.1:4500")?;
    for target in [
        AppServerTarget::LocalDaemon {
            endpoint: endpoint.clone(),
        },
        AppServerTarget::Remote { endpoint },
    ] {
        let (mut app, _codex_home) = make_history_test_app().await?;
        let thread_id =
            create_history_rollout(&app.config, ThreadHistoryMode::Legacy, "archive me")?;
        let (mut server, requests, proxy) = start_recording_app_server(
            &app.config,
            /*blocked_thread_list*/ None,
            /*failed_thread_name*/ None,
        )
        .await?;
        let resumed = server
            .resume_thread(
                &app.local_settings,
                app.config.clone(),
                thread_id,
                crate::app_server_session::ResumeModelSettings::RestoreFromThread,
            )
            .await?;
        let mut side_config = app.config.clone();
        side_config.ephemeral = true;
        let side = server
            .fork_side_thread(&app.local_settings, side_config, thread_id)
            .await?;
        let side_id = side.session.thread_id;
        app.side_threads
            .insert(side_id, SideThreadState::new(thread_id));
        app.app_server_target = target;
        app.enqueue_primary_thread_session(resumed.session.clone(), resumed.turns)
            .await?;
        app.chat_widget.handle_thread_session(resumed.session);
        app.chat_widget.insert_str("Unsent archived draft");
        let mut tui = crate::tui::test_support::make_test_tui()?;
        let (tx, mut events) = tokio::sync::mpsc::unbounded_channel();
        app.app_event_tx = AppEventSender::new(tx);

        assert_matches!(
            Box::pin(app.handle_event(&mut tui, &mut server, AppEvent::ArchiveCurrentThread))
                .await?,
            AppRunControl::Continue
        );
        loop {
            let event = tokio::time::timeout(Duration::from_secs(/*secs*/ 5), events.recv())
                .await?
                .expect("expected command center refresh");
            let refreshed = matches!(&event, AppEvent::AgentsOverviewThreadsLoaded { .. });
            Box::pin(app.handle_event(&mut tui, &mut server, event)).await?;
            if refreshed {
                break;
            }
        }
        assert!(app.agents_overview.initialized);
        assert_eq!(
            (
                app.active_thread_id,
                app.primary_thread_id,
                app.chat_widget.thread_id()
            ),
            (None, None, None)
        );
        assert!(!app.agents_overview.threads.contains_key(&thread_id));
        assert!(app.thread_event_channels.is_empty());
        assert!(app.side_threads.is_empty());
        assert_eq!(
            recorded_params(&requests, "thread/unsubscribe"),
            vec![serde_json::json!({"threadId": side_id.to_string()})]
        );
        assert!(app.chat_widget.composer_is_empty());
        assert_eq!(
            recorded_params(&requests, "thread/archive"),
            vec![serde_json::json!({"threadId": thread_id.to_string()})]
        );
        assert!(
            app.chat_widget
                .selected_index_for_present_view(
                    crate::app::agents_overview::AGENTS_OVERVIEW_VIEW_ID
                )
                .is_some()
        );
        assert_snapshot!(
            "agents_command_center_after_archive",
            render_bottom_popup(&app.chat_widget, /*width*/ 100)
        );

        server.shutdown().await?;
        proxy.await??;
    }
    Ok(())
}

#[tokio::test]
async fn local_daemon_registers_approval_gated_mcp_tools_for_both_start_paths() -> Result<()> {
    let (mut app, events, _ops) = make_test_app_with_channels().await;
    let codex_home = tempdir()?;
    app.config.codex_home = codex_home.path().to_path_buf().abs();
    app.config.sqlite = SqliteConfig::new_for_testing(codex_home.path().abs());
    app.config
        .web_search_mode
        .set(codex_protocol::config_types::WebSearchMode::Live)?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        "web_search = \"disabled\"\n",
    )?;
    let (mut app_server, mut requests, mut proxy) = start_recording_app_server(
        &app.config,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
    )
    .await?;
    app_server
        .start_dynamic_tool_mcp(
            app.config.clone(),
            app.app_event_tx.clone(),
            app.dynamic_tool_status_updates.clone(),
        )
        .await?;

    let started = app_server.start_thread(&app.config).await?;
    let thread_id = started.session.thread_id;
    assert!(started.task_tools_available);
    assert!(app_server.task_tools_available(thread_id));
    let startup = crate::app_server_session::start_thread_with_request_handle(
        app_server.request_handle(),
        &app.local_settings,
        app.config.clone(),
        crate::app_server_session::ThreadParamsMode::Embedded,
        /*remote_cwd_override*/ None,
        app_server.thread_tool_transport(),
    )
    .await?;
    assert!(startup.task_tools_available);

    let inventory: codex_app_server_protocol::ListMcpServerStatusResponse = app_server
        .request_handle()
        .request_typed(ClientRequest::McpServerStatusList {
            request_id: AppServerRequestId::String("tui-tool-inventory".to_string()),
            params: codex_app_server_protocol::ListMcpServerStatusParams {
                cursor: None,
                limit: None,
                detail: Some(codex_app_server_protocol::McpServerStatusDetail::ToolsAndAuthOnly),
                thread_id: Some(thread_id.to_string()),
            },
        })
        .await?;
    let tools = &inventory
        .data
        .iter()
        .find(|server| server.name == "codex_tui")
        .expect("local daemon must connect to the TUI MCP server")
        .tools;
    assert_eq!(tools.len(), 9);
    for tool in crate::dynamic_tools::DELEGATION_TOOLS {
        assert!(tools.contains_key(tool));
    }
    assert!(
        !tools["create_thread"]
            .input_schema
            .get("properties")
            .and_then(serde_json::Value::as_object)
            .is_some_and(|properties| properties.contains_key("permissions"))
    );

    let starts = recorded_params(&requests, "thread/start");
    assert_eq!(starts.len(), 2);
    for params in &starts {
        assert_eq!(params["dynamicTools"], serde_json::Value::Null);
        assert_eq!(params["config"]["web_search"], "live");
        let server = &params["config"]["mcp_servers.codex_tui"];
        assert!(
            server["url"]
                .as_str()
                .is_some_and(|url| url.starts_with("http://127.0.0.1:"))
        );
        assert!(
            server["http_headers"]["Authorization"]
                .as_str()
                .is_some_and(|header| header.starts_with("Bearer "))
        );
        assert_eq!(server["default_tools_approval_mode"], "approve");
        for tool in crate::dynamic_tools::DELEGATION_TOOLS {
            assert_eq!(server["tools"][tool]["approval_mode"], "prompt");
        }
    }

    let mcp_url = starts[0]["config"]["mcp_servers.codex_tui"]["url"]
        .as_str()
        .expect("MCP server URL");
    let unauthorized = codex_http_client::HttpClientBuilder::new()
        .build_direct()?
        .post(mcp_url)
        .send()
        .await?;
    assert_eq!(unauthorized.status().as_u16(), 401);

    app.config
        .web_search_mode
        .set(codex_protocol::config_types::WebSearchMode::Disabled)?;
    let delegation_source = create_history_rollout(
        &app.config,
        ThreadHistoryMode::Legacy,
        "Approved task source",
    )?;
    app_server
        .resume_thread(
            &app.local_settings,
            app.config.clone(),
            delegation_source,
            crate::app_server_session::ResumeModelSettings::RestoreFromThread,
        )
        .await?;
    let resumed = recorded_params(&requests, "thread/resume")
        .pop()
        .expect("resumed task request");
    assert_eq!(
        resumed["config"]["mcp_servers.codex_tui"],
        starts[0]["config"]["mcp_servers.codex_tui"]
    );
    app_server
        .resume_thread(
            &app.local_settings,
            app.config.clone(),
            delegation_source,
            crate::app_server_session::ResumeModelSettings::PreserveExistingThread,
        )
        .await?;
    let reattached = recorded_params(&requests, "thread/resume")
        .pop()
        .expect("reattached task request");
    assert_eq!(
        reattached["config"]["mcp_servers.codex_tui"],
        starts[0]["config"]["mcp_servers.codex_tui"]
    );
    app_server
        .fork_thread(&app.local_settings, app.config.clone(), delegation_source)
        .await?;
    let forked = recorded_params(&requests, "thread/fork")
        .pop()
        .expect("forked task request");
    assert_eq!(
        forked["config"]["mcp_servers.codex_tui"],
        starts[0]["config"]["mcp_servers.codex_tui"]
    );
    let authorization =
        starts[0]["config"]["mcp_servers.codex_tui"]["http_headers"]["Authorization"]
            .as_str()
            .expect("MCP bearer token");
    let client = codex_http_client::HttpClientBuilder::new().build_direct()?;
    let call_tool = |id: u32, tool: &'static str, arguments: serde_json::Value| {
        client
            .post(mcp_url)
            .header("Authorization", authorization)
            .header("Accept", "application/json, text/event-stream")
            .header("MCP-Protocol-Version", "2026-07-28")
            .header("MCP-Method", "tools/call")
            .header("MCP-Name", tool)
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "tools/call",
                "params": {
                    "name": tool,
                    "arguments": arguments,
                    "_meta": {
                        "threadId": delegation_source,
                        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                        "io.modelcontextprotocol/clientCapabilities": {}
                    }
                }
            }))
    };
    let transport = app_server.thread_tool_transport();
    let crate::dynamic_tools_mcp::ThreadToolTransport::Mcp(tool_server) = &transport else {
        panic!("expected the daemon task-tool bridge");
    };
    tool_server.suspend();
    let paused = call_tool(0, "list_threads", serde_json::json!({}))
        .send()
        .await?
        .text()
        .await?;
    assert!(
        paused.contains("TUI is reconnecting; tool was not sent"),
        "{paused}"
    );
    let (replacement, replacement_requests, replacement_proxy) = start_recording_app_server(
        &app.config,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
    )
    .await?;
    let (new_tx, new_rx) = mpsc::unbounded_channel();
    let new_sender = AppEventSender::new(new_tx);
    drop(events);
    let mut events = new_rx;
    assert!(app.app_event_tx.app_event_tx.is_closed());
    tool_server.reconnect(replacement.request_handle(), new_sender);
    let previous = std::mem::replace(
        &mut app_server,
        replacement.with_thread_tool_transport(transport),
    );
    previous.shutdown().await?;
    proxy.await??;
    requests = replacement_requests;
    proxy = replacement_proxy;
    // The same MCP URL and credentials now use the new connection and event receiver.
    let response = call_tool(1, "list_threads", serde_json::json!({}))
        .send()
        .await?;
    let status = response.status();
    let response = response.text().await?;
    assert!(status.is_success(), "{status}: {response}");
    assert!(response.contains("threads"), "{response}");

    let mut creation = tokio::spawn(
        call_tool(
            2,
            "create_thread",
            serde_json::json!({"prompt": "Start an approved task"}),
        )
        .send(),
    );
    let registration = tokio::select! {
        event = events.recv() => event.expect("approved MCP task must register before starting"),
        response = &mut creation => {
            let response = response??;
            panic!("MCP task creation completed without registration: {}", response.text().await?);
        }
        _ = tokio::time::sleep(std::time::Duration::from_secs(/*secs*/ 5)) => {
            panic!("timed out waiting for MCP task registration");
        }
    };
    let AppEvent::DynamicToolThreadStarted {
        thread_id: child_thread_id,
        task_tools_available,
        registered,
    } = registration
    else {
        panic!("expected the MCP-created task to register")
    };
    assert!(task_tools_available);
    assert!(registered.send(()).is_ok());
    let created = creation.await??;
    assert!(created.status().is_success());
    assert!(created.text().await?.contains(&child_thread_id.to_string()));
    let child = recorded_params(&requests, "thread/start")
        .pop()
        .expect("MCP child thread/start request");
    assert_eq!(child["dynamicTools"], serde_json::Value::Null);
    assert!(child["config"]["web_search"].is_null());
    assert_eq!(
        child["config"]["mcp_servers.codex_tui"],
        starts[0]["config"]["mcp_servers.codex_tui"]
    );
    let forked = call_tool(
        3,
        "fork_thread",
        serde_json::json!({"threadId": delegation_source}),
    )
    .send()
    .await?;
    assert!(forked.status().is_success());
    let forked = recorded_params(&requests, "thread/fork")
        .pop()
        .expect("MCP-created fork request");
    assert_eq!(
        forked["config"]["mcp_servers.codex_tui"],
        starts[0]["config"]["mcp_servers.codex_tui"]
    );

    app_server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn local_mcp_respects_configured_servers_and_managed_requirements() -> Result<()> {
    for scenario in ["conflicting", "blocked", "mismatched", "allowed"] {
        let (mut app, _codex_home) = make_history_test_app().await?;
        if scenario == "conflicting" {
            let raw = serde_json::from_value::<codex_config::RawMcpServerConfig>(
                serde_json::json!({"url": "http://127.0.0.1:1/mcp", "enabled": false}),
            )?;
            let mut servers = app.config.mcp_servers.get().clone();
            servers.insert(
                crate::dynamic_tools::NAMESPACE.to_string(),
                codex_config::McpServerConfig::try_from(raw)
                    .map_err(color_eyre::eyre::Report::msg)?,
            );
            app.config.mcp_servers.set(servers)?;
        } else {
            let mut allowed_servers = std::collections::BTreeMap::new();
            if matches!(scenario, "mismatched" | "allowed") {
                let requirement = if scenario == "allowed" {
                    codex_config::McpServerRequirement::Url(
                        codex_protocol::mcp_policy::McpServerValueMatcher::Prefix {
                            value: "http://127.0.0.1:".to_string(),
                        },
                    )
                } else {
                    codex_config::McpServerRequirement::Identity {
                        identity: codex_config::McpServerIdentity::Url {
                            url: "http://127.0.0.1:1/mcp".to_string(),
                        },
                    }
                };
                allowed_servers.insert(crate::dynamic_tools::NAMESPACE.to_string(), requirement);
            }
            let requirements = codex_config::ConfigRequirements {
                mcp_servers: Some(codex_config::Sourced::new(
                    allowed_servers,
                    codex_config::RequirementSource::Unknown,
                )),
                ..Default::default()
            };
            app.config.config_layer_stack = codex_config::ConfigLayerStack::new(
                Vec::new(),
                requirements,
                codex_config::ConfigRequirementsToml::default(),
            )?;
        }
        let (mut app_server, requests, proxy) = start_recording_app_server(
            &app.config,
            /*blocked_thread_list*/ None,
            /*failed_thread_name*/ None,
        )
        .await?;
        let result = app_server
            .start_dynamic_tool_mcp(
                app.config.clone(),
                app.app_event_tx.clone(),
                app.dynamic_tool_status_updates.clone(),
            )
            .await;
        if scenario == "allowed" {
            result?;
        } else {
            let error = result.expect_err("unavailable internal MCP must fail closed");
            assert_eq!(
                error.kind(),
                if scenario == "conflicting" {
                    std::io::ErrorKind::AlreadyExists
                } else {
                    std::io::ErrorKind::PermissionDenied
                }
            );
        }
        app_server.start_thread(&app.config).await?;
        let start = recorded_params(&requests, "thread/start")
            .pop()
            .expect("fallback task start");
        if scenario == "allowed" {
            assert!(start["dynamicTools"].is_null());
            assert!(start["config"]["mcp_servers.codex_tui"].is_object());
        } else {
            assert_eq!(
                start["dynamicTools"][0]["tools"].as_array().map(Vec::len),
                Some(6)
            );
            assert!(start["config"]["mcp_servers.codex_tui"].is_null());
        }
        app_server.shutdown().await?;
        proxy.await??;
    }
    Ok(())
}

#[tokio::test]
async fn older_external_server_starts_without_unsupported_dynamic_tools_or_history() -> Result<()> {
    let (app, _codex_home) = make_history_test_app().await?;
    let (mut app_server, requests, proxy) = start_recording_app_server_with_history(
        &app.config,
        HistoryCapabilities::LegacyDynamicToolsAndHistory,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
        crate::app_server_session::ThreadParamsMode::Embedded,
        LoaderOverrides::default(),
    )
    .await?;

    let started = app_server.start_thread(&app.config).await?;
    assert!(!started.task_tools_available);
    assert!(!app_server.task_tools_available(started.session.thread_id));
    let startup = crate::app_server_session::start_thread_with_request_handle(
        app_server.request_handle(),
        &app.local_settings,
        app.config.clone(),
        crate::app_server_session::ThreadParamsMode::Embedded,
        /*remote_cwd_override*/ None,
        app_server.thread_tool_transport(),
    )
    .await?;
    assert!(!startup.task_tools_available);

    let starts = recorded_params(&requests, "thread/start");
    assert_eq!(starts.len(), 6);
    for attempts in starts.chunks_exact(3) {
        assert_eq!(attempts[0]["dynamicTools"][0]["type"], "namespace");
        assert_eq!(attempts[0]["historyMode"], "paginated");
        assert_eq!(attempts[1]["dynamicTools"], serde_json::Value::Null);
        assert_eq!(attempts[1]["historyMode"], "paginated");
        assert_eq!(attempts[2]["dynamicTools"], serde_json::Value::Null);
        assert_eq!(attempts[2]["historyMode"], serde_json::Value::Null);
    }

    app_server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn embedded_server_rejects_unowned_dynamic_tool_calls() -> Result<()> {
    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    let codex_home = tempdir()?;
    app.config.codex_home = codex_home.path().to_path_buf().abs();
    app.config.sqlite = SqliteConfig::new_for_testing(codex_home.path().abs());
    let app_server = crate::start_embedded_app_server_for_picker(&app.config).await?;
    app.handle_app_server_event(
        &app_server,
        codex_app_server_client::AppServerEvent::ServerRequest(Box::new(
            ServerRequest::DynamicToolCall {
                request_id: AppServerRequestId::Integer(100),
                params: codex_app_server_protocol::DynamicToolCallParams {
                    thread_id: "thread-1".to_string(),
                    turn_id: "turn-1".to_string(),
                    call_id: "call-1".to_string(),
                    namespace: Some("codex_app".to_string()),
                    tool: "list_threads".to_string(),
                    arguments: serde_json::json!({}),
                },
            },
        )),
    )
    .await;
    let AppEvent::DynamicToolCallCompleted { response, .. } = events
        .try_recv()
        .expect("embedded dynamic calls must receive a response")
    else {
        panic!("expected a dynamic tool failure response")
    };
    assert!(!response.success);
    app_server.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn dynamic_tool_requests_ignore_other_namespaces_and_dispatch_tui_namespace() -> Result<()> {
    let (mut app, mut events, _ops) = make_test_app_with_channels().await;
    let codex_home = tempdir()?;
    app.config.codex_home = codex_home.path().to_path_buf().abs();
    app.config.sqlite = SqliteConfig::new_for_testing(codex_home.path().abs());
    app.config
        .permissions
        .set_permission_profile(PermissionProfile::workspace_write_with(
            &[app.config.cwd.clone()],
            codex_protocol::permissions::NetworkSandboxPolicy::Restricted,
            /*exclude_tmpdir_env_var*/ true,
            /*exclude_slash_tmp*/ true,
        ))?;
    let (mut app_server, requests, proxy) = start_recording_app_server(
        &app.config,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ Some("Unavailable name"),
    )
    .await?;
    let thread_id = app_server
        .start_thread(&app.config)
        .await?
        .session
        .thread_id
        .to_string();

    for namespace in [Some("codex_app"), None] {
        app.handle_app_server_event(
            &app_server,
            codex_app_server_client::AppServerEvent::ServerRequest(Box::new(
                ServerRequest::DynamicToolCall {
                    request_id: AppServerRequestId::Integer(100),
                    params: codex_app_server_protocol::DynamicToolCallParams {
                        thread_id: thread_id.clone(),
                        turn_id: "turn-1".to_string(),
                        call_id: "call-1".to_string(),
                        namespace: namespace.map(str::to_string),
                        tool: "list_threads".to_string(),
                        arguments: serde_json::json!({}),
                    },
                },
            )),
        )
        .await;
        assert!(events.try_recv().is_err());
    }

    app.handle_app_server_event(
        &app_server,
        codex_app_server_client::AppServerEvent::ServerRequest(Box::new(
            ServerRequest::DynamicToolCall {
                request_id: AppServerRequestId::Integer(101),
                params: codex_app_server_protocol::DynamicToolCallParams {
                    thread_id: thread_id.clone(),
                    turn_id: "turn-1".to_string(),
                    call_id: "call-2".to_string(),
                    namespace: Some("codex_tui".to_string()),
                    tool: "list_threads".to_string(),
                    arguments: serde_json::json!({}),
                },
            },
        )),
    )
    .await;

    let event = tokio::time::timeout(std::time::Duration::from_secs(/*secs*/ 5), events.recv())
        .await?
        .expect("dynamic tool completion event");
    let AppEvent::DynamicToolCallCompleted {
        request_id,
        response,
    } = event
    else {
        panic!("expected a dynamic tool completion event")
    };
    assert_eq!(request_id, AppServerRequestId::Integer(101));
    assert!(response.success, "{response:?}");
    let list_requests = recorded_params(&requests, "thread/list");
    assert_eq!(list_requests.len(), 1);
    assert_eq!(list_requests[0]["useStateDbOnly"], true);
    assert_eq!(list_requests[0]["sourceKinds"], serde_json::Value::Null);

    let mut tui = crate::tui::test_support::make_test_tui()?;
    // Box each dispatcher await so this test does not retain its large future inline.
    Box::pin(app.handle_event(
        &mut tui,
        &mut app_server,
        AppEvent::DynamicToolCallCompleted {
            request_id,
            response,
        },
    ))
    .await?;
    let completed = tokio::time::timeout(std::time::Duration::from_secs(/*secs*/ 5), async {
        loop {
            if let Some(response) = recorded_params(&requests, "server/request/response").pop() {
                break response;
            }
            tokio::task::yield_now().await;
        }
    })
    .await?;
    assert_eq!(completed["success"], true);

    app.handle_app_server_event(
        &app_server,
        codex_app_server_client::AppServerEvent::ServerRequest(Box::new(
            ServerRequest::DynamicToolCall {
                request_id: AppServerRequestId::Integer(102),
                params: codex_app_server_protocol::DynamicToolCallParams {
                    thread_id: thread_id.clone(),
                    turn_id: "turn-1".to_string(),
                    call_id: "call-3".to_string(),
                    namespace: Some("codex_tui".to_string()),
                    tool: "set_thread_title".to_string(),
                    arguments: serde_json::json!({"threadId": thread_id, "title": "Renamed"}),
                },
            },
        )),
    )
    .await;
    let AppEvent::DynamicToolCallCompleted { response, .. } =
        tokio::time::timeout(std::time::Duration::from_secs(/*secs*/ 5), events.recv())
            .await?
            .expect("dynamic mutation completion event")
    else {
        panic!("expected a dynamic mutation completion event")
    };
    assert!(response.success, "{response:?}");
    assert_eq!(
        recorded_params(&requests, "thread/name/set")[0]["name"],
        "Renamed"
    );

    for (index, tool) in crate::dynamic_tools::DELEGATION_TOOLS
        .into_iter()
        .enumerate()
    {
        app.handle_app_server_event(
            &app_server,
            codex_app_server_client::AppServerEvent::ServerRequest(Box::new(
                ServerRequest::DynamicToolCall {
                    request_id: AppServerRequestId::String(format!("rejected-{index}")),
                    params: codex_app_server_protocol::DynamicToolCallParams {
                        thread_id: thread_id.clone(),
                        turn_id: "turn-1".to_string(),
                        call_id: format!("rejected-{index}"),
                        namespace: Some("codex_tui".to_string()),
                        tool: tool.to_string(),
                        arguments: serde_json::json!({}),
                    },
                },
            )),
        )
        .await;
        let AppEvent::DynamicToolCallCompleted { response, .. } = events
            .try_recv()
            .expect("legacy delegation call must receive an immediate rejection")
        else {
            panic!("expected a legacy delegation failure response")
        };
        assert!(!response.success);
    }

    let creation_source = create_history_rollout(
        &app.config,
        ThreadHistoryMode::Legacy,
        "Background task source",
    )?;
    app_server
        .resume_thread(
            &app.local_settings,
            app.config.clone(),
            creation_source,
            crate::app_server_session::ResumeModelSettings::RestoreFromThread,
        )
        .await?;
    let project: codex_app_server_protocol::ProjectCreateResponse = app_server
        .request_handle()
        .request_typed(ClientRequest::ProjectCreate {
            request_id: AppServerRequestId::String("create-source-project".to_string()),
            params: codex_app_server_protocol::ProjectCreateParams {
                name: "Source project".to_string(),
                roots: vec![codex_app_server_protocol::ProjectRoot {
                    path: app.config.cwd.clone(),
                }],
                metadata: None,
                idempotency_key: "source-project".to_string(),
            },
        })
        .await?;
    let _: codex_app_server_protocol::ThreadMetadataUpdateResponse = app_server
        .request_handle()
        .request_typed(ClientRequest::ThreadMetadataUpdate {
            request_id: AppServerRequestId::String("assign-source-project".to_string()),
            params: codex_app_server_protocol::ThreadMetadataUpdateParams {
                thread_id: creation_source.to_string(),
                project_id: Some(project.project.id.clone()),
                daybreak_enabled: None,
                git_info: None,
            },
        })
        .await?;
    let source_settings: codex_app_server_protocol::ThreadResumeResponse = app_server
        .request_handle()
        .request_typed(ClientRequest::ThreadResume {
            request_id: AppServerRequestId::String("read-source-sandbox".to_string()),
            params: codex_app_server_protocol::ThreadResumeParams {
                thread_id: creation_source.to_string(),
                ..codex_app_server_protocol::ThreadResumeParams::default()
            },
        })
        .await?;
    assert!(source_settings.active_permission_profile.is_none());
    let source_sandbox = serde_json::to_value(source_settings.sandbox)?;
    spawn_approved_task_tool_call(
        &app,
        &app_server,
        AppServerRequestId::Integer(103),
        codex_app_server_protocol::DynamicToolCallParams {
            thread_id: creation_source.to_string(),
            turn_id: "turn-1".to_string(),
            call_id: "call-4".to_string(),
            namespace: Some("codex_tui".to_string()),
            tool: "create_thread".to_string(),
            arguments: serde_json::json!({
                "prompt": "Check <main> & report",
                "title": "Unavailable name"
            }),
        },
    );
    let registration =
        tokio::time::timeout(std::time::Duration::from_secs(/*secs*/ 5), events.recv())
            .await?
            .expect("background task registration event");
    let AppEvent::DynamicToolThreadStarted {
        thread_id: created_thread_id,
        task_tools_available,
        registered,
    } = registration
    else {
        panic!("expected background task registration before its first turn: {registration:?}")
    };
    assert!(recorded_params(&requests, "turn/start").is_empty());
    Box::pin(app.handle_event(
        &mut tui,
        &mut app_server,
        AppEvent::DynamicToolThreadStarted {
            thread_id: created_thread_id,
            task_tools_available,
            registered,
        },
    ))
    .await?;
    assert!(
        app.agents_overview
            .dispatched_requests
            .contains_key(&created_thread_id)
    );
    assert!(app_server.task_tools_available(created_thread_id));
    let AppEvent::DynamicToolCallCompleted { response, .. } =
        tokio::time::timeout(std::time::Duration::from_secs(/*secs*/ 5), events.recv())
            .await?
            .expect("background task creation completion")
    else {
        panic!("expected a background task completion event")
    };
    assert!(response.success, "{response:?}");
    assert_eq!(
        recorded_params(&requests, "thread/start")
            .last()
            .expect("background task creation")["projectId"],
        project.project.id
    );
    let turn = recorded_params(&requests, "turn/start")
        .pop()
        .expect("background task turn request");
    assert_eq!(turn["input"], serde_json::json!([]));
    assert_eq!(
        turn["toolOutput"],
        serde_json::json!({
            "name": "create_thread",
            "namespace": "codex_tui",
            "output": format!(
                "<codex_delegation>\n  <source_thread_id>{creation_source}</source_thread_id>\n  <input>Check &lt;main&gt; &amp; report</input>\n</codex_delegation>"
            )
        })
    );
    assert_eq!(turn["sandboxPolicy"], source_sandbox);
    app.handle_app_server_event(
        &app_server,
        codex_app_server_client::AppServerEvent::ServerRequest(Box::new(exec_approval_request(
            created_thread_id,
            "turn-2",
            "item-1",
            /*approval_id*/ None,
        ))),
    )
    .await;
    assert_eq!(
        app.agents_overview.dispatched_requests[&created_thread_id].len(),
        1
    );

    spawn_approved_task_tool_call(
        &app,
        &app_server,
        AppServerRequestId::Integer(104),
        codex_app_server_protocol::DynamicToolCallParams {
            thread_id: thread_id.clone(),
            turn_id: "turn-1".to_string(),
            call_id: "call-5".to_string(),
            namespace: Some("codex_tui".to_string()),
            tool: "send_message_to_thread".to_string(),
            arguments: serde_json::json!({
                "threadId": creation_source,
                "prompt": "Follow <up> & report"
            }),
        },
    );
    let AppEvent::DynamicToolThreadStarted {
        thread_id: continued_thread_id,
        task_tools_available,
        registered,
    } = tokio::time::timeout(std::time::Duration::from_secs(/*secs*/ 5), events.recv())
        .await?
        .expect("follow-up task registration event")
    else {
        panic!("expected follow-up task registration before its next turn")
    };
    assert_eq!(continued_thread_id, creation_source);
    assert_eq!(recorded_params(&requests, "turn/start").len(), 1);
    Box::pin(app.handle_event(
        &mut tui,
        &mut app_server,
        AppEvent::DynamicToolThreadStarted {
            thread_id: continued_thread_id,
            task_tools_available,
            registered,
        },
    ))
    .await?;
    let AppEvent::DynamicToolCallCompleted { response, .. } =
        tokio::time::timeout(std::time::Duration::from_secs(/*secs*/ 5), events.recv())
            .await?
            .expect("follow-up task completion")
    else {
        panic!("expected a follow-up task completion event")
    };
    assert!(response.success, "{response:?}");
    let turn = &recorded_params(&requests, "turn/start")[1];
    assert_eq!(turn["input"], serde_json::json!([]));
    assert_eq!(
        turn["toolOutput"],
        serde_json::json!({
            "name": "send_message_to_thread",
            "namespace": "codex_tui",
            "output": format!(
                "<codex_delegation>\n  <source_thread_id>{thread_id}</source_thread_id>\n  <input>Follow &lt;up&gt; &amp; report</input>\n</codex_delegation>"
            )
        })
    );

    app.dynamic_tool_tasks.insert(
        AppServerRequestId::Integer(105),
        (thread_id, tokio::spawn(std::future::pending::<()>())),
    );
    assert_matches!(
        app.handle_exit_mode(&mut app_server, ExitMode::ShutdownFirst)
            .await,
        AppRunControl::Exit(ExitReason::UserRequested)
    );
    let cancelled = tokio::time::timeout(Duration::from_secs(/*secs*/ 5), async {
        loop {
            if let Some(response) = recorded_params(&requests, "server/request/response")
                .into_iter()
                .find(|response| response["success"] == false)
            {
                break response;
            }
            tokio::task::yield_now().await;
        }
    })
    .await?;
    assert_eq!(cancelled["success"], false);
    assert!(app.dynamic_tool_tasks.is_empty());

    app_server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn older_pagination_reconciles_review_prompts_across_page_boundaries() -> Result<()> {
    let (mut app, codex_home) = make_history_test_app().await?;
    app.local_settings.tui.terminal_resize_reflow_max_rows = Some(100);
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
            delivery: None,
            questions: None,
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
        root_turn_id: None,
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
            &app.local_settings,
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
        Some(&app.config),
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
    app.local_settings.tui.terminal_resize_reflow_max_rows = Some(2);
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
        root_turn_id: None,
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
                delivery: None,
                questions: None,
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
            &app.local_settings,
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
        Some(&app.config),
    );
    app.enqueue_primary_thread_session(started.session, started.turns)
        .await?;
    app.transcript_cells = initial_cells;
    while app_event_rx.try_recv().is_ok() {}
    let initial_turn_requests = recorded_params(&requests, "thread/turns/list").len();
    let initial_item_requests = recorded_params(&requests, "thread/items/list").len();
    let export_path = codex_home.path().join("complete-export.md");
    app.chat_widget
        .set_queue_autosend_suppressed(/*suppressed*/ true);
    app.chat_widget.insert_str("queued after export");
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    let mut tui = crate::tui::test_support::make_test_tui()?;
    // Keep the large dispatcher future off the Windows test thread's stack.
    Box::pin(app.handle_event(
        &mut tui,
        &mut app_server,
        AppEvent::ExportTranscript {
            destination: TranscriptExportDestination::File(export_path.clone()),
        },
    ))
    .await?;
    assert!(app.chat_widget.queued_user_message_texts().is_empty());
    let markdown = std::fs::read_to_string(export_path)?;
    assert!(
        (0..305)
            .map(|index| format!("history output {index}"))
            .eq(markdown.lines().filter(|line| line.starts_with("history")))
    );
    assert!(
        recorded_params(&requests, "thread/turns/list")[initial_turn_requests..]
            .iter()
            .all(|params| params["itemsView"] == "notLoaded")
    );
    assert!(
        recorded_params(&requests, "thread/items/list")[initial_item_requests..]
            .iter()
            .all(|params| {
                params["limit"] == crate::app_server_session::HISTORY_ITEM_PAGE_LIMIT
            })
    );
    app.scrollback_has_older_history = app_server.has_older_history(thread_id);
    assert!(app.scrollback_has_older_history);
    while app_event_rx.try_recv().is_ok() {}
    let initial_page_requests = recorded_params(&requests, "thread/items/list").len();
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
            Box::pin(app.handle_event(&mut tui, &mut app_server, event)).await?;
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
    let legacy_thread_id =
        create_history_rollout(&app.config, ThreadHistoryMode::Legacy, "legacy history")?;
    let paginated_thread_id = create_history_rollout(
        &app.config,
        ThreadHistoryMode::Paginated,
        "paginated history",
    )?;
    let (mut app_server, requests, proxy) = start_recording_app_server_with_history(
        &app.config,
        HistoryCapabilities::LegacyOnly,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
        crate::app_server_session::ThreadParamsMode::Embedded,
        LoaderOverrides::default(),
    )
    .await?;

    let started = app_server.start_thread(&app.config).await?;
    let resumed = app_server
        .resume_thread(
            &app.local_settings,
            app.config.clone(),
            legacy_thread_id,
            crate::app_server_session::ResumeModelSettings::RestoreFromThread,
        )
        .await?;
    let forked = app_server
        .fork_thread(&app.local_settings, app.config.clone(), legacy_thread_id)
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

    let initial_read_count = recorded_params(&requests, "thread/read").len();
    let exported = crate::app::transcript_export::load_export_transcript(
        &mut app_server,
        paginated_thread_id,
        crate::thread_transcript::RawReasoningVisibility::Hidden,
        Some(&app.config),
        vec![Arc::new(PlainHistoryCell::new(vec!["visible".into()]))],
    )
    .await
    .map_err(|error| color_eyre::eyre::eyre!(error))?;
    assert_eq!(exported[0].raw_lines()[0].to_string(), "visible");
    assert!(
        recorded_params(&requests, "thread/read")[initial_read_count..]
            .iter()
            .any(|params| params["includeTurns"] == true)
    );
    assert_eq!(recorded_params(&requests, "thread/turns/list").len(), 1);

    let (_status_sender, status_updates) = tokio::sync::broadcast::channel(/*capacity*/ 1);
    let response = crate::dynamic_tools::execute(
        app_server.request_handle(),
        codex_app_server_protocol::DynamicToolCallParams {
            thread_id: started.session.thread_id.to_string(),
            turn_id: "source-turn".to_string(),
            call_id: "legacy-wait".to_string(),
            namespace: Some(crate::dynamic_tools::NAMESPACE.to_string()),
            tool: "wait_threads".to_string(),
            arguments: serde_json::json!({
                "targets": [{"threadId": legacy_thread_id}],
                "timeoutMs": 0
            }),
        },
        codex_app_server_protocol::ThreadStartParams::default(),
        status_updates,
        /*app_event_tx*/ None,
    )
    .await;
    assert!(response.success, "{response:?}");
    assert!(
        recorded_params(&requests, "thread/read")
            .iter()
            .any(|params| {
                params["threadId"] == legacy_thread_id.to_string() && params["includeTurns"] == true
            })
    );

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
        crate::app_server_session::ThreadParamsMode::Embedded,
        LoaderOverrides::default(),
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
    let legacy_thread_id =
        create_history_rollout(&app.config, ThreadHistoryMode::Legacy, "legacy history")?;
    let (mut app_server, requests, proxy) = start_recording_app_server_with_history(
        &app.config,
        HistoryCapabilities::LegacyOnly,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
        crate::app_server_session::ThreadParamsMode::Embedded,
        LoaderOverrides::default(),
    )
    .await?;

    let method = match request {
        LegacyHistoryRequest::Resume => {
            let resumed = app_server
                .resume_thread(
                    &app.local_settings,
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
                .fork_thread(&app.local_settings, app.config.clone(), legacy_thread_id)
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

#[tokio::test]
async fn paginated_fork_survives_post_response_hydration_failure() -> Result<()> {
    let (app, _codex_home) = make_history_test_app().await?;
    let parent_thread_id = create_history_rollout(
        &app.config,
        ThreadHistoryMode::Paginated,
        "paginated fork parent",
    )?;
    let (mut app_server, requests, proxy) = start_recording_app_server_with_history(
        &app.config,
        HistoryCapabilities::ForkHydrationFails,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
        crate::app_server_session::ThreadParamsMode::Embedded,
        LoaderOverrides::default(),
    )
    .await?;

    let started = app_server
        .resume_thread(
            &app.local_settings,
            app.config.clone(),
            parent_thread_id,
            crate::app_server_session::ResumeModelSettings::RestoreFromThread,
        )
        .await?;
    assert_eq!(started.session.thread_id, parent_thread_id);

    let forked = app_server
        .fork_thread(&app.local_settings, app.config.clone(), parent_thread_id)
        .await?;

    assert_ne!(forked.session.thread_id, parent_thread_id);
    assert_eq!(recorded_params(&requests, "thread/fork").len(), 1);

    app_server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn underfilled_scrollback_fetches_older_pages_without_opening_the_transcript() -> Result<()> {
    let (mut app, mut app_event_rx, _op_rx) = make_test_app_with_channels().await;
    let codex_home = tempdir()?;
    app.config.codex_home = codex_home.path().to_path_buf().abs();
    app.config.sqlite = SqliteConfig::new_for_testing(codex_home.path().abs());
    app.local_settings.tui.terminal_resize_reflow_max_rows = Some(8);
    let thread_id = create_history_rollout(
        &app.config,
        ThreadHistoryMode::Paginated,
        "scrollback pagination",
    )?;
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
        turn_id: "scrollback-pagination-turn".to_string(),
        root_turn_id: None,
        trace_id: None,
        started_at: None,
        model_context_window: None,
        collaboration_mode_kind: Default::default(),
    }))
    .chain((0..120).map(|index| {
        EventMsg::ItemCompleted(ItemCompletedEvent {
            thread_id,
            turn_id: "scrollback-pagination-turn".to_string(),
            item: TurnItem::AgentMessage(AgentMessageItem {
                id: format!("scrollback-item-{index}"),
                content: vec![AgentMessageContent::Text {
                    text: format!("scrollback output {index}"),
                }],
                phase: None,
                memory_citation: None,
                delivery: None,
                questions: None,
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
            &app.local_settings,
            app.config.clone(),
            thread_id,
            crate::app_server_session::ResumeModelSettings::RestoreFromThread,
        )
        .await?;
    let mut initial_cells = crate::thread_transcript::thread_items_to_transcript_cells(
        Some(thread_id),
        &app.config.cwd,
        started.turns.iter().flat_map(|turn| turn.items.clone()),
        crate::thread_transcript::RawReasoningVisibility::Hidden,
        Some(&app.config),
    );
    initial_cells.insert(
        /*index*/ 0,
        Arc::new(crate::history_cell::new_session_info(
            &app.config,
            &app.local_settings,
            started.session.model.as_str(),
            &started.session,
            /*is_first_event*/ false,
            Some("This is a test announcement".to_string()),
            /*auth_plan*/ None,
            /*show_fast_status*/ false,
        )),
    );
    app.enqueue_primary_thread_session(started.session, started.turns)
        .await?;
    app.transcript_cells = initial_cells;
    app.scrollback_has_older_history = app_server.has_older_history(thread_id);
    app.local_settings.tui.terminal_resize_reflow_max_rows = Some(32);
    let initial_cell_count = app.transcript_cells.len();
    let initial_page_requests = recorded_params(&requests, "thread/items/list").len();
    let mut tui = crate::tui::test_support::make_test_tui()?;

    app.scrollback_has_older_history = false;
    app.handle_key_event(
        &mut tui,
        &mut app_server,
        KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL),
    )
    .await;
    assert!(app.scrollback_has_older_history);
    if let Some(Overlay::Transcript(overlay)) = app.overlay.as_mut() {
        overlay.handle_event(
            &mut tui,
            TuiEvent::Key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE)),
        )?;
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 100, /*height*/ 16,
        );
        let render_overlay = |overlay: &mut crate::pager_overlay::TranscriptOverlay| {
            let mut buffer = Buffer::empty(area);
            overlay.render(area, &mut buffer);
            (area.y..area.bottom())
                .map(|y| {
                    (area.x..area.right())
                        .map(|x| buffer[(x, y)].symbol())
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        let partial = render_overlay(overlay);
        assert!(partial.contains("Earlier messages are available — scroll up to load them"));
        assert!(!partial.contains("OpenAI Codex"));
        assert!(!partial.contains("This is a test announcement"));
        assert!(!partial.contains('%'));

        overlay.set_history_state(crate::pager_overlay::TranscriptHistoryState::LoadingOlder);
        let loading = render_overlay(overlay);
        assert!(loading.contains("Loading earlier messages..."));
        assert!(!loading.contains("OpenAI Codex"));
        assert!(!loading.contains('%'));
    } else {
        panic!("expected transcript overlay");
    }
    app.close_transcript_overlay(&mut tui);

    let terminal_width = tui.terminal.last_known_screen_size.into();
    app.reflow_transcript_now(&mut tui, terminal_width)?;
    let request = loop {
        match app_event_rx.recv().await {
            Some(event @ AppEvent::RequestOlderScrollbackHistory { .. }) => break event,
            Some(_) => {}
            None => panic!("scrollback refill request channel closed"),
        }
    };
    // Keep the large dispatcher future off the Windows test thread's stack.
    Box::pin(app.handle_event(&mut tui, &mut app_server, request)).await?;
    let loaded = loop {
        match app_event_rx.recv().await {
            Some(event @ AppEvent::OlderThreadHistoryLoaded { .. }) => break event,
            Some(_) => {}
            None => panic!("older history page channel closed"),
        }
    };
    Box::pin(app.handle_event(&mut tui, &mut app_server, loaded)).await?;

    assert!(app.overlay.is_none());
    assert!(app.transcript_cells.len() > initial_cell_count);
    assert_eq!(
        recorded_params(&requests, "thread/items/list").len(),
        initial_page_requests + 1
    );
    assert_eq!(
        app.render_transcript_lines_for_reflow(/*width*/ 80)
            .lines
            .len(),
        32
    );

    app_server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn paginated_workflows_never_request_full_thread_history() -> Result<()> {
    let (app, _codex_home) = make_history_test_app().await?;
    let paginated_thread_id = create_history_rollout(
        &app.config,
        ThreadHistoryMode::Paginated,
        "paginated visible history",
    )?;
    let legacy_thread_id = create_history_rollout(
        &app.config,
        ThreadHistoryMode::Legacy,
        "legacy visible history",
    )?;
    let (mut app_server, requests, proxy) = start_recording_app_server(
        &app.config,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
    )
    .await?;

    app_server.remember_thread_history_mode(paginated_thread_id, ThreadHistoryMode::Legacy);
    let resumed = app_server
        .resume_thread(
            &app.local_settings,
            app.config.clone(),
            paginated_thread_id,
            crate::app_server_session::ResumeModelSettings::RestoreFromThread,
        )
        .await?;
    assert_eq!(resumed.session.thread_id, paginated_thread_id);
    assert!(recorded_params(&requests, "thread/read").is_empty());
    let resume_requests = recorded_params(&requests, "thread/resume");
    assert_eq!(resume_requests.len(), 1);
    assert_eq!(resume_requests[0]["excludeTurns"], true);
    let cells = crate::thread_transcript::load_session_transcript(
        &mut app_server,
        paginated_thread_id,
        crate::thread_transcript::RawReasoningVisibility::Hidden,
        Some(&app.config),
    )
    .await?;
    assert!(!cells.is_empty());
    app_server
        .fork_thread(&app.local_settings, app.config.clone(), paginated_thread_id)
        .await?;
    let mut side_config = app.config.clone();
    side_config.ephemeral = true;
    app_server
        .fork_side_thread(
            &crate::local_settings::LocalSettings::from(&side_config),
            side_config,
            paginated_thread_id,
        )
        .await?;

    let paginated_reads = recorded_params(&requests, "thread/read");
    assert!(!paginated_reads.is_empty());
    assert!(
        paginated_reads
            .iter()
            .all(|params| params["includeTurns"] != true),
        "paginated workflows requested full history: {paginated_reads:?}"
    );
    assert!(!recorded_params(&requests, "thread/turns/list").is_empty());
    assert!(!recorded_params(&requests, "thread/items/list").is_empty());

    let previous_read_count = paginated_reads.len();
    let preview = crate::resume_picker::load_transcript_preview(
        &mut app_server,
        legacy_thread_id,
        Some(&app.config),
    )
    .await?;
    assert!(!preview.is_empty());
    let preview_reads = recorded_params(&requests, "thread/read");
    let preview_include_turns = preview_reads[previous_read_count..]
        .iter()
        .map(|params| params["includeTurns"].as_bool().unwrap_or(false))
        .collect::<Vec<_>>();
    assert_eq!(preview_include_turns, vec![false]);

    let previous_read_count = preview_reads.len();
    crate::thread_transcript::load_session_transcript(
        &mut app_server,
        legacy_thread_id,
        crate::thread_transcript::RawReasoningVisibility::Hidden,
        Some(&app.config),
    )
    .await?;
    let legacy_reads = recorded_params(&requests, "thread/read");
    let legacy_include_turns = legacy_reads[previous_read_count..]
        .iter()
        .map(|params| params["includeTurns"].as_bool().unwrap_or(false))
        .collect::<Vec<_>>();
    assert_eq!(legacy_include_turns, vec![false, true]);

    app_server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn agents_overview_stop_uses_history_mode_for_turn_lookup() -> Result<()> {
    let (mut app, _codex_home) = make_history_test_app().await?;
    let paginated_thread_id = create_history_rollout(
        &app.config,
        ThreadHistoryMode::Paginated,
        "paginated background task",
    )?;
    let cases = [
        (paginated_thread_id, vec![false], 1),
        (
            create_history_rollout(
                &app.config,
                ThreadHistoryMode::Legacy,
                "legacy background task",
            )?,
            vec![false, true],
            0,
        ),
    ];
    let (mut app_server, requests, proxy) = start_recording_app_server_with_history(
        &app.config,
        HistoryCapabilities::Current,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
        crate::app_server_session::ThreadParamsMode::Embedded,
        LoaderOverrides::default(),
    )
    .await?;

    for (thread_id, expected_include_turns, expected_turn_page_count) in cases {
        let previous_reads = recorded_params(&requests, "thread/read");
        let previous_turn_page_count = recorded_params(&requests, "thread/turns/list").len();

        app.stop_agents_overview_thread(&mut app_server, thread_id)
            .await;

        let reads = recorded_params(&requests, "thread/read");
        let include_turns = reads[previous_reads.len()..]
            .iter()
            .map(|params| params["includeTurns"].as_bool().unwrap_or(false))
            .collect::<Vec<_>>();
        assert_eq!(include_turns, expected_include_turns);
        assert_eq!(
            recorded_params(&requests, "thread/turns/list").len() - previous_turn_page_count,
            expected_turn_page_count
        );
    }

    app_server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn agents_overview_seeds_loaded_threads_when_recent_listing_is_unavailable() -> Result<()> {
    for (capabilities, expected_sort_keys) in [
        (
            HistoryCapabilities::LegacyOnly,
            vec!["recency_at", "recency_at", "updated_at", "updated_at"],
        ),
        (
            HistoryCapabilities::ThreadListFails,
            vec!["recency_at", "recency_at", "recency_at", "recency_at"],
        ),
    ] {
        let (mut app, _codex_home) = make_history_test_app().await?;
        let (mut app_server, requests, proxy) = start_recording_app_server_with_history(
            &app.config,
            capabilities,
            /*blocked_thread_list*/ None,
            /*failed_thread_name*/ None,
            crate::app_server_session::ThreadParamsMode::Embedded,
            LoaderOverrides::default(),
        )
        .await?;
        let started = app_server.start_thread(&app.config).await?;
        app.app_server_target = AppServerTarget::LocalDaemon {
            endpoint: crate::RemoteAppServerEndpoint::UnixSocket {
                socket_path: test_path_buf("/tmp/unused.sock").abs(),
            },
        };
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        app.app_event_tx = AppEventSender::new(tx);
        for attempt in 0..2 {
            if attempt == 0 {
                app.refresh_agents_overview_threads(&app_server);
            } else {
                app.open_agents_overview(
                    &app_server,
                    crate::app::agents_overview_view::AgentsOverviewFocus::List,
                );
            }
            let Some(AppEvent::AgentsOverviewThreadsLoaded { request_id, result }) =
                tokio::time::timeout(Duration::from_secs(10), rx.recv()).await?
            else {
                panic!("expected overview result")
            };
            app.apply_agents_overview_thread_refresh(&app_server, request_id, result);
            assert_eq!(
                app.agents_overview
                    .threads
                    .keys()
                    .copied()
                    .collect::<Vec<_>>(),
                vec![started.session.thread_id]
            );
            assert_eq!(
                app.agents_overview.initialized,
                capabilities != HistoryCapabilities::ThreadListFails || attempt > 0
            );
            if attempt == 0 {
                app.handle_app_server_event(
                    &app_server,
                    AppServerEvent::ServerNotification(Box::new(
                        ServerNotification::ThreadStatusChanged(
                            codex_app_server_protocol::ThreadStatusChangedNotification {
                                thread_id: started.session.thread_id.to_string(),
                                status: codex_app_server_protocol::ThreadStatus::Idle,
                            },
                        ),
                    )),
                )
                .await;
                assert!(app.agents_overview.request_id.is_none());
            }
        }
        let list_requests = recorded_params(&requests, "thread/list");
        let mut sort_keys = list_requests
            .iter()
            .map(|params| params["sortKey"].as_str().unwrap())
            .collect::<Vec<_>>();
        sort_keys.sort_unstable();
        assert_eq!(sort_keys, expected_sort_keys);
        app_server.shutdown().await?;
        proxy.await??;
    }
    Ok(())
}

#[tokio::test]
async fn agents_overview_stop_uses_full_history_after_legacy_negotiation() -> Result<()> {
    let (mut app, _codex_home) = make_history_test_app().await?;
    let thread_id = create_history_rollout(
        &app.config,
        ThreadHistoryMode::Paginated,
        "paginated background task",
    )?;
    let (mut app_server, requests, proxy) = start_recording_app_server_with_history(
        &app.config,
        HistoryCapabilities::LegacyOnly,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
        crate::app_server_session::ThreadParamsMode::Embedded,
        LoaderOverrides::default(),
    )
    .await?;
    app_server.start_thread(&app.config).await?;

    app.stop_agents_overview_thread(&mut app_server, thread_id)
        .await;

    let include_turns = recorded_params(&requests, "thread/read")
        .into_iter()
        .map(|params| params["includeTurns"].as_bool().unwrap_or(false))
        .collect::<Vec<_>>();
    assert_eq!(include_turns, vec![false, true]);
    assert!(recorded_params(&requests, "thread/turns/list").is_empty());

    app_server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn cold_paginated_subagent_transcript_excludes_inherited_parent_history() -> Result<()> {
    let (app, codex_home) = make_history_test_app().await?;
    let parent_thread_id = create_history_rollout(
        &app.config,
        ThreadHistoryMode::Paginated,
        "parent-only paginated history",
    )?;
    let child_timestamp = "2026-01-02T00-00-01";
    let child_thread_id = ThreadId::from_string(
        &create_fake_parented_rollout_with_source(
            codex_home.path(),
            child_timestamp,
            "2026-01-02T00:00:01Z",
            "child-only paginated history",
            Some(app.config.model_provider_id.as_str()),
            /*git_info*/ None,
            RolloutSessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id,
                depth: 1,
                agent_path: Some(
                    AgentPath::try_from("/root/worker").map_err(color_eyre::eyre::Report::msg)?,
                ),
                agent_nickname: Some("worker".to_string()),
                agent_role: Some("worker".to_string()),
            }),
            parent_thread_id.into(),
            parent_thread_id,
        )
        .map_err(|err| color_eyre::eyre::eyre!("failed to create subagent rollout: {err}"))?,
    )?;
    let child_rollout_path = rollout_path(
        codex_home.path(),
        child_timestamp,
        &child_thread_id.to_string(),
    );
    let mut child_lines = std::fs::read_to_string(&child_rollout_path)?
        .lines()
        .map(serde_json::from_str::<serde_json::Value>)
        .collect::<Result<Vec<_>, _>>()?;
    let mut meta = child_lines.remove(/*index*/ 0);
    meta["payload"]["history_mode"] = serde_json::json!("paginated");
    meta["payload"]["subagent_history_start_ordinal"] = serde_json::json!(3);
    meta["ordinal"] = serde_json::json!(0);
    for (index, line) in child_lines.iter_mut().enumerate() {
        line["ordinal"] = serde_json::json!(index + 3);
    }
    let rollout_record = |ordinal: usize, kind: &str, payload: serde_json::Value| {
        serde_json::json!({
            "timestamp": "2026-01-02T00:00:01Z",
            "ordinal": ordinal,
            "type": kind,
            "payload": payload,
        })
    };
    let inherited_response = rollout_record(
        /*ordinal*/ 1,
        "response_item",
        serde_json::json!({
            "type": "message",
            "role": "user",
            "content": [{
                "type": "input_text",
                "text": "parent-only paginated history",
            }],
        }),
    );
    let inherited_event = rollout_record(
        /*ordinal*/ 2,
        "event_msg",
        serde_json::json!({
            "type": "user_message",
            "message": "parent-only paginated history",
            "kind": "plain",
        }),
    );
    let lines = std::iter::once(meta)
        .chain([inherited_response, inherited_event])
        .chain(child_lines)
        .chain([
            rollout_record(
                /*ordinal*/ 5,
                "event_msg",
                serde_json::json!({
                    "type": "task_started",
                    "turn_id": "child-visible-turn",
                    "model_context_window": null,
                }),
            ),
            rollout_record(
                /*ordinal*/ 6,
                "event_msg",
                serde_json::json!({
                    "type": "item_completed",
                    "thread_id": child_thread_id,
                    "turn_id": "child-visible-turn",
                    "item": {
                        "type": "UserMessage",
                        "id": "child-visible-user",
                        "content": [{
                            "type": "text",
                            "text": "child-only paginated history",
                        }],
                    },
                }),
            ),
        ])
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(child_rollout_path, format!("{lines}\n"))?;
    let (mut app_server, requests, proxy) = start_recording_app_server(
        &app.config,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
    )
    .await?;

    let resumed = app_server
        .resume_thread(
            &app.local_settings,
            app.config.clone(),
            child_thread_id,
            crate::app_server_session::ResumeModelSettings::RestoreFromThread,
        )
        .await?;
    let child_turn_page = app_server
        .thread_turns_page(child_thread_id, /*cursor*/ None)
        .await?;
    let child_item_page = app_server
        .thread_items_page(
            child_thread_id,
            /*turn_id*/ None,
            /*cursor*/ None,
            /*limit*/ 16,
        )
        .await?;
    let [child_turn] = child_turn_page.data.as_slice() else {
        panic!("paginated subagent should expose exactly one child turn");
    };
    let [child_entry] = child_item_page.data.as_slice() else {
        panic!("paginated subagent should expose exactly one child message");
    };
    let ThreadItem::UserMessage { content, .. } = &child_entry.item else {
        panic!("paginated subagent should expose its child user message");
    };
    assert_eq!(resumed.session.thread_id, child_thread_id);
    assert_eq!(child_entry.turn_id, child_turn.id);
    assert_eq!(
        content,
        &[UserInput::Text {
            text: "child-only paginated history".to_string(),
            text_elements: Vec::new(),
        }],
    );
    assert_eq!(
        resumed
            .turns
            .iter()
            .flat_map(|turn| turn.items.iter())
            .collect::<Vec<_>>(),
        vec![&child_entry.item],
    );

    let cells = crate::thread_transcript::load_session_transcript(
        &mut app_server,
        child_thread_id,
        crate::thread_transcript::RawReasoningVisibility::Hidden,
        Some(&app.config),
    )
    .await?;
    let visible_history = cells
        .iter()
        .map(|cell| lines_to_single_string(&cell.display_lines(/*width*/ 80)))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(visible_history.contains("child-only paginated history"));
    assert!(!visible_history.contains("parent-only paginated history"));
    assert!(
        recorded_params(&requests, "thread/read")
            .iter()
            .all(|params| params["includeTurns"] != true)
    );

    app_server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn managed_worktree_transitions_bind_owner_and_preserve_only_fork_history() -> Result<()> {
    use crate::app_event::ManagedWorktreeMode;
    use std::fs;
    use std::process::Command;

    let (mut app, mut events, _op_rx) = make_test_app_with_channels().await;
    let root = tempdir()?;
    let home = dunce::canonicalize(root.path())?.join("home");
    let source = dunce::canonicalize(root.path())?.join("source");
    let project_pool = dunce::canonicalize(root.path())?.join("project-pool");
    fs::create_dir_all(&home)?;
    fs::create_dir_all(source.join(".codex"))?;
    fs::write(home.join("config.toml"), "[features]\nworktrees = true\n")?;
    crate::legacy_core::config::set_project_trust_level(
        &home,
        &source,
        codex_protocol::config_types::TrustLevel::Trusted,
    )
    .map_err(|error| color_eyre::eyre::eyre!(error.to_string()))?;
    let destination = format!(
        r#"developer_instructions = "committed policy"
model = "gpt-5.2"
model_reasoning_effort = "high"
[desktop]
git-worktree-root = {}
"#,
        toml::Value::String(project_pool.display().to_string()),
    );
    fs::write(source.join(".codex/config.toml"), destination)?;
    fs::write(source.join("AGENTS.md"), "committed worktree instructions")?;
    for args in [
        vec!["init", "--quiet"],
        vec!["add", "."],
        vec!["commit", "--quiet", "--no-gpg-sign", "-m", "initial"],
    ] {
        let result = Command::new("git")
            .current_dir(&source)
            .args([
                "-c",
                "user.name=Worktree Test",
                "-c",
                "user.email=test@example.invalid",
            ])
            .args(args)
            .output()?;
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    fs::write(
        source.join(".codex/config.toml"),
        r#"developer_instructions = "dirty policy"
model = "gpt-5.4"
model_reasoning_effort = "low"
"#,
    )?;
    fs::write(source.join("AGENTS.md"), "dirty source instructions")?;
    app.config.codex_home = home.clone().abs();
    app.config.sqlite = SqliteConfig::new_for_testing(home.clone().abs());
    app.config.cwd = source.clone().abs();
    app.harness_overrides.permission_profile = Some(PermissionProfile::workspace_write());
    app.config = app.rebuild_config_for_cwd(source.clone()).await?;
    app.chat_widget
        .handle_thread_session_quiet(test_thread_session(ThreadId::new(), source.clone()));
    let (mut server, requests, proxy) = start_recording_app_server(
        &app.config,
        /*blocked_thread_list*/ None,
        /*failed_thread_name*/ None,
    )
    .await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    app.start_fresh_session_with_summary_hint(
        &mut tui,
        &mut server,
        /*session_start_source*/ None,
        /*initial_user_message*/ None,
        /*new_thread_name*/ None,
    )
    .await;
    assert_eq!(
        (
            app.chat_widget.current_model(),
            app.config.model_provider_id.as_str(),
            app.chat_widget.current_reasoning_effort()
        ),
        ("gpt-5.4", "openai", Some(ReasoningEffortConfig::Low))
    );
    // Provider selection belongs to host config, not project config.
    let host_config = home.join("config.toml");
    let contents = fs::read_to_string(&host_config)?;
    fs::write(
        host_config,
        format!("model_provider = \"ollama\"\n{contents}"),
    )?;
    let original = app.chat_widget.thread_id().expect("original thread");
    server.thread_inject_items(original, vec![serde_json::from_value(serde_json::json!({
        "type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "managed fork history"}]
    }))?]).await?;
    let manager = codex_worktree::WorktreeManager::new(
        codex_worktree::WorktreeSettings::for_cli(&home, /*desktop*/ None)
            .map_err(|error| color_eyre::eyre::eyre!(error.to_string()))?,
    );
    let mut browser_entries = Vec::new();
    requests.lock().expect("request recorder lock").clear();
    app.handle_event(
        &mut tui,
        &mut server,
        AppEvent::StartManagedWorktree {
            mode: ManagedWorktreeMode::Fork,
            name: None,
        },
    )
    .await?;
    complete_managed_worktree_creation(&mut app, &mut tui, &mut server, &mut events).await?;
    assert_eq!(app.chat_widget.thread_id(), Some(original));
    assert!(recorded_params(&requests, "thread/fork").is_empty());
    let unused = manager
        .list(&source)
        .map_err(|error| color_eyre::eyre::eyre!(error.to_string()))?
        .pop()
        .expect("unused checkout");
    browser_entries.push(crate::worktree_browser::Entry {
        root: unused.root.clone(),
        cwd: unused.cwd.clone(),
        owner: crate::worktree_browser::Owner::None,
    });
    assert_eq!(
        manager
            .owner(&unused.root)
            .map_err(|error| color_eyre::eyre::eyre!(error.to_string()))?,
        None
    );
    let error = std::iter::from_fn(|| events.try_recv().ok())
        .filter_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => {
                Some(lines_to_single_string(&cell.display_lines(/*width*/ 500)))
            }
            _ => None,
        })
        .find(|message| message.contains("Cannot fork into this worktree"))
        .expect("fork refusal");
    let (_, with_path) = error
        .split_once("An unused checkout was created at ")
        .expect("checkout marker");
    let (path, _) = with_path
        .split_once("; remove it with")
        .expect("cleanup marker");
    insta::assert_snapshot!(error.replace(path, "<CHECKOUT>"), @"■ Cannot fork into this worktree because developer instructions differ. Start a new conversation instead. An unused checkout was created at <CHECKOUT>; remove it with `git worktree remove <checkout-path>` from the source repository.");
    let retained = std::iter::from_fn(|| events.try_recv().ok())
        .filter_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => {
                Some(lines_to_single_string(&cell.display_lines(/*width*/ 500)))
            }
            _ => None,
        })
        .find(|message| message.contains("A checkout was retained at"))
        .expect("post-allocation recovery message");
    let (_, with_path) = retained
        .split_once("A checkout was retained at ")
        .expect("retained checkout marker");
    let (path, _) = with_path
        .split_once("; remove it with")
        .expect("retained checkout cleanup marker");
    assert_eq!(
        dunce::canonicalize(path)?,
        dunce::canonicalize(&unused.root)?
    );
    assert!(retained.contains("git worktree remove <checkout-path>"));
    fs::write(
        source.join(".codex/config.toml"),
        r#"developer_instructions = "committed policy"
[features]
terminal_visualization_instructions = true
"#,
    )?;
    app.config = app.rebuild_config_for_cwd(source.clone()).await?;
    requests.lock().expect("request recorder lock").clear();
    app.handle_event(
        &mut tui,
        &mut server,
        AppEvent::StartManagedWorktree {
            mode: ManagedWorktreeMode::Fork,
            name: None,
        },
    )
    .await?;
    complete_managed_worktree_creation(&mut app, &mut tui, &mut server, &mut events).await?;
    assert_eq!(app.chat_widget.thread_id(), Some(original));
    assert!(recorded_params(&requests, "thread/fork").is_empty());
    let feature_mismatch_checkout = manager
        .list(&source)
        .map_err(|error| color_eyre::eyre::eyre!(error.to_string()))?
        .into_iter()
        .find(|checkout| checkout.cwd != unused.cwd)
        .expect("feature mismatch leaves an unused checkout");
    browser_entries.push(crate::worktree_browser::Entry {
        root: feature_mismatch_checkout.root,
        cwd: feature_mismatch_checkout.cwd,
        owner: crate::worktree_browser::Owner::None,
    });
    while events.try_recv().is_ok() {}
    app.handle_event(
        &mut tui,
        &mut server,
        AppEvent::StartManagedWorktree {
            mode: ManagedWorktreeMode::New,
            name: None,
        },
    )
    .await?;
    assert_eq!(app.chat_widget.thread_id(), Some(original));
    drain_managed_worktree_start(&mut app, &mut server).await;
    assert!(app.pending_managed_worktree_creation);
    let created = loop {
        let event = tokio::time::timeout(Duration::from_secs(15), events.recv())
            .await?
            .ok_or_else(|| color_eyre::eyre::eyre!("worktree completion channel closed"))?;
        if let AppEvent::ManagedWorktreeCreated(created) = event {
            break created;
        }
    };
    let stale_checkout = created
        .result
        .as_ref()
        .expect("created checkout")
        .1
        .root
        .clone();
    let stale_cwd = created
        .result
        .as_ref()
        .expect("created checkout")
        .1
        .cwd
        .clone();
    app.primary_thread_id = Some(ThreadId::new());
    app.handle_event(
        &mut tui,
        &mut server,
        AppEvent::ManagedWorktreeCreated(created),
    )
    .await?;
    let created = app
        .pending_managed_worktree_created
        .take()
        .expect("deferred checkout completion");
    app.finish_managed_worktree(*created).await;
    app.primary_thread_id = Some(original);
    assert_eq!(app.chat_widget.thread_id(), Some(original));
    assert!(!app.pending_managed_worktree_creation);
    let stale_error = std::iter::from_fn(|| events.try_recv().ok())
        .filter_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => {
                Some(lines_to_single_string(&cell.display_lines(/*width*/ 500)))
            }
            _ => None,
        })
        .find(|message| message.contains("A checkout was retained at"))
        .expect("stale creation recovery message");
    assert!(stale_error.contains(&stale_checkout.display().to_string()));
    assert!(stale_error.contains("git worktree remove <checkout-path>"));
    assert_eq!(
        manager
            .owner(&stale_checkout)
            .map_err(|error| color_eyre::eyre::eyre!(error.to_string()))?,
        None
    );
    browser_entries.push(crate::worktree_browser::Entry {
        root: stale_checkout,
        cwd: stale_cwd,
        owner: crate::worktree_browser::Owner::None,
    });
    for mode in [ManagedWorktreeMode::New, ManagedWorktreeMode::Fork] {
        requests.lock().expect("request recorder lock").clear();
        let previous = app.chat_widget.thread_id();
        let previous_rollout = app.chat_widget.rollout_path().expect("previous rollout");
        app.handle_event(
            &mut tui,
            &mut server,
            AppEvent::StartManagedWorktree {
                mode,
                name: Some(format!("{mode:?} worktree")),
            },
        )
        .await?;
        complete_managed_worktree_creation(&mut app, &mut tui, &mut server, &mut events).await?;
        let replacement = app.chat_widget.thread_id().expect("replacement thread");
        assert_ne!(Some(replacement), previous);
        assert_eq!(
            app.chat_widget.thread_name(),
            Some(format!("{mode:?} worktree"))
        );
        let cwd = app.config.cwd.as_path().canonicalize()?;
        let checkouts = manager
            .list(&source)
            .map_err(|error| color_eyre::eyre::eyre!(error.to_string()))?;
        let checkout = checkouts
            .iter()
            .find(|checkout| checkout.cwd.canonicalize().ok().as_ref() == Some(&cwd))
            .expect("managed checkout");
        browser_entries.push(crate::worktree_browser::Entry {
            root: checkout.root.clone(),
            cwd: checkout.cwd.clone(),
            owner: crate::worktree_browser::Owner::Unavailable(replacement),
        });
        assert!(checkout.root.starts_with(home.join("worktrees")));
        assert!(!project_pool.exists());
        assert_eq!(
            manager
                .owner(&checkout.root)
                .map_err(|error| color_eyre::eyre::eyre!(error.to_string()))?,
            Some(replacement.to_string())
        );
        assert_eq!(
            app.config.developer_instructions.as_deref(),
            Some("committed policy")
        );
        assert_eq!(
            fs::read_to_string(checkout.cwd.join("AGENTS.md"))?,
            "committed worktree instructions"
        );
        let fork = matches!(mode, ManagedWorktreeMode::Fork);
        let method = if fork { "thread/fork" } else { "thread/start" };
        let params = recorded_params(&requests, method);
        assert_eq!(params.len(), 1);
        assert_eq!(
            [
                params[0]["model"].as_str(),
                params[0]["modelProvider"].as_str(),
                params[0]["config"]["model_reasoning_effort"].as_str(),
            ],
            [Some("gpt-5.2"), Some("ollama"), Some("high")]
        );
        // Injection flushes and materializes a new thread's otherwise lazy rollout.
        server.thread_inject_items(replacement, vec![serde_json::from_value(serde_json::json!({
            "type": "message", "role": "assistant", "content": [{"type": "output_text", "text": format!("replacement persistence probe {mode:?}")}]
        }))?]).await?;
        let rollout = app.chat_widget.rollout_path().expect("replacement rollout");
        let mut history = fs::read(&rollout)?;
        let metadata = codex_rollout::read_session_meta_line(&rollout).await?;
        assert_eq!(metadata.meta.history_base.is_some(), fork);
        if let Some(base) = metadata.meta.history_base {
            assert!(fork, "New must not inherit history");
            assert_eq!(Some(base.thread_id), previous);
            let parent = fs::read(&previous_rollout)?;
            history.extend_from_slice(
                parent
                    .get(..usize::try_from(base.end_byte_offset)?)
                    .expect("valid inherited prefix"),
            );
        }
        let history = String::from_utf8(history)?;
        assert!(!history.contains("managed fork history"));
        if fork {
            assert!(history.contains("replacement persistence probe New"));
        }
        let attached = server
            .thread_read(replacement, /*include_turns*/ false)
            .await?;
        assert_eq!(attached.cwd.as_path().canonicalize()?, cwd);
    }
    browser_entries.sort_by(|left, right| left.cwd.cmp(&right.cwd));
    let nested = source.join("browser-only");
    fs::create_dir(&nested)?;
    assert_eq!(
        crate::worktree_browser::list(home.clone(), nested)
            .await
            .map_err(|error| color_eyre::eyre::eyre!(error.to_string()))?,
        browser_entries
    );
    let unowned = manager
        .create(&codex_worktree::CreateWorktree {
            source_cwd: source.clone(),
            base: None,
        })
        .map_err(|error| color_eyre::eyre::eyre!(error.to_string()))?;
    browser_entries.push(crate::worktree_browser::Entry {
        root: unowned.root.clone(),
        cwd: unowned.cwd.clone(),
        owner: crate::worktree_browser::Owner::None,
    });
    browser_entries.sort_by(|left, right| left.cwd.cmp(&right.cwd));
    for owner in [None, Some("not-a-thread-uuid")] {
        if let Some(owner) = owner {
            manager
                .bind_thread(&unowned.root, owner)
                .map_err(|error| color_eyre::eyre::eyre!(error.to_string()))?;
        }
        assert_eq!(
            crate::worktree_browser::list(home.clone(), source.clone())
                .await
                .map_err(|error| color_eyre::eyre::eyre!(error.to_string()))?,
            browser_entries
        );
    }
    let missing_owner = ThreadId::new();
    let missing_checkout = manager
        .create(&codex_worktree::CreateWorktree {
            source_cwd: source.clone(),
            base: None,
        })
        .map_err(|error| color_eyre::eyre::eyre!(error.to_string()))?;
    manager
        .bind_thread(&missing_checkout.root, &missing_owner.to_string())
        .map_err(|error| color_eyre::eyre::eyre!(error.to_string()))?;
    let (browser_tx, mut browser_rx) = tokio::sync::mpsc::unbounded_channel();
    crate::worktree_browser::fetch(
        crate::worktree_browser::Request {
            id: uuid::Uuid::new_v4(),
            cwd: source.clone(),
            thread_id: Some(original),
        },
        home.clone(),
        server.request_handle(),
        crate::app_event_sender::AppEventSender::new(browser_tx),
    );
    let Some(AppEvent::ManagedWorktreesLoaded {
        result: Ok(entries),
        ..
    }) = tokio::time::timeout(
        std::time::Duration::from_secs(/*secs*/ 5),
        browser_rx.recv(),
    )
    .await?
    else {
        panic!("browser must return annotated worktrees");
    };
    assert_eq!(
        entries
            .iter()
            .find(|entry| entry.root == missing_checkout.root)
            .map(|entry| &entry.owner),
        Some(&crate::worktree_browser::Owner::Unavailable(missing_owner))
    );
    app.start_fresh_session_with_summary_hint(
        &mut tui,
        &mut server,
        /*session_start_source*/ None,
        /*initial_user_message*/ None,
        /*new_thread_name*/ None,
    )
    .await;
    let unsaved = app.chat_widget.thread_id().expect("unsaved thread");
    let path = app.chat_widget.rollout_path();
    assert!(path.is_none_or(|path| !rollout_path_is_resumable(&path)));
    let completed = test_turn("completed-unsaved-turn", TurnStatus::Completed, Vec::new());
    app.thread_event_channels[&unsaved]
        .store
        .lock()
        .await
        .set_turns(vec![completed]);
    for mode in [ManagedWorktreeMode::Fork, ManagedWorktreeMode::New] {
        requests.lock().expect("request recorder lock").clear();
        app.handle_event(
            &mut tui,
            &mut server,
            AppEvent::StartManagedWorktree { mode, name: None },
        )
        .await?;
        drain_managed_worktree_start(&mut app, &mut server).await;
        let is_new = mode == ManagedWorktreeMode::New;
        if is_new {
            complete_managed_worktree_creation(&mut app, &mut tui, &mut server, &mut events)
                .await?;
        }
        assert_eq!(app.chat_widget.thread_id() != Some(unsaved), is_new);
        assert_eq!(recorded_params(&requests, "thread/fork").len(), 0);
    }
    let saved_owner = entries
        .iter()
        .find_map(|entry| match &entry.owner {
            crate::worktree_browser::Owner::Resumable(thread) => Some(thread.clone()),
            _ => None,
        })
        .expect("browser should resolve a saved owner");
    // Archive stops the writer; unarchive leaves a stored thread eligible for compression.
    server.thread_archive(saved_owner.id).await?;
    let saved_path = server
        .thread_unarchive(saved_owner.id)
        .await?
        .path
        .expect("saved rollout path");
    fs::OpenOptions::new()
        .write(true)
        .open(&saved_path)?
        .set_times(std::fs::FileTimes::new().set_modified(
            std::time::SystemTime::now() - std::time::Duration::from_secs(8 * 24 * 60 * 60),
        ))?;
    codex_rollout::spawn_rollout_compression_worker(home.clone());
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while saved_path.exists() || !saved_path.with_extension("jsonl.zst").is_file() {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await?;
    let (browser_tx, mut browser_rx) = tokio::sync::mpsc::unbounded_channel();
    crate::worktree_browser::fetch(
        crate::worktree_browser::Request {
            id: uuid::Uuid::new_v4(),
            cwd: source.clone(),
            thread_id: Some(original),
        },
        home.clone(),
        server.request_handle(),
        crate::app_event_sender::AppEventSender::new(browser_tx),
    );
    let Some(AppEvent::ManagedWorktreesLoaded {
        result: Ok(compressed_entries),
        ..
    }) = tokio::time::timeout(std::time::Duration::from_secs(10), browser_rx.recv()).await?
    else {
        panic!("browser must return compressed owner metadata");
    };
    assert!(compressed_entries.iter().any(|entry| matches!(
        &entry.owner,
        crate::worktree_browser::Owner::Resumable(thread) if thread.id == saved_owner.id
    )));
    server.thread_archive(saved_owner.id).await?;
    let (browser_tx, mut browser_rx) = tokio::sync::mpsc::unbounded_channel();
    crate::worktree_browser::fetch(
        crate::worktree_browser::Request {
            id: uuid::Uuid::new_v4(),
            cwd: source.clone(),
            thread_id: Some(original),
        },
        home,
        server.request_handle(),
        crate::app_event_sender::AppEventSender::new(browser_tx),
    );
    let Some(AppEvent::ManagedWorktreesLoaded {
        result: Ok(entries),
        ..
    }) = tokio::time::timeout(
        std::time::Duration::from_secs(/*secs*/ 10),
        browser_rx.recv(),
    )
    .await?
    else {
        panic!("browser must return archived owner metadata");
    };
    assert!(entries.iter().any(|entry| matches!(
        &entry.owner,
        crate::worktree_browser::Owner::Archived(thread) if thread.id == saved_owner.id
    )));
    server.shutdown().await?;
    proxy.await??;
    Ok(())
}

#[tokio::test]
async fn changing_directory_preserves_project_trust_permissions_history_and_hooks() -> Result<()> {
    use codex_protocol::config_types::TrustLevel as T;
    use serde_json::json;
    use std::fs;

    let (mut app, mut events, _op_rx) = make_test_app_with_channels().await;
    let codex_home = tempdir()?;
    app.config.codex_home = codex_home.path().to_path_buf().abs();
    app.config.sqlite = SqliteConfig::new_for_testing(codex_home.path().abs());
    app.harness_overrides.permission_profile = Some(PermissionProfile::workspace_write());
    let names = ["root", "trusted", "unknown", "untrusted", "p", "failure"];
    let [current, trusted, unknown, untrusted, mismatch, failed] =
        names.map(|name| codex_home.path().join(name));
    fs::create_dir_all(&current)?;
    for directory in [&trusted, &unknown, &untrusted, &mismatch, &failed] {
        fs::create_dir_all(directory.join(".codex"))?;
        fs::write(directory.join(".codex/config.toml"), "")?;
    }
    let contents = "developer_instructions = \"destination policy\"\nmodel_reasoning_effort = \"high\"\napproval_policy = \"on-request\"\n[tui]\ntheme = \"dracula\"\n[tui.keymap.global]\nopen_transcript = \"f12\"";
    fs::write(trusted.join(".codex/config.toml"), contents)?;
    let agents = trusted.join("AGENTS.md");
    fs::write(&agents, "Follow destination project instructions.")?;
    let hooks = r#"{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"x"}]}]}}"#;
    fs::write(trusted.join(".codex/hooks.json"), hooks)?;
    let contents = "default_permissions = \"dev\"\n[permissions.dev.filesystem]\n\":root\" = \"write\"\n[tui.keymap.global]\nopen_transcript = \"ctrl-l\"";
    fs::write(mismatch.join(".codex/config.toml"), contents)?;
    let requirements = codex_home.path().join("requirements.toml");
    let rules = "allowed_approval_policies=[\"untrusted\"]\nallowed_sandbox_modes=[\"read-only\"]";
    fs::write(&requirements, rules)?;
    fs::create_dir_all(unknown.join(".git"))?;
    for dir in [&trusted, &untrusted, &mismatch, &failed] {
        let trust = [T::Trusted, T::Untrusted][usize::from(dir == &untrusted)];
        crate::legacy_core::config::set_project_trust_level(codex_home.path(), dir, trust)
            .map_err(|error| color_eyre::eyre::eyre!(error.to_string()))?;
    }
    app.config.cwd = current.clone().abs();
    app.chat_widget
        .handle_thread_session_quiet(test_thread_session(ThreadId::new(), current.clone()));
    let (no_list, nick, role, runtime, background) = (None, None, None, None, Some("background"));
    let (mut server, requests, proxy) =
        start_recording_app_server(&app.config, no_list, background).await?;
    let (rec, plain, req) = (recorded_params, crate::key_hint::plain, &requests);
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let (source, message, name) = (None, None, Some("Previous project".to_string()));
    app.start_fresh_session_with_summary_hint(&mut tui, &mut server, source, message, name)
        .await;
    let original = app.chat_widget.thread_id().expect("original thread");
    let rollout = app.chat_widget.rollout_path().expect("original rollout");
    let json = r#"{"type":"message","role":"assistant","content":[{"type":"output_text","text":"saved history"}]}"#;
    let item = serde_json::from_str(json)?;
    server.thread_inject_items(original, vec![item]).await?;
    let tracked = server.start_thread(&app.config).await?;
    let (child, capacity) = (tracked.session.thread_id, THREAD_EVENT_CHANNEL_CAPACITY);
    let channel = ThreadEventChannel::new_with_session(capacity, tracked.session, tracked.turns);
    app.thread_event_channels.insert(child, channel);
    app.agent_navigation
        .upsert(child, nick, role, /*is_closed*/ false);
    let store = app.thread_event_channels[&child].store.clone();
    let config_path = codex_home.path().join("config.toml");
    let original_user_config = fs::read_to_string(&config_path).ok();
    let (local, url) = (app.environment_manager.clone(), Some("ws://[::1]".into()));
    let remote = Arc::new(EnvironmentManager::create_for_tests(url, runtime).await);
    let mut history = || {
        std::iter::from_fn(|| events.try_recv().ok())
            .filter_map(|event| match event {
                AppEvent::InsertHistoryCell(cell) => Some(cell),
                _ => None,
            })
            .map(|cell| lines_to_single_string(&cell.display_lines(/*width*/ 200)))
            .collect::<Vec<_>>()
    };
    let change = |thread_id, path: &str| AppEvent::ChangeWorkingDirectory {
        thread_id,
        requested_cwd: path.into(),
    };
    for (path, kind, expected) in [
        ("missing", "local", "Cannot access directory"),
        ("../config.toml", "local", "Not a directory"),
        (r"C:\bad", "workspace", "not supported for remote"),
        ("~", "executor", "not supported for remote"),
        ("../trusted", "stale", "requires an idle primary session"),
        ("../trusted", "running", "another agent is running"),
        ("../trusted", "active", "another agent is running"),
        ("../trusted", "mcp", "inventory is still loading"),
        ("../trusted", "approval", "approval policy override"),
        ("../trusted", "profile", "permission profile override"),
        ("../trusted", "reviewer", "reviewer"),
        ("../p", "named", "different settings"),
        (
            "../trusted",
            "restored",
            "Permission profile cannot be preserved",
        ),
        ("../p", "keymap", "open_transcript"),
        ("../unknown", "local", "This directory is not trusted"),
        ("../trusted", "main", "background terminals"),
        ("../trusted", "child", "background terminals"),
    ] {
        app.config.approvals_reviewer = ApprovalsReviewer::User;
        if kind == "reviewer" {
            app.config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            fs::write(&requirements, "allowed_approvals_reviewers = [\"user\"]")?;
        }
        app.agent_navigation.set_running(child, kind == "running");
        store.lock().await.active_turn_id = (kind == "active").then(|| "active".into());
        app.loader_overrides.system_requirements_path =
            matches!(kind, "approval" | "profile" | "reviewer").then_some(requirements.clone());
        app.harness_overrides.permission_profile =
            (kind != "named").then_some(PermissionProfile::workspace_write());
        app.runtime_approval_policy_override = (kind == "approval").then_some(
            RuntimeApprovalPolicyOverride::Explicit(AskForApproval::OnRequest),
        );
        let mut profile = RuntimePermissionProfileOverride::from_config(&app.config);
        profile.active_permission_profile =
            (kind == "named").then(|| ActivePermissionProfile::new("dev"));
        if kind == "restored" {
            profile.permission_profile = PermissionProfile::workspace_write_with(
                &[failed.clone().abs()],
                codex_protocol::permissions::NetworkSandboxPolicy::Restricted,
                /*exclude_tmpdir_env_var*/ false,
                /*exclude_slash_tmp*/ false,
            );
            profile.turn_override = RuntimePermissionProfileTurnOverride::Preserve;
        }
        app.runtime_permission_profile_override =
            matches!(kind, "profile" | "reviewer" | "named" | "restored").then_some(profile);
        app.app_server_target = crate::AppServerTarget::Embedded;
        if kind == "workspace" {
            let endpoint = crate::resolve_remote_addr("ws://127.0.0.1:8765")?;
            app.app_server_target = crate::AppServerTarget::Remote { endpoint };
        }
        app.environment_manager = [&local, &remote][usize::from(kind == "executor")].clone();
        requests.lock().expect("request recorder lock").clear();
        if kind == "mcp" {
            let loading = history_cell::new_mcp_inventory_loading;
            app.transcript_cells
                .push(Arc::new(loading(/*animations_enabled*/ false)));
        } else if kind == "child" {
            app.thread_event_channels.remove(&child);
        }
        let thread_id = [original, ThreadId::new()][usize::from(kind == "stale")];
        app.handle_event(&mut tui, &mut server, change(thread_id, path))
            .await?;
        if let Some(pending) = app.pending_working_directory_change.take() {
            Box::pin(app.finish_working_directory_change(&mut tui, &mut server, pending)).await;
        }
        assert_eq!(app.chat_widget.thread_id(), Some(original));
        assert_eq!(app.config.cwd, current.clone().abs());
        assert!(app.runtime_working_directory_override.is_none());
        let count = requests.lock().expect("request recorder lock").len();
        let checked = usize::from(kind == "main") + 2 * usize::from(kind == "child");
        assert_eq!(count, checked, "{kind}");
        let listed = recorded_params(&requests, "thread/backgroundTerminals/list");
        let mut ids = listed.iter().zip([original, child]);
        assert!(ids.all(|(p, id)| p["threadId"] == id.to_string()));
        assert_eq!(fs::read_to_string(&config_path).ok(), original_user_config);
        let output = history().join("");
        if kind == "mcp" {
            assert_snapshot!(output, @"■ MCP inventory is still loading.");
        } else if kind == "restored" {
            assert_snapshot!(output, @"■ Permission profile cannot be preserved by /cd.");
        }
        assert!(output.contains(expected), "{path}");
        app.clear_committed_mcp_inventory_loading();
    }
    let tracked = server.start_thread(&app.config).await?;
    let closed = tracked.session.thread_id;
    let channel = ThreadEventChannel::new_with_session(
        THREAD_EVENT_CHANNEL_CAPACITY,
        tracked.session,
        tracked.turns,
    );
    app.thread_event_channels.insert(closed, channel);
    app.agent_navigation.mark_closed(child);
    for has_stale_replay_turn in [false, true] {
        app.agent_navigation.upsert(
            closed, /*agent_nickname*/ None, /*agent_role*/ None,
            /*is_closed*/ false,
        );
        let channel = app.thread_event_channels.get_mut(&closed).expect("channel");
        channel.store.lock().await.set_turns(vec![test_turn(
            "stale-turn",
            TurnStatus::InProgress,
            Vec::new(),
        )]);
        if has_stale_replay_turn {
            channel.mark_replay_only();
            app.agent_navigation.mark_closed(closed);
        } else {
            app.enqueue_thread_notification(closed, thread_closed_notification(closed))
                .await?;
        }
        requests.lock().expect("request recorder lock").clear();
        app.change_working_directory(&mut tui, &mut server, failed.clone().abs())
            .await;
        assert_eq!(
            recorded_params(&requests, "thread/backgroundTerminals/list"),
            vec![json!({"threadId": original.to_string(), "cursor": null, "limit": 1})],
        );
        let output = history().join("");
        insta::allow_duplicates! {
            assert_snapshot!(output, @"■ Failed to change: thread/fork failed during TUI bootstrap: thread/fork failed: forced thread/name/set failure (code -32603)");
        }
    }
    app.agent_navigation.upsert(
        child, /*agent_nickname*/ None, /*agent_role*/ None, /*is_closed*/ false,
    );
    app.set_approvals_reviewer_in_app_and_widget(ApprovalsReviewer::AutoReview);
    app.runtime_permission_profile_override =
        Some(RuntimePermissionProfileOverride::from_config(&app.config));
    for (path, expected) in [(&failed, 0), (&trusted, 2)] {
        requests.lock().expect("request recorder lock").clear();
        app.change_working_directory(&mut tui, &mut server, path.clone().abs())
            .await;
        assert_eq!(app.chat_widget.thread_id(), Some(original));
        assert_eq!(app.config.cwd, current.clone().abs());
        assert_eq!(rec(&requests, "thread/unsubscribe").len(), expected);
        assert!(history().join("").contains("change"));
    }
    let removed = recorded_params(&requests, "thread/unsubscribe");
    let archived = recorded_params(&requests, "thread/archive");
    assert_eq!(removed[0]["threadId"], original.to_string());
    assert_eq!(archived[0]["threadId"], removed[1]["threadId"]);
    requests.lock().expect("request recorder lock").clear();
    app.handle_event(&mut tui, &mut server, change(original, "../trusted"))
        .await?;
    // Dispatch only queues /cd: configuration loading happens on a fresh loop iteration.
    assert_eq!(app.config.cwd, current.clone().abs());
    let pending = app
        .pending_working_directory_change
        .take()
        .expect("valid /cd queued");
    Box::pin(app.finish_working_directory_change(&mut tui, &mut server, pending)).await;
    let forked = app.chat_widget.thread_id().expect("forked thread");
    assert_ne!(forked, original);
    let forked_rollout = app.chat_widget.rollout_path().expect("forked rollout");
    assert!(fs::read_to_string(&rollout)?.contains("saved history"));
    let copied = fs::read_to_string(&forked_rollout)?;
    let meta = codex_rollout::read_session_meta_line(&forked_rollout).await?;
    let base = meta.meta.history_base;
    assert!(copied.contains("saved history") || base.is_some_and(|h| h.thread_id == original));
    assert_eq!(app.config.cwd, trusted.clone().abs());
    let configured = app.primary_session_configured.as_ref().expect("session");
    let source = codex_utils_path_uri::PathUri::from_abs_path(&agents.abs());
    assert!(configured.instruction_source_paths.contains(&source));
    let (cwd, result) = (current.clone(), Err("stale skills".into()));
    let skills = AppEvent::SkillsListLoaded { cwd, result };
    let (cwd, plugins) = (current.clone(), Some(vec![]));
    let plugins = AppEvent::PluginMentionsLoaded { cwd, plugins };
    let diff = AppEvent::DiffResult(current.clone(), "stale diff".to_string());
    let branch = AppEvent::SyncThreadGitBranch {
        thread_id: original,
        branch: "stale".to_string(),
        cwd: current.clone(),
    };
    for event in [diff, skills, plugins, branch] {
        app.handle_event(&mut tui, &mut server, event).await?;
    }
    let path = trusted.to_str().expect("trusted path");
    let output = history().join("").replace(path, "<PROJECT>");
    let message = &output[output.rfind('•').expect("change")..];
    assert_snapshot!(message, @"• Working directory changed to: <PROJECT>");
    assert!(!output.contains("stale skills"));
    assert!(app.overlay.is_none());
    assert_eq!(app.keymap.app.open_transcript, vec![plain(KeyCode::F(12))]);
    let anchor = app.runtime_working_directory_override.as_deref();
    assert_eq!(anchor, Some(trusted.as_path()));
    let effort = app.chat_widget.current_reasoning_effort();
    assert_eq!(effort, Some(ReasoningEffortConfig::High));
    let approval = app.config.permissions.approval_policy.value();
    assert_eq!(approval, AskForApproval::OnRequest.to_core());
    let forks = recorded_params(&requests, "thread/fork");
    assert_eq!(forks.len(), 1);
    let params = &forks[0];
    assert_eq!(params["threadId"], serde_json::json!(original.to_string()));
    assert_eq!(params["cwd"], serde_json::json!(trusted));
    assert_eq!(params["approvalsReviewer"].as_str(), Some("auto_review"));
    assert_eq!(params["developerInstructions"], "destination policy");
    assert_eq!(params["deferGoalContinuation"], serde_json::json!(true));
    assert_eq!(&params["runtimeWorkspaceRoots"], &json!([trusted]));
    assert_eq!(rec(&requests, "hooks/list")[0]["cwds"], json!([trusted]));
    for suffix in "start resume settings/update archive".split(' ') {
        assert!(recorded_params(&requests, &format!("thread/{suffix}")).is_empty());
    }
    assert!(rec(req, "thread/metadata/update")[0]["threadId"] == params["threadId"]);
    let removed = recorded_params(&requests, "thread/unsubscribe");
    assert_eq!(removed.len(), 2);
    let found = |id: ThreadId| removed.iter().any(|p| p["threadId"] == id.to_string());
    assert!([original, child].into_iter().all(found));
    let retained = server.thread_read(original, /*include_turns*/ false);
    assert_eq!(retained.await?.cwd, current.abs().canonicalize()?);
    assert_eq!(app.chat_widget.config_ref().cwd, trusted.clone().abs());
    assert!(render_bottom_popup(&app.chat_widget, /*width*/ 80).contains("SessionStart"));
    app.chat_widget
        .handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    app.harness_overrides.bypass_hook_trust = Some(true);
    requests.lock().expect("request recorder lock").clear();
    app.change_working_directory(&mut tui, &mut server, trusted.abs())
        .await;
    assert!(app.config.bypass_hook_trust && !app.chat_widget.has_active_view());
    assert!(recorded_params(&requests, "hooks/list").is_empty());
    app.harness_overrides.bypass_hook_trust = None;
    requests.lock().expect("request recorder lock").clear();
    app.change_working_directory(&mut tui, &mut server, untrusted.clone().abs())
        .await;
    assert_eq!(app.config.active_project.trust_level, Some(T::Untrusted));
    let approval = app.config.permissions.approval_policy.value();
    assert_eq!(approval, AskForApproval::UnlessTrusted.to_core());
    assert_eq!(rec(req, "thread/fork")[0]["approvalPolicy"], "untrusted");
    let warning = "Project-local config, hooks, and exec policies are disabled";
    assert!(history().iter().any(|line| line.contains(warning)));
    server.shutdown().await?;
    proxy.await??;
    Ok(())
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
    const TEST_STACK_SIZE_BYTES: usize = 16 * 1024 * 1024;

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
                        &app.local_settings,
                        app.config.clone(),
                        root_thread_id,
                        app.resume_model_settings(),
                    )
                    .await?;
                app.enqueue_primary_thread_session(root.session, root.turns)
                    .await?;
                app_server
                    .resume_thread(
                        &app.local_settings,
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
                crate::legacy_core::config::set_project_trust_level(
                    app.config.codex_home.as_path(),
                    app.config.cwd.as_path(),
                    codex_protocol::config_types::TrustLevel::Trusted,
                )
                .map_err(std::io::Error::other)?;
                app.harness_overrides.cwd = Some(app.config.cwd.to_path_buf());

                let control = app
                    .resume_target_session(
                        &mut tui,
                        &mut app_server,
                        crate::resume_picker::SessionTarget {
                            path: Some(root_rollout_path),
                            thread_id: root_thread_id,
                            cwd: None,
                            history_mode: None,
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
                Box::pin(app.handle_event(&mut tui, &mut app_server, completion)).await?;
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

#[path = "new_session_tests.rs"]
mod new_session_tests;
#[path = "startup_defaults_tests.rs"]
mod startup_defaults_tests;

#[tokio::test]
async fn external_writer_escape_preserves_snapshot_and_explicit_quits() -> Result<()> {
    let endpoint = crate::resolve_remote_addr("ws://127.0.0.1:4500")?;
    for target in [
        AppServerTarget::LocalDaemon {
            endpoint: endpoint.clone(),
        },
        AppServerTarget::Remote { endpoint },
    ] {
        let (mut app, _codex_home) = make_history_test_app().await?;
        let thread_id =
            create_history_rollout(&app.config, ThreadHistoryMode::Legacy, "owned elsewhere")?;
        let (mut server, requests, proxy) = start_recording_app_server(
            &app.config,
            /*blocked_thread_list*/ None,
            /*failed_thread_name*/ None,
        )
        .await?;
        app.app_server_target = target;
        app.active_thread_id = Some(thread_id);
        app.primary_thread_id = Some(thread_id);
        app.ensure_thread_channel(thread_id).mark_external_writer();
        app.chat_widget.show_external_writer_thread();
        app.chat_widget.insert_str("Retained draft");
        let (tx, mut events) = tokio::sync::mpsc::unbounded_channel();
        app.app_event_tx = AppEventSender::new(tx);
        let mut tui = crate::tui::test_support::make_test_tui()?;
        for key in [
            KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('Q'), KeyModifiers::SHIFT),
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        ] {
            app.handle_key_event(&mut tui, &mut server, key).await;
            assert_matches!(events.try_recv(), Ok(AppEvent::Exit(ExitMode::Immediate)));
        }
        app.handle_key_event(
            &mut tui,
            &mut server,
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
        )
        .await;
        assert!(app.chat_widget.has_active_view());
        loop {
            let event = tokio::time::timeout(Duration::from_secs(/*secs*/ 5), events.recv())
                .await?
                .expect("command center refresh");
            let refreshed = matches!(&event, AppEvent::AgentsOverviewThreadsLoaded { .. });
            Box::pin(app.handle_event(&mut tui, &mut server, event)).await?;
            if refreshed {
                break;
            }
        }
        assert_matches!(
            Box::pin(app.handle_event(
                &mut tui,
                &mut server,
                AppEvent::SelectAgentsOverviewThread { thread_id },
            ))
            .await?,
            AppRunControl::Continue
        );
        assert!(!app.chat_widget.has_active_view());
        assert_eq!(app.active_thread_id, Some(thread_id));
        assert!(app.chat_widget.is_external_writer_view());
        assert_eq!(
            app.chat_widget.composer_text_with_pending(),
            "Retained draft"
        );
        assert_eq!(
            app.thread_event_channels[&thread_id].attachment(),
            ThreadEventAttachment::ExternalWriter
        );
        assert!(requests.lock().unwrap().iter().all(|request| matches!(
            request.method.as_str(),
            "thread/read" | "thread/list" | "thread/loaded/list" | "thread/turns/list"
        )));
        server.shutdown().await?;
        proxy.await??;
    }
    Ok(())
}

#[tokio::test]
async fn command_center_read_only_open_requests_and_failure_preservation() -> Result<()> {
    for history_capabilities in [
        HistoryCapabilities::Current,
        HistoryCapabilities::ReadAfterResumeFails,
    ] {
        let (mut app, _codex_home) = Box::pin(make_history_test_app()).await?;
        std::fs::write(
            app.config.codex_home.join("config.toml"),
            "[tui]\nresume_cwd = \"current\"\n",
        )?;
        for cwd in [test_path_buf("/"), app.config.cwd.to_path_buf()] {
            crate::legacy_core::config::set_project_trust_level(
                app.config.codex_home.as_path(),
                &cwd,
                codex_protocol::config_types::TrustLevel::Trusted,
            )
            .map_err(std::io::Error::other)?;
        }
        let thread_id =
            create_history_rollout(&app.config, ThreadHistoryMode::Legacy, "Locked task")?;
        let mut owner = Box::pin(crate::start_embedded_app_server_for_picker(&app.config)).await?;
        Box::pin(owner.resume_thread(
            &app.local_settings,
            app.config.clone(),
            thread_id,
            crate::app_server_session::ResumeModelSettings::PreserveExistingThread,
        ))
        .await?;
        let (mut server, requests, proxy) = start_recording_app_server_with_history(
            &app.config,
            history_capabilities,
            /*blocked_thread_list*/ None,
            /*failed_thread_name*/ None,
            crate::app_server_session::ThreadParamsMode::Embedded,
            LoaderOverrides::default(),
        )
        .await?;
        app.app_server_target = AppServerTarget::LocalDaemon {
            endpoint: crate::resolve_remote_addr("ws://127.0.0.1:4500")?,
        };
        let current = Box::pin(server.start_thread(&app.config)).await?;
        let current_id = current.session.thread_id;
        app.enqueue_primary_thread_session(current.session, current.turns)
            .await?;
        let thread = owner
            .thread_read(thread_id, /*include_turns*/ false)
            .await?;
        let mut view = app.agents_overview_view(vec![thread], Some(thread_id));
        view.handle_paste("Keep this draft".into());
        app.chat_widget.show_bottom_pane_view(Box::new(view));
        let before = render_bottom_popup(&app.chat_widget, /*width*/ 96);
        requests.lock().unwrap().clear();
        let mut tui = crate::tui::test_support::make_test_tui()?;

        Box::pin(app.select_agents_overview_thread(&mut tui, &mut server, thread_id)).await?;
        assert_eq!(recorded_params(&requests, "thread/resume").len(), 1);
        assert!(recorded_params(&requests, "turn/start").is_empty());
        if history_capabilities == HistoryCapabilities::ReadAfterResumeFails {
            let error = render_bottom_popup(&app.chat_widget, /*width*/ 96);
            assert!(
                error.contains("Failed to view task open elsewhere"),
                "{error}"
            );
            assert_eq!(app.current_displayed_thread_id(), Some(current_id));
            assert!(recorded_params(&requests, "thread/unsubscribe").is_empty());
            app.chat_widget.handle_key_event(KeyCode::Esc.into());
            assert_eq!(render_bottom_popup(&app.chat_widget, /*width*/ 96), before);
        } else {
            assert_eq!(app.current_displayed_thread_id(), Some(thread_id));
            assert!(app.chat_widget.is_external_writer_view());
            assert_eq!(
                app.thread_event_channels[&thread_id].attachment(),
                ThreadEventAttachment::ExternalWriter
            );
        }
        owner.shutdown().await?;
        server.shutdown().await?;
        proxy.await??;
    }
    Ok(())
}
