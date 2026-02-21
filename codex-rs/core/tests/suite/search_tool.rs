#![cfg(not(target_os = "windows"))]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use codex_core::CodexThread;
use codex_core::NewThread;
use codex_core::config::Config;
use codex_core::config::types::McpServerConfig;
use codex_core::config::types::McpServerTransportConfig;
use codex_core::features::Feature;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::user_input::UserInput;
use core_test_support::apps_test_server::AppsTestServer;
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
use core_test_support::test_codex::TestCodexBuilder;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

const SEARCH_TOOL_INSTRUCTION_SNIPPETS: [&str; 2] = [
    "MCP tools of the apps (Calendar) are hidden until you search for them with this tool",
    "Matching tools are added to available `tools` and available for the remainder of the current session/thread.",
];
const SEARCH_TOOL_BM25_TOOL_NAME: &str = "search_tool_bm25";
const CALENDAR_CREATE_TOOL: &str = "mcp__codex_apps__calendar_create_event";
const CALENDAR_LIST_TOOL: &str = "mcp__codex_apps__calendar_list_events";
const RMCP_ECHO_TOOL: &str = "mcp__rmcp__echo";
const RMCP_IMAGE_TOOL: &str = "mcp__rmcp__image";
const CALENDAR_CREATE_QUERY: &str = "create calendar event";
const CALENDAR_LIST_QUERY: &str = "list calendar events";

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

fn search_tool_description(body: &Value) -> Option<String> {
    body.get("tools")
        .and_then(Value::as_array)
        .and_then(|tools| {
            tools.iter().find_map(|tool| {
                if tool.get("name").and_then(Value::as_str) == Some(SEARCH_TOOL_BM25_TOOL_NAME) {
                    tool.get("description")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                } else {
                    None
                }
            })
        })
}

fn search_tool_output_payload(request: &ResponsesRequest, call_id: &str) -> Value {
    let (content, _success) = request
        .function_call_output_content_and_success(call_id)
        .unwrap_or_else(|| {
            panic!("{SEARCH_TOOL_BM25_TOOL_NAME} function_call_output should be present")
        });
    let content = content
        .unwrap_or_else(|| panic!("{SEARCH_TOOL_BM25_TOOL_NAME} output should include content"));
    serde_json::from_str(&content)
        .unwrap_or_else(|_| panic!("{SEARCH_TOOL_BM25_TOOL_NAME} content should be valid JSON"))
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

fn search_result_tools(payload: &Value) -> Vec<&Value> {
    payload
        .get("tools")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .collect()
}

fn rmcp_server_config(command: String) -> McpServerConfig {
    McpServerConfig {
        transport: McpServerTransportConfig::Stdio {
            command,
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
    }
}

fn configure_apps_with_optional_rmcp(
    config: &mut Config,
    apps_base_url: &str,
    rmcp_server_bin: Option<String>,
) {
    config.features.enable(Feature::Apps);
    config.features.disable(Feature::AppsMcpGateway);
    config.chatgpt_base_url = apps_base_url.to_string();
    if let Some(command) = rmcp_server_bin {
        let mut servers = config.mcp_servers.get().clone();
        servers.insert("rmcp".to_string(), rmcp_server_config(command));
        config
            .mcp_servers
            .set(servers)
            .expect("test mcp servers should accept any configuration");
    }
}

fn configured_builder(apps_base_url: String, rmcp_server_bin: Option<String>) -> TestCodexBuilder {
    test_codex().with_config(move |config| {
        configure_apps_with_optional_rmcp(config, apps_base_url.as_str(), rmcp_server_bin);
    })
}

async fn submit_user_input(thread: &Arc<CodexThread>, text: &str) -> Result<()> {
    thread
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: text.to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(thread, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_tool_flag_adds_tool() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    let mock = mount_sse_sequence(
        &server,
        vec![sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ])],
    )
    .await;

    let mut builder = configured_builder(apps_server.chatgpt_base_url.clone(), None);
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
        tools.iter().any(|name| name == SEARCH_TOOL_BM25_TOOL_NAME),
        "tools list should include {SEARCH_TOOL_BM25_TOOL_NAME} when enabled: {tools:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_tool_adds_discovery_instructions_to_tool_description() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    let mock = mount_sse_sequence(
        &server,
        vec![sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ])],
    )
    .await;

    let mut builder = configured_builder(apps_server.chatgpt_base_url.clone(), None);
    let test = builder.build(&server).await?;

    test.submit_turn_with_policies(
        "list tools",
        AskForApproval::Never,
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let body = mock.single_request().body_json();
    let description = search_tool_description(&body).expect("search tool description should exist");
    assert!(
        SEARCH_TOOL_INSTRUCTION_SNIPPETS
            .iter()
            .all(|snippet| description.contains(snippet)),
        "search tool description should include search tool workflow: {description:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_tool_hides_apps_tools_without_search_but_keeps_non_app_tools_visible() -> Result<()>
{
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
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
    let mut builder = configured_builder(
        apps_server.chatgpt_base_url.clone(),
        Some(rmcp_test_server_bin),
    );
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
        tools.iter().any(|name| name == SEARCH_TOOL_BM25_TOOL_NAME),
        "tools list should include {SEARCH_TOOL_BM25_TOOL_NAME}: {tools:?}"
    );
    assert!(
        tools.iter().any(|name| name == RMCP_ECHO_TOOL),
        "non-app MCP tools should remain visible in Apps mode: {tools:?}"
    );
    assert!(
        tools.iter().any(|name| name == RMCP_IMAGE_TOOL),
        "non-app MCP tools should remain visible in Apps mode: {tools:?}"
    );
    assert!(
        !tools.iter().any(|name| name == CALENDAR_CREATE_TOOL),
        "apps tools should stay hidden before search/mention: {tools:?}"
    );
    assert!(
        !tools.iter().any(|name| name == CALENDAR_LIST_TOOL),
        "apps tools should stay hidden before search/mention: {tools:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_app_mentions_expose_apps_tools_without_search() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
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
    let mut builder = configured_builder(
        apps_server.chatgpt_base_url.clone(),
        Some(rmcp_test_server_bin),
    );
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
        tools.iter().any(|name| name == RMCP_ECHO_TOOL),
        "non-app tools should remain visible: {tools:?}"
    );
    assert!(
        tools.iter().any(|name| name == CALENDAR_CREATE_TOOL),
        "apps tools should be available after explicit app mention: {tools:?}"
    );
    assert!(
        tools.iter().any(|name| name == CALENDAR_LIST_TOOL),
        "apps tools should be available after explicit app mention: {tools:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_tool_selection_persists_across_turns() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    let call_id = "tool-search";
    let args = json!({
        "query": CALENDAR_CREATE_QUERY,
        "limit": 1,
    });
    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(
                call_id,
                SEARCH_TOOL_BM25_TOOL_NAME,
                &serde_json::to_string(&args)?,
            ),
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
    let mut builder = configured_builder(
        apps_server.chatgpt_base_url.clone(),
        Some(rmcp_test_server_bin),
    );
    let test = builder.build(&server).await?;

    test.submit_turn_with_policies(
        "find calendar create tool",
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
        first_tools.iter().any(|name| name == RMCP_ECHO_TOOL),
        "non-app MCP tools should be available before search: {first_tools:?}"
    );
    assert!(
        !first_tools.iter().any(|name| name == CALENDAR_CREATE_TOOL),
        "apps tools should not be visible before search: {first_tools:?}"
    );

    let search_output_payload = search_tool_output_payload(&requests[1], call_id);
    assert!(
        search_output_payload.get("selected_tools").is_none(),
        "selected_tools should not be returned: {search_output_payload:?}"
    );
    for tool in search_result_tools(&search_output_payload) {
        assert_eq!(
            tool.get("server").and_then(Value::as_str),
            Some("codex_apps"),
            "search results should only include codex_apps tools: {search_output_payload:?}"
        );
    }

    let selected_tools = active_selected_tools(&search_output_payload);
    assert!(
        selected_tools
            .iter()
            .any(|tool| tool == CALENDAR_CREATE_TOOL),
        "calendar create tool should be selected: {search_output_payload:?}"
    );
    assert!(
        !selected_tools
            .iter()
            .any(|tool_name| tool_name.starts_with("mcp__rmcp__")),
        "search should not add rmcp tools to active selection: {search_output_payload:?}"
    );

    let second_tools = tool_names(&requests[1].body_json());
    assert!(
        second_tools.iter().any(|name| name == RMCP_ECHO_TOOL),
        "non-app MCP tools should remain visible after search: {second_tools:?}"
    );
    for selected_tool in &selected_tools {
        assert!(
            second_tools.iter().any(|name| name == selected_tool),
            "follow-up request should include selected tool {selected_tool:?}: {second_tools:?}"
        );
    }

    let third_tools = tool_names(&requests[2].body_json());
    assert!(
        third_tools.iter().any(|name| name == RMCP_ECHO_TOOL),
        "non-app MCP tools should remain visible on later turns: {third_tools:?}"
    );
    for selected_tool in &selected_tools {
        assert!(
            third_tools.iter().any(|name| name == selected_tool),
            "subsequent turn should include selected tool {selected_tool:?}: {third_tools:?}"
        );
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_tool_selection_unions_results_within_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    let first_call_id = "tool-search-create";
    let second_call_id = "tool-search-list";
    let first_args = json!({
        "query": CALENDAR_CREATE_QUERY,
        "limit": 1,
    });
    let second_args = json!({
        "query": CALENDAR_LIST_QUERY,
        "limit": 1,
    });
    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(
                first_call_id,
                SEARCH_TOOL_BM25_TOOL_NAME,
                &serde_json::to_string(&first_args)?,
            ),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_response_created("resp-2"),
            ev_function_call(
                second_call_id,
                SEARCH_TOOL_BM25_TOOL_NAME,
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
    let mut builder = configured_builder(
        apps_server.chatgpt_base_url.clone(),
        Some(rmcp_test_server_bin),
    );
    let test = builder.build(&server).await?;

    test.submit_turn_with_policies(
        "find create and list calendar tools",
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
        first_tools.iter().any(|name| name == RMCP_ECHO_TOOL),
        "non-app MCP tools should be visible before search: {first_tools:?}"
    );
    assert!(
        !first_tools.iter().any(|name| name == CALENDAR_CREATE_TOOL),
        "apps tools should be hidden before search: {first_tools:?}"
    );
    assert!(
        !first_tools.iter().any(|name| name == CALENDAR_LIST_TOOL),
        "apps tools should be hidden before search: {first_tools:?}"
    );

    let second_search_payload = search_tool_output_payload(&requests[2], second_call_id);
    assert!(
        second_search_payload.get("selected_tools").is_none(),
        "selected_tools should not be returned: {second_search_payload:?}"
    );
    for tool in search_result_tools(&second_search_payload) {
        assert_eq!(
            tool.get("server").and_then(Value::as_str),
            Some("codex_apps"),
            "search results should only include codex_apps tools: {second_search_payload:?}"
        );
    }

    let selected_tools = active_selected_tools(&second_search_payload);
    assert_eq!(
        selected_tools,
        vec![
            CALENDAR_CREATE_TOOL.to_string(),
            CALENDAR_LIST_TOOL.to_string(),
        ],
        "two searches in one turn should union selected apps tools"
    );

    let third_tools = tool_names(&requests[2].body_json());
    assert!(
        third_tools.iter().any(|name| name == RMCP_ECHO_TOOL),
        "non-app MCP tools should remain visible after repeated search: {third_tools:?}"
    );
    assert!(
        third_tools.iter().any(|name| name == CALENDAR_CREATE_TOOL),
        "calendar create should be available after repeated search: {third_tools:?}"
    );
    assert!(
        third_tools.iter().any(|name| name == CALENDAR_LIST_TOOL),
        "calendar list should be available after repeated search: {third_tools:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_tool_selection_restores_when_resumed() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    let call_id = "tool-search";
    let args = json!({
        "query": CALENDAR_CREATE_QUERY,
        "limit": 1,
    });
    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(
                call_id,
                SEARCH_TOOL_BM25_TOOL_NAME,
                &serde_json::to_string(&args)?,
            ),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_response_created("resp-2"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
        sse(vec![
            ev_response_created("resp-3"),
            ev_assistant_message("msg-2", "resumed done"),
            ev_completed("resp-3"),
        ]),
    ];
    let mock = mount_sse_sequence(&server, responses).await;

    let rmcp_test_server_bin = stdio_server_bin()?;
    let mut builder = configured_builder(
        apps_server.chatgpt_base_url.clone(),
        Some(rmcp_test_server_bin.clone()),
    );
    let test = builder.build(&server).await?;

    let home = test.home.clone();
    let rollout_path = test
        .session_configured
        .rollout_path
        .clone()
        .expect("rollout path should be available for resume");

    test.submit_turn_with_policies(
        "find calendar tools",
        AskForApproval::Never,
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let requests = mock.requests();
    assert_eq!(
        requests.len(),
        2,
        "expected 2 requests after initial turn, got {}",
        requests.len()
    );
    let search_output_payload = search_tool_output_payload(&requests[1], call_id);
    let selected_tools = active_selected_tools(&search_output_payload);
    assert!(
        selected_tools
            .iter()
            .any(|name| name == CALENDAR_CREATE_TOOL),
        "search should select calendar create before resume: {search_output_payload:?}"
    );

    let mut resume_builder = configured_builder(
        apps_server.chatgpt_base_url.clone(),
        Some(rmcp_test_server_bin),
    );
    let resumed = resume_builder.resume(&server, home, rollout_path).await?;
    resumed
        .submit_turn_with_policies(
            "hello after resume",
            AskForApproval::Never,
            SandboxPolicy::DangerFullAccess,
        )
        .await?;

    let requests = mock.requests();
    assert_eq!(
        requests.len(),
        3,
        "expected 3 requests after resumed turn, got {}",
        requests.len()
    );
    let resumed_tools = tool_names(&requests[2].body_json());
    assert!(
        resumed_tools.iter().any(|name| name == RMCP_ECHO_TOOL),
        "non-app tools should remain visible after resume: {resumed_tools:?}"
    );
    for selected_tool in &selected_tools {
        assert!(
            resumed_tools.iter().any(|name| name == selected_tool),
            "resumed request should include restored selected tool {selected_tool:?}: {resumed_tools:?}"
        );
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_tool_selection_union_restores_when_resumed_after_multiple_search_calls()
-> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    let first_call_id = "tool-search-create";
    let second_call_id = "tool-search-list";
    let first_args = json!({
        "query": CALENDAR_CREATE_QUERY,
        "limit": 1,
    });
    let second_args = json!({
        "query": CALENDAR_LIST_QUERY,
        "limit": 1,
    });
    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(
                first_call_id,
                SEARCH_TOOL_BM25_TOOL_NAME,
                &serde_json::to_string(&first_args)?,
            ),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_response_created("resp-2"),
            ev_assistant_message("msg-1", "first search done"),
            ev_completed("resp-2"),
        ]),
        sse(vec![
            ev_response_created("resp-3"),
            ev_function_call(
                second_call_id,
                SEARCH_TOOL_BM25_TOOL_NAME,
                &serde_json::to_string(&second_args)?,
            ),
            ev_completed("resp-3"),
        ]),
        sse(vec![
            ev_response_created("resp-4"),
            ev_assistant_message("msg-2", "second search done"),
            ev_completed("resp-4"),
        ]),
        sse(vec![
            ev_response_created("resp-5"),
            ev_assistant_message("msg-3", "resumed done"),
            ev_completed("resp-5"),
        ]),
    ];
    let mock = mount_sse_sequence(&server, responses).await;

    let rmcp_test_server_bin = stdio_server_bin()?;
    let mut builder = configured_builder(
        apps_server.chatgpt_base_url.clone(),
        Some(rmcp_test_server_bin.clone()),
    );
    let test = builder.build(&server).await?;

    let home = test.home.clone();
    let rollout_path = test
        .session_configured
        .rollout_path
        .clone()
        .expect("rollout path should be available for resume");

    test.submit_turn_with_policies(
        "find create calendar tool",
        AskForApproval::Never,
        SandboxPolicy::DangerFullAccess,
    )
    .await?;
    test.submit_turn_with_policies(
        "find list calendar tool",
        AskForApproval::Never,
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let requests = mock.requests();
    assert_eq!(
        requests.len(),
        4,
        "expected 4 requests before resume, got {}",
        requests.len()
    );

    let first_search_payload = search_tool_output_payload(&requests[1], first_call_id);
    let first_result_tools = search_result_tools(&first_search_payload);
    assert_eq!(
        first_result_tools.len(),
        1,
        "first search should return exactly one tool: {first_search_payload:?}"
    );
    assert_eq!(
        first_result_tools[0].get("name").and_then(Value::as_str),
        Some(CALENDAR_CREATE_TOOL),
        "first search should return calendar create tool: {first_search_payload:?}"
    );
    let first_selected_tools = active_selected_tools(&first_search_payload);
    assert_eq!(
        first_selected_tools,
        vec![CALENDAR_CREATE_TOOL.to_string()],
        "first search should only select create tool: {first_search_payload:?}"
    );

    let second_search_payload = search_tool_output_payload(&requests[3], second_call_id);
    let second_result_tools = search_result_tools(&second_search_payload);
    assert_eq!(
        second_result_tools.len(),
        1,
        "second search should return exactly one tool: {second_search_payload:?}"
    );
    assert_eq!(
        second_result_tools[0].get("name").and_then(Value::as_str),
        Some(CALENDAR_LIST_TOOL),
        "second search should return calendar list tool: {second_search_payload:?}"
    );
    let second_selected_tools = active_selected_tools(&second_search_payload);
    assert_eq!(
        second_selected_tools,
        vec![
            CALENDAR_CREATE_TOOL.to_string(),
            CALENDAR_LIST_TOOL.to_string(),
        ],
        "multiple searches should persist union before resume: {second_search_payload:?}"
    );

    let mut resume_builder = configured_builder(
        apps_server.chatgpt_base_url.clone(),
        Some(rmcp_test_server_bin),
    );
    let resumed = resume_builder.resume(&server, home, rollout_path).await?;
    resumed
        .submit_turn_with_policies(
            "hello after resume with union",
            AskForApproval::Never,
            SandboxPolicy::DangerFullAccess,
        )
        .await?;

    let requests = mock.requests();
    assert_eq!(
        requests.len(),
        5,
        "expected 5 requests after resumed turn, got {}",
        requests.len()
    );

    let resumed_tools = tool_names(&requests[4].body_json());
    assert!(
        resumed_tools.iter().any(|name| name == RMCP_ECHO_TOOL),
        "non-app tools should remain visible after resume: {resumed_tools:?}"
    );
    assert!(
        resumed_tools
            .iter()
            .any(|name| name == CALENDAR_CREATE_TOOL),
        "resumed turn should restore calendar create tool: {resumed_tools:?}"
    );
    assert!(
        resumed_tools.iter().any(|name| name == CALENDAR_LIST_TOOL),
        "resumed turn should restore calendar list tool: {resumed_tools:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_tool_selection_restores_when_forked_with_full_history() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    let call_id = "tool-search";
    let args = json!({
        "query": CALENDAR_CREATE_QUERY,
        "limit": 1,
    });
    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(
                call_id,
                SEARCH_TOOL_BM25_TOOL_NAME,
                &serde_json::to_string(&args)?,
            ),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_response_created("resp-2"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
        sse(vec![
            ev_response_created("resp-3"),
            ev_assistant_message("msg-2", "forked done"),
            ev_completed("resp-3"),
        ]),
    ];
    let mock = mount_sse_sequence(&server, responses).await;

    let rmcp_test_server_bin = stdio_server_bin()?;
    let mut builder = configured_builder(
        apps_server.chatgpt_base_url.clone(),
        Some(rmcp_test_server_bin),
    );
    let test = builder.build(&server).await?;

    test.submit_turn_with_policies(
        "find calendar tools",
        AskForApproval::Never,
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let requests = mock.requests();
    assert_eq!(
        requests.len(),
        2,
        "expected 2 requests after initial turn, got {}",
        requests.len()
    );
    let search_output_payload = search_tool_output_payload(&requests[1], call_id);
    let selected_tools = active_selected_tools(&search_output_payload);
    assert!(
        selected_tools
            .iter()
            .any(|name| name == CALENDAR_CREATE_TOOL),
        "search should select calendar create: {search_output_payload:?}"
    );

    let rollout_path = test
        .codex
        .rollout_path()
        .expect("rollout path should exist for fork");
    let NewThread { thread: forked, .. } = test
        .thread_manager
        .fork_thread(usize::MAX, test.config.clone(), rollout_path, false)
        .await?;
    submit_user_input(&forked, "hello after fork").await?;

    let requests = mock.requests();
    assert_eq!(
        requests.len(),
        3,
        "expected 3 requests after forked turn, got {}",
        requests.len()
    );
    let forked_tools = tool_names(&requests[2].body_json());
    assert!(
        forked_tools.iter().any(|name| name == RMCP_ECHO_TOOL),
        "non-app tools should remain visible in forked thread: {forked_tools:?}"
    );
    for selected_tool in &selected_tools {
        assert!(
            forked_tools.iter().any(|name| name == selected_tool),
            "forked request should include restored selected tool {selected_tool:?}: {forked_tools:?}"
        );
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_tool_selection_drops_when_fork_excludes_search_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    let call_id = "tool-search";
    let args = json!({
        "query": CALENDAR_CREATE_QUERY,
        "limit": 1,
    });
    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(
                call_id,
                SEARCH_TOOL_BM25_TOOL_NAME,
                &serde_json::to_string(&args)?,
            ),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_response_created("resp-2"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
        sse(vec![
            ev_response_created("resp-3"),
            ev_assistant_message("msg-2", "forked done"),
            ev_completed("resp-3"),
        ]),
    ];
    let mock = mount_sse_sequence(&server, responses).await;

    let rmcp_test_server_bin = stdio_server_bin()?;
    let mut builder = configured_builder(
        apps_server.chatgpt_base_url.clone(),
        Some(rmcp_test_server_bin),
    );
    let test = builder.build(&server).await?;

    test.submit_turn_with_policies(
        "find calendar tools",
        AskForApproval::Never,
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let requests = mock.requests();
    assert_eq!(
        requests.len(),
        2,
        "expected 2 requests after initial turn, got {}",
        requests.len()
    );
    let search_output_payload = search_tool_output_payload(&requests[1], call_id);
    let selected_tools = active_selected_tools(&search_output_payload);
    assert!(
        !selected_tools.is_empty(),
        "search turn should produce selected tools: {search_output_payload:?}"
    );

    let rollout_path = test
        .codex
        .rollout_path()
        .expect("rollout path should exist for fork");
    let NewThread { thread: forked, .. } = test
        .thread_manager
        .fork_thread(0, test.config.clone(), rollout_path, false)
        .await?;
    submit_user_input(&forked, "hello after fork").await?;

    let requests = mock.requests();
    assert_eq!(
        requests.len(),
        3,
        "expected 3 requests after forked turn, got {}",
        requests.len()
    );
    let forked_tools = tool_names(&requests[2].body_json());
    assert!(
        forked_tools.iter().any(|name| name == RMCP_ECHO_TOOL),
        "non-app tools should remain visible in forked thread: {forked_tools:?}"
    );
    assert!(
        !forked_tools
            .iter()
            .any(|name| name.starts_with("mcp__codex_apps__")),
        "forked history without search turn should not restore apps tools: {forked_tools:?}"
    );

    Ok(())
}
