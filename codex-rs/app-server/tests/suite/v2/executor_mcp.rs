use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use axum::Json;
use axum::Router;
use axum::body::Bytes;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::routing::post;
use codex_app_server_protocol::CapabilityRootLocation;
use codex_app_server_protocol::ListMcpServerStatusParams;
use codex_app_server_protocol::ListMcpServerStatusResponse;
use codex_app_server_protocol::McpServerOauthLoginCompletedNotification;
use codex_app_server_protocol::McpServerOauthLoginResponse;
use codex_app_server_protocol::McpServerStatus;
use codex_app_server_protocol::McpServerToolCallParams;
use codex_app_server_protocol::McpServerToolCallResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::SelectedCapabilityRoot;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput;
use codex_features::Feature;
use codex_http_client::HttpClientBuilder;
use codex_utils_path_uri::PathUri;
use core_test_support::responses;
use core_test_support::stdio_server_bin;
use futures::SinkExt;
use futures::StreamExt;
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
use serde_json::Value;
use serde_json::json;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tempfile::TempDir;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::net::TcpListener;
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use url::Url;

const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(20);
const EXECUTOR_HTTP_MCP_URL: &str = "http://executor-only.invalid/mcp";
const HTTP_MCP_SERVER_NAME: &str = "executor_http";
const MCP_SERVER_NAME: &str = "executor_demo";
const OAUTH_MCP_SERVER_NAME: &str = "executor_oauth";
const PRE_REGISTERED_OAUTH_MCP_SERVER_NAME: &str = "executor_oauth_preregistered";
const EXECUTOR_OAUTH_MCP_URL: &str = "http://oauth-only.invalid/oauth-mcp";
const HOST_OAUTH_ACCESS_TOKEN: &str = "host-access-token";
const EXECUTOR_OAUTH_ACCESS_TOKEN: &str = "executor-access-token";
const EXECUTOR_ENV_NAME: &str = "MCP_EXECUTOR_MARKER";
const EXECUTOR_ENV_VALUE: &str = "executor-only";
const EXECUTOR_HTTP_AUTH_ENV_NAME: &str = "NODE_REPL_AUTH_TOKEN";
const EXECUTOR_HTTP_AUTH_ENV_VALUE: &str = "executor-only-http-token";
const EXECUTOR_ID: &str = "executor-1";
const EXECUTOR_DISABLED_PLUGIN_SERVER_NAME: &str = "executor_disabled_plugin";
const PROJECT_MCP_SERVER_NAME: &str = "node_repl";
const PROJECT_MCP_BEARER_TOKEN: &str = "executor-browser-token";
const PROJECT_MCP_BEARER_ENV_NAME: &str = "NODE_REPL_AUTH_TOKEN";
const REFRESH_PROBE_SERVER_NAME: &str = "refresh_probe";
const TOOL_CALL_ID: &str = "executor-mcp-call";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn selected_executor_discovers_browser_mcp_with_executor_only_bearer_token() -> Result<()> {
    assert!(std::env::var_os(PROJECT_MCP_BEARER_ENV_NAME).is_none());
    let responses_server = responses::start_mock_server().await;
    let http_listener = TcpListener::bind("127.0.0.1:0").await?;
    let http_addr = http_listener.local_addr()?;
    let http_mcp_service = StreamableHttpService::new(
        || Ok(ExecutorHttpMcpServer),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default().with_allowed_hosts(["executor-only.invalid"]),
    );
    let browser_starting = Arc::new(AtomicBool::new(true));
    let http_router =
        Router::new()
            .nest_service("/mcp", http_mcp_service)
            .layer(axum::middleware::from_fn(
                move |request: axum::extract::Request, next: axum::middleware::Next| {
                    let browser_starting = Arc::clone(&browser_starting);
                    async move {
                        if browser_starting.swap(false, Ordering::Relaxed) {
                            tokio::time::sleep(Duration::from_secs(2)).await;
                        }
                        let authorized = request
                            .headers()
                            .get(axum::http::header::AUTHORIZATION)
                            .and_then(|value| value.to_str().ok())
                            .is_some_and(|value| value == "Bearer executor-browser-token");
                        if !authorized {
                            return axum::http::StatusCode::UNAUTHORIZED.into_response();
                        }
                        next.run(request).await
                    }
                },
            ));
    let http_server_handle = tokio::spawn(async move {
        let _ = axum::serve(http_listener, http_router).await;
    });
    let codex_home = TempDir::new()?;
    let executor_home = TempDir::new()?;
    MockResponsesConfig::new(&responses_server.uri())
        .with_sandbox_mode("danger-full-access")
        .write(codex_home.path())?;
    std::fs::write(
        codex_home.path().join("requirements.toml"),
        format!(
            "[mcp_servers.{PROJECT_MCP_SERVER_NAME}.identity]\nurl = \"{EXECUTOR_HTTP_MCP_URL}\"\n"
        ),
    )?;
    std::fs::write(
        executor_home.path().join("config.toml"),
        format!(
            "[mcp_servers.{PROJECT_MCP_SERVER_NAME}]\nurl = \"{EXECUTOR_HTTP_MCP_URL}\"\nbearer_token_env_var = \"{PROJECT_MCP_BEARER_ENV_NAME}\"\nrequired = true\nstartup_timeout_sec = 10\n\n[mcp_servers.ignored_stdio]\ncommand = \"executor-local-command\"\nenv_vars = [\"EXECUTOR_ONLY_TOKEN\"]\ncwd = \"./server\"\n\n[mcp_servers.policy_unlisted]\nurl = \"{EXECUTOR_HTTP_MCP_URL}\"\n"
        ),
    )?;
    // Browser auth tokens are not inherited by spawned executors, so use the production
    // WebSocket connection and put the token directly in the executor's environment.
    let mut executor = Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
        .args(["exec-server", "--listen", "ws://127.0.0.1:0"])
        .stdout(Stdio::piped())
        .kill_on_drop(true)
        .env("CODEX_HOME", executor_home.path())
        .env(PROJECT_MCP_BEARER_ENV_NAME, PROJECT_MCP_BEARER_TOKEN)
        .env("HTTP_PROXY", format!("http://{http_addr}"))
        .spawn()?;
    let stdout = executor.stdout.take().expect("executor stdout is piped");
    let mut lines = BufReader::new(stdout).lines();
    let executor_url = timeout(DEFAULT_READ_TIMEOUT, lines.next_line())
        .await??
        .expect("executor emits its websocket URL");
    std::fs::write(
        codex_home.path().join("environments.toml"),
        format!(
            "default = \"{EXECUTOR_ID}\"\ninclude_local = false\n\n[[environments]]\nid = \"{EXECUTOR_ID}\"\nurl = \"{executor_url}\"\n"
        ),
    )?;

    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;
    let thread_id = start_thread(&mut app_server, /*selected_capability_roots*/ None).await?;

    let namespace = format!("mcp__{PROJECT_MCP_SERVER_NAME}");
    let response_mock = responses::mount_sse_sequence(
        &responses_server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-browser-mcp-call"),
                responses::ev_function_call_with_namespace(
                    "browser-mcp-call",
                    &namespace,
                    "echo",
                    &json!({"message": "browser use works"}).to_string(),
                ),
                responses::ev_completed("resp-browser-mcp-call"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("resp-browser-mcp-done"),
                responses::ev_assistant_message("msg-browser-mcp-done", "Done"),
                responses::ev_completed("resp-browser-mcp-done"),
            ]),
        ],
    )
    .await;
    let request_id = app_server
        .send_turn_start_request(TurnStartParams {
            thread_id: thread_id.clone(),
            input: vec![UserInput::Text {
                text: "Use the executor browser".to_string(),
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
    let output = requests[1].function_call_output("browser-mcp-call");
    assert!(
        output
            .get("output")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|output| output.contains("ECHOING: browser use works"))
    );

    let servers = mcp_server_statuses(&mut app_server, thread_id).await?;
    assert!(
        servers
            .iter()
            .any(|server| { server.name == PROJECT_MCP_SERVER_NAME && server.plugin_id.is_none() })
    );
    assert!(servers.iter().all(|server| server.name != "ignored_stdio"));
    assert!(
        servers
            .iter()
            .any(|server| server.name == "policy_unlisted" && server.tools.is_empty())
    );

    http_server_handle.abort();
    let _ = http_server_handle.await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_executor_skips_required_browser_and_keeps_host_owned_mcp() -> Result<()> {
    let responses_server = responses::start_mock_server().await;
    let codex_home = TempDir::new()?;
    let executor_home = TempDir::new()?;

    let http_listener = TcpListener::bind("127.0.0.1:0").await?;
    let mcp_url = format!("http://{}/mcp", http_listener.local_addr()?);
    let expected_authorization = "Bearer host-only-token";
    let service = StreamableHttpService::new(
        || Ok(ExecutorHttpMcpServer),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );
    let router = Router::new()
        .nest_service("/mcp", service)
        .layer(axum::middleware::from_fn(
            move |request: axum::extract::Request, next: axum::middleware::Next| async move {
                let authorized = request
                    .headers()
                    .get(axum::http::header::AUTHORIZATION)
                    .and_then(|value| value.to_str().ok())
                    .is_some_and(|value| value == expected_authorization);
                if !authorized {
                    return axum::http::StatusCode::UNAUTHORIZED.into_response();
                }
                next.run(request).await
            },
        ));
    let http_server = tokio::spawn(async move {
        let _ = axum::serve(http_listener, router).await;
    });

    let root_config = format!(
        "[mcp_servers.host_remote]\nurl = \"{mcp_url}\"\nenvironment_id = \"{EXECUTOR_ID}\"\nbearer_token_env_var = \"HOST_MCP_TEST_TOKEN\"\nrequired = true\n"
    );
    MockResponsesConfig::new(&responses_server.uri())
        .with_sandbox_mode("danger-full-access")
        .with_extra_config(&root_config)
        .write(codex_home.path())?;
    std::fs::write(
        executor_home.path().join("config.toml"),
        format!(
            "[mcp_servers.node_repl]\nurl = \"http://127.0.0.1:9/mcp\"\nbearer_token_env_var = \"NODE_REPL_AUTH_TOKEN\"\nrequired = true\nstartup_timeout_sec = 1\n\n[mcp_servers.executor_public]\nurl = \"{mcp_url}\"\nhttp_headers = {{ Authorization = \"Bearer host-only-token\" }}\nrequired = true\n"
        ),
    )?;

    let mut executor = Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
        .args(["exec-server", "--listen", "ws://127.0.0.1:0"])
        .stdout(Stdio::piped())
        .kill_on_drop(true)
        .env("CODEX_HOME", executor_home.path())
        .spawn()?;
    let stdout = executor.stdout.take().expect("executor stdout is piped");
    let mut lines = BufReader::new(stdout).lines();
    let upstream_url = timeout(DEFAULT_READ_TIMEOUT, lines.next_line())
        .await??
        .expect("executor emits its websocket URL");

    let proxy_listener = TcpListener::bind("127.0.0.1:0").await?;
    let proxy_url = format!("ws://{}", proxy_listener.local_addr()?);
    let legacy_executor = tokio::spawn(async move {
        let (stream, _) = proxy_listener.accept().await?;
        let downstream = accept_async(stream).await?;
        let (upstream, _) = connect_async(upstream_url).await?;
        let (mut downstream_tx, mut downstream_rx) = downstream.split();
        let (mut upstream_tx, mut upstream_rx) = upstream.split();

        let requests = async {
            while let Some(message) = downstream_rx.next().await {
                let mut message = message?;
                if let Message::Text(text) = &message {
                    let mut request: Value = serde_json::from_str(text)?;
                    if request["method"] == "http/request"
                        && let Some(headers) = request["params"]["headers"].as_array_mut()
                    {
                        for header in headers {
                            if let Some(header) = header.as_object_mut() {
                                header.remove("valueEnvVar");
                            }
                        }
                        message = Message::Text(request.to_string().into());
                    }
                }
                upstream_tx.send(message).await?;
            }
            Ok::<_, anyhow::Error>(())
        };

        let responses = async {
            while let Some(message) = upstream_rx.next().await {
                let mut message = message?;
                if let Message::Text(text) = &message {
                    let mut response: Value = serde_json::from_str(text)?;
                    if let Some(capabilities) = response
                        .pointer_mut("/result/capabilities")
                        .and_then(Value::as_object_mut)
                    {
                        capabilities.remove("httpHeaderEnvVars");
                        message = Message::Text(response.to_string().into());
                    }
                }
                downstream_tx.send(message).await?;
            }
            Ok::<_, anyhow::Error>(())
        };

        tokio::select! {
            result = requests => result,
            result = responses => result,
        }
    });
    std::fs::write(
        codex_home.path().join("environments.toml"),
        format!(
            "default = \"{EXECUTOR_ID}\"\ninclude_local = false\n\n[[environments]]\nid = \"{EXECUTOR_ID}\"\nurl = \"{proxy_url}\"\n"
        ),
    )?;

    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[("HOST_MCP_TEST_TOKEN", Some("host-only-token"))])
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;
    let thread_id = start_thread(&mut app_server, /*selected_capability_roots*/ None).await?;
    let servers = mcp_server_statuses(&mut app_server, thread_id.clone()).await?;
    assert!(servers.iter().any(|server| server.name == "host_remote"));
    assert!(
        servers
            .iter()
            .any(|server| server.name == "executor_public")
    );
    assert!(servers.iter().all(|server| server.name != "node_repl"));

    let response = responses::mount_sse_once(
        &responses_server,
        responses::sse(vec![
            responses::ev_response_created("legacy-executor-turn"),
            responses::ev_assistant_message("legacy-executor-message", "Still works"),
            responses::ev_completed("legacy-executor-turn"),
        ]),
    )
    .await;
    let request_id = app_server
        .send_turn_start_request(TurnStartParams {
            thread_id,
            input: vec![UserInput::Text {
                text: "Check legacy executor compatibility".to_string(),
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
    assert!(
        response
            .single_request()
            .tool_by_name("mcp__host_remote", "echo")
            .is_some()
    );

    legacy_executor.abort();
    http_server.abort();
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn guardian_review_does_not_discover_executor_mcp() -> Result<()> {
    let responses_server = responses::start_mock_server().await;
    let codex_home = TempDir::new()?;
    let executor_home = TempDir::new()?;
    MockResponsesConfig::new(&responses_server.uri())
        .with_approval_policy("on-request")
        .with_root_config("approvals_reviewer = \"auto_review\"")
        .enable_feature(Feature::GuardianApproval)
        .write(codex_home.path())?;
    std::fs::write(executor_home.path().join("config.toml"), "")?;
    let codex_bin = toml::Value::String(
        codex_utils_cargo_bin::cargo_bin("codex")?
            .to_string_lossy()
            .into_owned(),
    );
    let executor_home_value =
        toml::Value::String(executor_home.path().to_string_lossy().into_owned());
    std::fs::write(
        codex_home.path().join("environments.toml"),
        format!(
            "default = \"{EXECUTOR_ID}\"\ninclude_local = false\n\n[[environments]]\nid = \"{EXECUTOR_ID}\"\nprogram = {codex_bin}\nargs = [\"exec-server\", \"--listen\", \"stdio\"]\n[environments.env]\nCODEX_HOME = {executor_home_value}\n"
        ),
    )?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;
    let thread_id = start_thread(&mut app_server, /*selected_capability_roots*/ None).await?;

    std::fs::write(
        executor_home.path().join("config.toml"),
        "[mcp_servers.unavailable_browser]\nurl = \"http://127.0.0.1:9/mcp\"\nrequired = true\nstartup_timeout_sec = 1\n",
    )?;
    let response_mock = responses::mount_sse_sequence(
        &responses_server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-guardian-command"),
                responses::ev_function_call(
                    "guardian-shell-call",
                    "exec_command",
                    &json!({
                        "cmd": "echo guardian",
                        "sandbox_permissions": "require_escalated",
                        "justification": "Test Guardian isolation.",
                    })
                    .to_string(),
                ),
                responses::ev_completed("resp-guardian-command"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("resp-guardian-review"),
                responses::ev_assistant_message(
                    "guardian-review",
                    &json!({
                        "outcome": "deny",
                        "rationale": "Guardian can review without the browser.",
                    })
                    .to_string(),
                ),
                responses::ev_completed("resp-guardian-review"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("resp-guardian-done"),
                responses::ev_assistant_message("guardian-done", "Done"),
                responses::ev_completed("resp-guardian-done"),
            ]),
        ],
    )
    .await;
    let request_id = app_server
        .send_turn_start_request(TurnStartParams {
            thread_id,
            input: vec![UserInput::Text {
                text: "Review a command without starting the browser".to_string(),
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
    assert_eq!(requests.len(), 3);
    assert_eq!(
        requests[1].body_json()["client_metadata"]["x-openai-subagent"],
        "guardian"
    );
    let output = requests[2].function_call_output("guardian-shell-call");
    assert!(
        output
            .get("output")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|output| output.contains("Guardian can review without the browser."))
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn selected_executor_plugin_exposes_its_mcps_only_to_that_thread() -> Result<()> {
    assert!(std::env::var_os(EXECUTOR_HTTP_AUTH_ENV_NAME).is_none());
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
    let http_mcp_router =
        Router::new()
            .nest_service("/mcp", http_mcp_service)
            .layer(axum::middleware::from_fn(
                move |request: axum::extract::Request, next: axum::middleware::Next| async move {
                    let authorized = request
                        .headers()
                        .get(axum::http::header::AUTHORIZATION)
                        .and_then(|value| value.to_str().ok())
                        .is_some_and(|value| {
                            value == format!("Bearer {EXECUTOR_HTTP_AUTH_ENV_VALUE}")
                        });
                    if !authorized {
                        return axum::http::StatusCode::UNAUTHORIZED.into_response();
                    }
                    next.run(request).await
                },
            ));
    let (oauth_authorization_tx, mut oauth_authorization_rx) = mpsc::unbounded_channel();
    let oauth_mcp_router = Router::new()
        .nest_service("/oauth-mcp", oauth_mcp_service)
        .layer(axum::middleware::from_fn(
            move |request: axum::extract::Request, next: axum::middleware::Next| {
                let oauth_authorization_tx = oauth_authorization_tx.clone();
                async move {
                    if let Some(authorization) = request
                        .headers()
                        .get(axum::http::header::AUTHORIZATION)
                        .and_then(|value| value.to_str().ok())
                    {
                        let _ = oauth_authorization_tx.send(authorization.to_string());
                    }
                    next.run(request).await
                }
            },
        ));
    let (registration_request_tx, mut registration_request_rx) = mpsc::unbounded_channel();
    let (token_request_tx, mut token_request_rx) = mpsc::unbounded_channel();
    let oauth_metadata = json!({
        "issuer": EXECUTOR_OAUTH_MCP_URL,
        "authorization_endpoint": "https://oauth-only.invalid/authorize",
        "token_endpoint": "http://oauth-only.invalid/token",
        "registration_endpoint": "http://oauth-only.invalid/register",
        "authorization_response_iss_parameter_supported": true,
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
            "/register",
            post(move |Json(request): Json<serde_json::Value>| {
                let registration_request_tx = registration_request_tx.clone();
                async move {
                    let _ = registration_request_tx.send(request.clone());
                    Json(json!({
                        "client_id": "executor-dcr-client",
                        "redirect_uris": request["redirect_uris"],
                    }))
                }
            }),
        )
        .route(
            "/token",
            post(move |body: Bytes| {
                let token_request_tx = token_request_tx.clone();
                async move {
                    let _ = token_request_tx.send(String::from_utf8_lossy(&body).into_owned());
                    Json(json!({
                        "access_token": EXECUTOR_OAUTH_ACCESS_TOKEN,
                        "token_type": "Bearer",
                        "expires_in": 3600,
                        "refresh_token": "executor-refresh-token",
                    }))
                }
            }),
        )
        .merge(http_mcp_router)
        .merge(oauth_mcp_router);
    let http_server_handle = tokio::spawn(async move {
        let _ = axum::serve(http_listener, http_router).await;
    });
    let plugin_callback_listener = TcpListener::bind("127.0.0.1:0").await?;
    let plugin_callback_port = plugin_callback_listener.local_addr()?.port();
    let global_callback_listener = TcpListener::bind("127.0.0.1:0").await?;
    let global_callback_port = global_callback_listener.local_addr()?.port();
    drop(plugin_callback_listener);
    let codex_home = TempDir::new()?;
    let executor_home = TempDir::new()?;
    std::fs::write(
        executor_home.path().join("config.toml"),
        format!(
            "[mcp_servers.{EXECUTOR_DISABLED_PLUGIN_SERVER_NAME}]\nurl = \"{EXECUTOR_HTTP_MCP_URL}\"\nenabled = false\n"
        ),
    )?;
    let root_config = format!(
        "compact_prompt = \"compact\"\nmodel_auto_compact_token_limit = 1024\nmcp_oauth_credentials_store = \"file\"\nmcp_oauth_callback_port = {global_callback_port}"
    );
    MockResponsesConfig::new(&responses_server.uri())
        .with_root_config(&root_config)
        .with_provider_config("supports_websockets = false")
        .write(codex_home.path())?;
    let executor_config: codex_config::types::McpServerConfig = serde_json::from_value(json!({
        "url": EXECUTOR_OAUTH_MCP_URL,
        "environment_id": EXECUTOR_ID,
    }))?;
    let host_oauth_credential = json!({
        "server_name": executor_config.oauth_credential_name(OAUTH_MCP_SERVER_NAME),
        "server_url": EXECUTOR_OAUTH_MCP_URL,
        "client_id": "host-oauth-client",
        "access_token": HOST_OAUTH_ACCESS_TOKEN,
        "expires_at": null,
        "refresh_token": null,
        "scopes": [],
    });
    let oauth_credentials_path = codex_home.path().join(".credentials.json");
    std::fs::write(
        &oauth_credentials_path,
        serde_json::to_vec(&json!({"host": host_oauth_credential.clone()}))?,
    )?;
    let mut executor = Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
        .args(["exec-server", "--listen", "ws://127.0.0.1:0"])
        .stdout(Stdio::piped())
        .kill_on_drop(true)
        .env("CODEX_HOME", executor_home.path())
        .env(EXECUTOR_ENV_NAME, EXECUTOR_ENV_VALUE)
        .env(EXECUTOR_HTTP_AUTH_ENV_NAME, EXECUTOR_HTTP_AUTH_ENV_VALUE)
        .env("HTTP_PROXY", format!("http://{http_addr}"))
        .spawn()?;
    let stdout = executor.stdout.take().expect("executor stdout is piped");
    let mut lines = BufReader::new(stdout).lines();
    let executor_url = timeout(DEFAULT_READ_TIMEOUT, lines.next_line())
        .await??
        .expect("executor emits its websocket URL");
    std::fs::write(
        codex_home.path().join("environments.toml"),
        format!(
            r#"
include_local = true

[[environments]]
id = "{EXECUTOR_ID}"
url = "{executor_url}"
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
                    "bearer_token_env_var": EXECUTOR_HTTP_AUTH_ENV_NAME,
                    "startup_timeout_sec": 10,
                },
                (EXECUTOR_DISABLED_PLUGIN_SERVER_NAME): {
                    "url": EXECUTOR_HTTP_MCP_URL,
                    "environment_id": "local",
                    "startup_timeout_sec": 10,
                },
                (OAUTH_MCP_SERVER_NAME): {
                    "url": EXECUTOR_OAUTH_MCP_URL,
                    "environment_id": "local",
                    "oauth": {"callbackPort": plugin_callback_port},
                    "startup_timeout_sec": 10,
                },
                (PRE_REGISTERED_OAUTH_MCP_SERVER_NAME): {
                    "url": EXECUTOR_OAUTH_MCP_URL,
                    "environment_id": "local",
                    "oauth": {
                        "clientId": "configured-client",
                        "callbackPort": plugin_callback_port,
                    },
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
                "name": PRE_REGISTERED_OAUTH_MCP_SERVER_NAME,
                "threadId": selected_thread.clone(),
                "clientRegistration": "dcr",
                "timeoutSecs": 10,
            })),
        )
        .await?;
    let response: McpServerOauthLoginResponse =
        timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(request_id)).await??;
    let authorization_url = Url::parse(&response.authorization_url)?;
    let parameters = authorization_url
        .query_pairs()
        .into_owned()
        .collect::<BTreeMap<String, String>>();
    assert_eq!(
        parameters.get("client_id").map(String::as_str),
        Some("configured-client")
    );
    assert!(
        registration_request_rx.try_recv().is_err(),
        "configured OAuth client must skip dynamic registration"
    );
    let mut callback_url = Url::parse(&parameters["redirect_uri"])?;
    callback_url
        .query_pairs_mut()
        .append_pair("code", "configured-test-code")
        .append_pair("state", &parameters["state"])
        .append_pair("iss", EXECUTOR_OAUTH_MCP_URL);
    HttpClientBuilder::new()
        .build_direct()?
        .get(callback_url)
        .send()
        .await?
        .error_for_status()?;
    let token_request = timeout(DEFAULT_READ_TIMEOUT, token_request_rx.recv())
        .await?
        .expect("configured client should exchange its authorization code");
    assert!(token_request.contains("client_id=configured-client"));
    let completed: McpServerOauthLoginCompletedNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_notification("mcpServer/oauthLogin/completed"),
    )
    .await??;
    assert_eq!(completed.name, PRE_REGISTERED_OAUTH_MCP_SERVER_NAME);
    assert!(completed.success);

    let request_id = app_server
        .send_raw_request(
            "mcpServer/oauth/login",
            Some(json!({
                "name": OAUTH_MCP_SERVER_NAME,
                "threadId": selected_thread.clone(),
                "clientRegistration": "dcr",
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
    let authorization_url = Url::parse(&response.authorization_url)?;
    let client_id = authorization_url
        .query_pairs()
        .find_map(|(key, value)| (key == "client_id").then(|| value.into_owned()));
    assert_eq!(client_id.as_deref(), Some("executor-dcr-client"));
    let state = authorization_url
        .query_pairs()
        .find_map(|(key, value)| (key == "state").then(|| value.into_owned()))
        .expect("authorization URL should include state");
    let redirect_uri = authorization_url
        .query_pairs()
        .find_map(|(key, value)| (key == "redirect_uri").then(|| value.into_owned()))
        .expect("authorization URL should include redirect_uri");
    let registration_request = timeout(DEFAULT_READ_TIMEOUT, registration_request_rx.recv())
        .await?
        .expect("executor registration endpoint should receive a request");
    assert_eq!(registration_request["client_name"], json!("Codex"));
    assert_eq!(
        registration_request["redirect_uris"],
        json!([redirect_uri.clone()])
    );
    let mut callback_url = Url::parse(&redirect_uri)?;
    assert_eq!(callback_url.port(), Some(plugin_callback_port));
    callback_url
        .query_pairs_mut()
        .append_pair("code", "executor-test-code")
        .append_pair("state", &state)
        .append_pair("iss", EXECUTOR_OAUTH_MCP_URL);
    HttpClientBuilder::new()
        .build_direct()?
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
    assert!(token_request.contains("client_id=executor-dcr-client"));
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
            server: OAUTH_MCP_SERVER_NAME.to_string(),
            tool: "echo".to_string(),
            arguments: Some(json!({"message": "hello over executor OAuth"})),
            meta: None,
        })
        .await?;
    let response: McpServerToolCallResponse =
        timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(request_id)).await??;
    assert_eq!(
        response.structured_content,
        Some(json!({"echo": "ECHOING: hello over executor OAuth"}))
    );

    let authorization_headers =
        std::iter::from_fn(|| oauth_authorization_rx.try_recv().ok()).collect::<Vec<_>>();
    assert!(
        !authorization_headers
            .iter()
            .any(|header| header == &format!("Bearer {HOST_OAUTH_ACCESS_TOKEN}")),
        "host-owned OAuth credentials must never reach the executor: {authorization_headers:?}"
    );
    assert!(
        authorization_headers
            .iter()
            .any(|header| header == &format!("Bearer {EXECUTOR_OAUTH_ACCESS_TOKEN}")),
        "executor-owned OAuth credentials must authenticate executor requests: {authorization_headers:?}"
    );
    let oauth_credentials: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&oauth_credentials_path)?)?;
    assert_eq!(oauth_credentials.get("host"), Some(&host_oauth_credential));
    assert!(
        oauth_credentials
            .as_object()
            .expect("OAuth credentials should remain a JSON object")
            .values()
            .any(|credential| {
                credential.get("server_name") != Some(&json!(OAUTH_MCP_SERVER_NAME))
                    && credential.get("access_token") == Some(&json!(EXECUTOR_OAUTH_ACCESS_TOKEN))
            }),
        "executor login must persist credentials separately from the host: {oauth_credentials}"
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

    let selected_server_owners = mcp_server_statuses(&mut app_server, selected_thread)
        .await?
        .into_iter()
        .map(|server| (server.name, server.plugin_id))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        selected_server_owners,
        BTreeMap::from([
            (EXECUTOR_DISABLED_PLUGIN_SERVER_NAME.to_string(), None),
            (
                HTTP_MCP_SERVER_NAME.to_string(),
                Some("executor-demo@1".to_string()),
            ),
            (
                MCP_SERVER_NAME.to_string(),
                Some("executor-demo@1".to_string()),
            ),
            (
                OAUTH_MCP_SERVER_NAME.to_string(),
                Some("executor-demo@1".to_string()),
            ),
            (
                PRE_REGISTERED_OAUTH_MCP_SERVER_NAME.to_string(),
                Some("executor-demo@1".to_string()),
            ),
            (REFRESH_PROBE_SERVER_NAME.to_string(), None),
        ])
    );

    let unselected_thread =
        start_thread(&mut app_server, /*selected_capability_roots*/ None).await?;
    let unselected_server_names = mcp_server_statuses(&mut app_server, unselected_thread)
        .await?
        .into_iter()
        .map(|server| server.name)
        .collect::<Vec<_>>();
    assert!(unselected_server_names.iter().all(|name| {
        name != MCP_SERVER_NAME
            && name != HTTP_MCP_SERVER_NAME
            && name != OAUTH_MCP_SERVER_NAME
            && name != PRE_REGISTERED_OAUTH_MCP_SERVER_NAME
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

        Ok(ListToolsResult::with_all_items(vec![tool]))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<rmcp::model::CallToolResponse, rmcp::ErrorData> {
        let message = request
            .arguments
            .as_ref()
            .and_then(|arguments| arguments.get("message"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        Ok(CallToolResult::structured(json!({
            "echo": format!("ECHOING: {message}")
        }))
        .into())
    }
}

async fn mcp_server_statuses(
    app_server: &mut TestAppServer,
    thread_id: String,
) -> Result<Vec<McpServerStatus>> {
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
    Ok(response.data)
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
