use std::time::Duration;

use codex_core::Codex;
use codex_core::ModelProviderInfo;
use codex_core::exec::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use codex_core::protocol::SessionConfiguredEvent;
mod test_support;
use tempfile::TempDir;
use test_support::load_default_config_for_test;
use test_support::load_sse_fixture_with_id;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

/// Build minimal SSE stream with completed marker using the JSON fixture.
fn sse_completed(id: &str) -> String {
    load_sse_fixture_with_id("tests/fixtures/completed_template.json", id)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn includes_session_id_and_model_headers_in_request() {
    #![allow(clippy::unwrap_used)]

    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    // Mock server
    let server = MockServer::start().await;

    // First request – must NOT include `previous_response_id`.
    let first = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse_completed("resp1"), "text/event-stream");

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(first)
        .expect(1)
        .mount(&server)
        .await;

    // Environment
    // Update environment – `set_var` is `unsafe` starting with the 2024
    // edition so we group the calls into a single `unsafe { … }` block.
    unsafe {
        std::env::set_var("OPENAI_REQUEST_MAX_RETRIES", "0");
        std::env::set_var("OPENAI_STREAM_MAX_RETRIES", "0");
    }
    let model_provider = ModelProviderInfo {
        name: "openai".into(),
        base_url: format!("{}/v1", server.uri()),
        // Environment variable that should exist in the test environment.
        // ModelClient will return an error if the environment variable for the
        // provider is not set.
        env_key: Some("PATH".into()),
        env_key_instructions: None,
        wire_api: codex_core::WireApi::Responses,
        query_params: None,
        http_headers: Some(
            [("originator".to_string(), "codex_cli_rs".to_string())]
                .into_iter()
                .collect(),
        ),
        env_http_headers: None,
    };

    // Init session
    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home);
    config.model_provider = model_provider;
    let ctrl_c = std::sync::Arc::new(tokio::sync::Notify::new());
    let (codex, _init_id) = Codex::spawn(config, ctrl_c.clone()).await.unwrap();

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    let mut current_session_id = None;
    // Wait for TaskComplete
    loop {
        let ev = timeout(Duration::from_secs(1), codex.next_event())
            .await
            .unwrap()
            .unwrap();

        if let EventMsg::SessionConfigured(SessionConfiguredEvent { session_id, .. }) = ev.msg {
            current_session_id = Some(session_id.to_string());
        }
        if matches!(ev.msg, EventMsg::TaskComplete(_)) {
            break;
        }
    }

    // get request from the server
    let request = &server.received_requests().await.unwrap()[0];
    let request_body = request.headers.get("session_id").unwrap();
    let originator = request.headers.get("originator").unwrap();

    assert!(current_session_id.is_some());
    assert_eq!(request_body.to_str().unwrap(), &current_session_id.unwrap());
    assert_eq!(originator.to_str().unwrap(), "codex_cli_rs");
}
