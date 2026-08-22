use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use axum::Json;
use axum::Router;
use axum::body::Bytes;
use axum::http::HeaderMap;
use axum::routing::get;
use axum::routing::post;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::ListMcpServerStatusParams;
use codex_app_server_protocol::ListMcpServerStatusResponse;
use codex_app_server_protocol::McpServerConnectionStatus;
use codex_app_server_protocol::McpServerOauthLoginCompletedNotification;
use codex_app_server_protocol::McpServerOauthLoginResponse;
use codex_app_server_protocol::McpServerStatusDetail;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_core::config::set_project_trust_level;
use codex_http_client::HttpClientBuilder;
use codex_protocol::config_types::TrustLevel;
use core_test_support::skip_if_remote;
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
use test_case::test_case;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tokio::time::timeout;
use url::Url;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn oauth_login_uses_http_headers_helper() -> Result<()> {
    let oauth = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server/mcp"))
        .and(header("x-gateway", "gateway-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issuer": format!("{}/mcp", oauth.uri()),
            "authorization_endpoint": format!("{}/oauth/authorize", oauth.uri()),
            "token_endpoint": format!("{}/oauth/token", oauth.uri()),
        })))
        .mount(&oauth)
        .await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(header("x-gateway", "gateway-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "oauth-token",
            "token_type": "Bearer",
        })))
        .expect(1)
        .mount(&oauth)
        .await;

    let codex_home = TempDir::new()?;
    let helper_command = if cfg!(windows) {
        r#"echo {"X-Gateway":"gateway-token"}"#
    } else {
        r#"printf '{"X-Gateway":"gateway-token"}'"#
    };
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            "mcp_oauth_credentials_store = \"file\"\n\
             [mcp_servers.gateway]\n\
             url = \"{}/mcp\"\n\
             http_headers_helper = {}\n\
             [mcp_servers.gateway.oauth]\n\
             client_id = \"test-client\"\n",
            oauth.uri(),
            toml::Value::String(helper_command.to_string()),
        ),
    )?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_raw_request(
            "mcpServer/oauth/login",
            Some(json!({"name": "gateway", "timeoutSecs": 10})),
        )
        .await?;
    let response: McpServerOauthLoginResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(request_id)).await??;
    let authorization_url = Url::parse(&response.authorization_url)?;
    let query: BTreeMap<_, _> = authorization_url.query_pairs().into_owned().collect();
    let mut callback_url = Url::parse(&query["redirect_uri"])?;
    callback_url
        .query_pairs_mut()
        .append_pair("code", "test-code")
        .append_pair("state", &query["state"]);
    HttpClientBuilder::new()
        .build_direct()?
        .get(callback_url)
        .send()
        .await?
        .error_for_status()?;
    let completed: McpServerOauthLoginCompletedNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_notification("mcpServer/oauthLogin/completed"),
    )
    .await??;
    assert_eq!(
        completed,
        McpServerOauthLoginCompletedNotification {
            name: "gateway".to_string(),
            thread_id: None,
            success: true,
            error: None,
        }
    );
    oauth.verify().await;
    Ok(())
}

#[test_case(false; "plain HTTP")]
#[test_case(true; "HTTP headers helper")]
#[tokio::test]
async fn oauth_login_rejects_servers_disabled_by_managed_requirements(
    with_headers_helper: bool,
) -> Result<()> {
    let codex_home = TempDir::new()?;
    let marker = codex_home.path().join("helper-ran");
    let helper = with_headers_helper
        .then(|| toml::Value::String(format!("echo invoked > \"{}\"", marker.display())))
        .map_or_else(String::new, |command| {
            format!("http_headers_helper = {command}\n")
        });
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!("[mcp_servers.blocked]\nurl = \"https://example.com/mcp\"\n{helper}"),
    )?;
    std::fs::write(
        codex_home.path().join("requirements.toml"),
        "[mcp_servers.blocked.identity]\nurl = \"https://allowed.example.com/mcp\"\n",
    )?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_raw_request(
            "mcpServer/oauth/login",
            Some(json!({"name": "blocked", "timeoutSecs": 10})),
        )
        .await?;
    let error: JSONRPCError = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert!(
        error
            .error
            .message
            .contains("disabled by managed requirements")
    );
    assert!(!marker.exists());
    Ok(())
}

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
async fn oauth_login_automatically_selects_callback_specific_cimd_without_metadata_issuer()
-> Result<()> {
    let responses_server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let base_url = format!("http://{}", listener.local_addr()?);
    let metadata = json!({
        "authorization_endpoint": format!("{base_url}/authorize"),
        "token_endpoint": format!("{base_url}/token"),
        "registration_endpoint": format!("{base_url}/register"),
        "client_id_metadata_document_supported": true,
        "token_endpoint_auth_methods_supported": ["none"],
        "response_types_supported": ["code"],
        "code_challenge_methods_supported": ["S256"],
    });
    let registrations = Arc::new(AtomicUsize::new(0));
    let registration_count = Arc::clone(&registrations);
    let (token_request_tx, mut token_request_rx) = mpsc::unbounded_channel();
    let (mcp_authorization_tx, mut mcp_authorization_rx) = mpsc::unbounded_channel();
    let tool_name = Arc::new("cimd".to_string());
    let mcp_service = StreamableHttpService::new(
        move || {
            Ok(McpStatusServer {
                tool_name: Arc::clone(&tool_name),
            })
        },
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );
    let mcp_router =
        Router::new()
            .nest_service("/mcp", mcp_service)
            .layer(axum::middleware::from_fn(
                move |request: axum::extract::Request, next: axum::middleware::Next| {
                    let mcp_authorization_tx = mcp_authorization_tx.clone();
                    async move {
                        if let Some(authorization) = request
                            .headers()
                            .get(axum::http::header::AUTHORIZATION)
                            .and_then(|value| value.to_str().ok())
                        {
                            let _ = mcp_authorization_tx.send(authorization.to_string());
                        }
                        next.run(request).await
                    }
                },
            ));
    let oauth_server = Router::new()
        .route(
            "/.well-known/oauth-authorization-server/mcp",
            get(move || {
                let metadata = metadata.clone();
                async move { Json(metadata) }
            }),
        )
        .route(
            "/register",
            post(move || {
                let registrations = Arc::clone(&registration_count);
                async move {
                    registrations.fetch_add(1, Ordering::SeqCst);
                    Json(json!({"client_id": "unexpected-dcr-client"}))
                }
            }),
        )
        .route(
            "/token",
            post(move |headers: HeaderMap, body: Bytes| {
                let token_request_tx = token_request_tx.clone();
                async move {
                    let _ = token_request_tx.send((
                        String::from_utf8_lossy(&body).into_owned(),
                        headers
                            .get(axum::http::header::AUTHORIZATION)
                            .and_then(|value| value.to_str().ok())
                            .map(str::to_string),
                    ));
                    Json(json!({
                        "access_token": "cimd-access-token",
                        "token_type": "Bearer",
                        "expires_in": 3600,
                        "refresh_token": "test-refresh-token",
                    }))
                }
            }),
        )
        .merge(mcp_router);
    let oauth_server_handle = tokio::spawn(async move {
        let _ = axum::serve(listener, oauth_server).await;
    });

    let codex_home = TempDir::new()?;
    mock_responses_config(&responses_server.uri())
        .with_extra_config(&format!(
            "mcp_oauth_credentials_store = \"file\"\n[mcp_servers.cimd]\nurl = \"{base_url}/mcp\""
        ))
        .write(codex_home.path())?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;

    let request_id = app_server
        .send_raw_request(
            "mcpServer/oauth/login",
            Some(json!({"name": "cimd", "timeoutSecs": 10})),
        )
        .await?;
    let response: McpServerOauthLoginResponse =
        timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(request_id)).await??;
    let authorization_url = Url::parse(&response.authorization_url)?;
    let parameters = authorization_url
        .query_pairs()
        .into_owned()
        .collect::<BTreeMap<String, String>>();
    let redirect_uri = parameters["redirect_uri"].clone();
    let mut callback_url = Url::parse(&redirect_uri)?;
    let callback_id = callback_url
        .path()
        .strip_prefix("/callback/")
        .expect("issuerless CIMD should use a resource-specific callback");
    let client_id = format!("https://chatgpt.com/oauth/codex/{callback_id}/client.json");
    assert_eq!(parameters.get("client_id"), Some(&client_id));
    assert_eq!(
        parameters.get("code_challenge_method").map(String::as_str),
        Some("S256")
    );
    assert_eq!(registrations.load(Ordering::SeqCst), 0);

    callback_url
        .query_pairs_mut()
        .append_pair("code", "cimd-authorization-code")
        .append_pair("state", &parameters["state"]);
    HttpClientBuilder::new()
        .build_direct()?
        .get(callback_url)
        .send()
        .await?
        .error_for_status()?;
    let (token_request, token_authorization) =
        timeout(DEFAULT_READ_TIMEOUT, token_request_rx.recv())
            .await?
            .expect("CIMD authorization should exchange its authorization code");
    let token_parameters = url::form_urlencoded::parse(token_request.as_bytes())
        .into_owned()
        .collect::<BTreeMap<String, String>>();
    assert_eq!(token_parameters.get("client_id"), Some(&client_id));
    assert!(token_parameters.contains_key("code_verifier"));
    assert_eq!(token_authorization, None);

    let completed: McpServerOauthLoginCompletedNotification = timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_notification("mcpServer/oauthLogin/completed"),
    )
    .await??;
    assert_eq!(
        completed,
        McpServerOauthLoginCompletedNotification {
            name: "cimd".to_string(),
            thread_id: None,
            success: true,
            error: None,
        }
    );
    assert_eq!(registrations.load(Ordering::SeqCst), 0);

    let request_id = app_server
        .send_raw_request("config/mcpServer/reload", /*params*/ None)
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let _: ListMcpServerStatusResponse = app_server
        .request(|request_id| ClientRequest::McpServerStatusList {
            request_id,
            params: ListMcpServerStatusParams {
                cursor: None,
                limit: None,
                detail: Some(McpServerStatusDetail::Full),
                thread_id: None,
            },
        })
        .await?;
    assert_eq!(
        timeout(DEFAULT_READ_TIMEOUT, mcp_authorization_rx.recv())
            .await?
            .expect("MCP startup should use the access token"),
        "Bearer cimd-access-token"
    );
    assert_eq!(registrations.load(Ordering::SeqCst), 0);

    oauth_server_handle.abort();
    let _ = oauth_server_handle.await;
    Ok(())
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
    assert_eq!(status.runtime_status, None);
    assert_eq!(status.plugin_id, None);
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
    assert_eq!(status.runtime_status, None);
    assert_eq!(
        status.tools.keys().cloned().collect::<BTreeSet<_>>(),
        BTreeSet::from(["project_lookup".to_string()])
    );

    mcp_server_handle.abort();
    let _ = mcp_server_handle.await;

    Ok(())
}

#[tokio::test]
async fn mcp_server_status_list_reports_thread_runtime_connections() -> Result<()> {
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let (mcp_server_url, mcp_server_handle) = start_mcp_server("lookup").await?;
    let codex_home = TempDir::new()?;
    mock_responses_config(&server.uri())
        .with_extra_config(&format!(
            "[mcp_servers.connected]\nurl = \"{mcp_server_url}/mcp\"\n\
             [mcp_servers.disabled]\nurl = \"{mcp_server_url}/mcp\"\nenabled = false\n"
        ))
        .write(codex_home.path())?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let ThreadStartResponse { thread, .. } = mcp.start_thread(ThreadStartParams::default()).await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_matching_notification("connected MCP server ready", |notification| {
            notification.method == "mcpServer/startupStatus/updated"
                && notification.params.as_ref().is_some_and(|params| {
                    params["name"] == "connected" && params["status"] == "ready"
                })
        }),
    )
    .await??;
    let response: ListMcpServerStatusResponse = mcp
        .request(|request_id| ClientRequest::McpServerStatusList {
            request_id,
            params: ListMcpServerStatusParams {
                cursor: None,
                limit: None,
                detail: Some(McpServerStatusDetail::ToolsAndAuthOnly),
                thread_id: Some(thread.id.clone()),
            },
        })
        .await?;
    assert_eq!(
        response
            .data
            .into_iter()
            .map(|status| (status.name, status.runtime_status))
            .collect::<Vec<_>>(),
        vec![
            (
                "connected".to_string(),
                Some(McpServerConnectionStatus::Connected)
            ),
            (
                "disabled".to_string(),
                Some(McpServerConnectionStatus::Disabled)
            ),
        ]
    );
    // Inventory uses the latest config, but a same-name replacement is not the
    // connection that this thread started. Unchanged registrations retain status.
    let (replacement_url, replacement_handle) = start_mcp_server("replacement_lookup").await?;
    mock_responses_config(&server.uri())
        .with_extra_config(&format!(
            "[mcp_servers.connected]\nurl = \"{replacement_url}/mcp\"\n\
             [mcp_servers.disabled]\nurl = \"{mcp_server_url}/mcp\"\nenabled = false\n"
        ))
        .write(codex_home.path())?;
    let response: ListMcpServerStatusResponse = mcp
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
    assert_eq!(
        response
            .data
            .into_iter()
            .map(|status| (
                status.name,
                status.runtime_status,
                status.tools.into_keys().collect::<BTreeSet<_>>()
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                "connected".to_string(),
                None,
                BTreeSet::from(["replacement_lookup".to_string()])
            ),
            (
                "disabled".to_string(),
                Some(McpServerConnectionStatus::Disabled),
                BTreeSet::new()
            ),
        ]
    );
    replacement_handle.abort();
    let _ = replacement_handle.await;
    mcp_server_handle.abort();
    let _ = mcp_server_handle.await;
    Ok(())
}

#[tokio::test]
async fn mcp_server_status_list_reports_disconnected_stdio_transport() -> Result<()> {
    skip_if_remote!(
        Ok(()),
        "the stdio executable and shutdown marker are host-local"
    );
    let server = create_mock_responses_server_sequence_unchecked(Vec::new()).await;
    let codex_home = TempDir::new()?;
    let exit_file = codex_home.path().join("exit-mcp-server");
    mock_responses_config(&server.uri())
        .with_extra_config(&format!(
            "[mcp_servers.stdio]\ncommand = {}\nstartup_timeout_sec = 2\n\
             [mcp_servers.stdio.env]\nMCP_TEST_EXIT_FILE = {}\n",
            toml::Value::String(stdio_server_bin()?),
            toml::Value::String(exit_file.to_string_lossy().into_owned()),
        ))
        .write(codex_home.path())?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let ThreadStartResponse { thread, .. } = mcp.start_thread(ThreadStartParams::default()).await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_matching_notification("stdio MCP server ready", |notification| {
            notification.method == "mcpServer/startupStatus/updated"
                && notification
                    .params
                    .as_ref()
                    .is_some_and(|params| params["name"] == "stdio" && params["status"] == "ready")
        }),
    )
    .await??;
    for expected in [
        McpServerConnectionStatus::Connected,
        McpServerConnectionStatus::Failed,
    ] {
        if expected == McpServerConnectionStatus::Failed {
            std::fs::write(&exit_file, "exit")?;
        }
        timeout(DEFAULT_READ_TIMEOUT, async {
            loop {
                let response: ListMcpServerStatusResponse = mcp
                    .request(|request_id| ClientRequest::McpServerStatusList {
                        request_id,
                        params: ListMcpServerStatusParams {
                            cursor: None,
                            limit: None,
                            detail: Some(McpServerStatusDetail::ToolsAndAuthOnly),
                            thread_id: Some(thread.id.clone()),
                        },
                    })
                    .await?;
                let statuses = response
                    .data
                    .into_iter()
                    .map(|status| (status.name, status.runtime_status))
                    .collect::<Vec<_>>();
                if statuses == vec![("stdio".to_string(), Some(expected))] {
                    return Ok::<(), anyhow::Error>(());
                }
                sleep(Duration::from_millis(/*millis*/ 20)).await;
            }
        })
        .await??;
    }
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

        Ok(ListToolsResult::with_all_items(vec![tool]))
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

        Ok(ListToolsResult::with_all_items(vec![tool]))
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<rmcp::service::RoleServer>,
    ) -> Result<ListResourcesResult, rmcp::ErrorData> {
        tokio::time::sleep(Duration::from_secs(2)).await;
        Ok(ListResourcesResult::with_all_items(Vec::new()))
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<rmcp::service::RoleServer>,
    ) -> Result<ListResourceTemplatesResult, rmcp::ErrorData> {
        tokio::time::sleep(Duration::from_secs(2)).await;
        Ok(ListResourceTemplatesResult::with_all_items(Vec::new()))
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
