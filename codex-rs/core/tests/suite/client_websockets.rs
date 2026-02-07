#![allow(clippy::expect_used, clippy::unwrap_used)]
use codex_core::AuthManager;
use codex_core::CodexAuth;
use codex_core::ContentItem;
use codex_core::ModelClient;
use codex_core::ModelClientSession;
use codex_core::ModelProviderInfo;
use codex_core::Prompt;
use codex_core::ResponseEvent;
use codex_core::ResponseItem;
use codex_core::WireApi;
use codex_core::X_RESPONSESAPI_INCLUDE_TIMING_METRICS_HEADER;
use codex_core::features::Feature;
use codex_core::models_manager::manager::ModelsManager;
use codex_core::protocol::SessionSource;
use codex_core::windows_sandbox::WindowsSandboxLevelExt;
use codex_otel::OtelManager;
use codex_otel::TelemetryAuthMode;
use codex_otel::metrics::MetricsClient;
use codex_otel::metrics::MetricsConfig;
use codex_protocol::ThreadId;
use codex_protocol::account::PlanType;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::protocol::SandboxPolicy;
use core_test_support::load_default_config_for_test;
use core_test_support::responses::WebSocketConnectionConfig;
use core_test_support::responses::WebSocketTestServer;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::start_websocket_server;
use core_test_support::responses::start_websocket_server_with_headers;
use core_test_support::skip_if_no_network;
use futures::StreamExt;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tracing_test::traced_test;

const MODEL: &str = "gpt-5.2-codex";
const OPENAI_BETA_HEADER: &str = "OpenAI-Beta";
const OPENAI_BETA_RESPONSES_WEBSOCKETS: &str = "responses_websockets=2026-02-04";
const WS_V2_BETA_HEADER_VALUE: &str = "responses_websockets=2026-02-06";

struct WebsocketTestHarness {
    _codex_home: TempDir,
    client: ModelClient,
    model_info: ModelInfo,
    effort: Option<ReasoningEffortConfig>,
    summary: ReasoningSummary,
    sandbox_policy: SandboxPolicy,
    windows_sandbox_level: WindowsSandboxLevel,
    otel_manager: OtelManager,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_websocket_streams_request() {
    skip_if_no_network!();

    let server = start_websocket_server(vec![vec![vec![
        ev_response_created("resp-1"),
        ev_completed("resp-1"),
    ]]])
    .await;

    let harness = websocket_harness(&server).await;
    let mut client_session = harness.client.new_session();
    let prompt = prompt_with_input(vec![message_item("hello")]);

    stream_until_complete(&mut client_session, &harness, &prompt).await;

    let connection = server.single_connection();
    assert_eq!(connection.len(), 1);
    let body = connection.first().expect("missing request").body_json();

    assert_eq!(body["type"].as_str(), Some("response.create"));
    assert_eq!(body["model"].as_str(), Some(MODEL));
    assert_eq!(body["stream"], serde_json::Value::Bool(true));
    assert_eq!(body["input"].as_array().map(Vec::len), Some(1));
    let handshake = server.single_handshake();
    assert_eq!(
        handshake.header(OPENAI_BETA_HEADER),
        Some(OPENAI_BETA_RESPONSES_WEBSOCKETS.to_string())
    );
    assert!(
        handshake.header("x-codex-sandbox").is_some(),
        "missing x-codex-sandbox header"
    );

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_websocket_preconnect_reuses_connection() {
    skip_if_no_network!();

    let server = start_websocket_server(vec![vec![vec![
        ev_response_created("resp-1"),
        ev_completed("resp-1"),
    ]]])
    .await;

    let harness = websocket_harness(&server).await;
    assert!(harness.client.preconnect(&harness.otel_manager, None).await);

    let mut client_session = harness.client.new_session();
    let prompt = prompt_with_input(vec![message_item("hello")]);
    stream_until_complete(&mut client_session, &harness, &prompt).await;

    assert_eq!(server.handshakes().len(), 1);
    assert_eq!(server.single_connection().len(), 1);

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_websocket_preconnect_is_reused_even_with_header_changes() {
    skip_if_no_network!();

    let server = start_websocket_server(vec![vec![vec![
        ev_response_created("resp-1"),
        ev_completed("resp-1"),
    ]]])
    .await;

    let harness = websocket_harness(&server).await;
    assert!(harness.client.preconnect(&harness.otel_manager, None).await);

    let mut client_session = harness.client.new_session();
    let prompt = prompt_with_input(vec![message_item("hello")]);
    let mut stream = client_session
        .stream(
            &prompt,
            &harness.model_info,
            &harness.otel_manager,
            harness.effort,
            harness.summary,
            None,
            &harness.sandbox_policy,
            harness.windows_sandbox_level,
        )
        .await
        .expect("websocket stream failed");

    while let Some(event) = stream.next().await {
        if matches!(event, Ok(ResponseEvent::Completed { .. })) {
            break;
        }
    }

    assert_eq!(server.handshakes().len(), 1);
    assert_eq!(server.single_connection().len(), 1);

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[traced_test]
async fn responses_websocket_emits_websocket_telemetry_events() {
    skip_if_no_network!();

    let server = start_websocket_server(vec![vec![vec![
        ev_response_created("resp-1"),
        ev_completed("resp-1"),
    ]]])
    .await;

    let harness = websocket_harness(&server).await;
    harness.otel_manager.reset_runtime_metrics();
    let mut client_session = harness.client.new_session();
    let prompt = prompt_with_input(vec![message_item("hello")]);

    stream_until_complete(&mut client_session, &harness, &prompt).await;

    tokio::time::sleep(Duration::from_millis(10)).await;

    let summary = harness
        .otel_manager
        .runtime_metrics_summary()
        .expect("runtime metrics summary");
    assert_eq!(summary.api_calls.count, 0);
    assert_eq!(summary.streaming_events.count, 0);
    assert_eq!(summary.websocket_calls.count, 1);
    assert_eq!(summary.websocket_events.count, 2);

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_websocket_includes_timing_metrics_header_when_runtime_metrics_enabled() {
    skip_if_no_network!();

    let server = start_websocket_server(vec![vec![vec![
        ev_response_created("resp-1"),
        serde_json::json!({
            "type": "responsesapi.websocket_timing",
            "timing_metrics": {
                "responses_duration_excl_engine_and_client_tool_time_ms": 120,
                "engine_service_total_ms": 450,
                "engine_iapi_ttft_total_ms": 310,
                "engine_service_ttft_total_ms": 340,
                "engine_iapi_tbt_across_engine_calls_ms": 220,
                "engine_service_tbt_across_engine_calls_ms": 260
            }
        }),
        ev_completed("resp-1"),
    ]]])
    .await;

    let harness = websocket_harness_with_runtime_metrics(&server, true).await;
    harness.otel_manager.reset_runtime_metrics();
    let mut client_session = harness.client.new_session();
    let prompt = prompt_with_input(vec![message_item("hello")]);

    stream_until_complete(&mut client_session, &harness, &prompt).await;
    tokio::time::sleep(Duration::from_millis(10)).await;

    let handshake = server.single_handshake();
    assert_eq!(
        handshake.header(X_RESPONSESAPI_INCLUDE_TIMING_METRICS_HEADER),
        Some("true".to_string())
    );

    let summary = harness
        .otel_manager
        .runtime_metrics_summary()
        .expect("runtime metrics summary");
    assert_eq!(summary.responses_api_overhead_ms, 120);
    assert_eq!(summary.responses_api_inference_time_ms, 450);
    assert_eq!(summary.responses_api_engine_iapi_ttft_ms, 310);
    assert_eq!(summary.responses_api_engine_service_ttft_ms, 340);
    assert_eq!(summary.responses_api_engine_iapi_tbt_ms, 220);
    assert_eq!(summary.responses_api_engine_service_tbt_ms, 260);

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_websocket_omits_timing_metrics_header_when_runtime_metrics_disabled() {
    skip_if_no_network!();

    let server = start_websocket_server(vec![vec![vec![
        ev_response_created("resp-1"),
        ev_completed("resp-1"),
    ]]])
    .await;

    let harness = websocket_harness_with_runtime_metrics(&server, false).await;
    let mut client_session = harness.client.new_session();
    let prompt = prompt_with_input(vec![message_item("hello")]);

    stream_until_complete(&mut client_session, &harness, &prompt).await;

    let handshake = server.single_handshake();
    assert_eq!(
        handshake.header(X_RESPONSESAPI_INCLUDE_TIMING_METRICS_HEADER),
        None
    );

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_websocket_emits_reasoning_included_event() {
    skip_if_no_network!();

    let server = start_websocket_server_with_headers(vec![WebSocketConnectionConfig {
        requests: vec![vec![ev_response_created("resp-1"), ev_completed("resp-1")]],
        response_headers: vec![("X-Reasoning-Included".to_string(), "true".to_string())],
        accept_delay: None,
    }])
    .await;

    let harness = websocket_harness(&server).await;
    let mut client_session = harness.client.new_session();
    let prompt = prompt_with_input(vec![message_item("hello")]);

    let mut stream = client_session
        .stream(
            &prompt,
            &harness.model_info,
            &harness.otel_manager,
            harness.effort,
            harness.summary,
            None,
            &harness.sandbox_policy,
            harness.windows_sandbox_level,
        )
        .await
        .expect("websocket stream failed");

    let mut saw_reasoning_included = false;
    while let Some(event) = stream.next().await {
        match event.expect("event") {
            ResponseEvent::ServerReasoningIncluded(true) => {
                saw_reasoning_included = true;
            }
            ResponseEvent::Completed { .. } => break,
            _ => {}
        }
    }

    assert!(saw_reasoning_included);
    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_websocket_emits_rate_limit_events() {
    skip_if_no_network!();

    let rate_limit_event = json!({
        "type": "codex.rate_limits",
        "plan_type": "plus",
        "rate_limits": {
            "allowed": true,
            "limit_reached": false,
            "primary": {
                "used_percent": 42,
                "window_minutes": 60,
                "reset_at": 1700000000
            },
            "secondary": null
        },
        "code_review_rate_limits": null,
        "credits": {
            "has_credits": true,
            "unlimited": false,
            "balance": "123"
        },
        "promo": null
    });

    let server = start_websocket_server_with_headers(vec![WebSocketConnectionConfig {
        requests: vec![vec![
            rate_limit_event,
            ev_response_created("resp-1"),
            ev_completed("resp-1"),
        ]],
        response_headers: vec![
            ("X-Models-Etag".to_string(), "etag-123".to_string()),
            ("X-Reasoning-Included".to_string(), "true".to_string()),
        ],
        accept_delay: None,
    }])
    .await;

    let harness = websocket_harness(&server).await;
    let mut client_session = harness.client.new_session();
    let prompt = prompt_with_input(vec![message_item("hello")]);

    let mut stream = client_session
        .stream(
            &prompt,
            &harness.model_info,
            &harness.otel_manager,
            harness.effort,
            harness.summary,
            None,
            &harness.sandbox_policy,
            harness.windows_sandbox_level,
        )
        .await
        .expect("websocket stream failed");

    let mut saw_rate_limits = None;
    let mut saw_models_etag = None;
    let mut saw_reasoning_included = false;

    while let Some(event) = stream.next().await {
        match event.expect("event") {
            ResponseEvent::RateLimits(snapshot) => {
                saw_rate_limits = Some(snapshot);
            }
            ResponseEvent::ModelsEtag(etag) => {
                saw_models_etag = Some(etag);
            }
            ResponseEvent::ServerReasoningIncluded(true) => {
                saw_reasoning_included = true;
            }
            ResponseEvent::Completed { .. } => break,
            _ => {}
        }
    }

    let rate_limits = saw_rate_limits.expect("missing rate limits");
    let primary = rate_limits.primary.expect("missing primary window");
    assert_eq!(primary.used_percent, 42.0);
    assert_eq!(primary.window_minutes, Some(60));
    assert_eq!(primary.resets_at, Some(1_700_000_000));
    assert_eq!(rate_limits.plan_type, Some(PlanType::Plus));
    let credits = rate_limits.credits.expect("missing credits");
    assert!(credits.has_credits);
    assert!(!credits.unlimited);
    assert_eq!(credits.balance.as_deref(), Some("123"));
    assert_eq!(saw_models_etag.as_deref(), Some("etag-123"));
    assert!(saw_reasoning_included);

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_websocket_appends_on_prefix() {
    skip_if_no_network!();

    let server = start_websocket_server(vec![vec![
        vec![ev_response_created("resp-1"), ev_completed("resp-1")],
        vec![ev_response_created("resp-2"), ev_completed("resp-2")],
    ]])
    .await;

    let harness = websocket_harness(&server).await;
    let mut client_session = harness.client.new_session();
    let prompt_one = prompt_with_input(vec![message_item("hello")]);
    let prompt_two = prompt_with_input(vec![message_item("hello"), message_item("second")]);

    stream_until_complete(&mut client_session, &harness, &prompt_one).await;
    stream_until_complete(&mut client_session, &harness, &prompt_two).await;

    let connection = server.single_connection();
    assert_eq!(connection.len(), 2);
    let first = connection.first().expect("missing request").body_json();
    let second = connection.get(1).expect("missing request").body_json();

    assert_eq!(first["type"].as_str(), Some("response.create"));
    assert_eq!(first["model"].as_str(), Some(MODEL));
    assert_eq!(first["stream"], serde_json::Value::Bool(true));
    assert_eq!(first["input"].as_array().map(Vec::len), Some(1));
    let expected_append = serde_json::json!({
        "type": "response.append",
        "input": serde_json::to_value(&prompt_two.input[1..]).expect("serialize append items"),
    });
    assert_eq!(second, expected_append);

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_websocket_creates_on_non_prefix() {
    skip_if_no_network!();

    let server = start_websocket_server(vec![vec![
        vec![ev_response_created("resp-1"), ev_completed("resp-1")],
        vec![ev_response_created("resp-2"), ev_completed("resp-2")],
    ]])
    .await;

    let harness = websocket_harness(&server).await;
    let mut client_session = harness.client.new_session();
    let prompt_one = prompt_with_input(vec![message_item("hello")]);
    let prompt_two = prompt_with_input(vec![message_item("different")]);

    stream_until_complete(&mut client_session, &harness, &prompt_one).await;
    stream_until_complete(&mut client_session, &harness, &prompt_two).await;

    let connection = server.single_connection();
    assert_eq!(connection.len(), 2);
    let second = connection.get(1).expect("missing request").body_json();

    assert_eq!(second["type"].as_str(), Some("response.create"));
    assert_eq!(second["model"].as_str(), Some(MODEL));
    assert_eq!(second["stream"], serde_json::Value::Bool(true));
    assert_eq!(
        second["input"],
        serde_json::to_value(&prompt_two.input).unwrap()
    );

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_websocket_v2_creates_with_previous_response_id_on_prefix() {
    skip_if_no_network!();

    let server = start_websocket_server(vec![vec![
        vec![ev_response_created("resp-1"), ev_completed("resp-1")],
        vec![ev_response_created("resp-2"), ev_completed("resp-2")],
    ]])
    .await;

    let harness = websocket_harness_with_v2(&server, true).await;
    let mut session = harness.client.new_session();
    let prompt_one = prompt_with_input(vec![message_item("hello")]);
    let prompt_two = prompt_with_input(vec![message_item("hello"), message_item("second")]);

    stream_until_complete(&mut session, &harness, &prompt_one).await;
    stream_until_complete(&mut session, &harness, &prompt_two).await;

    let connection = server.single_connection();
    assert_eq!(connection.len(), 2);
    let first = connection.first().expect("missing request").body_json();
    let second = connection.get(1).expect("missing request").body_json();

    assert_eq!(first["type"].as_str(), Some("response.create"));
    assert_eq!(second["type"].as_str(), Some("response.create"));
    assert_eq!(second["previous_response_id"].as_str(), Some("resp-1"));
    assert_eq!(
        second["input"],
        serde_json::to_value(&prompt_two.input[1..]).unwrap()
    );

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_websocket_v2_after_error_uses_full_create_without_previous_response_id() {
    skip_if_no_network!();

    let server = start_websocket_server(vec![
        vec![
            vec![ev_response_created("resp-1"), ev_completed("resp-1")],
            vec![json!({
                "type": "response.failed",
                "response": {
                    "error": {
                        "code": "invalid_prompt",
                        "message": "synthetic websocket failure"
                    }
                }
            })],
        ],
        vec![vec![ev_response_created("resp-3"), ev_completed("resp-3")]],
    ])
    .await;

    let harness = websocket_harness_with_v2(&server, true).await;
    let mut session = harness.client.new_session();
    let prompt_one = prompt_with_input(vec![message_item("hello")]);
    let prompt_two = prompt_with_input(vec![message_item("hello"), message_item("second")]);
    let prompt_three = prompt_with_input(vec![
        message_item("hello"),
        message_item("second"),
        message_item("third"),
    ]);

    stream_until_complete(&mut session, &harness, &prompt_one).await;

    let mut second_stream = session
        .stream(
            &prompt_two,
            &harness.model_info,
            &harness.otel_manager,
            harness.effort,
            harness.summary,
            None,
            &harness.sandbox_policy,
            harness.windows_sandbox_level,
        )
        .await
        .expect("websocket stream failed");
    let mut saw_error = false;
    while let Some(event) = second_stream.next().await {
        if event.is_err() {
            saw_error = true;
            break;
        }
    }
    assert!(saw_error, "expected second websocket stream to error");

    stream_until_complete(&mut session, &harness, &prompt_three).await;

    assert_eq!(server.handshakes().len(), 2);

    let connections = server.connections();
    assert_eq!(connections.len(), 2);
    let first_connection = connections.first().expect("missing first connection");
    assert_eq!(first_connection.len(), 2);

    let first = first_connection
        .first()
        .expect("missing first request")
        .body_json();
    let second = first_connection
        .get(1)
        .expect("missing second request")
        .body_json();
    let third = connections
        .get(1)
        .and_then(|connection| connection.first())
        .expect("missing third request")
        .body_json();

    assert_eq!(first["type"].as_str(), Some("response.create"));
    assert_eq!(second["type"].as_str(), Some("response.create"));
    assert_eq!(second["previous_response_id"].as_str(), Some("resp-1"));
    assert_eq!(third["type"].as_str(), Some("response.create"));
    assert_eq!(third.get("previous_response_id"), None);
    assert_eq!(
        third["input"],
        serde_json::to_value(&prompt_three.input).unwrap()
    );

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_websocket_v2_sets_openai_beta_header() {
    skip_if_no_network!();

    let server = start_websocket_server(vec![vec![vec![
        ev_response_created("resp-1"),
        ev_completed("resp-1"),
    ]]])
    .await;

    let harness = websocket_harness_with_v2(&server, true).await;
    let mut session = harness.client.new_session();
    let prompt = prompt_with_input(vec![message_item("hello")]);

    stream_until_complete(&mut session, &harness, &prompt).await;

    let handshake = server.single_handshake();
    let openai_beta_header = handshake
        .header(OPENAI_BETA_HEADER)
        .expect("missing OpenAI-Beta header");
    assert!(
        openai_beta_header
            .split(',')
            .map(str::trim)
            .any(|value| value == WS_V2_BETA_HEADER_VALUE)
    );
    assert!(
        !openai_beta_header
            .split(',')
            .map(str::trim)
            .any(|value| value == OPENAI_BETA_RESPONSES_WEBSOCKETS)
    );

    server.shutdown().await;
}

fn message_item(text: &str) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: "user".into(),
        content: vec![ContentItem::InputText { text: text.into() }],
        end_turn: None,
        phase: None,
    }
}

fn prompt_with_input(input: Vec<ResponseItem>) -> Prompt {
    let mut prompt = Prompt::default();
    prompt.input = input;
    prompt
}

fn websocket_provider(server: &WebSocketTestServer) -> ModelProviderInfo {
    ModelProviderInfo {
        name: "mock-ws".into(),
        base_url: Some(format!("{}/v1", server.uri())),
        env_key: None,
        env_key_instructions: None,
        experimental_bearer_token: None,
        wire_api: WireApi::Responses,
        query_params: None,
        http_headers: None,
        env_http_headers: None,
        request_max_retries: Some(0),
        stream_max_retries: Some(0),
        stream_idle_timeout_ms: Some(5_000),
        requires_openai_auth: false,
        supports_websockets: true,
    }
}

async fn websocket_harness(server: &WebSocketTestServer) -> WebsocketTestHarness {
    websocket_harness_with_runtime_metrics(server, false).await
}

async fn websocket_harness_with_runtime_metrics(
    server: &WebSocketTestServer,
    runtime_metrics_enabled: bool,
) -> WebsocketTestHarness {
    websocket_harness_with_options(server, runtime_metrics_enabled, false).await
}

async fn websocket_harness_with_v2(
    server: &WebSocketTestServer,
    websocket_v2_enabled: bool,
) -> WebsocketTestHarness {
    websocket_harness_with_options(server, false, websocket_v2_enabled).await
}

async fn websocket_harness_with_options(
    server: &WebSocketTestServer,
    runtime_metrics_enabled: bool,
    websocket_v2_enabled: bool,
) -> WebsocketTestHarness {
    let provider = websocket_provider(server);
    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home).await;
    config.model = Some(MODEL.to_string());
    config.features.enable(Feature::ResponsesWebsockets);
    if runtime_metrics_enabled {
        config.features.enable(Feature::RuntimeMetrics);
    }
    if websocket_v2_enabled {
        config.features.enable(Feature::ResponsesWebsocketsV2);
    }
    let config = Arc::new(config);
    let model_info = ModelsManager::construct_model_info_offline(MODEL, &config);
    let conversation_id = ThreadId::new();
    let auth_manager = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("Test API Key"));
    let exporter = InMemoryMetricExporter::default();
    let metrics = MetricsClient::new(
        MetricsConfig::in_memory("test", "codex-core", env!("CARGO_PKG_VERSION"), exporter)
            .with_runtime_reader(),
    )
    .expect("in-memory metrics client");
    let otel_manager = OtelManager::new(
        conversation_id,
        MODEL,
        model_info.slug.as_str(),
        None,
        Some("test@test.com".to_string()),
        auth_manager.auth_mode().map(TelemetryAuthMode::from),
        "test_originator".to_string(),
        false,
        "test".to_string(),
        SessionSource::Exec,
    )
    .with_metrics(metrics);
    let effort = None;
    let summary = ReasoningSummary::Auto;
    let sandbox_policy = config.sandbox_policy.get().clone();
    let windows_sandbox_level = WindowsSandboxLevel::from_config(config.as_ref());
    let client = ModelClient::new(
        None,
        conversation_id,
        provider.clone(),
        SessionSource::Exec,
        config.model_verbosity,
        true,
        websocket_v2_enabled,
        false,
        runtime_metrics_enabled,
        None,
    );

    WebsocketTestHarness {
        _codex_home: codex_home,
        client,
        model_info,
        effort,
        summary,
        sandbox_policy,
        windows_sandbox_level,
        otel_manager,
    }
}

async fn stream_until_complete(
    client_session: &mut ModelClientSession,
    harness: &WebsocketTestHarness,
    prompt: &Prompt,
) {
    let mut stream = client_session
        .stream(
            prompt,
            &harness.model_info,
            &harness.otel_manager,
            harness.effort,
            harness.summary,
            None,
            &harness.sandbox_policy,
            harness.windows_sandbox_level,
        )
        .await
        .expect("websocket stream failed");

    while let Some(event) = stream.next().await {
        if matches!(event, Ok(ResponseEvent::Completed { .. })) {
            break;
        }
    }
}
