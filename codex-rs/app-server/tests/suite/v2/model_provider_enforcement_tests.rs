//! Verifies provider requirements at app-server input and queue admission boundaries.

use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use codex_app_server::in_process;
use codex_app_server::in_process::InProcessServerEvent;
use codex_app_server::in_process::InProcessStartArgs;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadQueueAddParams;
use codex_app_server_protocol::ThreadQueueAddResponse;
use codex_app_server_protocol::ThreadQueueListParams;
use codex_app_server_protocol::ThreadQueueListResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput;
use codex_arg0::Arg0DispatchPaths;
use codex_config::CloudConfigBundleLoader;
use codex_config::LoaderOverrides;
use codex_config::NoopThreadConfigLoader;
use codex_core::config::ConfigBuilder;
use codex_exec_server::EnvironmentManager;
use codex_feedback::CodexFeedback;
use codex_protocol::protocol::SessionSource;
use core_test_support::responses;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use test_case::test_case;
use tokio::sync::oneshot;
use tokio::time::timeout;
use wiremock::MockServer;

#[test_case("other"; "selection_changes")]
#[test_case("gateway"; "definition_changes")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn provider_requirement_changes_reject_inputs_to_existing_threads(
    required_selection: &str,
) -> Result<()> {
    let (release_response, response_gate) = oneshot::channel();
    let (gateway, _) = start_streaming_sse_server(vec![vec![
        StreamingSseChunk {
            gate: None,
            body: responses::sse(vec![responses::ev_response_created("response-1")]),
        },
        StreamingSseChunk {
            gate: Some(response_gate),
            body: responses::sse(vec![responses::ev_completed("response-1")]),
        },
    ]])
    .await;
    let other = MockServer::start().await;
    let home = TempDir::new()?;
    MockResponsesConfig::new(gateway.uri())
        .enable_feature(codex_features::Feature::Goals)
        .write(home.path())?;
    let requirements = format!(
        r#"
model_provider = "gateway"
[model_providers.gateway]
name = "Gateway"
base_url = "{}/v1"
[model_providers.other]
name = "Other"
base_url = "{}/v1"
"#,
        gateway.uri(),
        other.uri(),
    );
    let requirements_path = home.path().join("requirements.toml");
    std::fs::write(&requirements_path, &requirements)?;
    let mut server = TestAppServer::builder()
        .with_codex_home(home.path())
        .with_json_logging("warn")
        .build_initialized()
        .await?;
    let started = server.start_thread(ThreadStartParams::default()).await?;
    let input = vec![UserInput::Text {
        text: "Continue.".to_string(),
        text_elements: Vec::new(),
    }];
    let id = server
        .send_turn_start_request(TurnStartParams {
            thread_id: started.thread.id.clone(),
            input: input.clone(),
            ..Default::default()
        })
        .await?;
    let wait = Duration::from_secs(/*secs*/ 60);
    let active: TurnStartResponse = timeout(wait, server.read_response(id)).await??;
    timeout(wait, gateway.wait_for_request_count(/*count*/ 1)).await?;
    let queued: ThreadQueueAddResponse = server
        .request(|request_id| ClientRequest::ThreadQueueAdd {
            request_id,
            params: ThreadQueueAddParams {
                thread_id: started.thread.id.clone(),
                input: input.clone(),
                client_user_message_id: "queued-message".into(),
            },
        })
        .await?;

    let id = server
        .send_raw_request(
            "thread/goal/set",
            Some(json!({
                "threadId": started.thread.id, "objective": "Original goal"
            })),
        )
        .await?;
    let _: codex_app_server_protocol::ThreadGoalSetResponse = server.read_response(id).await?;

    let updated = if required_selection == "other" {
        requirements.replace("model_provider = \"gateway\"", "model_provider = \"other\"")
    } else {
        requirements.replace(gateway.uri(), &other.uri())
    };
    std::fs::write(requirements_path, updated)?;
    let expected_error = JSONRPCErrorError {
        code: -32600,
        message: "failed to load configuration: Your organization's required model provider settings changed. Restart Codex to apply them; this request was not sent".to_string(),
        data: None,
    };
    for (method, params) in [
        (
            "thread/goal/set",
            json!({"threadId": started.thread.id, "objective": "Blocked objective"}),
        ),
        (
            "turn/start",
            json!({ "threadId": started.thread.id, "input": input }),
        ),
        (
            "turn/steer",
            json!({ "threadId": started.thread.id, "input": input, "expectedTurnId": active.turn.id }),
        ),
        (
            "review/start",
            json!({ "threadId": started.thread.id, "target": { "type": "custom", "instructions": "Review this change." }, "delivery": "inline" }),
        ),
        (
            "review/start",
            json!({ "threadId": started.thread.id, "target": { "type": "custom", "instructions": "Review this change." }, "delivery": "detached" }),
        ),
    ] {
        let id = server.send_raw_request(method, Some(params)).await?;
        let error = server
            .read_stream_until_error_message(RequestId::Integer(id))
            .await?;
        assert_eq!(error.error, expected_error);
    }

    let id = server
        .send_raw_request(
            "thread/goal/get",
            Some(json!({"threadId": started.thread.id})),
        )
        .await?;
    let goal: codex_app_server_protocol::ThreadGoalGetResponse = server.read_response(id).await?;
    assert_eq!(goal.goal.expect("existing goal").objective, "Original goal");
    let id = server
        .send_raw_request(
            "thread/goal/set",
            Some(json!({"threadId": started.thread.id, "status": "paused"})),
        )
        .await?;
    let _: codex_app_server_protocol::ThreadGoalSetResponse = server.read_response(id).await?;
    let id = server
        .send_raw_request(
            "thread/goal/clear",
            Some(json!({"threadId": started.thread.id})),
        )
        .await?;
    let _: codex_app_server_protocol::ThreadGoalClearResponse = server.read_response(id).await?;
    let id = server
        .send_raw_request(
            "thread/queue/start",
            Some(json!({
                "threadId": started.thread.id,
                "queuedSubmissionId": queued.queued_submission.id,
            })),
        )
        .await?;
    let error = server
        .read_stream_until_error_message(RequestId::Integer(id))
        .await?;
    assert_eq!(error.error, expected_error);
    let queue: ThreadQueueListResponse = server
        .request(|request_id| ClientRequest::ThreadQueueList {
            request_id,
            params: ThreadQueueListParams {
                thread_id: started.thread.id.clone(),
                cursor: None,
                limit: None,
            },
        })
        .await?;
    assert_eq!(
        queue,
        ThreadQueueListResponse {
            data: vec![queued.queued_submission.clone()],
            next_cursor: None,
        }
    );
    let id = server
        .send_raw_request(
            "thread/queue/delete",
            Some(json!({
                "threadId": started.thread.id, "queuedSubmissionId": queued.queued_submission.id
            })),
        )
        .await?;
    let _: codex_app_server_protocol::ThreadQueueDeleteResponse = server.read_response(id).await?;
    release_response.send(()).expect("release active response");
    timeout(
        wait,
        server.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    for (method, params) in [
        (
            "thread/compact/start",
            json!({"threadId": started.thread.id}),
        ),
        (
            "thread/goal/set",
            json!({"threadId": started.thread.id, "objective": "Blocked new goal"}),
        ),
    ] {
        let id = server.send_raw_request(method, Some(params)).await?;
        let error = server
            .read_stream_until_error_message(RequestId::Integer(id))
            .await?;
        assert_eq!(error.error, expected_error);
    }
    assert_eq!(gateway.requests().await.len(), 1);
    assert!(
        other
            .received_requests()
            .await
            .expect("recorded requests")
            .is_empty()
    );
    gateway.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_config_changes_do_not_block_existing_threads() -> Result<()> {
    let provider = MockServer::start().await;
    let home = TempDir::new()?;
    MockResponsesConfig::new(&provider.uri()).write(home.path())?;
    let mut server = TestAppServer::builder()
        .with_codex_home(home.path())
        .build_initialized()
        .await?;
    let started = server.start_thread(ThreadStartParams::default()).await?;
    let requests = responses::mount_sse_sequence(
        &provider,
        vec![
            responses::sse(vec![responses::ev_completed("first")]),
            responses::sse(vec![responses::ev_completed("second")]),
        ],
    )
    .await;
    for local_settings in ["model_provider = 'openai'", "invalid toml !!!"] {
        std::fs::write(home.path().join("config.toml"), local_settings)?;
        let id = server
            .send_turn_start_request(TurnStartParams {
                thread_id: started.thread.id.clone(),
                input: vec![UserInput::Text {
                    text: "Continue on the original provider.".into(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            })
            .await?;
        let _: TurnStartResponse = server.read_response(id).await?;
        server
            .read_stream_until_notification_message("turn/completed")
            .await?;
    }
    assert_eq!(requests.requests().len(), 2);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn malformed_system_defaults_do_not_block_existing_thread_turn() -> Result<()> {
    let provider = MockServer::start().await;
    let home = TempDir::new()?;
    MockResponsesConfig::new(&provider.uri()).write(home.path())?;
    let system_config_path = home.path().join("system-config.toml");
    let mut overrides = LoaderOverrides::without_managed_config_for_tests();
    overrides.system_config_path = Some(system_config_path.clone());
    let config = Arc::new(
        ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .fallback_cwd(Some(home.path().to_path_buf()))
            .loader_overrides(overrides.clone())
            .build()
            .await?,
    );
    let mut client = in_process::start(InProcessStartArgs {
        arg0_paths: Arg0DispatchPaths::default(),
        config,
        cli_overrides: Vec::new(),
        loader_overrides: overrides,
        strict_config: false,
        cloud_config_bundle: CloudConfigBundleLoader::default(),
        thread_config_loader: Arc::new(NoopThreadConfigLoader),
        feedback: CodexFeedback::new(),
        log_db: None,
        state_db: None,
        environment_manager: Arc::new(EnvironmentManager::default_for_tests()),
        config_warnings: Vec::new(),
        session_source: SessionSource::Cli,
        enable_codex_api_key_env: false,
        initialize: InitializeParams {
            client_info: ClientInfo {
                name: "codex-app-server-tests".to_string(),
                title: None,
                version: "0.1.0".to_string(),
            },
            capabilities: None,
        },
        channel_capacity: in_process::DEFAULT_IN_PROCESS_CHANNEL_CAPACITY,
    })
    .await?;
    let response = client
        .request(ClientRequest::ThreadStart {
            request_id: RequestId::Integer(1),
            params: ThreadStartParams::default(),
        })
        .await?
        .expect("thread/start should succeed");
    let started: ThreadStartResponse = serde_json::from_value(response)?;

    let requests = responses::mount_sse_once(
        &provider,
        responses::sse(vec![responses::ev_completed("response-1")]),
    )
    .await;
    std::fs::write(system_config_path, "invalid toml !!!")?;
    let response = client
        .request(ClientRequest::TurnStart {
            request_id: RequestId::Integer(2),
            params: TurnStartParams {
                thread_id: started.thread.id.clone(),
                input: vec![UserInput::Text {
                    text: "Continue on the original provider.".to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            },
        })
        .await?
        .expect("turn/start should succeed");
    let _: TurnStartResponse = serde_json::from_value(response)?;
    timeout(Duration::from_secs(/*secs*/ 60), async {
        loop {
            let Some(event) = client.next_event().await else {
                anyhow::bail!("app-server stopped before turn/completed");
            };
            if let InProcessServerEvent::ServerNotification(notification) = event
                && let ServerNotification::TurnCompleted(completed) = notification.as_ref()
                && completed.thread_id == started.thread.id
            {
                return Ok::<(), anyhow::Error>(());
            }
        }
    })
    .await??;
    assert_eq!(requests.requests().len(), 1);
    client.shutdown().await?;
    Ok(())
}
