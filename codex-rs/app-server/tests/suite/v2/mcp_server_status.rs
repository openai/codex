use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::process::Command as StdCommand;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use app_test_support::McpProcess;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use app_test_support::to_response;
use app_test_support::write_mock_responses_config_toml;
use axum::Router;
use codex_app_server_protocol::ListMcpServerStatusParams;
use codex_app_server_protocol::ListMcpServerStatusResponse;
use codex_app_server_protocol::McpAuthStatus;
use codex_app_server_protocol::McpServerOauthLoginCompletedNotification;
use codex_app_server_protocol::McpServerOauthLoginResponse;
use codex_app_server_protocol::McpServerStatusDetail;
use codex_app_server_protocol::RequestId;
use codex_utils_cargo_bin::cargo_bin;
use core_test_support::remote_env_env_var;
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

const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);
const REMOTE_EXEC_SERVER_URL_ENV_VAR: &str = "CODEX_TEST_REMOTE_EXEC_SERVER_URL";

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

#[tokio::test]
async fn remote_streamable_http_oauth_status_and_login_use_selected_environment() -> Result<()> {
    let Some(container_name) = remote_env_container_name()? else {
        return Ok(());
    };
    let exec_server_url = std::env::var(REMOTE_EXEC_SERVER_URL_ENV_VAR).with_context(|| {
        format!("{REMOTE_EXEC_SERVER_URL_ENV_VAR} must be set for remote MCP OAuth E2E")
    })?;
    let remote_mcp = RemoteStreamableHttpServer::start(&container_name)?;
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
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
    std::fs::write(
        codex_home.path().join("environments.toml"),
        format!(
            r#"include_local = false

[[environments]]
id = "remote"
url = "{exec_server_url}"
"#
        ),
    )?;

    let config_path = codex_home.path().join("config.toml");
    let mut config_toml = std::fs::read_to_string(&config_path)?;
    config_toml.push_str(&format!(
        r#"
[mcp_servers.remote-oauth]
url = "{}/mcp"
environment_id = "remote"

[mcp_servers.remote-oauth.oauth]
client_id = "codex-app-server-test"
"#,
        remote_mcp.server_url()
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
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: ListMcpServerStatusResponse = to_response(response)?;

    assert_eq!(response.next_cursor, None);
    assert_eq!(response.data.len(), 1);
    let status = &response.data[0];
    assert_eq!(status.name, "remote-oauth");
    assert_eq!(status.auth_status, McpAuthStatus::NotLoggedIn);
    assert_eq!(
        status.tools.keys().cloned().collect::<BTreeSet<_>>(),
        BTreeSet::from(["echo".to_string()])
    );

    let request_id = mcp
        .send_raw_request(
            "mcpServer/oauth/login",
            Some(json!({
                "name": "remote-oauth",
            })),
        )
        .await?;
    let response = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: McpServerOauthLoginResponse = to_response(response)?;
    assert!(
        response
            .authorization_url
            .starts_with(&format!("{}/oauth/authorize?", remote_mcp.server_url()))
    );
    let browser_response = reqwest::Client::new()
        .get(&response.authorization_url)
        .send()
        .await?;
    assert_eq!(browser_response.status(), reqwest::StatusCode::OK);

    let notification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("mcpServer/oauthLogin/completed"),
    )
    .await??;
    let notification: McpServerOauthLoginCompletedNotification =
        serde_json::from_value(notification.params.context("oauth completion params")?)?;
    assert_eq!(
        notification,
        McpServerOauthLoginCompletedNotification {
            name: "remote-oauth".to_string(),
            success: true,
            error: None,
        }
    );

    let request_id = mcp
        .send_list_mcp_server_status_request(ListMcpServerStatusParams {
            cursor: None,
            limit: None,
            detail: Some(McpServerStatusDetail::ToolsAndAuthOnly),
        })
        .await?;
    let response = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: ListMcpServerStatusResponse = to_response(response)?;
    assert_eq!(response.data[0].auth_status, McpAuthStatus::OAuth);

    Ok(())
}

struct RemoteStreamableHttpServer {
    container_name: String,
    pid: String,
    remote_path: String,
    bound_addr_file: String,
    log_file: String,
    server_url: String,
}

impl RemoteStreamableHttpServer {
    fn start(container_name: &str) -> Result<Self> {
        let host_path = cargo_bin("test_streamable_http_server")
            .context("should find test_streamable_http_server binary")?;
        let remote_path = unique_remote_path("test_streamable_http_server")?;
        let bound_addr_file = format!("{remote_path}.addr");
        let log_file = format!("{remote_path}.log");
        docker_exec(container_name, ["mkdir", "-p", "/tmp/codex-remote-env"])?;
        docker_cp(container_name, host_path.as_path(), &remote_path)?;
        docker_exec(container_name, ["chmod", "+x", remote_path.as_str()])?;

        let script = format!(
            "MCP_STREAMABLE_HTTP_BIND_ADDR={} MCP_STREAMABLE_HTTP_BOUND_ADDR_FILE={} nohup {} > {} 2>&1 < /dev/null & echo $!",
            sh_single_quote("127.0.0.1:0"),
            sh_single_quote(&bound_addr_file),
            sh_single_quote(&remote_path),
            sh_single_quote(&log_file),
        );
        let output = StdCommand::new("docker")
            .args(["exec", container_name, "sh", "-lc", &script])
            .output()
            .context("start remote OAuth MCP test server")?;
        ensure!(
            output.status.success(),
            "docker start remote OAuth MCP test server failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
        let pid = String::from_utf8(output.stdout)
            .context("remote OAuth MCP test server pid must be utf-8")?
            .trim()
            .to_string();
        ensure!(!pid.is_empty(), "remote OAuth MCP test server pid is empty");
        let bound_addr = wait_for_remote_bound_addr(container_name, &bound_addr_file)?;
        Ok(Self {
            container_name: container_name.to_string(),
            pid,
            remote_path,
            bound_addr_file,
            log_file,
            server_url: format!("http://127.0.0.1:{}", bound_addr.port()),
        })
    }

    fn server_url(&self) -> &str {
        &self.server_url
    }
}

impl Drop for RemoteStreamableHttpServer {
    fn drop(&mut self) {
        let _ = docker_exec(&self.container_name, ["kill", self.pid.as_str()]);
        let script = format!(
            "rm -f {} {} {}",
            sh_single_quote(&self.remote_path),
            sh_single_quote(&self.bound_addr_file),
            sh_single_quote(&self.log_file),
        );
        let _ = StdCommand::new("docker")
            .args(["exec", &self.container_name, "sh", "-lc", &script])
            .output();
    }
}

fn remote_env_container_name() -> Result<Option<String>> {
    let Some(container_name) = std::env::var_os(remote_env_env_var()) else {
        return Ok(None);
    };
    Ok(Some(container_name.into_string().map_err(|value| {
        anyhow::anyhow!("remote env container name must be utf-8: {value:?}")
    })?))
}

fn unique_remote_path(binary_name: &str) -> Result<String> {
    let unique_suffix = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    Ok(format!(
        "/tmp/codex-remote-env/{binary_name}-{}-{unique_suffix}",
        std::process::id()
    ))
}

fn docker_cp(container_name: &str, host_path: &std::path::Path, remote_path: &str) -> Result<()> {
    let output = StdCommand::new("docker")
        .arg("cp")
        .arg(host_path)
        .arg(format!("{container_name}:{remote_path}"))
        .output()
        .with_context(|| format!("copy {} to remote MCP test env", host_path.display()))?;
    ensure!(
        output.status.success(),
        "docker cp test_streamable_http_server failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout).trim(),
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(())
}

fn docker_exec<const N: usize>(container_name: &str, args: [&str; N]) -> Result<()> {
    let output = StdCommand::new("docker")
        .arg("exec")
        .arg(container_name)
        .args(args)
        .output()
        .context("run docker exec for remote MCP OAuth E2E")?;
    ensure!(
        output.status.success(),
        "docker exec failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout).trim(),
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(())
}

fn wait_for_remote_bound_addr(
    container_name: &str,
    bound_addr_file: &str,
) -> Result<std::net::SocketAddr> {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let output = StdCommand::new("docker")
            .args(["exec", container_name, "cat", bound_addr_file])
            .output()
            .context("read remote OAuth MCP test server bound address")?;
        if output.status.success() {
            return String::from_utf8(output.stdout)
                .context("remote OAuth MCP bound address must be utf-8")?
                .trim()
                .parse()
                .context("parse remote OAuth MCP bound address");
        }
        if std::time::Instant::now() >= deadline {
            return Err(anyhow::anyhow!(
                "timed out waiting for remote OAuth MCP bound address: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn sh_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
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
