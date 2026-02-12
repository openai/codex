#![cfg(not(target_os = "windows"))]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use anyhow::Result;
use codex_core::config::types::McpServerConfig;
use codex_core::config::types::McpServerTransportConfig;
use codex_core::features::Feature;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::SandboxPolicy;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::stdio_server_bin;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

const SEARCH_TOOL_INSTRUCTION_SNIPPETS: [&str; 2] = [
    "app tools from `codex_apps` (`mcp__codex_apps__...`) are hidden until you search for them.",
    "Core tools and non-app MCP tools remain available without searching.",
];

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

fn developer_messages(body: &Value) -> Vec<String> {
    body.get("input")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    if item.get("role").and_then(Value::as_str) != Some("developer") {
                        return None;
                    }
                    let content = item.get("content").and_then(Value::as_array)?;
                    let texts: Vec<&str> = content
                        .iter()
                        .filter_map(|entry| entry.get("text").and_then(Value::as_str))
                        .collect();
                    if texts.is_empty() {
                        None
                    } else {
                        Some(texts.join("\n"))
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn search_tool_output_payload(request: &ResponsesRequest, call_id: &str) -> Value {
    let (content, _success) = request
        .function_call_output_content_and_success(call_id)
        .expect("search_tool_bm25 function_call_output should be present");
    let content = content.expect("search_tool_bm25 output should include content");
    serde_json::from_str(&content).expect("search_tool_bm25 content should be valid JSON")
}

fn active_selected_tools(payload: &Value) -> Vec<String> {
    payload
        .get("active_selected_tools")
        .and_then(Value::as_array)
        .expect("active_selected_tools should be an array")
        .iter()
        .map(|value| {
            value
                .as_str()
                .expect("active_selected_tools entries should be strings")
                .to_string()
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_tool_flag_adds_tool() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mock = mount_sse_sequence(
        &server,
        vec![sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ])],
    )
    .await;

    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::SearchTool);
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
    assert!(
        tools.iter().any(|name| name == "search_tool_bm25"),
        "tools list should include search_tool_bm25 when enabled: {tools:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_tool_adds_developer_instructions() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mock = mount_sse_sequence(
        &server,
        vec![sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ])],
    )
    .await;

    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::SearchTool);
    });
    let test = builder.build(&server).await?;

    test.submit_turn_with_policies(
        "list tools",
        AskForApproval::Never,
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let body = mock.single_request().body_json();
    let developer_texts = developer_messages(&body);
    assert!(
        developer_texts.iter().any(|text| {
            SEARCH_TOOL_INSTRUCTION_SNIPPETS
                .iter()
                .all(|snippet| text.contains(snippet))
        }),
        "developer instructions should include search tool workflow: {developer_texts:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_tool_keeps_non_app_mcp_tools_without_search() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mock = mount_sse_sequence(
        &server,
        vec![sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ])],
    )
    .await;

    let rmcp_test_server_bin = stdio_server_bin()?;
    let mut builder = test_codex().with_config(move |config| {
        config.features.enable(Feature::SearchTool);
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
                enabled_tools: None,
                disabled_tools: None,
                scopes: None,
            },
        );
        config
            .mcp_servers
            .set(servers)
            .expect("test mcp servers should accept any configuration");
    });
    let test = builder.build(&server).await?;

    test.submit_turn_with_policies(
        "hello tools",
        AskForApproval::Never,
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let body = mock.single_request().body_json();
    let tools = tool_names(&body);
    assert!(
        tools.iter().any(|name| name == "search_tool_bm25"),
        "tools list should include search_tool_bm25 when enabled: {tools:?}"
    );
    assert!(
        tools.iter().any(|name| name == "mcp__rmcp__echo"),
        "tools list should include non-app MCP tools without search: {tools:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_tool_selection_persists_within_turn_and_resets_next_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let call_id = "tool-search";
    let args = json!({
        "query": "echo",
        "limit": 1,
    });
    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "search_tool_bm25", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
        sse(vec![
            ev_assistant_message("msg-2", "done again"),
            ev_completed("resp-3"),
        ]),
    ];
    let mock = mount_sse_sequence(&server, responses).await;

    let rmcp_test_server_bin = stdio_server_bin()?;
    let mut builder = test_codex().with_config(move |config| {
        config.features.enable(Feature::SearchTool);
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
                enabled_tools: None,
                disabled_tools: None,
                scopes: None,
            },
        );
        config
            .mcp_servers
            .set(servers)
            .expect("test mcp servers should accept any configuration");
    });
    let test = builder.build(&server).await?;

    test.submit_turn_with_policies(
        "find the echo tool",
        AskForApproval::Never,
        SandboxPolicy::DangerFullAccess,
    )
    .await?;
    test.submit_turn_with_policies(
        "hello again",
        AskForApproval::Never,
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let requests = mock.requests();
    assert_eq!(
        requests.len(),
        3,
        "expected 3 requests, got {}",
        requests.len()
    );

    let first_tools = tool_names(&requests[0].body_json());
    assert!(
        first_tools.iter().any(|name| name == "mcp__rmcp__echo"),
        "first request should include non-app MCP tools without search: {first_tools:?}"
    );

    let second_tools = tool_names(&requests[1].body_json());
    assert!(
        second_tools.iter().any(|name| name == "mcp__rmcp__echo"),
        "second request should include non-app MCP tools: {second_tools:?}"
    );

    let search_output_payload = search_tool_output_payload(&requests[1], call_id);
    assert!(
        search_output_payload.get("selected_tools").is_none(),
        "selected_tools should not be returned: {search_output_payload:?}"
    );
    assert!(
        search_output_payload.get("query").is_some(),
        "search_tool_bm25 output should include query: {search_output_payload:?}"
    );
    assert!(
        search_output_payload.get("total_tools").is_some(),
        "search_tool_bm25 output should include total_tools: {search_output_payload:?}"
    );
    assert!(
        search_output_payload.get("tools").is_some(),
        "search_tool_bm25 output should include tools: {search_output_payload:?}"
    );
    assert_eq!(
        active_selected_tools(&search_output_payload),
        Vec::<String>::new(),
    );

    let third_tools = tool_names(&requests[2].body_json());
    assert!(
        third_tools.iter().any(|name| name == "mcp__rmcp__echo"),
        "third request should include non-app MCP tools in the next turn: {third_tools:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_tool_selection_unions_results_within_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let first_call_id = "tool-search-echo";
    let second_call_id = "tool-search-image";
    let first_args = json!({
        "query": "echo",
        "limit": 1,
    });
    let second_args = json!({
        "query": "image",
        "limit": 1,
    });
    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(
                first_call_id,
                "search_tool_bm25",
                &serde_json::to_string(&first_args)?,
            ),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_response_created("resp-2"),
            ev_function_call(
                second_call_id,
                "search_tool_bm25",
                &serde_json::to_string(&second_args)?,
            ),
            ev_completed("resp-2"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-3"),
        ]),
    ];
    let mock = mount_sse_sequence(&server, responses).await;

    let rmcp_test_server_bin = stdio_server_bin()?;
    let mut builder = test_codex().with_config(move |config| {
        config.features.enable(Feature::SearchTool);
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
                enabled_tools: None,
                disabled_tools: None,
                scopes: None,
            },
        );
        config
            .mcp_servers
            .set(servers)
            .expect("test mcp servers should accept any configuration");
    });
    let test = builder.build(&server).await?;

    test.submit_turn_with_policies(
        "find echo and image tools",
        AskForApproval::Never,
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let requests = mock.requests();
    assert_eq!(
        requests.len(),
        3,
        "expected 3 requests, got {}",
        requests.len()
    );

    let first_tools = tool_names(&requests[0].body_json());
    assert!(
        first_tools.iter().any(|name| name == "mcp__rmcp__echo"),
        "first request should include non-app MCP tools without search: {first_tools:?}"
    );

    let second_tools = tool_names(&requests[1].body_json());
    assert!(
        second_tools.iter().any(|name| name == "mcp__rmcp__echo"),
        "second request should include non-app MCP tools: {second_tools:?}"
    );

    let third_tools = tool_names(&requests[2].body_json());
    assert!(
        third_tools.iter().any(|name| name == "mcp__rmcp__echo"),
        "third request should include non-app MCP tools: {third_tools:?}"
    );

    let first_search_payload = search_tool_output_payload(&requests[1], first_call_id);
    assert_eq!(
        active_selected_tools(&first_search_payload),
        Vec::<String>::new(),
    );
    let second_search_payload = search_tool_output_payload(&requests[2], second_call_id);
    assert!(
        second_search_payload.get("selected_tools").is_none(),
        "selected_tools should not be returned: {second_search_payload:?}"
    );
    assert_eq!(
        active_selected_tools(&second_search_payload),
        Vec::<String>::new(),
    );

    Ok(())
}
