use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use app_test_support::to_response;
use app_test_support::write_mock_responses_config_toml;
use axum::Router;
use codex_app_server_protocol::ListMcpServerStatusParams;
use codex_app_server_protocol::ListMcpServerStatusResponse;
use codex_app_server_protocol::McpServerStatusDetail;
use codex_app_server_protocol::RequestId;
use pretty_assertions::assert_eq;
use rmcp::handler::server::ServerHandler;
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
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(30);

#[tokio::test]
async fn mcp_server_status_list_returns_raw_server_and_tool_names() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let (mcp_server_url, mcp_server_handle) = start_mcp_server("look-up.raw").await?;
    let codex_home = TempDir::new()?;
    write_mock_responses_config_toml(
        codex_home.path(),
        &server.uri(),
        &BTreeMap::new(),
        /*auto_compact_limit*/ 1024,
        /*requires_openai_auth*/ None,
        "mock_provider",
        "compact",
    )?;

    let config_path = codex_home.path().join("config.toml");
    let mut config_toml = std::fs::read_to_string(&config_path)?;
    config_toml.push_str(&format!(
        r#"
[mcp_servers.some-server]
url = "{mcp_server_url}/mcp"
"#
    ));
    std::fs::write(config_path, config_toml)?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_list_mcp_server_status_request(ListMcpServerStatusParams {
            cursor: None,
            limit: None,
            detail: None,
        })
        .await?;
    let response = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: ListMcpServerStatusResponse = to_response(response)?;

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
        ServerInfo {
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..ServerInfo::default()
        }
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
        ServerInfo {
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
            ..ServerInfo::default()
        }
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
    write_mock_responses_config_toml(
        codex_home.path(),
        &server.uri(),
        &BTreeMap::new(),
        /*auto_compact_limit*/ 1024,
        /*requires_openai_auth*/ None,
        "mock_provider",
        "compact",
    )?;

    let config_path = codex_home.path().join("config.toml");
    let mut config_toml = std::fs::read_to_string(&config_path)?;
    config_toml.push_str(&format!(
        r#"
[mcp_servers.some-server]
url = "{mcp_server_url}/mcp"
"#
    ));
    std::fs::write(config_path, config_toml)?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_list_mcp_server_status_request(ListMcpServerStatusParams {
            cursor: None,
            limit: None,
            detail: Some(McpServerStatusDetail::ToolsAndAuthOnly),
        })
        .await?;
    let response = timeout(
        Duration::from_millis(500),
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: ListMcpServerStatusResponse = to_response(response)?;

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
    write_mock_responses_config_toml(
        codex_home.path(),
        &server.uri(),
        &BTreeMap::new(),
        /*auto_compact_limit*/ 1024,
        /*requires_openai_auth*/ None,
        "mock_provider",
        "compact",
    )?;

    let config_path = codex_home.path().join("config.toml");
    let mut config_toml = std::fs::read_to_string(&config_path)?;
    config_toml.push_str(&format!(
        r#"
[mcp_servers.some-server]
url = "{dash_server_url}/mcp"

[mcp_servers.some_server]
url = "{underscore_server_url}/mcp"
"#
    ));
    std::fs::write(config_path, config_toml)?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_READ_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_list_mcp_server_status_request(ListMcpServerStatusParams {
            cursor: None,
            limit: None,
            detail: None,
        })
        .await?;
    let response = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: ListMcpServerStatusResponse = to_response(response)?;

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
