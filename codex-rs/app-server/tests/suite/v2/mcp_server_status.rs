use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use axum::Router;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ListMcpServerStatusParams;
use codex_app_server_protocol::ListMcpServerStatusResponse;
use codex_app_server_protocol::McpServerStatusDetail;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_core::config::set_project_trust_level;
use codex_protocol::config_types::TrustLevel;
use core_test_support::stdio_server_bin;
use pretty_assertions::assert_eq;
use rmcp::handler::server::ServerHandler;
use rmcp::model::Implementation;
use rmcp::model::JsonObject;
use rmcp::model::ListResourceTemplatesResult;
use rmcp::model::ListResourcesResult;
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
use tokio::time::sleep;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);

async fn wait_for_new_pid(path: &Path, previous_pid: Option<&str>) -> Result<String> {
    Ok(timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            if let Ok(contents) = std::fs::read_to_string(path) {
                let pid = contents.trim();
                if !pid.is_empty() && Some(pid) != previous_pid {
                    return pid.to_string();
                }
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await?)
}

fn assert_dynamic_status(response: &ListMcpServerStatusResponse, process_label: &str) {
    assert_eq!(response.data.len(), 1);
    let status = &response.data[0];
    assert_eq!(status.name, "cached-stdio");
    assert_eq!(
        status
            .server_info
            .as_ref()
            .and_then(|info| info.title.as_deref()),
        Some(process_label)
    );
    assert_eq!(
        status
            .tools
            .get("echo")
            .and_then(|tool| tool.description.as_deref()),
        Some(format!("Echo from {process_label}.").as_str())
    );
}

#[tokio::test]
async fn mcp_server_status_list_returns_raw_server_and_tool_names() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let (mcp_server_url, mcp_server_handle) = start_mcp_server("look-up.raw").await?;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri())
        .with_extra_config(&format!(
            "[mcp_servers.some-server]\nurl = \"{mcp_server_url}/mcp\""
        ))
        .write(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;
    let response: ListMcpServerStatusResponse = mcp
        .request(|request_id| ClientRequest::McpServerStatusList {
            request_id,
            params: ListMcpServerStatusParams {
                cursor: None,
                limit: None,
                detail: None,
                thread_id: None,
            },
        })
        .await?;

    assert_eq!(response.next_cursor, None);
    assert_eq!(response.data.len(), 1);
    let status = &response.data[0];
    assert_eq!(status.name, "some-server");
    assert_eq!(
        status.tools.keys().cloned().collect::<BTreeSet<_>>(),
        BTreeSet::from(["look-up.raw".to_string()])
    );
    assert_eq!(
        status
            .tools
            .get("look-up.raw")
            .map(|tool| tool.name.as_str()),
        Some("look-up.raw")
    );
    assert_eq!(
        status
            .server_info
            .as_ref()
            .and_then(|info| info.title.as_deref()),
        Some("Lookup Server")
    );

    mcp_server_handle.abort();
    let _ = mcp_server_handle.await;

    Ok(())
}

#[tokio::test]
async fn mcp_server_status_list_waits_for_live_stdio_metadata_before_using_cached_tools()
-> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    let barrier_file = codex_home.path().join("allow-initialize");
    let pid_file = codex_home.path().join("mcp.pid");
    std::fs::write(&barrier_file, "ready")?;
    mock_responses_config(&server.uri())
        .with_extra_config(&format!(
            r#"[mcp_servers.cached-stdio]
command = {}
enabled_tools = ["echo"]
startup_timeout_sec = 10

[mcp_servers.cached-stdio.env]
MCP_TEST_DYNAMIC_SERVER_METADATA = "1"
MCP_TEST_INITIALIZE_BARRIER_FILE = {}
MCP_TEST_PID_FILE = {}
"#,
            toml::Value::String(stdio_server_bin()?),
            toml::Value::String(barrier_file.to_string_lossy().into_owned()),
            toml::Value::String(pid_file.to_string_lossy().into_owned()),
        ))
        .write(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;
    let first_response: ListMcpServerStatusResponse = mcp
        .request(|request_id| ClientRequest::McpServerStatusList {
            request_id,
            params: ListMcpServerStatusParams {
                cursor: None,
                limit: None,
                detail: Some(McpServerStatusDetail::ToolsAndAuthOnly),
                thread_id: None,
            },
        })
        .await?;
    let first_pid = wait_for_new_pid(&pid_file, /*previous_pid*/ None).await?;
    assert_dynamic_status(&first_response, &format!("rmcp-test-process-{first_pid}"));

    std::fs::remove_file(&barrier_file)?;
    let second_request_id = mcp
        .send_list_mcp_server_status_request(ListMcpServerStatusParams {
            cursor: None,
            limit: None,
            detail: Some(McpServerStatusDetail::ToolsAndAuthOnly),
            thread_id: None,
        })
        .await?;
    let second_pid = wait_for_new_pid(&pid_file, Some(&first_pid)).await?;
    assert!(
        timeout(
            Duration::from_millis(200),
            mcp.read_stream_until_response_message(RequestId::Integer(second_request_id)),
        )
        .await
        .is_err(),
        "status/list should wait for the live stdio server to initialize"
    );

    std::fs::write(&barrier_file, "ready")?;
    let second_response: ListMcpServerStatusResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(second_request_id)).await??;
    assert_dynamic_status(&second_response, &format!("rmcp-test-process-{second_pid}"));

    Ok(())
}

#[tokio::test]
async fn mcp_server_status_list_uses_thread_project_local_config() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let (mcp_server_url, mcp_server_handle) = start_mcp_server("project_lookup").await?;
    let codex_home = TempDir::new()?;
    let workspace = TempDir::new()?;
    mock_responses_config(&server.uri()).write(codex_home.path())?;
    std::fs::create_dir_all(workspace.path().join(".git"))?;
    set_project_trust_level(codex_home.path(), workspace.path(), TrustLevel::Trusted)?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let ThreadStartResponse { thread, .. } = mcp
        .start_thread(ThreadStartParams {
            cwd: Some(workspace.path().to_string_lossy().into_owned()),
            ..Default::default()
        })
        .await?;

    let project_config_dir = workspace.path().join(".codex");
    std::fs::create_dir_all(&project_config_dir)?;
    std::fs::write(
        project_config_dir.join("config.toml"),
        format!(
            r#"
[mcp_servers.project-server]
url = "{mcp_server_url}/mcp"
"#
        ),
    )?;

    let threadless_response: ListMcpServerStatusResponse = mcp
        .request(|request_id| ClientRequest::McpServerStatusList {
            request_id,
            params: ListMcpServerStatusParams {
                cursor: None,
                limit: None,
                detail: Some(McpServerStatusDetail::ToolsAndAuthOnly),
                thread_id: None,
            },
        })
        .await?;
    assert_eq!(threadless_response.data, Vec::new());

    let thread_response: ListMcpServerStatusResponse = mcp
        .request(|request_id| ClientRequest::McpServerStatusList {
            request_id,
            params: ListMcpServerStatusParams {
                cursor: None,
                limit: None,
                detail: Some(McpServerStatusDetail::ToolsAndAuthOnly),
                thread_id: Some(thread.id),
            },
        })
        .await?;

    assert_eq!(thread_response.next_cursor, None);
    assert_eq!(thread_response.data.len(), 1);
    let status = &thread_response.data[0];
    assert_eq!(status.name, "project-server");
    assert_eq!(
        status.tools.keys().cloned().collect::<BTreeSet<_>>(),
        BTreeSet::from(["project_lookup".to_string()])
    );

    mcp_server_handle.abort();
    let _ = mcp_server_handle.await;

    Ok(())
}

#[derive(Clone)]
struct McpStatusServer {
    tool_name: Arc<String>,
}

impl ServerHandler for McpStatusServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(
            Implementation::new("lookup-server", "1.0.0").with_title("Lookup Server"),
        )
    }

    async fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> Result<ListToolsResult, rmcp::ErrorData> {
        let input_schema: JsonObject = serde_json::from_value(json!({
            "type": "object",
            "additionalProperties": false
        }))
        .map_err(|err| rmcp::ErrorData::internal_error(err.to_string(), None))?;

        let mut tool = Tool::new(
            Cow::Owned(self.tool_name.as_ref().clone()),
            Cow::Borrowed("Look up test data."),
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

#[derive(Clone)]
struct SlowInventoryServer {
    tool_name: Arc<String>,
}

impl ServerHandler for SlowInventoryServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<rmcp::service::RoleServer>,
    ) -> Result<ListToolsResult, rmcp::ErrorData> {
        let input_schema: JsonObject = serde_json::from_value(json!({
            "type": "object",
            "additionalProperties": false
        }))
        .map_err(|err| rmcp::ErrorData::internal_error(err.to_string(), None))?;

        let mut tool = Tool::new(
            Cow::Owned(self.tool_name.as_ref().clone()),
            Cow::Borrowed("Look up test data."),
            Arc::new(input_schema),
        );
        tool.annotations = Some(ToolAnnotations::new().read_only(true));

        Ok(ListToolsResult {
            tools: vec![tool],
            next_cursor: None,
            meta: None,
        })
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<rmcp::service::RoleServer>,
    ) -> Result<ListResourcesResult, rmcp::ErrorData> {
        tokio::time::sleep(Duration::from_secs(2)).await;
        Ok(ListResourcesResult {
            resources: Vec::new(),
            next_cursor: None,
            meta: None,
        })
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<rmcp::service::RoleServer>,
    ) -> Result<ListResourceTemplatesResult, rmcp::ErrorData> {
        tokio::time::sleep(Duration::from_secs(2)).await;
        Ok(ListResourceTemplatesResult {
            resource_templates: Vec::new(),
            next_cursor: None,
            meta: None,
        })
    }
}

#[tokio::test]
async fn mcp_server_status_list_tools_and_auth_only_skips_slow_inventory_calls() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let (mcp_server_url, mcp_server_handle) = start_slow_inventory_mcp_server("lookup").await?;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri())
        .with_extra_config(&format!(
            "[mcp_servers.some-server]\nurl = \"{mcp_server_url}/mcp\""
        ))
        .write(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;

    let request_id = mcp
        .send_list_mcp_server_status_request(ListMcpServerStatusParams {
            cursor: None,
            limit: None,
            detail: Some(McpServerStatusDetail::ToolsAndAuthOnly),
            thread_id: None,
        })
        .await?;
    let response: ListMcpServerStatusResponse =
        timeout(Duration::from_millis(500), mcp.read_response(request_id)).await??;

    assert_eq!(response.next_cursor, None);
    assert_eq!(response.data.len(), 1);
    let status = &response.data[0];
    assert_eq!(status.name, "some-server");
    assert_eq!(
        status.tools.keys().cloned().collect::<BTreeSet<_>>(),
        BTreeSet::from(["lookup".to_string()])
    );
    assert_eq!(status.resources, Vec::new());
    assert_eq!(status.resource_templates, Vec::new());

    mcp_server_handle.abort();
    let _ = mcp_server_handle.await;

    Ok(())
}

#[tokio::test]
async fn mcp_server_status_list_keeps_tools_for_sanitized_name_collisions() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let (dash_server_url, dash_server_handle) = start_mcp_server("dash_lookup").await?;
    let (underscore_server_url, underscore_server_handle) =
        start_mcp_server("underscore_lookup").await?;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri())
        .with_extra_config(&format!(
            r#"[mcp_servers.some-server]
url = "{dash_server_url}/mcp"

[mcp_servers.some_server]
url = "{underscore_server_url}/mcp"
"#
        ))
        .write(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;
    let response: ListMcpServerStatusResponse = mcp
        .request(|request_id| ClientRequest::McpServerStatusList {
            request_id,
            params: ListMcpServerStatusParams {
                cursor: None,
                limit: None,
                detail: None,
                thread_id: None,
            },
        })
        .await?;

    assert_eq!(response.next_cursor, None);
    assert_eq!(response.data.len(), 2);
    let status_tools = response
        .data
        .iter()
        .map(|status| {
            (
                status.name.as_str(),
                status.tools.keys().cloned().collect::<BTreeSet<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        status_tools,
        BTreeMap::from([
            ("some-server", BTreeSet::from(["dash_lookup".to_string()])),
            (
                "some_server",
                BTreeSet::from(["underscore_lookup".to_string()])
            )
        ])
    );

    dash_server_handle.abort();
    let _ = dash_server_handle.await;
    underscore_server_handle.abort();
    let _ = underscore_server_handle.await;

    Ok(())
}

async fn start_mcp_server(tool_name: &str) -> Result<(String, JoinHandle<()>)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let tool_name = Arc::new(tool_name.to_string());
    let mcp_service = StreamableHttpService::new(
        move || {
            Ok(McpStatusServer {
                tool_name: Arc::clone(&tool_name),
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

async fn start_slow_inventory_mcp_server(tool_name: &str) -> Result<(String, JoinHandle<()>)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let tool_name = Arc::new(tool_name.to_string());
    let mcp_service = StreamableHttpService::new(
        move || {
            Ok(SlowInventoryServer {
                tool_name: Arc::clone(&tool_name),
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

fn mock_responses_config(server_uri: &str) -> MockResponsesConfig {
    MockResponsesConfig::new(server_uri)
        .with_root_config("compact_prompt = \"compact\"\nmodel_auto_compact_token_limit = 1024")
        .with_provider_config("supports_websockets = false")
}
