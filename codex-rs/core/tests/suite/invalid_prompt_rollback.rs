use codex_core::ModelProviderInfo;
use codex_core::WireApi;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::load_sse_fixture_with_id;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_string_contains;
use wiremock::matchers::method;
use wiremock::matchers::path;

fn sse_completed(id: &str) -> String {
    load_sse_fixture_with_id("tests/fixtures/completed_template.json", id)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_prompt_triggers_rollback_and_retry() {
    skip_if_no_network!();

    let server = MockServer::start().await;

    // Mount mocks in order of specificity (most specific first)

    // Third request (after rollback with error message) succeeds
    let ok2 = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse_completed("resp_ok2"), "text/event-stream");

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(body_string_contains("API rejected the previous request"))
        .respond_with(ok2)
        .up_to_n_times(1)
        .mount(&server)
        .await;

    // Second request (with "follow up" but NOT error message) fails with 400
    let bad_request = ResponseTemplate::new(400)
        .insert_header("content-type", "application/json")
        .set_body_string(
            serde_json::json!({
                "error": {"type": "invalid_request_error", "message": "Invalid content in request"}
            })
            .to_string(),
        );

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(body_string_contains("follow up"))
        .respond_with(bad_request)
        .up_to_n_times(1)
        .mount(&server)
        .await;

    // First request succeeds
    let ok1 = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse_completed("resp_ok1"), "text/event-stream");

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(body_string_contains("initial message"))
        .respond_with(ok1)
        .up_to_n_times(1)
        .mount(&server)
        .await;

    let provider = ModelProviderInfo {
        name: "mock-openai".into(),
        base_url: Some(format!("{}/v1", server.uri())),
        env_key: Some("PATH".into()),
        env_key_instructions: None,
        experimental_bearer_token: None,
        wire_api: WireApi::Responses,
        query_params: None,
        http_headers: None,
        env_http_headers: None,
        request_max_retries: Some(0),
        stream_max_retries: Some(0),
        stream_idle_timeout_ms: Some(2_000),
        requires_openai_auth: false,
    };

    let TestCodex { codex, .. } = test_codex()
        .with_config(move |config| {
            config.base_instructions = Some("You are a helpful assistant".to_string());
            config.model_provider = provider;
        })
        .build(&server)
        .await
        .unwrap();

    // First turn succeeds
    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "initial message".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // Second turn gets 400, triggers rollback, and retries
    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "follow up".into(),
            }],
        })
        .await
        .unwrap();

    // Should complete successfully after rollback and retry
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_prompt_without_previous_success_reports_error() {
    skip_if_no_network!();

    let server = MockServer::start().await;

    // First request fails with 400 (no previous success to rollback to)
    let bad_request = ResponseTemplate::new(400)
        .insert_header("content-type", "application/json")
        .set_body_string(
            serde_json::json!({
                "error": {"type": "invalid_request_error", "message": "Invalid content in first request"}
            })
            .to_string(),
        );

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(body_string_contains("initial message"))
        .respond_with(bad_request)
        .expect(1)
        .mount(&server)
        .await;

    let provider = ModelProviderInfo {
        name: "mock-openai".into(),
        base_url: Some(format!("{}/v1", server.uri())),
        env_key: Some("PATH".into()),
        env_key_instructions: None,
        experimental_bearer_token: None,
        wire_api: WireApi::Responses,
        query_params: None,
        http_headers: None,
        env_http_headers: None,
        request_max_retries: Some(0),
        stream_max_retries: Some(0),
        stream_idle_timeout_ms: Some(2_000),
        requires_openai_auth: false,
    };

    let TestCodex { codex, .. } = test_codex()
        .with_config(move |config| {
            config.base_instructions = Some("You are a helpful assistant".to_string());
            config.model_provider = provider;
        })
        .build(&server)
        .await
        .unwrap();

    // First turn gets 400 with no previous success
    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "initial message".into(),
            }],
        })
        .await
        .unwrap();

    // Should report error and complete (no rollback possible)
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::Error(_))).await;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_prompt_consecutive_400_errors() {
    skip_if_no_network!();

    let server = MockServer::start().await;

    // First request succeeds
    let ok1 = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse_completed("resp_ok1"), "text/event-stream");

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(body_string_contains("initial message"))
        .respond_with(ok1)
        .up_to_n_times(1)
        .mount(&server)
        .await;

    // All subsequent requests return 400
    let bad_request = ResponseTemplate::new(400)
        .insert_header("content-type", "application/json")
        .set_body_string(
            serde_json::json!({
                "error": {"type": "invalid_request_error", "message": "Invalid content persists"}
            })
            .to_string(),
        );

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(bad_request)
        .mount(&server)
        .await;

    let provider = ModelProviderInfo {
        name: "mock-openai".into(),
        base_url: Some(format!("{}/v1", server.uri())),
        env_key: Some("PATH".into()),
        env_key_instructions: None,
        experimental_bearer_token: None,
        wire_api: WireApi::Responses,
        query_params: None,
        http_headers: None,
        env_http_headers: None,
        request_max_retries: Some(0),
        stream_max_retries: Some(0),
        stream_idle_timeout_ms: Some(2_000),
        requires_openai_auth: false,
    };

    let TestCodex { codex, .. } = test_codex()
        .with_config(move |config| {
            config.base_instructions = Some("You are a helpful assistant".to_string());
            config.model_provider = provider;
        })
        .build(&server)
        .await
        .unwrap();

    // First turn succeeds
    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "initial message".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // Second turn: First attempt gets 400, rollback happens, retry also gets 400
    // Should eventually error out after rollback attempt
    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "follow up".into(),
            }],
        })
        .await
        .unwrap();

    // Should get error (after rollback attempt also fails)
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::Error(_))).await;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_prompt_verifies_recovery_message_content() {
    skip_if_no_network!();

    let server = MockServer::start().await;

    // First request succeeds
    let ok1 = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse_completed("resp_ok1"), "text/event-stream");

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(body_string_contains("initial message"))
        .respond_with(ok1)
        .up_to_n_times(1)
        .mount(&server)
        .await;

    // Second request (with follow up) fails with specific error
    let bad_request = ResponseTemplate::new(400)
        .insert_header("content-type", "application/json")
        .set_body_string(
            serde_json::json!({
                "error": {
                    "type": "invalid_request_error",
                    "message": "messages.3.content.0.source.media_type: Expected a supported media type"
                }
            })
            .to_string(),
        );

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(body_string_contains("follow up"))
        .respond_with(bad_request)
        .up_to_n_times(1)
        .mount(&server)
        .await;

    // Third request (after rollback with recovery message) succeeds
    let ok2 = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse_completed("resp_ok2"), "text/event-stream");

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(body_string_contains("API rejected the previous request"))
        .and(body_string_contains("media_type"))
        .respond_with(ok2)
        .up_to_n_times(1)
        .mount(&server)
        .await;

    let provider = ModelProviderInfo {
        name: "mock-openai".into(),
        base_url: Some(format!("{}/v1", server.uri())),
        env_key: Some("PATH".into()),
        env_key_instructions: None,
        experimental_bearer_token: None,
        wire_api: WireApi::Responses,
        query_params: None,
        http_headers: None,
        env_http_headers: None,
        request_max_retries: Some(0),
        stream_max_retries: Some(0),
        stream_idle_timeout_ms: Some(2_000),
        requires_openai_auth: false,
    };

    let TestCodex { codex, .. } = test_codex()
        .with_config(move |config| {
            config.base_instructions = Some("You are a helpful assistant".to_string());
            config.model_provider = provider;
        })
        .build(&server)
        .await
        .unwrap();

    // First turn succeeds
    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "initial message".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // Second turn: 400 → rollback → retry with recovery message containing original error
    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "follow up".into(),
            }],
        })
        .await
        .unwrap();

    // Should complete successfully
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;
}
