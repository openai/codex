#![allow(clippy::unwrap_used)]

use codex_core::WireApi;
use codex_core::built_in_model_providers;
use codex_core::features::Feature;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::WebSearchMode;
use core_test_support::responses;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::Value;

#[allow(clippy::expect_used)]
fn find_web_search_tool(body: &Value) -> &Value {
    body["tools"]
        .as_array()
        .expect("request body should include tools array")
        .iter()
        .find(|tool| tool.get("type").and_then(Value::as_str) == Some("web_search"))
        .expect("tools should include a web_search tool")
}

#[allow(clippy::expect_used)]
fn has_web_search_tool(body: &Value) -> bool {
    body["tools"]
        .as_array()
        .expect("request body should include tools array")
        .iter()
        .any(|tool| tool.get("type").and_then(Value::as_str) == Some("web_search"))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn web_search_mode_cached_sets_external_web_access_false() {
    skip_if_no_network!();

    let server = start_mock_server().await;
    let sse = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_completed("resp-1"),
    ]);
    let resp_mock = responses::mount_sse_once(&server, sse).await;

    let mut builder = test_codex()
        .with_model("gpt-5-codex")
        .with_config(|config| {
            config.web_search_mode = Some(WebSearchMode::Cached);
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
async fn web_search_mode_takes_precedence_over_legacy_flags() {
    skip_if_no_network!();

    let server = start_mock_server().await;
    let sse = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_completed("resp-1"),
    ]);
    let resp_mock = responses::mount_sse_once(&server, sse).await;

    let mut builder = test_codex()
        .with_model("gpt-5-codex")
        .with_config(|config| {
            config.features.enable(Feature::WebSearchRequest);
            config.web_search_mode = Some(WebSearchMode::Cached);
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn web_search_mode_defaults_to_cached_when_unset() {
    skip_if_no_network!();

    let server = start_mock_server().await;
    let sse = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_completed("resp-1"),
    ]);
    let resp_mock = responses::mount_sse_once(&server, sse).await;

    let mut builder = test_codex()
        .with_model("gpt-5-codex")
        .with_config(|config| {
            config.web_search_mode = None;
            config.features.disable(Feature::WebSearchCached);
            config.features.disable(Feature::WebSearchRequest);
        });
    let test = builder
        .build(&server)
        .await
        .expect("create test Codex conversation");

    test.submit_turn_with_policy("hello default cached web search", SandboxPolicy::ReadOnly)
        .await
        .expect("submit turn");

    let body = resp_mock.single_request().body_json();
    let tool = find_web_search_tool(&body);
    assert_eq!(
        tool.get("external_web_access").and_then(Value::as_bool),
        Some(false),
        "default web_search should be cached when unset"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn web_search_mode_updates_between_turns_with_sandbox_policy() {
    skip_if_no_network!();

    let server = start_mock_server().await;
    let resp_mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_completed("resp-1"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("resp-2"),
                responses::ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex()
        .with_model("gpt-5-codex")
        .with_config(|config| {
            config.web_search_mode = None;
            config.features.disable(Feature::WebSearchCached);
            config.features.disable(Feature::WebSearchRequest);
        });
    let test = builder
        .build(&server)
        .await
        .expect("create test Codex conversation");

    test.submit_turn_with_policy("hello cached", SandboxPolicy::ReadOnly)
        .await
        .expect("submit first turn");
    test.submit_turn_with_policy("hello live", SandboxPolicy::DangerFullAccess)
        .await
        .expect("submit second turn");

    let requests = resp_mock.requests();
    assert_eq!(requests.len(), 2, "expected two response requests");

    let first_body = requests[0].body_json();
    let first_tool = find_web_search_tool(&first_body);
    assert_eq!(
        first_tool
            .get("external_web_access")
            .and_then(Value::as_bool),
        Some(false),
        "read-only policy should default web_search to cached"
    );

    let second_body = requests[1].body_json();
    let second_tool = find_web_search_tool(&second_body);
    assert_eq!(
        second_tool
            .get("external_web_access")
            .and_then(Value::as_bool),
        Some(true),
        "danger-full-access policy should default web_search to live"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn web_search_mode_defaults_to_disabled_for_azure_responses() {
    skip_if_no_network!();

    let server = start_mock_server().await;
    let sse = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_completed("resp-1"),
    ]);
    let resp_mock = responses::mount_sse_once(&server, sse).await;

    let mut builder = test_codex()
        .with_model("gpt-5-codex")
        .with_config(|config| {
            let base_url = config.model_provider.base_url.clone();
            let mut provider = built_in_model_providers()["openai"].clone();
            provider.name = "Azure".to_string();
            provider.base_url = base_url;
            provider.wire_api = WireApi::Responses;
            config.model_provider_id = provider.name.clone();
            config.model_provider = provider;
            config.web_search_mode = None;
            config.features.disable(Feature::WebSearchCached);
            config.features.disable(Feature::WebSearchRequest);
        });
    let test = builder
        .build(&server)
        .await
        .expect("create test Codex conversation");

    test.submit_turn_with_policy(
        "hello azure default web search",
        SandboxPolicy::DangerFullAccess,
    )
    .await
    .expect("submit turn");

    let body = resp_mock.single_request().body_json();
    assert_eq!(
        has_web_search_tool(&body),
        false,
        "azure responses requests should disable web_search by default"
    );
}
