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
use codex_core::TransportManager;
use codex_core::WireApi;
use codex_core::features::Feature;
use codex_core::models_manager::manager::ModelsManager;
use codex_core::protocol::SessionSource;
use codex_otel::OtelManager;
use codex_otel::metrics::MetricsClient;
use codex_otel::metrics::MetricsConfig;
use codex_protocol::ThreadId;
use codex_protocol::config_types::ReasoningSummary;
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
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tracing_test::traced_test;

const MODEL: &str = "gpt-5.2-codex";

struct WebsocketTestHarness {
    _codex_home: TempDir,
    client: ModelClient,
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
    let mut session = harness.client.new_session();
    let prompt = prompt_with_input(vec![message_item("hello")]);

    stream_until_complete(&mut session, &prompt).await;

    let connection = server.single_connection();
    assert_eq!(connection.len(), 1);
    let body = connection.first().expect("missing request").body_json();

    assert_eq!(body["type"].as_str(), Some("response.create"));
    assert_eq!(body["model"].as_str(), Some(MODEL));
    assert_eq!(body["stream"], serde_json::Value::Bool(true));
    assert_eq!(body["input"].as_array().map(Vec::len), Some(1));

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
    let mut session = harness.client.new_session();
    let prompt = prompt_with_input(vec![message_item("hello")]);

    stream_until_complete(&mut session, &prompt).await;

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
async fn responses_websocket_emits_reasoning_included_event() {
    skip_if_no_network!();

    let server = start_websocket_server_with_headers(vec![WebSocketConnectionConfig {
        requests: vec![vec![ev_response_created("resp-1"), ev_completed("resp-1")]],
        response_headers: vec![("X-Reasoning-Included".to_string(), "true".to_string())],
    }])
    .await;

    let harness = websocket_harness(&server).await;
    let mut session = harness.client.new_session();
    let prompt = prompt_with_input(vec![message_item("hello")]);

    let mut stream = session
        .stream(&prompt)
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
async fn responses_websocket_appends_on_prefix() {
    skip_if_no_network!();

    let server = start_websocket_server(vec![vec![
        vec![ev_response_created("resp-1"), ev_completed("resp-1")],
        vec![ev_response_created("resp-2"), ev_completed("resp-2")],
    ]])
    .await;

    let harness = websocket_harness(&server).await;
    let mut session = harness.client.new_session();
    let prompt_one = prompt_with_input(vec![message_item("hello")]);
    let prompt_two = prompt_with_input(vec![message_item("hello"), message_item("second")]);

    stream_until_complete(&mut session, &prompt_one).await;
    stream_until_complete(&mut session, &prompt_two).await;

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
    let mut session = harness.client.new_session();
    let prompt_one = prompt_with_input(vec![message_item("hello")]);
    let prompt_two = prompt_with_input(vec![message_item("different")]);

    stream_until_complete(&mut session, &prompt_one).await;
    stream_until_complete(&mut session, &prompt_two).await;

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

fn message_item(text: &str) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: "user".into(),
        content: vec![ContentItem::InputText { text: text.into() }],
        end_turn: None,
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
    let provider = websocket_provider(server);
    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home).await;
    config.model = Some(MODEL.to_string());
    config.features.enable(Feature::ResponsesWebsockets);
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
        auth_manager.get_auth_mode(),
        false,
        "test".to_string(),
        SessionSource::Exec,
    )
    .with_metrics(metrics);
    let client = ModelClient::new(
        Arc::clone(&config),
        None,
        model_info,
        otel_manager.clone(),
        provider.clone(),
        None,
        ReasoningSummary::Auto,
        conversation_id,
        SessionSource::Exec,
        TransportManager::new(),
    );

    WebsocketTestHarness {
        _codex_home: codex_home,
        client,
        otel_manager,
    }
}

async fn stream_until_complete(session: &mut ModelClientSession, prompt: &Prompt) {
    let mut stream = session
        .stream(prompt)
        .await
        .expect("websocket stream failed");

    while let Some(event) = stream.next().await {
        if matches!(event, Ok(ResponseEvent::Completed { .. })) {
            break;
        }
    }
}
