#![allow(clippy::unwrap_used)]

use codex_core::features::Feature;
use codex_protocol::config_types::WebSearchMode;
use core_test_support::load_sse_fixture_with_id;
use core_test_support::responses;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;

fn sse_completed(id: &str) -> String {
    load_sse_fixture_with_id("../fixtures/completed_template.json", id)
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
    let server = start_mock_server().await;
    let sse = sse_completed(model);
    let resp_mock = responses::mount_sse_once(&server, sse).await;

    let mut builder = test_codex()
        .with_model(model)
        // Keep tool expectations stable when the default web_search mode changes.
        .with_config(|config| {
            config.web_search_mode = Some(WebSearchMode::Cached);
            config.features.enable(Feature::CollaborationModes);
        });
    let test = builder
        .build(&server)
        .await
        .expect("create test Codex conversation");

    test.submit_turn("hello tools").await.expect("submit turn");

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
            "update_plan".to_string(),
            "request_user_input".to_string(),
            "web_search".to_string(),
            "view_image".to_string()
        ],
        "codex-mini-latest should expose the local shell tool",
    );

    let gpt5_codex_tools = collect_tool_identifiers_for_model("gpt-5-codex").await;
    assert_eq!(
        gpt5_codex_tools,
        vec![
            "shell_command".to_string(),
            "list_mcp_resources".to_string(),
            "list_mcp_resource_templates".to_string(),
            "read_mcp_resource".to_string(),
            "update_plan".to_string(),
            "request_user_input".to_string(),
            "apply_patch".to_string(),
            "web_search".to_string(),
            "view_image".to_string()
        ],
        "gpt-5-codex should expose the apply_patch tool",
    );

    let gpt51_codex_tools = collect_tool_identifiers_for_model("gpt-5.1-codex").await;
    assert_eq!(
        gpt51_codex_tools,
        vec![
            "shell_command".to_string(),
            "list_mcp_resources".to_string(),
            "list_mcp_resource_templates".to_string(),
            "read_mcp_resource".to_string(),
            "update_plan".to_string(),
            "request_user_input".to_string(),
            "apply_patch".to_string(),
            "web_search".to_string(),
            "view_image".to_string()
        ],
        "gpt-5.1-codex should expose the apply_patch tool",
    );

    let gpt5_tools = collect_tool_identifiers_for_model("gpt-5").await;
    assert_eq!(
        gpt5_tools,
        vec![
            "shell".to_string(),
            "list_mcp_resources".to_string(),
            "list_mcp_resource_templates".to_string(),
            "read_mcp_resource".to_string(),
            "update_plan".to_string(),
            "request_user_input".to_string(),
            "web_search".to_string(),
            "view_image".to_string()
        ],
        "gpt-5 should expose the apply_patch tool",
    );

    let gpt51_tools = collect_tool_identifiers_for_model("gpt-5.1").await;
    assert_eq!(
        gpt51_tools,
        vec![
            "shell_command".to_string(),
            "list_mcp_resources".to_string(),
            "list_mcp_resource_templates".to_string(),
            "read_mcp_resource".to_string(),
            "update_plan".to_string(),
            "request_user_input".to_string(),
            "apply_patch".to_string(),
            "web_search".to_string(),
            "view_image".to_string()
        ],
        "gpt-5.1 should expose the apply_patch tool",
    );
    let exp_tools = collect_tool_identifiers_for_model("exp-5.1").await;
    assert_eq!(
        exp_tools,
        vec![
            "exec_command".to_string(),
            "write_stdin".to_string(),
            "list_mcp_resources".to_string(),
            "list_mcp_resource_templates".to_string(),
            "read_mcp_resource".to_string(),
            "update_plan".to_string(),
            "request_user_input".to_string(),
            "apply_patch".to_string(),
            "web_search".to_string(),
            "view_image".to_string()
        ],
        "exp-5.1 should expose the apply_patch tool",
    );
}
