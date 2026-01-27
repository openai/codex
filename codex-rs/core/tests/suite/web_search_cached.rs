#![allow(clippy::unwrap_used)]

use codex_core::features::Feature;
use codex_protocol::config_types::WebSearchMode;
use core_test_support::load_sse_fixture_with_id;
use core_test_support::responses;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::Value;

fn sse_completed(id: &str) -> String {
    load_sse_fixture_with_id("../fixtures/completed_template.json", id)
}

#[allow(clippy::expect_used)]
fn find_web_search_tool(body: &Value) -> &Value {
    body["tools"]
        .as_array()
        .expect("request body should include tools array")
        .iter()
        .find(|tool| tool.get("type").and_then(Value::as_str) == Some("web_search"))
        .expect("tools should include a web_search tool")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn web_search_mode_cached_sets_external_web_access_false_in_request_body() {
    skip_if_no_network!();

    let server = start_mock_server().await;
    let sse = sse_completed("resp-1");
    let resp_mock = responses::mount_sse_once(&server, sse).await;

    let mut builder = test_codex()
        .with_model("gpt-5-codex")
        .with_config(|config| {
            config.web_search_mode = WebSearchMode::Cached;
        });
    let test = builder
        .build(&server)
        .await
        .expect("create test Codex conversation");

    test.submit_turn("hello cached web search")
        .await
        .expect("submit turn");

    let body = resp_mock.single_request().body_json();
    let tool = find_web_search_tool(&body);
    assert_eq!(
        tool.get("external_web_access").and_then(Value::as_bool),
        Some(false),
        "web_search cached mode should force external_web_access=false"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn web_search_mode_takes_precedence_over_legacy_flags_in_request_body() {
    skip_if_no_network!();

    let server = start_mock_server().await;
    let sse = sse_completed("resp-1");
    let resp_mock = responses::mount_sse_once(&server, sse).await;

    let mut builder = test_codex()
        .with_model("gpt-5-codex")
        .with_config(|config| {
            config.features.enable(Feature::WebSearchRequest);
            config.web_search_mode = WebSearchMode::Cached;
        });
    let test = builder
        .build(&server)
        .await
        .expect("create test Codex conversation");

    test.submit_turn("hello cached+live flags")
        .await
        .expect("submit turn");

    let body = resp_mock.single_request().body_json();
    let tool = find_web_search_tool(&body);
    assert_eq!(
        tool.get("external_web_access").and_then(Value::as_bool),
        Some(false),
        "web_search mode should win over legacy web_search_request"
    );
}
