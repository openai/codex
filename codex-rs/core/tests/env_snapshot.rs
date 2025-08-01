#![allow(clippy::expect_used, clippy::unwrap_used)]

use codex_core::Codex;
use codex_core::CodexSpawnOk;
use codex_core::ModelProviderInfo;
use codex_core::built_in_model_providers;
use codex_core::exec::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use codex_login::CodexAuth;
use core_test_support::load_default_config_for_test;
use core_test_support::load_sse_fixture_with_id;
use core_test_support::wait_for_event;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn first_turn_includes_environment_snapshot() {
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    // Create a temporary working directory with a few files (including a hidden one).
    let cwd = TempDir::new().unwrap();
    std::fs::write(cwd.path().join("a.txt"), b"x").unwrap();
    std::fs::write(cwd.path().join("b.txt"), b"x").unwrap();
    std::fs::write(cwd.path().join(".hidden"), b"x").unwrap();

    // Mock Responses API server that immediately completes the turn.
    let server = MockServer::start().await;
    let sse = load_sse_fixture_with_id("tests/fixtures/completed_template.json", "resp1");
    let first = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse, "text/event-stream");
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(first)
        .mount(&server)
        .await;

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

    // Initialize session using the temp cwd and the mock provider.
    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home);
    config.model_provider = model_provider;
    config.cwd = cwd.path().to_path_buf();

    let ctrl_c = std::sync::Arc::new(tokio::sync::Notify::new());
    let CodexSpawnOk { codex, .. } = Codex::spawn(
        config,
        Some(CodexAuth::from_api_key("Test API Key".to_string())),
        ctrl_c.clone(),
    )
    .await
    .unwrap();

    // Submit a simple user message – the agent should inject the environment snapshot as
    // an additional content item at the start of the first user message.
    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    // Wait for the task to complete so the request is dispatched.
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // Read the captured request and verify the first message content includes the snapshot.
    let request = &server.received_requests().await.unwrap()[0];
    let body = request.body_json::<serde_json::Value>().unwrap();

    // We expect the first (and only) input item to be a user message with multiple content entries.
    let first_input = &body["input"][0];
    assert_eq!(first_input["role"], "user");

    // The first content item should be the injected environment snapshot.
    let first_text = first_input["content"][0]["text"].as_str().unwrap();
    assert!(first_text.starts_with("Environment snapshot (output of `ls | head -n 50` in cwd):"));
    // It should reference the cwd and include visible files, but not hidden ones.
    assert!(first_text.contains(&cwd.path().display().to_string()));
    assert!(first_text.contains("a.txt"));
    assert!(first_text.contains("b.txt"));
    assert!(!first_text.contains(".hidden"));

    // The user's original message should appear in the second content item.
    let second_text = first_input["content"][1]["text"].as_str().unwrap();
    assert_eq!(second_text, "hello");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn snapshot_is_not_injected_on_second_turn() {
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    // Prepare cwd with a couple of files (and a hidden one).
    let cwd = TempDir::new().unwrap();
    std::fs::write(cwd.path().join("first.txt"), b"x").unwrap();
    std::fs::write(cwd.path().join("second.txt"), b"x").unwrap();
    std::fs::write(cwd.path().join(".dot"), b"x").unwrap();

    // Mock server that accepts two requests and completes both.
    let server = MockServer::start().await;
    let sse = load_sse_fixture_with_id("tests/fixtures/completed_template.json", "resp1");
    let responder = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse, "text/event-stream");
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(responder)
        .expect(2)
        .mount(&server)
        .await;

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home);
    config.model_provider = model_provider;
    config.cwd = cwd.path().to_path_buf();

    let ctrl_c = std::sync::Arc::new(tokio::sync::Notify::new());
    let CodexSpawnOk { codex, .. } = Codex::spawn(
        config,
        Some(CodexAuth::from_api_key("Test API Key".to_string())),
        ctrl_c.clone(),
    )
    .await
    .unwrap();

    // First user message.
    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "first".into(),
            }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // Second user message.
    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "second".into(),
            }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // Verify the second request's last user message does not include the environment snapshot.
    let requests = server.received_requests().await.unwrap();
    assert!(
        requests.len() >= 2,
        "expected two requests to the mock server"
    );
    let second_req = &requests[1];
    let body = second_req.body_json::<serde_json::Value>().unwrap();

    let input = body["input"].as_array().expect("input array");
    let last = input.last().expect("at least one input item");
    assert_eq!(last["role"], "user");
    let last_text = last["content"][0]["text"].as_str().unwrap();
    // Should be exactly the submitted text, without the snapshot header prefix.
    assert_eq!(last_text, "second");
}
