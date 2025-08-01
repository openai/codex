use std::path::PathBuf;

use chrono::Utc;
use codex_core::Codex;
use codex_core::CodexSpawnOk;
use codex_core::ModelProviderInfo;
use codex_core::built_in_model_providers;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use codex_core::protocol::SessionConfiguredEvent;
use codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use codex_login::AuthDotJson;
use codex_login::AuthMode;
use codex_login::CodexAuth;
use codex_login::TokenData;
use core_test_support::load_default_config_for_test;
use core_test_support::load_sse_fixture_with_id;
use core_test_support::wait_for_event;
use tempfile::TempDir;
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

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

    // Init session
    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home);
    config.model_provider = model_provider;

    let ctrl_c = std::sync::Arc::new(tokio::sync::Notify::new());
    let CodexSpawnOk { codex, .. } = Codex::spawn(
        config,
        Some(CodexAuth::from_api_key("Test API Key".to_string())),
        ctrl_c.clone(),
    )
    .await
    .unwrap();

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    let EventMsg::SessionConfigured(SessionConfiguredEvent { session_id, .. }) =
        wait_for_event(&codex, |ev| matches!(ev, EventMsg::SessionConfigured(_))).await
    else {
        unreachable!()
    };

    let current_session_id = Some(session_id.to_string());
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // get request from the server
    let request = &server.received_requests().await.unwrap()[0];
    let request_session_id = request.headers.get("session_id").unwrap();
    let request_originator = request.headers.get("originator").unwrap();
    let request_authorization = request.headers.get("authorization").unwrap();

    assert!(current_session_id.is_some());
    assert_eq!(
        request_session_id.to_str().unwrap(),
        current_session_id.as_ref().unwrap()
    );
    assert_eq!(request_originator.to_str().unwrap(), "codex_cli_rs");
    assert_eq!(
        request_authorization.to_str().unwrap(),
        "Bearer Test API Key"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn includes_base_instructions_override_in_request() {
    #![allow(clippy::unwrap_used)]

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

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };
    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home);

    config.base_instructions = Some("test instructions".to_string());
    config.model_provider = model_provider;

    let ctrl_c = std::sync::Arc::new(tokio::sync::Notify::new());
    let CodexSpawnOk { codex, .. } = Codex::spawn(
        config,
        Some(CodexAuth::from_api_key("Test API Key".to_string())),
        ctrl_c.clone(),
    )
    .await
    .unwrap();

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let request = &server.received_requests().await.unwrap()[0];
    let request_body = request.body_json::<serde_json::Value>().unwrap();

    assert!(
        request_body["instructions"]
            .as_str()
            .unwrap()
            .contains("test instructions")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn chatgpt_auth_sends_correct_request() {
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
        .and(path("/api/codex/responses"))
        .respond_with(first)
        .expect(1)
        .mount(&server)
        .await;

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/api/codex", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

    // Init session
    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home);
    config.model_provider = model_provider;
    let ctrl_c = std::sync::Arc::new(tokio::sync::Notify::new());
    let CodexSpawnOk { codex, .. } = Codex::spawn(
        config,
        Some(auth_from_token("Access Token".to_string())),
        ctrl_c.clone(),
    )
    .await
    .unwrap();

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    let EventMsg::SessionConfigured(SessionConfiguredEvent { session_id, .. }) =
        wait_for_event(&codex, |ev| matches!(ev, EventMsg::SessionConfigured(_))).await
    else {
        unreachable!()
    };

    let current_session_id = Some(session_id.to_string());
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // get request from the server
    let request = &server.received_requests().await.unwrap()[0];
    let request_session_id = request.headers.get("session_id").unwrap();
    let request_originator = request.headers.get("originator").unwrap();
    let request_authorization = request.headers.get("authorization").unwrap();
    let request_chatgpt_account_id = request.headers.get("chatgpt-account-id").unwrap();
    let request_body = request.body_json::<serde_json::Value>().unwrap();

    assert!(current_session_id.is_some());
    assert_eq!(
        request_session_id.to_str().unwrap(),
        current_session_id.as_ref().unwrap()
    );
    assert_eq!(request_originator.to_str().unwrap(), "codex_cli_rs");
    assert_eq!(
        request_authorization.to_str().unwrap(),
        "Bearer Access Token"
    );
    assert_eq!(request_chatgpt_account_id.to_str().unwrap(), "account_id");
    assert!(!request_body["store"].as_bool().unwrap());
    assert!(request_body["stream"].as_bool().unwrap());
    assert_eq!(
        request_body["include"][0].as_str().unwrap(),
        "reasoning.encrypted_content"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn includes_user_instructions_message_in_request() {
    #![allow(clippy::unwrap_used)]

    let server = MockServer::start().await;

    let first = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse_completed("resp1"), "text/event-stream");

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(first)
        .expect(1)
        .mount(&server)
        .await;

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home);
    config.model_provider = model_provider;
    config.user_instructions = Some("be nice".to_string());

    let ctrl_c = std::sync::Arc::new(tokio::sync::Notify::new());
    let CodexSpawnOk { codex, .. } = Codex::spawn(
        config,
        Some(CodexAuth::from_api_key("Test API Key".to_string())),
        ctrl_c.clone(),
    )
    .await
    .unwrap();

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let request = &server.received_requests().await.unwrap()[0];
    let request_body = request.body_json::<serde_json::Value>().unwrap();

    assert!(
        !request_body["instructions"]
            .as_str()
            .unwrap()
            .contains("be nice")
    );
    assert_eq!(request_body["input"][0]["role"], "user");
    assert!(
        request_body["input"][0]["content"][0]["text"]
            .as_str()
            .unwrap()
            .starts_with("be nice")
    );
}
fn auth_from_token(id_token: String) -> CodexAuth {
    CodexAuth::new(
        None,
        AuthMode::ChatGPT,
        PathBuf::new(),
        Some(AuthDotJson {
            openai_api_key: None,
            tokens: Some(TokenData {
                id_token,
                access_token: "Access Token".to_string(),
                refresh_token: "test".to_string(),
                account_id: Some("account_id".to_string()),
            }),
            last_refresh: Some(Utc::now()),
        }),
    )
}
