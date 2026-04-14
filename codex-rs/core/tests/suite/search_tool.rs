#![cfg(not(target_os = "windows"))]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use anyhow::Result;
use codex_config::types::McpServerConfig;
use codex_config::types::McpServerTransportConfig;
use codex_core::config::Config;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_models_manager::bundled_models_response;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::McpInvocation;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::user_input::UserInput;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::CALENDAR_CREATE_EVENT_RESOURCE_URI;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::ev_tool_search_call;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::stdio_server_bin;
use core_test_support::test_codex::TestCodexBuilder;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::collections::HashMap;
use std::time::Duration;

const SEARCH_TOOL_DESCRIPTION_SNIPPETS: [&str; 2] = [
    "You have access to tools from the following MCP servers/connectors",
    "- Calendar: Plan events and manage your calendar.",
];
const TOOL_SEARCH_TOOL_NAME: &str = "tool_search";
const CALENDAR_CREATE_TOOL: &str = "mcp__codex_apps__calendar_create_event";
const CALENDAR_LIST_TOOL: &str = "mcp__codex_apps__calendar_list_events";
const SEARCH_CALENDAR_NAMESPACE: &str = "mcp__codex_apps__calendar";
const SEARCH_CALENDAR_CREATE_TOOL: &str = "_create_event";

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

fn tool_search_description(body: &Value) -> Option<String> {
    body.get("tools")
        .and_then(Value::as_array)
        .and_then(|tools| {
            tools.iter().find_map(|tool| {
                if tool.get("type").and_then(Value::as_str) == Some(TOOL_SEARCH_TOOL_NAME) {
                    tool.get("description")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                } else {
                    None
                }
            })
        })
}

fn tool_search_output_item(request: &ResponsesRequest, call_id: &str) -> Value {
    request.tool_search_output(call_id)
}

fn tool_search_output_tools(request: &ResponsesRequest, call_id: &str) -> Vec<Value> {
    tool_search_output_item(request, call_id)
        .get("tools")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn configure_search_capable_apps(config: &mut Config, apps_base_url: &str) {
    config
        .features
        .enable(Feature::Apps)
        .expect("test config should allow feature update");
    config.chatgpt_base_url = apps_base_url.to_string();
    config.model = Some("gpt-5-codex".to_string());

    let mut model_catalog = bundled_models_response()
        .unwrap_or_else(|err| panic!("bundled models.json should parse: {err}"));
    let model = model_catalog
        .models
        .iter_mut()
        .find(|model| model.slug == "gpt-5-codex")
        .expect("gpt-5-codex exists in bundled models.json");
    model.supports_search_tool = true;
    config.model_catalog = Some(model_catalog);
}

fn configure_apps_without_tool_search(config: &mut Config, apps_base_url: &str) {
    configure_search_capable_apps(config, apps_base_url);
    config
        .features
        .disable(Feature::ToolSearch)
        .expect("test config should allow feature update");
}

fn configure_apps(config: &mut Config, apps_base_url: &str) {
    configure_search_capable_apps(config, apps_base_url);
}

fn configured_builder(apps_base_url: String) -> TestCodexBuilder {
    test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(move |config| configure_apps(config, apps_base_url.as_str()))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_tool_enabled_by_default_adds_tool_search() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount_searchable(&server).await?;
    let mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let mut builder = configured_builder(apps_server.chatgpt_base_url.clone());
    let test = builder.build(&server).await?;

    test.submit_turn_with_policies(
        "list tools",
        AskForApproval::Never,
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let body = mock.single_request().body_json();
    let tools = body
        .get("tools")
        .and_then(Value::as_array)
        .expect("tools array should exist");
    let tool_search = tools
        .iter()
        .find(|tool| tool.get("type").and_then(Value::as_str) == Some(TOOL_SEARCH_TOOL_NAME))
        .cloned()
        .expect("tool_search should be present");

    assert_eq!(
        tool_search,
        json!({
            "type": "tool_search",
            "execution": "client",
            "description": tool_search["description"].as_str().expect("description should exist"),
            "parameters": {
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Search query for MCP tools."},
                    "limit": {"type": "number", "description": "Maximum number of tools to return (defaults to 8)."},
                },
                "required": ["query"],
                "additionalProperties": false,
            }
        })
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tool_search_disabled_exposes_apps_tools_directly() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount_searchable(&server).await?;
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
            configure_apps_without_tool_search(config, apps_server.chatgpt_base_url.as_str())
        });
    let test = builder.build(&server).await?;

    test.submit_turn_with_policies(
        "list tools",
        AskForApproval::Never,
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let body = mock.single_request().body_json();
    let tools = tool_names(&body);
    assert!(!tools.iter().any(|name| name == TOOL_SEARCH_TOOL_NAME));
    assert!(tools.iter().any(|name| name == CALENDAR_CREATE_TOOL));
    assert!(tools.iter().any(|name| name == CALENDAR_LIST_TOOL));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_tool_is_hidden_for_api_key_auth() -> Result<()> {
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
        .with_auth(CodexAuth::from_api_key("Test API Key"))
        .with_config(move |config| configure_apps(config, apps_server.chatgpt_base_url.as_str()));
    let test = builder.build(&server).await?;

    test.submit_turn_with_policies(
        "list tools",
        AskForApproval::Never,
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let body = mock.single_request().body_json();
    let tools = tool_names(&body);
    assert!(
        !tools.iter().any(|name| name == TOOL_SEARCH_TOOL_NAME),
        "tools list should not include {TOOL_SEARCH_TOOL_NAME} for API key auth: {tools:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_tool_adds_discovery_instructions_to_tool_description() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount_searchable(&server).await?;
    let mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let mut builder = configured_builder(apps_server.chatgpt_base_url.clone());
    let test = builder.build(&server).await?;

    test.submit_turn_with_policies(
        "list tools",
        AskForApproval::Never,
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let body = mock.single_request().body_json();
    let description = tool_search_description(&body).expect("tool_search description should exist");
    assert!(
        SEARCH_TOOL_DESCRIPTION_SNIPPETS
            .iter()
            .all(|snippet| description.contains(snippet)),
        "tool_search description should include the updated workflow: {description:?}"
    );
    assert!(
        !description.contains("remainder of the current session/thread"),
        "tool_search description should not mention legacy client-side persistence: {description:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_tool_hides_apps_tools_without_search() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount_searchable(&server).await?;
    let mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let mut builder = configured_builder(apps_server.chatgpt_base_url.clone());
    let test = builder.build(&server).await?;

    test.submit_turn_with_policies(
        "hello tools",
        AskForApproval::Never,
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let body = mock.single_request().body_json();
    let tools = tool_names(&body);
    assert!(tools.iter().any(|name| name == TOOL_SEARCH_TOOL_NAME));
    assert!(!tools.iter().any(|name| name == CALENDAR_CREATE_TOOL));
    assert!(!tools.iter().any(|name| name == CALENDAR_LIST_TOOL));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_app_mentions_expose_apps_tools_without_search() -> Result<()> {
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

    let mut builder = configured_builder(apps_server.chatgpt_base_url.clone());
    let test = builder.build(&server).await?;

    test.submit_turn_with_policies(
        "Use [$calendar](app://calendar) and then call tools.",
        AskForApproval::Never,
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let body = mock.single_request().body_json();
    let tools = tool_names(&body);
    assert!(
        tools.iter().any(|name| name == CALENDAR_CREATE_TOOL),
        "expected explicit app mention to expose create tool, got tools: {tools:?}"
    );
    assert!(
        tools.iter().any(|name| name == CALENDAR_LIST_TOOL),
        "expected explicit app mention to expose list tool, got tools: {tools:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tool_search_returns_deferred_tools_without_follow_up_tool_injection() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount_searchable(&server).await?;
    let call_id = "tool-search-1";
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_tool_search_call(
                    call_id,
                    &json!({
                        "query": "create calendar event",
                        "limit": 1,
                    }),
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                json!({
                    "type": "response.output_item.done",
                    "item": {
                        "type": "function_call",
                        "call_id": "calendar-call-1",
                        "name": SEARCH_CALENDAR_CREATE_TOOL,
                        "namespace": SEARCH_CALENDAR_NAMESPACE,
                        "arguments": serde_json::to_string(&json!({
                            "title": "Lunch",
                            "starts_at": "2026-03-10T12:00:00Z"
                        })).expect("serialize calendar args")
                    }
                }),
                ev_completed("resp-2"),
            ]),
            sse(vec![
                ev_response_created("resp-3"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-3"),
            ]),
        ],
    )
    .await;

    let mut builder = configured_builder(apps_server.chatgpt_base_url.clone());
    let test = builder.build(&server).await?;
    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "Find the calendar create tool".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
        })
        .await?;

    let EventMsg::McpToolCallEnd(end) = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::McpToolCallEnd(_))
    })
    .await
    else {
        unreachable!("event guard guarantees McpToolCallEnd");
    };
    assert_eq!(end.call_id, "calendar-call-1");
    assert_eq!(
        end.invocation,
        McpInvocation {
            server: "codex_apps".to_string(),
            tool: "calendar_create_event".to_string(),
            arguments: Some(json!({
                "title": "Lunch",
                "starts_at": "2026-03-10T12:00:00Z"
            })),
        }
    );
    assert_eq!(
        end.result
            .as_ref()
            .expect("tool call should succeed")
            .structured_content,
        Some(json!({
            "_codex_apps": {
                "resource_uri": CALENDAR_CREATE_EVENT_RESOURCE_URI,
                "contains_mcp_source": true,
                "connector_id": "calendar",
            },
        }))
    );

    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = mock.requests();
    assert_eq!(requests.len(), 3);

    let apps_tool_call = server
        .received_requests()
        .await
        .unwrap_or_default()
        .into_iter()
        .find_map(|request| {
            let body: Value = serde_json::from_slice(&request.body).ok()?;
            (request.url.path() == "/api/codex/apps"
                && body.get("method").and_then(Value::as_str) == Some("tools/call"))
            .then_some(body)
        })
        .expect("apps tools/call request should be recorded");

    assert_eq!(
        apps_tool_call.pointer("/params/_meta/_codex_apps"),
        Some(&json!({
            "resource_uri": CALENDAR_CREATE_EVENT_RESOURCE_URI,
            "contains_mcp_source": true,
            "connector_id": "calendar",
        }))
    );
    assert_eq!(
        apps_tool_call.pointer("/params/_meta/x-codex-turn-metadata/session_id"),
        Some(&json!(test.session_configured.session_id.to_string()))
    );
    assert!(
        apps_tool_call
            .pointer("/params/_meta/x-codex-turn-metadata/turn_id")
            .and_then(Value::as_str)
            .is_some_and(|turn_id| !turn_id.is_empty()),
        "apps tools/call should include turn metadata turn_id: {apps_tool_call:?}"
    );

    let first_request_tools = tool_names(&requests[0].body_json());
    assert!(
        first_request_tools
            .iter()
            .any(|name| name == TOOL_SEARCH_TOOL_NAME),
        "first request should advertise tool_search: {first_request_tools:?}"
    );
    assert!(
        !first_request_tools
            .iter()
            .any(|name| name == CALENDAR_CREATE_TOOL),
        "app tools should still be hidden before search: {first_request_tools:?}"
    );

    let output_item = tool_search_output_item(&requests[1], call_id);
    assert_eq!(
        output_item.get("status").and_then(Value::as_str),
        Some("completed")
    );
    assert_eq!(
        output_item.get("execution").and_then(Value::as_str),
        Some("client")
    );

    let tools = tool_search_output_tools(&requests[1], call_id);
    assert_eq!(
        tools,
        vec![json!({
            "type": "namespace",
            "name": SEARCH_CALENDAR_NAMESPACE,
            "description": "Plan events and manage your calendar.",
            "tools": [
                {
                    "type": "function",
                    "name": SEARCH_CALENDAR_CREATE_TOOL,
                    "description": "Create a calendar event.",
                    "strict": false,
                    "defer_loading": true,
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "starts_at": {"type": "string"},
                            "timezone": {"type": "string"},
                            "title": {"type": "string"},
                        },
                        "required": ["title", "starts_at"],
                        "additionalProperties": false,
                    }
                }
            ]
        })]
    );

    let second_request_tools = tool_names(&requests[1].body_json());
    assert!(
        !second_request_tools
            .iter()
            .any(|name| name == CALENDAR_CREATE_TOOL),
        "follow-up request should rely on tool_search_output history, not tool injection: {second_request_tools:?}"
    );

    let output_item = requests[2].function_call_output("calendar-call-1");
    assert_eq!(
        output_item.get("call_id").and_then(Value::as_str),
        Some("calendar-call-1")
    );

    let third_request_tools = tool_names(&requests[2].body_json());
    assert!(
        !third_request_tools
            .iter()
            .any(|name| name == CALENDAR_CREATE_TOOL),
        "post-tool follow-up should still rely on tool_search_output history, not tool injection: {third_request_tools:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tool_search_indexes_only_enabled_non_app_mcp_tools() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount_searchable(&server).await?;
    let echo_call_id = "tool-search-echo";
    let image_call_id = "tool-search-image";
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_tool_search_call(
                    echo_call_id,
                    &json!({
                        "query": "Echo back the provided message and include environment data.",
                        "limit": 8,
                    }),
                ),
                ev_tool_search_call(
                    image_call_id,
                    &json!({
                        "query": "Return a single image content block.",
                        "limit": 8,
                    }),
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let rmcp_test_server_bin = stdio_server_bin()?;
    let mut builder =
        configured_builder(apps_server.chatgpt_base_url.clone()).with_config(move |config| {
            let mut servers = config.mcp_servers.get().clone();
            servers.insert(
                "rmcp".to_string(),
                McpServerConfig {
                    transport: McpServerTransportConfig::Stdio {
                        command: rmcp_test_server_bin,
                        args: Vec::new(),
                        env: None,
                        env_vars: Vec::new(),
                        cwd: None,
                    },
                    enabled: true,
                    required: false,
                    disabled_reason: None,
                    startup_timeout_sec: Some(Duration::from_secs(10)),
                    tool_timeout_sec: None,
                    enabled_tools: Some(vec!["echo".to_string(), "image".to_string()]),
                    disabled_tools: Some(vec!["image".to_string()]),
                    scopes: None,
                    oauth_resource: None,
                    supports_parallel_tool_calls: false,
                    tools: HashMap::new(),
                },
            );
            config
                .mcp_servers
                .set(servers)
                .expect("test mcp servers should accept any configuration");
        });
    let test = builder.build(&server).await?;

    test.submit_turn_with_policies(
        "Find the rmcp echo and image tools.",
        AskForApproval::Never,
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let requests = mock.requests();
    assert_eq!(requests.len(), 2);

    let first_request_tools = tool_names(&requests[0].body_json());
    assert!(
        first_request_tools
            .iter()
            .any(|name| name == TOOL_SEARCH_TOOL_NAME),
        "first request should advertise tool_search: {first_request_tools:?}"
    );
    assert!(
        !first_request_tools
            .iter()
            .any(|name| name == "mcp__rmcp__echo"),
        "non-app MCP tools should be hidden before search in large-search mode: {first_request_tools:?}"
    );

    let echo_tools = tool_search_output_tools(&requests[1], echo_call_id);
    let rmcp_echo_tools = echo_tools
        .iter()
        .filter(|tool| tool.get("name").and_then(Value::as_str) == Some("mcp__rmcp__"))
        .flat_map(|namespace| namespace.get("tools").and_then(Value::as_array))
        .flatten()
        .filter_map(|tool| tool.get("name").and_then(Value::as_str).map(str::to_string))
        .collect::<Vec<_>>();
    assert_eq!(rmcp_echo_tools, vec!["echo".to_string()]);

    let image_tools = tool_search_output_tools(&requests[1], image_call_id);
    let found_rmcp_image_tool = image_tools
        .iter()
        .filter(|tool| tool.get("name").and_then(Value::as_str) == Some("mcp__rmcp__"))
        .flat_map(|namespace| namespace.get("tools").and_then(Value::as_array))
        .flatten()
        .any(|tool| tool.get("name").and_then(Value::as_str).is_some());
    assert!(
        !found_rmcp_image_tool,
        "disabled non-app MCP tools should not be searchable: {image_tools:?}"
    );

    Ok(())
}
