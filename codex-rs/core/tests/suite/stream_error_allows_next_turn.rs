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
    load_sse_fixture_with_id("../fixtures/completed_template.json", id)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn continue_after_stream_error() {
    skip_if_no_network!();

    let server = MockServer::start().await;

    let fail = ResponseTemplate::new(500)
        .insert_header("content-type", "application/json")
        .set_body_string(
            serde_json::json!({
                "error": {"type": "bad_request", "message": "synthetic client error"}
            })
            .to_string(),
        );

    // The provider below disables request retries (request_max_retries = 0),
    // so the failing request should only occur once.
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(body_string_contains("first message"))
        .respond_with(fail)
        .up_to_n_times(2)
        .mount(&server)
        .await;

    let ok = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse_completed("resp_ok2"), "text/event-stream");

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(body_string_contains("follow up"))
        .respond_with(ok)
        .expect(1)
        .mount(&server)
        .await;

    // Configure a provider that uses the Responses API and points at our mock
    // server. Use an existing env var (PATH) to satisfy the auth plumbing
    // without requiring a real secret.
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
        request_max_retries: Some(1),
        stream_max_retries: Some(1),
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

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "first message".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();

    // Expect an Error followed by TurnComplete so the session is released.
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::Error(_))).await;

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    // 2) Second turn: now send another prompt that should succeed using the
    // mock server SSE stream. If the agent failed to clear the running task on
    // error above, this submission would be rejected/queued indefinitely.
    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "follow up".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;
}
