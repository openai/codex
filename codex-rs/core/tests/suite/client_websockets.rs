#![allow(clippy::expect_used, clippy::unwrap_used)]
use codex_api::WS_REQUEST_HEADER_TRACEPARENT_CLIENT_METADATA_KEY;
use codex_api::WS_REQUEST_HEADER_TRACESTATE_CLIENT_METADATA_KEY;
use codex_core::ModelClient;
use codex_core::ModelClientSession;
use codex_core::Prompt;
use codex_core::ResponseEvent;
use codex_core::X_RESPONSESAPI_INCLUDE_TIMING_METRICS_HEADER;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_model_provider_info::ModelProviderInfo;
use codex_model_provider_info::WireApi;
use codex_otel::MetricsClient;
use codex_otel::MetricsConfig;
use codex_otel::SessionTelemetry;
use codex_otel::TelemetryAuthMode;
use codex_otel::current_span_w3c_trace_context;
use codex_protocol::ThreadId;
use codex_protocol::account::PlanType;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::config_types::ServiceTier;
use codex_protocol::models::BaseInstructions;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::W3cTraceContext;
use codex_protocol::user_input::UserInput;
use core_test_support::load_default_config_for_test;
use core_test_support::responses::WebSocketConnectionConfig;
use core_test_support::responses::WebSocketTestServer;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::start_websocket_server;
use core_test_support::responses::start_websocket_server_with_headers;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::tracing::install_test_tracing;
use core_test_support::wait_for_event;
use futures::StreamExt;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tracing::Instrument;
use tracing_test::traced_test;

const MODEL: &str = "gpt-5.2-codex";
const OPENAI_BETA_HEADER: &str = "OpenAI-Beta";
const WS_V2_BETA_HEADER_VALUE: &str = "responses_websockets=2026-02-06";
const X_CLIENT_REQUEST_ID_HEADER: &str = "x-client-request-id";
const TEST_INSTALLATION_ID: &str = "11111111-1111-4111-8111-111111111111";

fn assert_request_trace_matches(body: &serde_json::Value, expected_trace: &W3cTraceContext) {
    let client_metadata = body["client_metadata"]
        .as_object()
        .expect("missing client_metadata payload");
    let actual_traceparent = client_metadata
        .get(WS_REQUEST_HEADER_TRACEPARENT_CLIENT_METADATA_KEY)
        .and_then(serde_json::Value::as_str)
        .expect("missing traceparent");
    let expected_traceparent = expected_trace
        .traceparent
        .as_deref()
        .expect("missing expected traceparent");

    assert_eq!(actual_traceparent, expected_traceparent);
    assert_eq!(
        client_metadata
            .get(WS_REQUEST_HEADER_TRACESTATE_CLIENT_METADATA_KEY)
            .and_then(serde_json::Value::as_str),
        expected_trace.tracestate.as_deref()
    );
    assert!(
        body.get("trace").is_none(),
        "top-level trace should not be sent"
    );
}

struct WebsocketTestHarness {
    _codex_home: TempDir,
    client: ModelClient,
    conversation_id: ThreadId,
    model_info: ModelInfo,
    effort: Option<ReasoningEffortConfig>,
    summary: ReasoningSummary,
    session_telemetry: SessionTelemetry,
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
        Some(WS_V2_BETA_HEADER_VALUE.to_string())
    );
    assert_eq!(
        handshake.header(X_CLIENT_REQUEST_ID_HEADER),
        Some(harness.conversation_id.to_string())
    );
    assert_eq!(
        body["client_metadata"]["x-codex-installation-id"].as_str(),
        Some(TEST_INSTALLATION_ID)
    );

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_websocket_streams_without_feature_flag_when_provider_supports_websockets() {
    skip_if_no_network!();

    let server = start_websocket_server(vec![vec![vec![
        ev_response_created("resp-1"),
        ev_completed("resp-1"),
    ]]])
    .await;

    let harness = websocket_harness_with_options(&server, /*runtime_metrics_enabled*/ false).await;
    let mut client_session = harness.client.new_session();
    let prompt = prompt_with_input(vec![message_item("hello")]);

    stream_until_complete(&mut client_session, &harness, &prompt).await;

    assert_eq!(server.handshakes().len(), 1);
    assert_eq!(server.single_connection().len(), 1);

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_websocket_reuses_connection_with_per_turn_trace_payloads() {
    skip_if_no_network!();

    let _trace_test_context = install_test_tracing("client-websocket-test");

    let server = start_websocket_server(vec![vec![
        vec![ev_response_created("resp-1"), ev_completed("resp-1")],
        vec![ev_response_created("resp-2"), ev_completed("resp-2")],
    ]])
    .await;

    let harness = websocket_harness(&server).await;
    let prompt_one = prompt_with_input(vec![message_item("hello")]);
    let prompt_two = prompt_with_input(vec![message_item("again")]);

    let first_trace = {
        let mut client_session = harness.client.new_session();
        async {
            let expected_trace =
                current_span_w3c_trace_context().expect("current span should have trace context");
            stream_until_complete(&mut client_session, &harness, &prompt_one).await;
            expected_trace
        }
        .instrument(tracing::info_span!("client.websocket.turn_one"))
        .await
    };

    let second_trace = {
        let mut client_session = harness.client.new_session();
        async {
            let expected_trace =
                current_span_w3c_trace_context().expect("current span should have trace context");
            stream_until_complete(&mut client_session, &harness, &prompt_two).await;
            expected_trace
        }
        .instrument(tracing::info_span!("client.websocket.turn_two"))
        .await
    };

    assert_eq!(server.handshakes().len(), 1);
    let connection = server.single_connection();
    assert_eq!(connection.len(), 2);

    let first_request = connection
        .first()
        .expect("missing first request")
        .body_json();
    let second_request = connection
        .get(1)
        .expect("missing second request")
        .body_json();
    assert_request_trace_matches(&first_request, &first_trace);
    assert_request_trace_matches(&second_request, &second_trace);

    let first_traceparent = first_request["client_metadata"]
        [WS_REQUEST_HEADER_TRACEPARENT_CLIENT_METADATA_KEY]
        .as_str()
        .expect("missing first traceparent");
    let second_traceparent = second_request["client_metadata"]
        [WS_REQUEST_HEADER_TRACEPARENT_CLIENT_METADATA_KEY]
        .as_str()
        .expect("missing second traceparent");
    assert_ne!(first_traceparent, second_traceparent);

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_websocket_preconnect_does_not_replace_turn_trace_payload() {
    skip_if_no_network!();

    let _trace_test_context = install_test_tracing("client-websocket-test");

    let server = start_websocket_server(vec![vec![vec![
        ev_response_created("resp-1"),
        ev_completed("resp-1"),
    ]]])
    .await;

    let harness = websocket_harness(&server).await;
    let mut client_session = harness.client.new_session();
    client_session
        .preconnect_websocket(&harness.session_telemetry, &harness.model_info)
        .await
        .expect("websocket preconnect failed");
    let prompt = prompt_with_input(vec![message_item("hello")]);

    let expected_trace = async {
        let expected_trace =
            current_span_w3c_trace_context().expect("current span should have trace context");
        stream_until_complete(&mut client_session, &harness, &prompt).await;
        expected_trace
    }
    .instrument(tracing::info_span!("client.websocket.request"))
    .await;

    assert_eq!(server.handshakes().len(), 1);
    let connection = server.single_connection();
    assert_eq!(connection.len(), 1);
    let request = connection.first().expect("missing request").body_json();
    assert_request_trace_matches(&request, &expected_trace);

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
    let mut client_session = harness.client.new_session();
    client_session
        .preconnect_websocket(&harness.session_telemetry, &harness.model_info)
        .await
        .expect("websocket preconnect failed");
    let prompt = prompt_with_input(vec![message_item("hello")]);
    stream_until_complete(&mut client_session, &harness, &prompt).await;

    assert_eq!(server.handshakes().len(), 1);
    assert_eq!(server.single_connection().len(), 1);

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_websocket_request_prewarm_reuses_connection() {
    skip_if_no_network!();

    let server = start_websocket_server(vec![vec![
        vec![ev_response_created("warm-1"), ev_completed("warm-1")],
        vec![ev_response_created("resp-1"), ev_completed("resp-1")],
    ]])
    .await;

    let harness = websocket_harness_with_options(&server, /*runtime_metrics_enabled*/ true).await;
    let mut client_session = harness.client.new_session();
    let prompt = prompt_with_input(vec![message_item("hello")]);
    client_session
        .prewarm_websocket(
            &prompt,
            &harness.model_info,
            &harness.session_telemetry,
            harness.effort,
            harness.summary,
            /*service_tier*/ None,
            /*turn_metadata_header*/ None,
        )
        .await
        .expect("websocket prewarm failed");
    stream_until_complete(&mut client_session, &harness, &prompt).await;

    assert_eq!(server.handshakes().len(), 1);
    let connection = server.single_connection();
    assert_eq!(connection.len(), 2);
    let warmup = connection
        .first()
        .expect("missing warmup request")
        .body_json();
    let follow_up = connection
        .get(1)
        .expect("missing follow-up request")
        .body_json();

    assert_eq!(warmup["type"].as_str(), Some("response.create"));
    assert_eq!(warmup["generate"].as_bool(), Some(false));
    assert_eq!(warmup["tools"], serde_json::json!([]));
    assert_eq!(follow_up["type"].as_str(), Some("response.create"));
    assert_eq!(follow_up["previous_response_id"].as_str(), Some("warm-1"));
    assert_eq!(follow_up["input"], serde_json::json!([]));

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_websocket_reuses_connection_after_session_drop() {
    skip_if_no_network!();

    let server = start_websocket_server(vec![vec![
        vec![ev_response_created("resp-1"), ev_completed("resp-1")],
        vec![ev_response_created("resp-2"), ev_completed("resp-2")],
    ]])
    .await;

    let harness = websocket_harness(&server).await;
    let prompt_one = prompt_with_input(vec![message_item("hello")]);
    let prompt_two = prompt_with_input(vec![message_item("again")]);

    {
        let mut client_session = harness.client.new_session();
        stream_until_complete(&mut client_session, &harness, &prompt_one).await;
    }

    let mut client_session = harness.client.new_session();
    stream_until_complete(&mut client_session, &harness, &prompt_two).await;

    assert_eq!(server.handshakes().len(), 1);
    assert_eq!(server.single_connection().len(), 2);

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
    let mut client_session = harness.client.new_session();
    client_session
        .preconnect_websocket(&harness.session_telemetry, &harness.model_info)
        .await
        .expect("websocket preconnect failed");
    let prompt = prompt_with_input(vec![message_item("hello")]);
    let mut stream = client_session
        .stream(
            &prompt,
            &harness.model_info,
            &harness.session_telemetry,
            harness.effort,
            harness.summary,
            /*service_tier*/ None,
            /*turn_metadata_header*/ None,
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
async fn responses_websocket_request_prewarm_is_reused_even_with_header_changes() {
    skip_if_no_network!();

    let server = start_websocket_server(vec![vec![
        vec![ev_response_created("warm-1"), ev_completed("warm-1")],
        vec![ev_response_created("resp-1"), ev_completed("resp-1")],
    ]])
    .await;

    let harness = websocket_harness_with_options(&server, /*runtime_metrics_enabled*/ true).await;
    let mut client_session = harness.client.new_session();
    let prompt = prompt_with_input(vec![message_item("hello")]);
    client_session
        .prewarm_websocket(
            &prompt,
            &harness.model_info,
            &harness.session_telemetry,
            harness.effort,
            harness.summary,
            /*service_tier*/ None,
            /*turn_metadata_header*/ None,
        )
        .await
        .expect("websocket prewarm failed");
    let mut stream = client_session
        .stream(
            &prompt,
            &harness.model_info,
            &harness.session_telemetry,
            harness.effort,
            harness.summary,
            /*service_tier*/ None,
            /*turn_metadata_header*/ None,
        )
        .await
        .expect("websocket stream failed");

    while let Some(event) = stream.next().await {
        if matches!(event, Ok(ResponseEvent::Completed { .. })) {
            break;
        }
    }

    assert_eq!(server.handshakes().len(), 1);
    let connection = server.single_connection();
    assert_eq!(connection.len(), 2);
    let warmup = connection
        .first()
        .expect("missing warmup request")
        .body_json();
    let follow_up = connection
        .get(1)
        .expect("missing follow-up request")
        .body_json();
    assert_eq!(warmup["type"].as_str(), Some("response.create"));
    assert_eq!(warmup["generate"].as_bool(), Some(false));
    assert_eq!(warmup["tools"], serde_json::json!([]));
    assert_eq!(follow_up["type"].as_str(), Some("response.create"));
    assert_eq!(follow_up["previous_response_id"].as_str(), Some("warm-1"));
    assert_eq!(follow_up["input"], serde_json::json!([]));

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_websocket_prewarm_uses_v2_when_provider_supports_websockets() {
    skip_if_no_network!();

    let server = start_websocket_server(vec![vec![vec![
        ev_response_created("resp-1"),
        ev_completed("resp-1"),
    ]]])
    .await;

    let harness = websocket_harness_with_options(&server, /*runtime_metrics_enabled*/ false).await;
    let mut client_session = harness.client.new_session();
    let prompt = prompt_with_input(vec![message_item("hello")]);
    client_session
        .prewarm_websocket(
            &prompt,
            &harness.model_info,
            &harness.session_telemetry,
            harness.effort,
            harness.summary,
            /*service_tier*/ None,
            /*turn_metadata_header*/ None,
        )
        .await
        .expect("websocket prewarm failed");

    // V2 prewarm issues a request on the websocket connection.
    assert_eq!(server.handshakes().len(), 1);
    assert_eq!(server.single_connection().len(), 1);

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
    stream_until_complete(&mut client_session, &harness, &prompt).await;
    assert_eq!(server.handshakes().len(), 1);
    let connection = server.single_connection();
    assert_eq!(connection.len(), 1);
    let prewarm = connection
        .first()
        .expect("missing prewarm request")
        .body_json();
    assert_eq!(prewarm["type"].as_str(), Some("response.create"));
    assert_eq!(
        prewarm["input"],
        serde_json::to_value(&prompt.input).unwrap()
    );

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_websocket_preconnect_runs_when_only_v2_feature_enabled() {
    skip_if_no_network!();

    let server = start_websocket_server(vec![vec![vec![
        ev_response_created("resp-1"),
        ev_completed("resp-1"),
    ]]])
    .await;

    let harness = websocket_harness_with_options(&server, /*runtime_metrics_enabled*/ true).await;
    let mut client_session = harness.client.new_session();
    client_session
        .preconnect_websocket(&harness.session_telemetry, &harness.model_info)
        .await
        .expect("websocket preconnect failed");

    assert_eq!(server.handshakes().len(), 1);
    assert_eq!(server.single_connection().len(), 0);

    let prompt = prompt_with_input(vec![message_item("hello")]);
    stream_until_complete(&mut client_session, &harness, &prompt).await;

    assert_eq!(server.handshakes().len(), 1);
    assert_eq!(server.single_connection().len(), 1);

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
    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_websocket_v2_requests_use_v2_when_provider_supports_websockets() {
    skip_if_no_network!();

    let server = start_websocket_server(vec![vec![
        vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "assistant output"),
            ev_completed("resp-1"),
        ],
        vec![ev_response_created("resp-2"), ev_completed("resp-2")],
    ]])
    .await;

    let harness = websocket_harness_with_options(&server, /*runtime_metrics_enabled*/ true).await;
    let mut client_session = harness.client.new_session();
    let prompt_one = prompt_with_input(vec![message_item("hello")]);
    let prompt_two = prompt_with_input(vec![
        message_item("hello"),
        assistant_message_item("msg-1", "assistant output"),
        message_item("second"),
    ]);

    stream_until_complete(&mut client_session, &harness, &prompt_one).await;
    stream_until_complete(&mut client_session, &harness, &prompt_two).await;

    let connection = server.single_connection();
    assert_eq!(connection.len(), 2);
    let second = connection.get(1).expect("missing request").body_json();
    assert_eq!(second["type"].as_str(), Some("response.create"));
    assert_eq!(second["previous_response_id"].as_str(), Some("resp-1"));
    assert_eq!(
        second["input"],
        serde_json::to_value(&prompt_two.input[2..]).unwrap()
    );

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
    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_websocket_v2_incremental_requests_are_reused_across_turns() {
    skip_if_no_network!();

    let server = start_websocket_server(vec![vec![
        vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "assistant output"),
            ev_completed("resp-1"),
        ],
        vec![ev_response_created("resp-2"), ev_completed("resp-2")],
    ]])
    .await;

    let harness = websocket_harness_with_options(&server, /*runtime_metrics_enabled*/ false).await;
    let prompt_one = prompt_with_input(vec![message_item("hello")]);
    let prompt_two = prompt_with_input(vec![
        message_item("hello"),
        assistant_message_item("msg-1", "assistant output"),
        message_item("second"),
    ]);

    {
        let mut client_session = harness.client.new_session();
        stream_until_complete(&mut client_session, &harness, &prompt_one).await;
    }

    let mut client_session = harness.client.new_session();
    stream_until_complete(&mut client_session, &harness, &prompt_two).await;

    assert_eq!(server.handshakes().len(), 1);
    let connection = server.single_connection();
    assert_eq!(connection.len(), 2);
    let second = connection.get(1).expect("missing request").body_json();
    assert_eq!(second["type"].as_str(), Some("response.create"));
    assert_eq!(second["previous_response_id"].as_str(), Some("resp-1"));
    assert_eq!(
        second["input"],
        serde_json::to_value(&prompt_two.input[2..]).unwrap()
    );

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_websocket_v2_wins_when_both_features_enabled() {
    skip_if_no_network!();

    let server = start_websocket_server(vec![vec![
        vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "assistant output"),
            ev_completed("resp-1"),
        ],
        vec![ev_response_created("resp-2"), ev_completed("resp-2")],
    ]])
    .await;

    let harness = websocket_harness_with_options(&server, /*runtime_metrics_enabled*/ false).await;
    let mut client_session = harness.client.new_session();
    let prompt_one = prompt_with_input(vec![message_item("hello")]);
    let prompt_two = prompt_with_input(vec![
        message_item("hello"),
        assistant_message_item("msg-1", "assistant output"),
        message_item("second"),
    ]);

    stream_until_complete(&mut client_session, &harness, &prompt_one).await;
    stream_until_complete(&mut client_session, &harness, &prompt_two).await;

    let connection = server.single_connection();
    assert_eq!(connection.len(), 2);
    let second = connection.get(1).expect("missing request").body_json();
    assert_eq!(second["type"].as_str(), Some("response.create"));
    assert_eq!(second["previous_response_id"].as_str(), Some("resp-1"));
    assert_eq!(
        second["input"],
        serde_json::to_value(&prompt_two.input[2..]).unwrap()
    );

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
    harness.session_telemetry.reset_runtime_metrics();
    let mut client_session = harness.client.new_session();
    let prompt = prompt_with_input(vec![message_item("hello")]);

    stream_until_complete(&mut client_session, &harness, &prompt).await;

    tokio::time::sleep(Duration::from_millis(10)).await;

    let summary = harness
        .session_telemetry
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

    let harness =
        websocket_harness_with_runtime_metrics(&server, /*runtime_metrics_enabled*/ true).await;
    harness.session_telemetry.reset_runtime_metrics();
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
        .session_telemetry
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

    let harness =
        websocket_harness_with_runtime_metrics(&server, /*runtime_metrics_enabled*/ false).await;
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
        close_after_requests: true,
    }])
    .await;

    let harness = websocket_harness(&server).await;
    let mut client_session = harness.client.new_session();
    let prompt = prompt_with_input(vec![message_item("hello")]);

    let mut stream = client_session
        .stream(
            &prompt,
            &harness.model_info,
            &harness.session_telemetry,
            harness.effort,
            harness.summary,
            /*service_tier*/ None,
            /*turn_metadata_header*/ None,
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
        close_after_requests: true,
    }])
    .await;

    let harness = websocket_harness(&server).await;
    let mut client_session = harness.client.new_session();
    let prompt = prompt_with_input(vec![message_item("hello")]);

    let mut stream = client_session
        .stream(
            &prompt,
            &harness.model_info,
            &harness.session_telemetry,
            harness.effort,
            harness.summary,
            /*service_tier*/ None,
            /*turn_metadata_header*/ None,
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
async fn responses_websocket_usage_limit_error_emits_rate_limit_event() {
    skip_if_no_network!();

    let usage_limit_error = json!({
        "type": "error",
        "status": 429,
        "error": {
            "type": "usage_limit_reached",
            "message": "The usage limit has been reached",
            "plan_type": "pro",
            "resets_at": 1704067242,
            "resets_in_seconds": 1234
        },
        "headers": {
            "x-codex-primary-used-percent": "100.0",
            "x-codex-secondary-used-percent": "87.5",
            "x-codex-primary-over-secondary-limit-percent": "95.0",
            "x-codex-primary-window-minutes": "15",
            "x-codex-secondary-window-minutes": "60"
        }
    });

    let server = start_websocket_server(vec![vec![
        vec![
            ev_response_created("resp-prewarm"),
            ev_completed("resp-prewarm"),
        ],
        vec![usage_limit_error],
    ]])
    .await;
    let mut builder = test_codex().with_config(|config| {
        config.model_provider.request_max_retries = Some(0);
        config.model_provider.stream_max_retries = Some(0);
    });
    let test = builder
        .build_with_websocket_server(&server)
        .await
        .expect("build websocket codex");

    let submission_id = test
        .codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
        })
        .await
        .expect("submission should succeed while emitting usage limit error events");

    let token_event =
        wait_for_event(&test.codex, |msg| matches!(msg, EventMsg::TokenCount(_))).await;
    let EventMsg::TokenCount(event) = token_event else {
        unreachable!();
    };

    let event_json = serde_json::to_value(&event).expect("serialize token count event");
    pretty_assertions::assert_eq!(
        event_json,
        json!({
            "info": null,
            "rate_limits": {
                "limit_id": "codex",
                "limit_name": null,
                "primary": {
                    "used_percent": 100.0,
                    "window_minutes": 15,
                    "resets_at": null
                },
                "secondary": {
                    "used_percent": 87.5,
                    "window_minutes": 60,
                    "resets_at": null
                },
                "credits": null,
                "plan_type": null
            }
        })
    );

    let error_event = wait_for_event(&test.codex, |msg| matches!(msg, EventMsg::Error(_))).await;
    let EventMsg::Error(error_event) = error_event else {
        unreachable!();
    };
    assert!(
        error_event.message.to_lowercase().contains("usage limit"),
        "unexpected error message for submission {submission_id}: {}",
        error_event.message
    );

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_websocket_invalid_request_error_with_status_is_forwarded() {
    skip_if_no_network!();

    let invalid_request_error = json!({
        "type": "error",
        "status": 400,
        "error": {
            "type": "invalid_request_error",
            "message": "Model 'castor-raikou-0205-ev3' does not support image inputs."
        }
    });

    let server = start_websocket_server(vec![vec![
        vec![
            ev_response_created("resp-prewarm"),
            ev_completed("resp-prewarm"),
        ],
        vec![invalid_request_error],
    ]])
    .await;
    let mut builder = test_codex().with_config(|config| {
        config.model_provider.request_max_retries = Some(0);
        config.model_provider.stream_max_retries = Some(0);
    });
    let test = builder
        .build_with_websocket_server(&server)
        .await
        .expect("build websocket codex");

    let submission_id = test
        .codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
        })
        .await
        .expect("submission should succeed while emitting invalid request events");

    let error_event = wait_for_event(&test.codex, |msg| matches!(msg, EventMsg::Error(_))).await;
    let EventMsg::Error(error_event) = error_event else {
        unreachable!();
    };
    assert!(
        error_event
            .message
            .to_lowercase()
            .contains("does not support image inputs"),
        "unexpected error message for submission {submission_id}: {}",
        error_event.message
    );

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_websocket_connection_limit_error_reconnects_and_completes() {
    skip_if_no_network!();

    let websocket_connection_limit_error = json!({
        "type": "error",
        "status": 400,
        "error": {
            "type": "invalid_request_error",
            "code": "websocket_connection_limit_reached",
            "message": "Responses websocket connection limit reached (60 minutes). Create a new websocket connection to continue."
        }
    });

    let server = start_websocket_server(vec![
        vec![vec![websocket_connection_limit_error]],
        vec![vec![ev_response_created("resp-1"), ev_completed("resp-1")]],
    ])
    .await;
    let mut builder = test_codex().with_config(|config| {
        config.model_provider.request_max_retries = Some(0);
        config.model_provider.stream_max_retries = Some(1);
    });
    let test = builder
        .build_with_websocket_server(&server)
        .await
        .expect("build websocket codex");

    test.submit_turn("hello")
        .await
        .expect("submission should reconnect after websocket connection limit error");

    let total_websocket_requests: usize = server.connections().iter().map(Vec::len).sum();
    assert_eq!(total_websocket_requests, 2);

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_websocket_uses_incremental_create_on_prefix() {
    skip_if_no_network!();

    let server = start_websocket_server(vec![vec![
        vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "assistant output"),
            ev_completed("resp-1"),
        ],
        vec![ev_response_created("resp-2"), ev_completed("resp-2")],
    ]])
    .await;

    let harness = websocket_harness(&server).await;
    let mut client_session = harness.client.new_session();
    let prompt_one = prompt_with_input(vec![message_item("hello")]);
    let prompt_two = prompt_with_input(vec![
        message_item("hello"),
        assistant_message_item("msg-1", "assistant output"),
        message_item("second"),
    ]);

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
    assert_eq!(second["type"].as_str(), Some("response.create"));
    assert_eq!(second["previous_response_id"].as_str(), Some("resp-1"));
    assert_eq!(
        second["input"],
        serde_json::to_value(&prompt_two.input[2..]).expect("serialize incremental items")
    );

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_websocket_forwards_turn_metadata_on_initial_and_incremental_create() {
    skip_if_no_network!();

    let server = start_websocket_server(vec![vec![
        vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "assistant output"),
            ev_completed("resp-1"),
        ],
        vec![ev_response_created("resp-2"), ev_completed("resp-2")],
    ]])
    .await;

    let harness = websocket_harness(&server).await;
    let mut client_session = harness.client.new_session();
    let first_turn_metadata =
        r#"{"turn_id":"turn-123","thread_source":"user","sandbox":"workspace-write"}"#;
    let enriched_turn_metadata = r#"{"turn_id":"turn-123","thread_source":"user","sandbox":"workspace-write","workspaces":[{"root_path":"/tmp/repo","latest_git_commit_hash":"abc123","associated_remote_urls":["git@github.com:openai/codex.git"],"has_changes":true}]}"#;
    let prompt_one = prompt_with_input(vec![message_item("hello")]);
    let prompt_two = prompt_with_input(vec![
        message_item("hello"),
        assistant_message_item("msg-1", "assistant output"),
        message_item("second"),
    ]);

    stream_until_complete_with_turn_metadata(
        &mut client_session,
        &harness,
        &prompt_one,
        /*service_tier*/ None,
        Some(first_turn_metadata),
    )
    .await;
    stream_until_complete_with_turn_metadata(
        &mut client_session,
        &harness,
        &prompt_two,
        /*service_tier*/ None,
        Some(enriched_turn_metadata),
    )
    .await;

    let connection = server.single_connection();
    assert_eq!(connection.len(), 2);
    let first = connection.first().expect("missing request").body_json();
    let second = connection.get(1).expect("missing request").body_json();

    assert_eq!(first["type"].as_str(), Some("response.create"));
    assert_eq!(
        first["client_metadata"]["x-codex-turn-metadata"].as_str(),
        Some(first_turn_metadata)
    );
    assert_eq!(second["type"].as_str(), Some("response.create"));
    assert_eq!(second["previous_response_id"].as_str(), Some("resp-1"));
    assert_eq!(
        second["client_metadata"]["x-codex-turn-metadata"].as_str(),
        Some(enriched_turn_metadata)
    );

    let first_metadata: serde_json::Value =
        serde_json::from_str(first_turn_metadata).expect("first metadata should be valid json");
    let second_metadata: serde_json::Value = serde_json::from_str(enriched_turn_metadata)
        .expect("enriched metadata should be valid json");

    assert_eq!(first_metadata["turn_id"].as_str(), Some("turn-123"));
    assert_eq!(second_metadata["turn_id"].as_str(), Some("turn-123"));
    assert_eq!(first_metadata["thread_source"].as_str(), Some("user"));
    assert_eq!(second_metadata["thread_source"].as_str(), Some("user"));
    assert_eq!(
        second_metadata["workspaces"][0]["has_changes"].as_bool(),
        Some(true)
    );

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_websocket_preserves_custom_turn_metadata_fields() {
    skip_if_no_network!();

    let server = start_websocket_server(vec![vec![vec![
        ev_response_created("resp-1"),
        ev_completed("resp-1"),
    ]]])
    .await;

    let harness = websocket_harness(&server).await;
    let mut client_session = harness.client.new_session();
    let prompt = prompt_with_input(vec![message_item("hello")]);
    let turn_metadata = json!({
        "turn_id": "turn-123",
        "fiber_run_id": "fiber-123",
        "origin": "app-server",
    })
    .to_string();

    stream_until_complete_with_turn_metadata(
        &mut client_session,
        &harness,
        &prompt,
        /*service_tier*/ None,
        Some(&turn_metadata),
    )
    .await;

    let body = server
        .single_connection()
        .first()
        .expect("missing request")
        .body_json();

    assert_eq!(body["type"].as_str(), Some("response.create"));
    assert_eq!(
        body["client_metadata"]["x-codex-turn-metadata"]
            .as_str()
            .map(|value| serde_json::from_str::<serde_json::Value>(value).expect("valid json")),
        Some(json!({
            "turn_id": "turn-123",
            "fiber_run_id": "fiber-123",
            "origin": "app-server",
        }))
    );

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_websocket_uses_previous_response_id_when_prefix_after_completed() {
    skip_if_no_network!();

    let server = start_websocket_server(vec![vec![
        vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "assistant output"),
            ev_completed("resp-1"),
        ],
        vec![ev_response_created("resp-2"), ev_completed("resp-2")],
    ]])
    .await;

    let harness = websocket_harness(&server).await;
    let mut client_session = harness.client.new_session();
    let prompt_one = prompt_with_input(vec![message_item("hello")]);
    let prompt_two = prompt_with_input(vec![
        message_item("hello"),
        assistant_message_item("msg-1", "assistant output"),
        message_item("second"),
    ]);

    stream_until_complete(&mut client_session, &harness, &prompt_one).await;
    stream_until_complete(&mut client_session, &harness, &prompt_two).await;

    let connection = server.single_connection();
    assert_eq!(connection.len(), 2);
    let second = connection.get(1).expect("missing request").body_json();

    assert_eq!(second["type"].as_str(), Some("response.create"));
    assert_eq!(second["previous_response_id"].as_str(), Some("resp-1"));
    assert_eq!(
        second["input"],
        serde_json::to_value(&prompt_two.input[2..]).expect("serialize incremental input")
    );

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
async fn responses_websocket_creates_when_non_input_request_fields_change() {
    skip_if_no_network!();

    let server = start_websocket_server(vec![vec![
        vec![ev_response_created("resp-1"), ev_completed("resp-1")],
        vec![ev_response_created("resp-2"), ev_completed("resp-2")],
    ]])
    .await;

    let harness = websocket_harness(&server).await;
    let mut client_session = harness.client.new_session();
    let prompt_one =
        prompt_with_input_and_instructions(vec![message_item("hello")], "base instructions one");
    let prompt_two = prompt_with_input_and_instructions(
        vec![message_item("hello"), message_item("second")],
        "base instructions two",
    );

    stream_until_complete(&mut client_session, &harness, &prompt_one).await;
    stream_until_complete(&mut client_session, &harness, &prompt_two).await;

    let connection = server.single_connection();
    assert_eq!(connection.len(), 2);
    let second = connection.get(1).expect("missing request").body_json();

    assert_eq!(second["type"].as_str(), Some("response.create"));
    assert_eq!(second.get("previous_response_id"), None);
    assert_eq!(
        second["input"],
        serde_json::to_value(&prompt_two.input).expect("serialize full input")
    );

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_websocket_v2_creates_with_previous_response_id_on_prefix() {
    skip_if_no_network!();

    let server = start_websocket_server(vec![vec![
        vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "assistant output"),
            ev_completed("resp-1"),
        ],
        vec![ev_response_created("resp-2"), ev_completed("resp-2")],
    ]])
    .await;

    let harness = websocket_harness_with_v2(&server, /*runtime_metrics_enabled*/ true).await;
    let mut session = harness.client.new_session();
    let prompt_one = prompt_with_input(vec![message_item("hello")]);
    let prompt_two = prompt_with_input(vec![
        message_item("hello"),
        assistant_message_item("msg-1", "assistant output"),
        message_item("second"),
    ]);

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
        serde_json::to_value(&prompt_two.input[2..]).unwrap()
    );

    server.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_websocket_v2_creates_without_previous_response_id_when_non_input_fields_change()
{
    skip_if_no_network!();

    let server = start_websocket_server(vec![vec![
        vec![ev_response_created("resp-1"), ev_completed("resp-1")],
        vec![ev_response_created("resp-2"), ev_completed("resp-2")],
    ]])
    .await;

    let harness = websocket_harness_with_v2(&server, /*runtime_metrics_enabled*/ true).await;
    let mut session = harness.client.new_session();
    let prompt_one =
        prompt_with_input_and_instructions(vec![message_item("hello")], "base instructions one");
    let prompt_two = prompt_with_input_and_instructions(
        vec![message_item("hello"), message_item("second")],
        "base instructions two",
    );

    stream_until_complete(&mut session, &harness, &prompt_one).await;
    stream_until_complete(&mut session, &harness, &prompt_two).await;

    let connection = server.single_connection();
    assert_eq!(connection.len(), 2);
    let second = connection.get(1).expect("missing request").body_json();

    assert_eq!(second["type"].as_str(), Some("response.create"));
    assert_eq!(second.get("previous_response_id"), None);
    assert_eq!(
        second["input"],
        serde_json::to_value(&prompt_two.input).expect("serialize full input")
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

    let harness = websocket_harness_with_v2(&server, /*runtime_metrics_enabled*/ true).await;
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
            &harness.session_telemetry,
            harness.effort,
            harness.summary,
            /*service_tier*/ None,
            /*turn_metadata_header*/ None,
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
async fn responses_websocket_v2_surfaces_terminal_error_without_close_handshake() {
    skip_if_no_network!();

    let server = start_websocket_server_with_headers(vec![WebSocketConnectionConfig {
        requests: vec![
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
        response_headers: Vec::new(),
        accept_delay: None,
        close_after_requests: false,
    }])
    .await;

    let harness = websocket_harness_with_v2(&server, /*runtime_metrics_enabled*/ true).await;
    let mut session = harness.client.new_session();
    let prompt_one = prompt_with_input(vec![message_item("hello")]);
    let prompt_two = prompt_with_input(vec![message_item("hello"), message_item("second")]);

    stream_until_complete(&mut session, &harness, &prompt_one).await;

    let mut second_stream = session
        .stream(
            &prompt_two,
            &harness.model_info,
            &harness.session_telemetry,
            harness.effort,
            harness.summary,
            /*service_tier*/ None,
            /*turn_metadata_header*/ None,
        )
        .await
        .expect("websocket stream failed");

    let saw_error = tokio::time::timeout(Duration::from_secs(2), async {
        while let Some(event) = second_stream.next().await {
            if event.is_err() {
                return true;
            }
        }
        false
    })
    .await
    .expect("timed out waiting for terminal websocket error");

    assert!(saw_error, "expected second websocket stream to error");

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

    let harness = websocket_harness_with_v2(&server, /*runtime_metrics_enabled*/ true).await;
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

fn assistant_message_item(id: &str, text: &str) -> ResponseItem {
    ResponseItem::Message {
        id: Some(id.to_string()),
        role: "assistant".into(),
        content: vec![ContentItem::OutputText { text: text.into() }],
        end_turn: None,
        phase: None,
    }
}

fn prompt_with_input(input: Vec<ResponseItem>) -> Prompt {
    let mut prompt = Prompt::default();
    prompt.input = input;
    prompt
}

fn prompt_with_input_and_instructions(input: Vec<ResponseItem>, instructions: &str) -> Prompt {
    let mut prompt = prompt_with_input(input);
    prompt.base_instructions = BaseInstructions {
        text: instructions.to_string(),
    };
    prompt
}

fn websocket_provider(server: &WebSocketTestServer) -> ModelProviderInfo {
    websocket_provider_with_connect_timeout(server, /*websocket_connect_timeout_ms*/ None)
}

fn websocket_provider_with_connect_timeout(
    server: &WebSocketTestServer,
    websocket_connect_timeout_ms: Option<u64>,
) -> ModelProviderInfo {
    ModelProviderInfo {
        name: "mock-ws".into(),
        base_url: Some(format!("{}/v1", server.uri())),
        env_key: None,
        env_key_instructions: None,
        experimental_bearer_token: None,
        auth: None,
        wire_api: WireApi::Responses,
        query_params: None,
        http_headers: None,
        env_http_headers: None,
        request_max_retries: Some(0),
        stream_max_retries: Some(0),
        stream_idle_timeout_ms: Some(5_000),
        websocket_connect_timeout_ms,
        requires_openai_auth: false,
        supports_websockets: true,
    }
}

async fn websocket_harness(server: &WebSocketTestServer) -> WebsocketTestHarness {
    websocket_harness_with_runtime_metrics(server, /*runtime_metrics_enabled*/ false).await
}

async fn websocket_harness_with_runtime_metrics(
    server: &WebSocketTestServer,
    runtime_metrics_enabled: bool,
) -> WebsocketTestHarness {
    websocket_harness_with_options(server, runtime_metrics_enabled).await
}

async fn websocket_harness_with_v2(
    server: &WebSocketTestServer,
    runtime_metrics_enabled: bool,
) -> WebsocketTestHarness {
    websocket_harness_with_options(server, runtime_metrics_enabled).await
}

async fn websocket_harness_with_options(
    server: &WebSocketTestServer,
    runtime_metrics_enabled: bool,
) -> WebsocketTestHarness {
    websocket_harness_with_provider_options(websocket_provider(server), runtime_metrics_enabled)
        .await
}

async fn websocket_harness_with_provider_options(
    provider: ModelProviderInfo,
    runtime_metrics_enabled: bool,
) -> WebsocketTestHarness {
    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home).await;
    config.model = Some(MODEL.to_string());
    if runtime_metrics_enabled {
        config
            .features
            .enable(Feature::RuntimeMetrics)
            .expect("test config should allow feature update");
    }
    let config = Arc::new(config);
    let model_info = codex_core::test_support::construct_model_info_offline(MODEL, &config);
    let conversation_id = ThreadId::new();
    let auth_manager =
        codex_core::test_support::auth_manager_from_auth(CodexAuth::from_api_key("Test API Key"));
    let exporter = InMemoryMetricExporter::default();
    let metrics = MetricsClient::new(
        MetricsConfig::in_memory("test", "codex-core", env!("CARGO_PKG_VERSION"), exporter)
            .with_runtime_reader(),
    )
    .expect("in-memory metrics client");
    let session_telemetry = SessionTelemetry::new(
        conversation_id,
        MODEL,
        model_info.slug.as_str(),
        /*account_id*/ None,
        Some("test@test.com".to_string()),
        auth_manager.auth_mode().map(TelemetryAuthMode::from),
        "test_originator".to_string(),
        /*log_user_prompts*/ false,
        "test".to_string(),
        SessionSource::Exec,
    )
    .with_metrics(metrics);
    let effort = None;
    let summary = ReasoningSummary::Auto;
    let client = ModelClient::new(
        /*auth_manager*/ None,
        conversation_id,
        /*installation_id*/ TEST_INSTALLATION_ID.to_string(),
        /*prompt_cache_key_override*/ None,
        provider.clone(),
        SessionSource::Exec,
        config.model_verbosity,
        /*enable_request_compression*/ false,
        runtime_metrics_enabled,
        /*beta_features_header*/ None,
    );

    WebsocketTestHarness {
        _codex_home: codex_home,
        client,
        conversation_id,
        model_info,
        effort,
        summary,
        session_telemetry,
    }
}

async fn stream_until_complete(
    client_session: &mut ModelClientSession,
    harness: &WebsocketTestHarness,
    prompt: &Prompt,
) {
    stream_until_complete_with_service_tier(
        client_session,
        harness,
        prompt,
        /*service_tier*/ None,
    )
    .await;
}

async fn stream_until_complete_with_service_tier(
    client_session: &mut ModelClientSession,
    harness: &WebsocketTestHarness,
    prompt: &Prompt,
    service_tier: Option<ServiceTier>,
) {
    stream_until_complete_with_turn_metadata(
        client_session,
        harness,
        prompt,
        service_tier,
        /*turn_metadata_header*/ None,
    )
    .await;
}

async fn stream_until_complete_with_turn_metadata(
    client_session: &mut ModelClientSession,
    harness: &WebsocketTestHarness,
    prompt: &Prompt,
    service_tier: Option<ServiceTier>,
    turn_metadata_header: Option<&str>,
) {
    stream_until_complete_with_request_metadata(
        client_session,
        harness,
        prompt,
        service_tier,
        turn_metadata_header,
    )
    .await;
}

async fn stream_until_complete_with_request_metadata(
    client_session: &mut ModelClientSession,
    harness: &WebsocketTestHarness,
    prompt: &Prompt,
    service_tier: Option<ServiceTier>,
    turn_metadata_header: Option<&str>,
) {
    let mut stream = client_session
        .stream(
            prompt,
            &harness.model_info,
            &harness.session_telemetry,
            harness.effort,
            harness.summary,
            service_tier,
            turn_metadata_header,
        )
        .await
        .expect("websocket stream failed");

    while let Some(event) = stream.next().await {
        if matches!(event, Ok(ResponseEvent::Completed { .. })) {
            break;
        }
    }
}
