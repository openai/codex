#![allow(clippy::unwrap_used)]

use codex_core::CodexAuth;
use codex_core::ConversationManager;
use codex_core::ModelProviderInfo;
use codex_core::built_in_model_providers;
use codex_core::model_family::find_family_for_model;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use core_test_support::load_default_config_for_test;
use core_test_support::load_sse_fixture_with_id;
use core_test_support::skip_if_no_network;
use core_test_support::wait_for_event;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

fn sse_completed(id: &str) -> String {
    load_sse_fixture_with_id("tests/fixtures/completed_template.json", id)
}

#[allow(clippy::expect_used)]
fn tool_identifiers(body: &serde_json::Value) -> Vec<String> {
    body["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| {
            tool.get("name")
                .and_then(|v| v.as_str())
                .or_else(|| tool.get("type").and_then(|v| v.as_str()))
                .map(std::string::ToString::to_string)
                .expect("tool should have either name or type")
        })
        .collect()
}

#[allow(clippy::expect_used)]
async fn collect_tool_identifiers_for_model(model: &str) -> Vec<String> {
    let server = MockServer::start().await;

    let sse = sse_completed(model);
    let template = ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(sse, "text/event-stream");

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
    config.model = model.to_string();
    config.model_family =
        find_family_for_model(model).unwrap_or_else(|| panic!("unknown model family for {model}"));
    config.include_plan_tool = false;
    config.include_apply_patch_tool = false;
    config.include_view_image_tool = false;
    config.tools_web_search_request = false;
    config.use_experimental_streamable_shell_tool = false;
    config.use_experimental_unified_exec_tool = false;

    let conversation_manager =
        ConversationManager::with_auth(CodexAuth::from_api_key("Test API Key"));
    let codex = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation")
        .conversation;

    codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "hello tools".into(),
            }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests.len(),
        1,
        "expected a single request for model {model}"
    );
    let body = requests[0].body_json::<serde_json::Value>().unwrap();
    tool_identifiers(&body)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_selects_expected_tools() {
    skip_if_no_network!();
    use pretty_assertions::assert_eq;

    let codex_tools = collect_tool_identifiers_for_model("codex-mini-latest").await;
    assert_eq!(
        codex_tools,
        vec!["local_shell".to_string(), "read_file".to_string()],
        "codex-mini-latest should expose the local shell tool",
    );

    let o3_tools = collect_tool_identifiers_for_model("o3").await;
    assert_eq!(
        o3_tools,
        vec!["shell".to_string(), "read_file".to_string()],
        "o3 should expose the generic shell tool",
    );
}
