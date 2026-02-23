#![allow(clippy::unwrap_used)]

use codex_core::features::Feature;
use codex_protocol::config_types::WebSearchMode;
use core_test_support::responses;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;

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
    let sse = responses::sse(vec![
        responses::ev_response_created(model),
        responses::ev_completed(model),
    ]);
    let resp_mock = responses::mount_sse_once(&server, sse).await;

    let mut builder = test_codex()
        .with_model(model)
        // Keep tool expectations stable when the default web_search mode changes.
        .with_config(|config| {
            config
                .web_search_mode
                .set(WebSearchMode::Cached)
                .expect("test web_search_mode should satisfy constraints");
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

fn expected_default_tools(shell_tool: &str, tail: &[&str]) -> Vec<String> {
    let mut tools = if cfg!(windows) {
        vec![shell_tool.to_string()]
    } else {
        vec!["exec_command".to_string(), "write_stdin".to_string()]
    };
    tools.extend(tail.iter().map(|tool| (*tool).to_string()));
    tools
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_selects_expected_tools() {
    skip_if_no_network!();
    use pretty_assertions::assert_eq;

    let gpt51_codex_max_tools = collect_tool_identifiers_for_model("gpt-5.1-codex-max").await;
    assert_eq!(
        gpt51_codex_max_tools,
        expected_default_tools(
            "shell_command",
            &[
                "update_plan",
                "request_user_input",
                "apply_patch",
                "web_search",
                "view_image",
            ],
        ),
        "gpt-5.1-codex-max should expose the apply_patch tool",
    );

    let gpt5_codex_tools = collect_tool_identifiers_for_model("gpt-5-codex").await;
    assert_eq!(
        gpt5_codex_tools,
        expected_default_tools(
            "shell_command",
            &[
                "update_plan",
                "request_user_input",
                "apply_patch",
                "web_search",
                "view_image",
            ],
        ),
        "gpt-5-codex should expose the apply_patch tool",
    );

    let gpt51_codex_tools = collect_tool_identifiers_for_model("gpt-5.1-codex").await;
    assert_eq!(
        gpt51_codex_tools,
        expected_default_tools(
            "shell_command",
            &[
                "update_plan",
                "request_user_input",
                "apply_patch",
                "web_search",
                "view_image",
            ],
        ),
        "gpt-5.1-codex should expose the apply_patch tool",
    );

    let gpt5_tools = collect_tool_identifiers_for_model("gpt-5").await;
    assert_eq!(
        gpt5_tools,
        expected_default_tools(
            "shell",
            &[
                "update_plan",
                "request_user_input",
                "web_search",
                "view_image",
            ],
        ),
        "gpt-5 should expose the apply_patch tool",
    );

    let gpt51_tools = collect_tool_identifiers_for_model("gpt-5.1").await;
    assert_eq!(
        gpt51_tools,
        expected_default_tools(
            "shell_command",
            &[
                "update_plan",
                "request_user_input",
                "apply_patch",
                "web_search",
                "view_image",
            ],
        ),
        "gpt-5.1 should expose the apply_patch tool",
    );
}
