#![cfg(not(target_os = "windows"))]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use anyhow::Result;
use codex_config::types::ToolSuggestDiscoverable;
use codex_config::types::ToolSuggestDiscoverableType;
use codex_core::config::Config;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_models_manager::bundled_models_response;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use serde_json::Value;

const TOOL_SEARCH_TOOL_NAME: &str = "tool_search";
const TOOL_SUGGEST_TOOL_NAME: &str = "tool_suggest";
const DISCOVERABLE_GMAIL_ID: &str = "connector_68df038e0ba48191908c8434991bbac2";

fn tool_names(body: &Value) -> Vec<String> {
    body.get("tools")
        .and_then(Value::as_array)
        .map(|tools| {
            tools
                .iter()
                .filter_map(|tool| {
                    tool.get("name")
                        .or_else(|| tool.get("type"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn function_tool_description(body: &Value, name: &str) -> Option<String> {
    body.get("tools")
        .and_then(Value::as_array)
        .and_then(|tools| {
            tools.iter().find_map(|tool| {
                if tool.get("name").and_then(Value::as_str) == Some(name) {
                    tool.get("description")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                } else {
                    None
                }
            })
        })
}

fn configure_apps_without_search_tool(config: &mut Config, apps_base_url: &str) {
    config
        .features
        .enable(Feature::Apps)
        .expect("test config should allow feature update");
    config
        .features
        .enable(Feature::Plugins)
        .expect("test config should allow feature update");
    config
        .features
        .enable(Feature::ToolSuggest)
        .expect("test config should allow feature update");
    let mut model_catalog = bundled_models_response()
        .unwrap_or_else(|err| panic!("bundled models.json should parse: {err}"));
    let model = model_catalog
        .models
        .iter_mut()
        .find(|model| model.slug == "gpt-5.4")
        .expect("gpt-5.4 exists in bundled models.json");
    config.chatgpt_base_url = apps_base_url.to_string();
    config.model = Some("gpt-5.4".to_string());
    config.tool_suggest.discoverables = vec![ToolSuggestDiscoverable {
        kind: ToolSuggestDiscoverableType::Connector,
        id: DISCOVERABLE_GMAIL_ID.to_string(),
    }];
    model.supports_search_tool = false;
    config.model_catalog = Some(model_catalog);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tool_suggest_is_available_without_search_tool_after_discovery_attempts() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    let mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(move |config| {
            configure_apps_without_search_tool(config, apps_server.chatgpt_base_url.as_str())
        });
    let test = builder.build(&server).await?;

    test.submit_turn_with_approval_and_permission_profile(
        "list tools",
        AskForApproval::Never,
        PermissionProfile::Disabled,
    )
    .await?;

    let body = mock.single_request().body_json();
    let tools = tool_names(&body);
    assert!(
        !tools.iter().any(|name| name == TOOL_SEARCH_TOOL_NAME),
        "tools list should not include {TOOL_SEARCH_TOOL_NAME}: {tools:?}"
    );
    assert!(
        tools.iter().any(|name| name == TOOL_SUGGEST_TOOL_NAME),
        "tools list should include {TOOL_SUGGEST_TOOL_NAME}: {tools:?}"
    );

    let description =
        function_tool_description(&body, TOOL_SUGGEST_TOOL_NAME).expect("description");
    assert!(description.contains(
        "Use this tool only to ask the user to install one known plugin or connector from the list below"
    ));
    assert!(description.contains(
        "`tool_search` is not available, or it has already been called and did not find or make the requested tool callable."
    ));
    assert!(description.contains("IMPORTANT: DO NOT call this tool in parallel with other tools."));
    assert!(!description.contains("tool_search fails to find a good match"));

    Ok(())
}
