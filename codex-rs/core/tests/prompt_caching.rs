#![allow(clippy::expect_used, clippy::unwrap_used)]

use codex_core::ConversationManager;
use codex_core::ModelProviderInfo;
use codex_core::built_in_model_providers;
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

/// Build minimal SSE stream with completed marker using the JSON fixture.
fn sse_completed(id: &str) -> String {
    load_sse_fixture_with_id("tests/fixtures/completed_template.json", id)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prefixes_context_and_instructions_once_and_consistently_across_requests() {
    #![allow(clippy::unwrap_used)]
    use pretty_assertions::assert_eq;

    let server = MockServer::start().await;

    let sse = sse_completed("resp");
    let template = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse, "text/event-stream");

    // Expect two POSTs to /v1/responses
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(template)
        .expect(2)
        .mount(&server)
        .await;

    let model_provider = ModelProviderInfo {
        base_url: Some(format!("{}/v1", server.uri())),
        ..built_in_model_providers()["openai"].clone()
    };

    let cwd = TempDir::new().unwrap();
    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home);
    config.cwd = cwd.path().to_path_buf();
    config.model_provider = model_provider;
    config.user_instructions = Some("be consistent and helpful".to_string());

    let conversation_manager = ConversationManager::default();
    let codex = conversation_manager
        .new_conversation_with_auth(config, Some(CodexAuth::from_api_key("Test API Key")))
        .await
        .expect("create new conversation")
        .conversation;

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "hello 1".into(),
            }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "hello 2".into(),
            }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2, "expected two POST requests");

    let expected_env_text = format!(
        "<environment_context>\nCurrent working directory: {}\nApproval policy: on-request\nSandbox mode: read-only\nNetwork access: restricted\n</environment_context>",
        cwd.path().to_string_lossy()
    );
    let expected_ui_text =
        "<user_instructions>\n\nbe consistent and helpful\n\n</user_instructions>";

    let expected_env_msg = serde_json::json!({
        "type": "message",
        "id": serde_json::Value::Null,
        "role": "user",
        "content": [ { "type": "input_text", "text": expected_env_text } ]
    });
    let expected_ui_msg = serde_json::json!({
        "type": "message",
        "id": serde_json::Value::Null,
        "role": "user",
        "content": [ { "type": "input_text", "text": expected_ui_text } ]
    });

    let expected_user_message_1 = serde_json::json!({
        "type": "message",
        "id": serde_json::Value::Null,
        "role": "user",
        "content": [ { "type": "input_text", "text": "hello 1" } ]
    });
    let body1 = requests[0].body_json::<serde_json::Value>().unwrap();
    assert_eq!(
        body1["input"],
        serde_json::json!([expected_ui_msg, expected_env_msg, expected_user_message_1])
    );

    let expected_user_message_2 = serde_json::json!({
        "type": "message",
        "id": serde_json::Value::Null,
        "role": "user",
        "content": [ { "type": "input_text", "text": "hello 2" } ]
    });
    let body2 = requests[1].body_json::<serde_json::Value>().unwrap();
    let expected_body2 = serde_json::json!(
        [
            body1["input"].as_array().unwrap().as_slice(),
            [expected_user_message_2].as_slice(),
        ]
        .concat()
    );
    assert_eq!(body2["input"], expected_body2);
}
