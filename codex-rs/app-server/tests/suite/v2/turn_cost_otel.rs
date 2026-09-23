//! Exercises the best-effort SiWC cost happy path through real app-server startup and OTLP.
//! The public in-process transport permits advancing the production timer without test-only hooks.

use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::write_chatgpt_auth;
use app_test_support::write_models_cache;
use codex_app_server::in_process;
use codex_app_server::in_process::InProcessServerEvent;
use codex_app_server::in_process::InProcessStartArgs;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnEnvironmentParams;
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
use codex_login::AuthCredentialsStoreMode;
use codex_protocol::protocol::SessionSource;
use core_test_support::responses;
use core_test_support::test_codex::test_env;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

const COST_PATH: &str = "/api/codex/usage/thread-estimates/query";

#[tokio::test]
async fn chatgpt_turn_cost_reaches_otlp_on_success() -> Result<()> {
    let server = MockServer::start().await;
    let collector = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/metrics"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&collector)
        .await;
    Mock::given(method("POST")).and(path(COST_PATH))
        .and(header("authorization", "Bearer siwc-token"))
        .and(header("chatgpt-account-id", "workspace-a"))
        .respond_with(move |request: &wiremock::Request| {
            let body: Value = serde_json::from_slice(&request.body).expect("cost request");
            assert_eq!(body["include_settled_response_ids"], true);
            let threads: Vec<_> = body["threads"].as_array().expect("threads").iter().map(|thread| {
                let turns: Vec<_> = thread["turn_ids"].as_array().expect("turns").iter().map(|turn_id| json!({
                    "turn_id": turn_id, "model": "mock-model", "estimated_usage_usd_micros": 1250001,
                    "settled_response_ids": ["resp-cost"]
                })).collect();
                json!({"thread_id": thread["thread_id"], "turns": turns})
            }).collect();
            ResponseTemplate::new(200).set_body_json(json!({"threads": threads}))
        }).mount(&server).await;
    let model = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("resp-cost"),
            responses::ev_assistant_message("msg-cost", "Done"),
            responses::ev_completed("resp-cost"),
        ]),
    )
    .await;
    let home = TempDir::new()?;
    std::fs::write(
        home.path().join("config.toml"),
        format!(
            r#"
model = "mock-model"
model_provider = "openai"
openai_base_url = "{}/v1"
chatgpt_base_url = "{}"
approval_policy = "never"
[features]
responses_websockets = false
responses_websockets_v2 = false
runtime_metrics = true
[analytics]
enabled = true
[otel]
exporter = "none"
metrics_exporter = {{ otlp-http = {{ endpoint = "{}/metrics", protocol = "json" }} }}
"#,
            server.uri(),
            server.uri(),
            collector.uri()
        ),
    )?;
    write_chatgpt_auth(
        home.path(),
        ChatGptAuthFixture::new("siwc-token")
            .account_id("workspace-a")
            .chatgpt_account_id("workspace-a")
            .chatgpt_user_id("user-a"),
        AuthCredentialsStoreMode::File,
    )?;
    write_models_cache(home.path()).await?;
    let loader_overrides = LoaderOverrides::without_managed_config_for_tests();
    let config = Arc::new(
        ConfigBuilder::default()
            .codex_home(home.path().to_path_buf())
            .loader_overrides(loader_overrides.clone())
            .build()
            .await?,
    );
    let provider = codex_core::otel_init::build_provider(
        &config,
        "test",
        Some("codex-app-server"),
        /*default_analytics_enabled*/ false,
    )
    .map_err(|error| anyhow::anyhow!(error.to_string()))?
    .expect("OTLP enabled");
    let metrics = provider.metrics.clone().expect("metrics exporter");
    let env = test_env().await?;
    let environment_manager = if let Some(url) = env.exec_server_url() {
        EnvironmentManager::create_for_tests(
            Some(url.to_string()),
            /*local_runtime_paths*/ None,
        )
        .await
    } else {
        EnvironmentManager::default_for_tests()
    };
    let mut client = in_process::start(InProcessStartArgs {
        arg0_paths: Arg0DispatchPaths::default(),
        config,
        cli_overrides: Vec::new(),
        loader_overrides,
        strict_config: false,
        cloud_config_bundle: CloudConfigBundleLoader::default(),
        embedded_network_policy: Default::default(),
        thread_config_loader: Arc::new(NoopThreadConfigLoader),
        feedback: CodexFeedback::new(),
        log_db: None,
        state_db: None,
        environment_manager: Arc::new(environment_manager),
        config_warnings: Vec::new(),
        session_source: SessionSource::Cli,
        enable_codex_api_key_env: false,
        initialize: InitializeParams {
            client_info: ClientInfo {
                name: "codex-app-server-tests".to_string(),
                title: None,
                version: "test".to_string(),
            },
            capabilities: Some(InitializeCapabilities {
                experimental_api: true,
                ..Default::default()
            }),
        },
        channel_capacity: in_process::DEFAULT_IN_PROCESS_CHANNEL_CAPACITY,
    })
    .await?;
    wait_for_cost_queries(&server, /*count*/ 1).await;
    let response = client
        .request(ClientRequest::ThreadStart {
            request_id: RequestId::Integer(1),
            params: ThreadStartParams {
                environments: Some(vec![TurnEnvironmentParams {
                    environment_id: env.selection().environment_id.clone(),
                    cwd: env.selection().cwd.clone().into(),
                    runtime_workspace_roots: None,
                }]),
                ..Default::default()
            },
        })
        .await?
        .expect("thread/start");
    let ThreadStartResponse { thread, .. } = serde_json::from_value(response)?;
    let response = client
        .request(ClientRequest::TurnStart {
            request_id: RequestId::Integer(2),
            params: TurnStartParams {
                thread_id: thread.id.clone(),
                input: vec![UserInput::Text {
                    text: "Hello".to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            },
        })
        .await?
        .expect("turn/start");
    let TurnStartResponse { turn } = serde_json::from_value(response)?;
    timeout(Duration::from_secs(/*secs*/ 30), async {
        while let Some(event) = client.next_event().await {
            if let InProcessServerEvent::ServerNotification(event) = event
                && let ServerNotification::TurnCompleted(event) = *event
            {
                assert!(
                    event.turn.error.is_none(),
                    "turn failed: {:?}",
                    event.turn.error
                );
                break;
            }
        }
    })
    .await?;
    assert_eq!(
        model.single_request().header("authorization").as_deref(),
        Some("Bearer siwc-token")
    );
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(/*secs*/ 301)).await;
    tokio::time::resume();
    wait_for_cost_queries(&server, /*count*/ 2).await;
    // Synchronize with actual recording, not just the backend receiving a request.
    timeout(Duration::from_secs(/*secs*/ 10), async {
        loop {
            if metrics
                .snapshot()
                .expect("snapshot")
                .scope_metrics()
                .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
                .any(|metric| metric.name() == "codex.turn.cost_microusd")
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await?;
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(/*secs*/ 301)).await;
    tokio::time::resume();
    client.shutdown().await?;
    tokio::task::spawn_blocking(move || provider.shutdown()).await?;
    let mut points = Vec::new();
    for request in collector.received_requests().await.expect("OTLP requests") {
        let body: Value = serde_json::from_slice(&request.body)?;
        for resource in body["resourceMetrics"]
            .as_array()
            .expect("resource metrics")
        {
            for scope in resource["scopeMetrics"].as_array().expect("scope metrics") {
                for metric in scope["metrics"].as_array().expect("metrics") {
                    if metric["name"] == "codex.turn.cost_microusd" {
                        points.extend(
                            metric["sum"]["dataPoints"]
                                .as_array()
                                .expect("cost points")
                                .clone(),
                        );
                    }
                }
            }
        }
    }
    assert_eq!(points.len(), 1);
    assert_eq!(points[0]["asInt"], json!(1250001));
    let attrs: std::collections::BTreeMap<_, _> = points[0]["attributes"]
        .as_array()
        .expect("attributes")
        .iter()
        .map(|attr| {
            (
                attr["key"].as_str().expect("key"),
                attr["value"]["stringValue"].as_str().expect("value"),
            )
        })
        .collect();
    assert_eq!(attrs["turn.id"], turn.id);
    assert_eq!(attrs["conversation.id"], thread.id);
    assert_eq!(attrs["auth_mode"], "Chatgpt");
    let requests = server
        .received_requests()
        .await
        .expect("settlement requests");
    let cost_requests: Vec<_> = requests
        .iter()
        .filter(|request| request.url.path() == COST_PATH)
        .collect();
    assert_eq!(cost_requests.len(), 2);
    assert_eq!(
        serde_json::from_slice::<Value>(&cost_requests[1].body)?,
        json!({
            "threads": [{"thread_id": thread.id, "turn_ids": [turn.id]}], "include_settled_response_ids": true
        })
    );
    server.verify().await;
    Ok(())
}

async fn wait_for_cost_queries(server: &MockServer, count: usize) {
    timeout(Duration::from_secs(/*secs*/ 15), async {
        loop {
            if server
                .received_requests()
                .await
                .expect("requests")
                .iter()
                .filter(|request| request.url.path() == COST_PATH)
                .count()
                >= count
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("cost query");
}
