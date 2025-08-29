#![allow(clippy::unwrap_used)]

use codex_core::ConversationManager;
use codex_core::ModelProviderInfo;
use codex_core::built_in_model_providers;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_core::protocol_config_types::ReasoningEffort;
use codex_core::protocol_config_types::ReasoningSummary;
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

fn tools_include_web_search(body: &serde_json::Value) -> bool {
    body["tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["type"].as_str() == Some("web_search"))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn web_search_tool_present_after_override_turn_context() {
    // Mock server
    let server = MockServer::start().await;

    let sse = sse_completed("resp");
    let template = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse, "text/event-stream");

    // Expect one POST to /v1/responses
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(template)
        .expect(1)
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

    let conversation_manager =
        ConversationManager::with_auth(CodexAuth::from_api_key("Test API Key"));
    let codex = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation")
        .conversation;

    // Enable web search for subsequent turns
    codex
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: None,
            sandbox_policy: None,
            model: None,
            effort: None,
            summary: None,
            enable_web_search: Some(true),
        })
        .await
        .unwrap();

    // Trigger a user turn
    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "hello".into(),
            }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1, "expected one POST request");
    let body = requests[0].body_json::<serde_json::Value>().unwrap();
    assert!(
        tools_include_web_search(&body),
        "tools should include web_search when enable_web_search is true via OverrideTurnContext",
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn web_search_tool_present_in_user_turn_when_enabled() {
    // Mock server
    let server = MockServer::start().await;

    let sse = sse_completed("resp");
    let template = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse, "text/event-stream");

    // Expect one POST to /v1/responses
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(template)
        .expect(1)
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

    let conversation_manager =
        ConversationManager::with_auth(CodexAuth::from_api_key("Test API Key"));
    let codex = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation")
        .conversation;

    // Submit a per-turn override with enable_web_search = true
    codex
        .submit(Op::UserTurn {
            items: vec![InputItem::Text {
                text: "hello".into(),
            }],
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            model: "o3".to_string(),
            effort: ReasoningEffort::High,
            summary: ReasoningSummary::Detailed,
            enable_web_search: true,
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1, "expected one POST request");
    let body = requests[0].body_json::<serde_json::Value>().unwrap();
    assert!(
        tools_include_web_search(&body),
        "tools should include web_search when enable_web_search is true in UserTurn",
    );
}
