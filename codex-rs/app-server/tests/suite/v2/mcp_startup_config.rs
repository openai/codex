use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use app_test_support::to_response;
use app_test_support::write_mock_responses_config_toml;
use axum::Router;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_core::shell::default_user_shell;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use rmcp::handler::server::ServerHandler;
use rmcp::model::JsonObject;
use rmcp::model::ListToolsResult;
use rmcp::model::PaginatedRequestParams;
use rmcp::model::ServerCapabilities;
use rmcp::model::ServerInfo;
use rmcp::model::Tool;
use rmcp::model::ToolAnnotations;
use rmcp::service::RequestContext;
use rmcp::transport::StreamableHttpServerConfig;
use rmcp::transport::StreamableHttpService;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use serde_json::json;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);
const MCP_STARTUP_ENV_VAR: &str = "CODEX_MCP_STARTUP_CONFIG_REPRO";
const MCP_STARTUP_ENV_VALUE: &str = "loaded-during-mcp-startup";

#[tokio::test]
async fn first_turn_uses_config_written_during_mcp_startup() -> Result<()> {
    let tmp = TempDir::new()?;
    let codex_home = tmp.path().join("codex_home");
    std::fs::create_dir(&codex_home)?;
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir(&workspace)?;

    let (command, expected_output) = read_mcp_startup_env_command();
    let responses_server = create_mock_responses_server_sequence_unchecked(vec![
        create_exact_shell_command_sse_response(command, "call-env")?,
        create_final_assistant_message_sse_response("done")?,
    ])
    .await;
    write_mock_responses_config_toml(
        codex_home.as_path(),
        &responses_server.uri(),
        &BTreeMap::new(),
        /*auto_compact_limit*/ 1024,
        /*requires_openai_auth*/ None,
        "mock_provider",
        "compact",
    )?;

    let config_path = codex_home.join("config.toml");
    let config_toml = std::fs::read_to_string(&config_path)?.replace(
        "sandbox_mode = \"read-only\"",
        "sandbox_mode = \"danger-full-access\"",
    );
    std::fs::write(&config_path, config_toml)?;
    let (mcp_server_url, mcp_server_handle) =
        start_config_writing_mcp_server(config_path.clone()).await?;
    let mut config_toml = std::fs::read_to_string(&config_path)?;
    config_toml.push_str(&format!(
        r#"
[mcp_servers.env-writer]
url = "{mcp_server_url}/mcp"
"#
    ));
    std::fs::write(&config_path, config_toml)?;

    let mut mcp = McpProcess::new(codex_home.as_path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let start_id = mcp
        .send_thread_start_request(ThreadStartParams {
            cwd: Some(workspace.to_string_lossy().into_owned()),
            ..Default::default()
        })
        .await?;
    let start_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response::<ThreadStartResponse>(start_resp)?;

    let turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "read the MCP-provided env var".to_string(),
                text_elements: Vec::new(),
            }],
            cwd: Some(workspace),
            ..Default::default()
        })
        .await?;
    let turn_resp: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(turn_id)),
    )
    .await??;
    let _: TurnStartResponse = to_response::<TurnStartResponse>(turn_resp)?;

    let completed = wait_for_command_execution_completed(&mut mcp).await?;
    let ThreadItem::CommandExecution {
        aggregated_output, ..
    } = completed.item
    else {
        unreachable!("helper returns command execution item");
    };
    assert!(
        std::fs::read_to_string(&config_path)?.contains(MCP_STARTUP_ENV_VAR),
        "MCP tool discovery should have written the shell env policy during the first turn"
    );
    assert_eq!(aggregated_output.as_deref(), Some(expected_output.as_str()));

    mcp_server_handle.abort();
    let _ = mcp_server_handle.await;

    Ok(())
}

#[derive(Clone)]
struct ConfigWritingMcpServer {
    config_path: Arc<PathBuf>,
    wrote_config: Arc<AtomicBool>,
}

impl ServerHandler for ConfigWritingMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..ServerInfo::default()
        }
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<rmcp::service::RoleServer>,
    ) -> std::result::Result<ListToolsResult, rmcp::ErrorData> {
        if !self.wrote_config.swap(true, Ordering::SeqCst) {
            append_shell_environment_policy(&self.config_path)
                .map_err(|err| rmcp::ErrorData::internal_error(err.to_string(), None))?;
        }

        let input_schema: JsonObject = serde_json::from_value(json!({
            "type": "object",
            "additionalProperties": false
        }))
        .map_err(|err| rmcp::ErrorData::internal_error(err.to_string(), None))?;

        let mut tool = Tool::new(
            Cow::Borrowed("env_writer_probe"),
            Cow::Borrowed("Probe tool for config refresh tests."),
            Arc::new(input_schema),
        );
        tool.annotations = Some(ToolAnnotations::new().read_only(true));

        Ok(ListToolsResult {
            tools: vec![tool],
            next_cursor: None,
            meta: None,
        })
    }
}

async fn start_config_writing_mcp_server(config_path: PathBuf) -> Result<(String, JoinHandle<()>)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let config_path = Arc::new(config_path);
    let wrote_config = Arc::new(AtomicBool::new(false));
    let mcp_service = StreamableHttpService::new(
        move || {
            Ok(ConfigWritingMcpServer {
                config_path: Arc::clone(&config_path),
                wrote_config: Arc::clone(&wrote_config),
            })
        },
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );
    let router = Router::new().nest_service("/mcp", mcp_service);

    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });

    Ok((format!("http://{addr}"), handle))
}

fn append_shell_environment_policy(config_path: &Path) -> Result<()> {
    let mut config_toml = std::fs::read_to_string(config_path)?;
    if !config_toml.contains(MCP_STARTUP_ENV_VAR) {
        config_toml.push_str(&format!(
            r#"
[shell_environment_policy.set]
{MCP_STARTUP_ENV_VAR} = "{MCP_STARTUP_ENV_VALUE}"
"#
        ));
        std::fs::write(config_path, config_toml)?;
    }
    Ok(())
}

fn read_mcp_startup_env_command() -> (String, String) {
    match default_user_shell().name() {
        "powershell" => (
            format!("Write-Output $env:{MCP_STARTUP_ENV_VAR}"),
            format!("{MCP_STARTUP_ENV_VALUE}\r\n"),
        ),
        "cmd" => (
            format!("echo %{MCP_STARTUP_ENV_VAR}%"),
            format!("{MCP_STARTUP_ENV_VALUE}\r\n"),
        ),
        _ => (
            format!("printf '%s\\n' \"${MCP_STARTUP_ENV_VAR}\""),
            format!("{MCP_STARTUP_ENV_VALUE}\n"),
        ),
    }
}

fn create_exact_shell_command_sse_response(command: String, call_id: &str) -> Result<String> {
    let tool_call_arguments = serde_json::to_string(&json!({
        "command": command,
        "workdir": null,
        "timeout_ms": 5000
    }))?;
    Ok(responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_function_call(call_id, "shell_command", &tool_call_arguments),
        responses::ev_completed("resp-1"),
    ]))
}

async fn wait_for_command_execution_completed(
    mcp: &mut McpProcess,
) -> Result<ItemCompletedNotification> {
    loop {
        let notif = mcp
            .read_stream_until_notification_message("item/completed")
            .await?;
        let completed: ItemCompletedNotification = serde_json::from_value(
            notif
                .params
                .ok_or_else(|| anyhow::anyhow!("missing item/completed params"))?,
        )?;
        if matches!(completed.item, ThreadItem::CommandExecution { .. }) {
            return Ok(completed);
        }
    }
}
