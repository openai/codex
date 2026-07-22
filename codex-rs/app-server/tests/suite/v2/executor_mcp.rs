use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use axum::Json;
use axum::Router;
use axum::body::Bytes;
use axum::routing::get;
use axum::routing::post;
use codex_app_server_protocol::CapabilityRootLocation;
use codex_app_server_protocol::ListMcpServerStatusParams;
use codex_app_server_protocol::ListMcpServerStatusResponse;
use codex_app_server_protocol::McpServerOauthLoginCompletedNotification;
use codex_app_server_protocol::McpServerOauthLoginResponse;
use codex_app_server_protocol::McpServerToolCallParams;
use codex_app_server_protocol::McpServerToolCallResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SelectedCapabilityRoot;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput;
use codex_utils_path_uri::PathUri;
use core_test_support::responses;
use core_test_support::stdio_server_bin;
use pretty_assertions::assert_eq;
use rmcp::handler::server::ServerHandler;
use rmcp::model::CallToolRequestParams;
use rmcp::model::CallToolResult;
use rmcp::model::JsonObject;
use rmcp::model::ListToolsResult;
use rmcp::model::ServerCapabilities;
use rmcp::model::ServerInfo;
use rmcp::model::Tool;
use rmcp::model::ToolAnnotations;
use rmcp::service::RequestContext;
use rmcp::service::RoleServer;
use rmcp::transport::StreamableHttpServerConfig;
use rmcp::transport::StreamableHttpService;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use serde_json::json;
use std::borrow::Cow;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(20);
const EXECUTOR_HTTP_MCP_URL: &str = "http://executor-only.invalid/mcp";
const HTTP_MCP_SERVER_NAME: &str = "executor_http";
const MCP_SERVER_NAME: &str = "executor_demo";
const OAUTH_MCP_SERVER_NAME: &str = "executor_oauth";
const EXECUTOR_OAUTH_MCP_URL: &str = "http://oauth-only.invalid/oauth-mcp";
const EXECUTOR_ENV_NAME: &str = "MCP_EXECUTOR_MARKER";
const EXECUTOR_ENV_VALUE: &str = "executor-only";
const EXECUTOR_ID: &str = "executor-1";
const REFRESH_PROBE_SERVER_NAME: &str = "refresh_probe";
const TOOL_CALL_ID: &str = "executor-mcp-call";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn selected_executor_plugin_exposes_its_mcps_only_to_that_thread() -> Result<()> {
    let responses_server = responses::start_mock_server().await;
    let http_listener = TcpListener::bind("127.0.0.1:0").await?;
    let http_addr = http_listener.local_addr()?;
    let http_server_config = StreamableHttpServerConfig::default()
        .with_allowed_hosts(["executor-only.invalid", "oauth-only.invalid"]);
    let http_mcp_service = StreamableHttpService::new(
        || Ok(ExecutorHttpMcpServer),
        Arc::new(LocalSessionManager::default()),
        http_server_config.clone(),
    );
    let oauth_mcp_service = StreamableHttpService::new(
        || Ok(ExecutorHttpMcpServer),
        Arc::new(LocalSessionManager::default()),
        http_server_config,
    );
    let (token_request_tx, mut token_request_rx) = mpsc::unbounded_channel();
    let oauth_metadata = json!({
        "authorization_endpoint": "https://oauth-only.invalid/authorize",
        "token_endpoint": "http://oauth-only.invalid/token",
        "scopes_supported": ["read", "write"],
        "response_types_supported": ["code"],
        "code_challenge_methods_supported": ["S256"],
    });
    let http_router = Router::new()
        .route(
            "/.well-known/oauth-authorization-server/oauth-mcp",
            get(move || {
                let metadata = oauth_metadata.clone();
                async move { Json(metadata) }
            }),
        )
        .route(
            "/token",
            post(move |body: Bytes| {
                let token_request_tx = token_request_tx.clone();
                async move {
                    let _ = token_request_tx.send(String::from_utf8_lossy(&body).into_owned());
                    Json(json!({
                        "access_token": "executor-access-token",
                        "token_type": "Bearer",
                        "expires_in": 3600,
                        "refresh_token": "executor-refresh-token",
                    }))
                }
            }),
        )
        .nest_service("/mcp", http_mcp_service)
        .nest_service("/oauth-mcp", oauth_mcp_service);
    let http_server_handle = tokio::spawn(async move {
        let _ = axum::serve(http_listener, http_router).await;
    });
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&responses_server.uri())
        .with_root_config("compact_prompt = \"compact\"\nmodel_auto_compact_token_limit = 1024")
        .with_provider_config("supports_websockets = false")
        .write(codex_home.path())?;
    let codex_bin = toml::Value::String(
        codex_utils_cargo_bin::cargo_bin("codex")?
            .to_string_lossy()
            .into_owned(),
    );
    let http_proxy = toml::Value::String(format!("http://{http_addr}"));
    std::fs::write(
        codex_home.path().join("environments.toml"),
        format!(
            r#"
include_local = true

[[environments]]
id = "{EXECUTOR_ID}"
program = {codex_bin}
args = ["exec-server", "--listen", "stdio"]
[environments.env]
{EXECUTOR_ENV_NAME} = "{EXECUTOR_ENV_VALUE}"
HTTP_PROXY = {http_proxy}
"#
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
                },
                (HTTP_MCP_SERVER_NAME): {
                    "url": EXECUTOR_HTTP_MCP_URL,
                    "environment_id": "local",
                    "startup_timeout_sec": 10,
                },
                (OAUTH_MCP_SERVER_NAME): {
                    "url": EXECUTOR_OAUTH_MCP_URL,
                    "environment_id": "local",
                    "oauth": {"clientId": "executor-oauth-client"},
                    "startup_timeout_sec": 10,
                }
            }
        }))?,
    )?;

    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        // This suite owns environments.toml to exercise explicit executor selection.
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;

    let selected_thread = start_thread(
        &mut app_server,
        Some(vec![SelectedCapabilityRoot {
            id: "executor-demo@1".to_string(),
            location: CapabilityRootLocation::Environment {
                environment_id: EXECUTOR_ID.to_string(),
                path: PathUri::from_host_native_path(plugin.path())?,
            },
        }]),
    )
    .await?;

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

    let request_id = app_server
        .send_raw_request(
            "mcpServer/oauth/login",
            Some(json!({
                "name": OAUTH_MCP_SERVER_NAME,
                "threadId": selected_thread.clone(),
                "timeoutSecs": 10,
            })),
        )
        .await?;
    let response: McpServerOauthLoginResponse =
        timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(request_id)).await??;
    assert!(
        response
            .authorization_url
            .starts_with("https://oauth-only.invalid/authorize?")
    );
    assert!(
        response
            .authorization_url
            .contains("client_id=executor-oauth-client")
    );
    let authorization_url = reqwest::Url::parse(&response.authorization_url)?;
    let state = authorization_url
        .query_pairs()
        .find_map(|(key, value)| (key == "state").then(|| value.into_owned()))
        .expect("authorization URL should include state");
    let redirect_uri = authorization_url
        .query_pairs()
        .find_map(|(key, value)| (key == "redirect_uri").then(|| value.into_owned()))
        .expect("authorization URL should include redirect_uri");
    let mut callback_url = reqwest::Url::parse(&redirect_uri)?;
    callback_url
        .query_pairs_mut()
        .append_pair("code", "executor-test-code")
        .append_pair("state", &state);
    reqwest::Client::builder()
        .no_proxy()
        .build()?
        .get(callback_url)
        .send()
        .await?
        .error_for_status()?;
    let token_request = timeout(DEFAULT_READ_TIMEOUT, token_request_rx.recv())
        .await?
        .expect("executor token endpoint should receive a request");
    assert!(token_request.contains("grant_type=authorization_code"));
    assert!(token_request.contains("code=executor-test-code"));
    assert!(token_request.contains("code_verifier="));
    let completed: McpServerOauthLoginCompletedNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_notification("mcpServer/oauthLogin/completed"),
    )
    .await??;
    assert_eq!(
        completed,
        McpServerOauthLoginCompletedNotification {
            name: OAUTH_MCP_SERVER_NAME.to_string(),
            thread_id: Some(selected_thread.clone()),
            success: true,
            error: None,
        }
    );

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
    let _: TurnStartResponse =
        timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(request_id)).await??;
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
            server: HTTP_MCP_SERVER_NAME.to_string(),
            tool: "echo".to_string(),
            arguments: Some(json!({"message": "hello over executor HTTP"})),
            meta: None,
        })
        .await?;
    let response: McpServerToolCallResponse =
        timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(request_id)).await??;
    assert_eq!(
        response.structured_content,
        Some(json!({"echo": "ECHOING: hello over executor HTTP"}))
    );

    let request_id = app_server
        .send_mcp_server_tool_call_request(McpServerToolCallParams {
            thread_id: selected_thread.clone(),
            server: REFRESH_PROBE_SERVER_NAME.to_string(),
            tool: "echo".to_string(),
            arguments: Some(json!({"message": "refresh applied"})),
            meta: None,
        })
        .await?;
    let response: McpServerToolCallResponse =
        timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(request_id)).await??;
    assert_eq!(
        response
            .structured_content
            .and_then(|content| content.get("echo").cloned()),
        Some(json!("ECHOING: refresh applied"))
    );

    let selected_server_names = mcp_server_names(&mut app_server, selected_thread).await?;
    assert!(
        selected_server_names
            .iter()
            .any(|name| name == MCP_SERVER_NAME)
    );
    assert!(
        selected_server_names
            .iter()
            .any(|name| name == HTTP_MCP_SERVER_NAME)
    );
    assert!(
        selected_server_names
            .iter()
            .any(|name| name == OAUTH_MCP_SERVER_NAME)
    );

    let unselected_thread =
        start_thread(&mut app_server, /*selected_capability_roots*/ None).await?;
    let unselected_server_names = mcp_server_names(&mut app_server, unselected_thread).await?;
    assert!(unselected_server_names.iter().all(|name| {
        name != MCP_SERVER_NAME && name != HTTP_MCP_SERVER_NAME && name != OAUTH_MCP_SERVER_NAME
    }));

    http_server_handle.abort();
    let _ = http_server_handle.await;

    Ok(())
}

#[derive(Clone, Copy)]
struct ExecutorHttpMcpServer;

impl ServerHandler for ExecutorHttpMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
    }

    async fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, rmcp::ErrorData> {
        let input_schema: JsonObject = serde_json::from_value(json!({
            "type": "object",
            "properties": {"message": {"type": "string"}},
            "required": ["message"],
            "additionalProperties": false
        }))
        .map_err(|err| rmcp::ErrorData::internal_error(err.to_string(), None))?;
        let mut tool = Tool::new(
            Cow::Borrowed("echo"),
            Cow::Borrowed("Echo a message."),
            Arc::new(input_schema),
        );
        tool.annotations = Some(ToolAnnotations::new().read_only(true));

        Ok(ListToolsResult {
            tools: vec![tool],
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let message = request
            .arguments
            .as_ref()
            .and_then(|arguments| arguments.get("message"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        Ok(CallToolResult::structured(json!({
            "echo": format!("ECHOING: {message}")
        })))
    }
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
    let response: ListMcpServerStatusResponse =
        timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(request_id)).await??;
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
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(request_id)).await??;
    Ok(thread.id)
}
