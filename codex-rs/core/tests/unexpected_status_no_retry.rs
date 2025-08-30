use std::time::Duration;

use codex_core::ConversationManager;
use codex_core::ModelProviderInfo;
use codex_core::WireApi;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use codex_login::CodexAuth;
use core_test_support::load_default_config_for_test;
use core_test_support::wait_for_event_with_timeout;
use serde_json::json;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fails_fast_on_unexpected_status() {
    if std::env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    let server = MockServer::start().await;

    let err_body = json!({
        "error": {"message": "bad request"}
    });
    let tmpl = ResponseTemplate::new(400)
        .insert_header("content-type", "application/json")
        .set_body_string(err_body.to_string());

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(tmpl)
        .expect(1)
        .mount(&server)
        .await;

    let provider = ModelProviderInfo {
        name: "openai".into(),
        base_url: Some(format!("{}/v1", server.uri())),
        env_key: Some("PATH".into()),
        env_key_instructions: None,
        wire_api: WireApi::Responses,
        query_params: None,
        http_headers: None,
        env_http_headers: None,
        request_max_retries: Some(0),
        stream_max_retries: Some(3),
        stream_idle_timeout_ms: Some(2000),
        requires_openai_auth: false,
    };

    let home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&home);
    config.model_provider = provider;

    let conversation_manager =
        ConversationManager::with_auth(CodexAuth::from_api_key("Test API Key"));
    let codex = conversation_manager
        .new_conversation(config)
        .await
        .unwrap()
        .conversation;

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();

    wait_for_event_with_timeout(
        &codex,
        |ev| matches!(ev, EventMsg::Error(_)),
        Duration::from_secs(5),
    )
    .await;
}
