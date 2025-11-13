#![allow(clippy::unwrap_used)]

use codex_core::CodexAuth;
use codex_core::ConversationManager;
use codex_core::ModelProviderInfo;
use codex_core::built_in_model_providers;
use codex_core::features::Feature;
use codex_core::model_family::find_family_for_model;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::load_default_config_for_test;
use core_test_support::load_sse_fixture_with_id;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::wait_for_event;
use tempfile::TempDir;
use wiremock::MockServer;

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
    let resp_mock = responses::mount_sse_once_match(&server, wiremock::matchers::any(), sse).await;

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
    config.features.disable(Feature::ApplyPatchFreeform);
    config.features.disable(Feature::ViewImageTool);
    config.features.disable(Feature::WebSearchRequest);
    config.features.disable(Feature::UnifiedExec);

    let conversation_manager =
        ConversationManager::with_auth(CodexAuth::from_api_key("Test API Key"));
    let codex = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation")
        .conversation;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello tools".into(),
            }],
        })
        .await
        .unwrap();
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let body = resp_mock.single_request().body_json();
    tool_identifiers(&body)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_selects_expected_tools() {
    skip_if_no_network!();
    use pretty_assertions::assert_eq;

    let codex_tools = collect_tool_identifiers_for_model("codex-mini-latest").await;
    assert_eq!(
        codex_tools,
        vec![
            "local_shell".to_string(),
            "list_mcp_resources".to_string(),
            "list_mcp_resource_templates".to_string(),
            "read_mcp_resource".to_string(),
            "update_plan".to_string()
        ],
        "codex-mini-latest should expose the local shell tool",
    );

    let o3_tools = collect_tool_identifiers_for_model("o3").await;
    assert_eq!(
        o3_tools,
        vec![
            "shell".to_string(),
            "list_mcp_resources".to_string(),
            "list_mcp_resource_templates".to_string(),
            "read_mcp_resource".to_string(),
            "update_plan".to_string()
        ],
        "o3 should expose the generic shell tool",
    );

    let gpt5_codex_tools = collect_tool_identifiers_for_model("gpt-5-codex").await;
    assert_eq!(
        gpt5_codex_tools,
        vec![
            "shell".to_string(),
            "list_mcp_resources".to_string(),
            "list_mcp_resource_templates".to_string(),
            "read_mcp_resource".to_string(),
            "update_plan".to_string(),
            "apply_patch".to_string()
        ],
        "gpt-5-codex should expose the apply_patch tool",
    );

    let gpt51_codex_tools = collect_tool_identifiers_for_model("gpt-5.1-codex").await;
    assert_eq!(
        gpt51_codex_tools,
        vec![
            "shell".to_string(),
            "list_mcp_resources".to_string(),
            "list_mcp_resource_templates".to_string(),
            "read_mcp_resource".to_string(),
            "update_plan".to_string(),
            "apply_patch".to_string()
        ],
        "gpt-5.1-codex should expose the apply_patch tool",
    );

    let gpt51_tools = collect_tool_identifiers_for_model("gpt-5.1").await;
    assert_eq!(
        gpt51_tools,
        vec![
            "shell".to_string(),
            "list_mcp_resources".to_string(),
            "list_mcp_resource_templates".to_string(),
            "read_mcp_resource".to_string(),
            "update_plan".to_string(),
            "apply_patch".to_string()
        ],
        "gpt-5.1 should expose the apply_patch tool",
    );
}
