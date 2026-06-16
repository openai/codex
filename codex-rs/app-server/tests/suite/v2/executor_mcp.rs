use anyhow::Result;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use app_test_support::write_mock_responses_config_toml;
use codex_app_server_protocol::CapabilityRootLocation;
use codex_app_server_protocol::ListMcpServerStatusParams;
use codex_app_server_protocol::ListMcpServerStatusResponse;
use codex_app_server_protocol::McpServerToolCallParams;
use codex_app_server_protocol::McpServerToolCallResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SelectedCapabilityRoot;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::UserInput;
use core_test_support::responses;
use core_test_support::stdio_server_bin;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::BTreeMap;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(20);
const MCP_SERVER_NAME: &str = "executor_demo";
const EXECUTOR_ENV_NAME: &str = "MCP_EXECUTOR_MARKER";
const EXECUTOR_ENV_VALUE: &str = "executor-only";
const EXECUTOR_ID: &str = "executor-1";
const REFRESH_PROBE_SERVER_NAME: &str = "refresh_probe";
const TOOL_CALL_ID: &str = "executor-mcp-call";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn selected_executor_plugin_exposes_its_stdio_mcp_only_to_that_thread() -> Result<()> {
    let responses_server = responses::start_mock_server().await;
    let codex_home = TempDir::new()?;
    write_mock_responses_config_toml(
        codex_home.path(),
        &responses_server.uri(),
        &BTreeMap::new(),
        /*auto_compact_limit*/ 1024,
        /*requires_openai_auth*/ None,
        "mock_provider",
        "compact",
    )?;
    std::fs::write(
        codex_home.path().join("environments.toml"),
        format!(
            r#"
include_local = true

[[environments]]
id = "{EXECUTOR_ID}"
program = {}
args = ["exec-server", "--listen", "stdio"]
[environments.env]
{EXECUTOR_ENV_NAME} = "{EXECUTOR_ENV_VALUE}"
"#,
            toml::Value::String(
                codex_utils_cargo_bin::cargo_bin("codex")?
                    .to_string_lossy()
                    .into_owned()
            )
        ),
    )?;

    let plugin = TempDir::new()?;
    std::fs::create_dir_all(plugin.path().join(".codex-plugin"))?;
    std::fs::write(
        plugin.path().join(".codex-plugin/plugin.json"),
        r#"{"name":"executor-demo"}"#,
    )?;
    std::fs::write(
        plugin.path().join(".mcp.json"),
        serde_json::to_vec_pretty(&json!({
            "mcpServers": {
                (MCP_SERVER_NAME): {
                    "command": stdio_server_bin()?,
                    "env_vars": [EXECUTOR_ENV_NAME],
                    "startup_timeout_sec": 10,
                }
            }
        }))?,
    )?;

    let mut app_server = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, app_server.initialize()).await??;

    let selected_thread = start_thread(
        &mut app_server,
        Some(vec![SelectedCapabilityRoot {
            id: "executor-demo@1".to_string(),
            location: CapabilityRootLocation::Environment {
                environment_id: EXECUTOR_ID.to_string(),
                path: plugin.path().to_string_lossy().into_owned(),
            },
        }]),
    )
    .await?;

    std::fs::write(plugin.path().join(".mcp.json"), r#"{"mcpServers":{}}"#)?;
    let config_path = codex_home.path().join("config.toml");
    let mut config = std::fs::read_to_string(&config_path)?;
    config.push_str(&format!(
        r#"
[mcp_servers.{REFRESH_PROBE_SERVER_NAME}]
command = {}
startup_timeout_sec = 10
"#,
        toml::Value::String(stdio_server_bin()?)
    ));
    std::fs::write(config_path, config)?;
    let request_id = app_server
        .send_raw_request("config/mcpServer/reload", /*params*/ None)
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;

    let namespace = format!("mcp__{MCP_SERVER_NAME}");
    let response_mock = responses::mount_sse_sequence(
        &responses_server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-executor-mcp-call"),
                responses::ev_function_call_with_namespace(
                    TOOL_CALL_ID,
                    &namespace,
                    "echo",
                    &json!({
                        "message": "hello from executor",
                        "env_var": EXECUTOR_ENV_NAME,
                    })
                    .to_string(),
                ),
                responses::ev_completed("resp-executor-mcp-call"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("resp-executor-mcp-done"),
                responses::ev_assistant_message("msg-executor-mcp-done", "Done"),
                responses::ev_completed("resp-executor-mcp-done"),
            ]),
        ],
    )
    .await;
    let request_id = app_server
        .send_turn_start_request(TurnStartParams {
            thread_id: selected_thread.clone(),
            input: vec![UserInput::Text {
                text: "Call the executor MCP echo tool".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].tool_by_name(&namespace, "echo").is_some());
    let output = requests[1].function_call_output(TOOL_CALL_ID);
    let output = output
        .get("output")
        .and_then(serde_json::Value::as_str)
        .expect("MCP function output should be text");
    assert!(output.contains("ECHOING: hello from executor"));
    assert!(output.contains(EXECUTOR_ENV_VALUE));

    let request_id = app_server
        .send_mcp_server_tool_call_request(McpServerToolCallParams {
            thread_id: selected_thread.clone(),
            server: REFRESH_PROBE_SERVER_NAME.to_string(),
            tool: "echo".to_string(),
            arguments: Some(json!({"message": "refresh applied"})),
            meta: None,
        })
        .await?;
    let response = timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: McpServerToolCallResponse = to_response(response)?;
    assert_eq!(
        response
            .structured_content
            .and_then(|content| content.get("echo").cloned()),
        Some(json!("ECHOING: refresh applied"))
    );

    assert!(
        mcp_server_names(&mut app_server, selected_thread)
            .await?
            .iter()
            .any(|name| name == MCP_SERVER_NAME)
    );

    let unselected_thread =
        start_thread(&mut app_server, /*selected_capability_roots*/ None).await?;
    assert!(
        mcp_server_names(&mut app_server, unselected_thread)
            .await?
            .iter()
            .all(|name| name != MCP_SERVER_NAME)
    );

    Ok(())
}

async fn mcp_server_names(
    app_server: &mut TestAppServer,
    thread_id: String,
) -> Result<Vec<String>> {
    let request_id = app_server
        .send_list_mcp_server_status_request(ListMcpServerStatusParams {
            cursor: None,
            limit: None,
            detail: None,
            thread_id: Some(thread_id),
        })
        .await?;
    let response = timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: ListMcpServerStatusResponse = to_response(response)?;
    Ok(response
        .data
        .into_iter()
        .map(|server| server.name)
        .collect())
}

async fn start_thread(
    app_server: &mut TestAppServer,
    selected_capability_roots: Option<Vec<SelectedCapabilityRoot>>,
) -> Result<String> {
    let request_id = app_server
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            selected_capability_roots,
            ..Default::default()
        })
        .await?;
    let response = timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response(response)?;
    Ok(thread.id)
}
