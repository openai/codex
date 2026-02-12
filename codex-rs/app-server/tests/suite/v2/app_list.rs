use std::borrow::Cow;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use anyhow::bail;
use app_test_support::ChatGptAuthFixture;
use app_test_support::McpProcess;
use app_test_support::to_response;
use app_test_support::write_chatgpt_auth;
use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::http::header::AUTHORIZATION;
use axum::routing::get;
use codex_app_server_protocol::AppInfo;
use codex_app_server_protocol::AppListUpdatedNotification;
use codex_app_server_protocol::AppsListParams;
use codex_app_server_protocol::AppsListResponse;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_core::auth::AuthCredentialsStoreMode;
use pretty_assertions::assert_eq;
use rmcp::handler::server::ServerHandler;
use rmcp::model::JsonObject;
use rmcp::model::ListToolsResult;
use rmcp::model::Meta;
use rmcp::model::ServerCapabilities;
use rmcp::model::ServerInfo;
use rmcp::model::Tool;
use rmcp::model::ToolAnnotations;
use rmcp::transport::StreamableHttpServerConfig;
use rmcp::transport::StreamableHttpService;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use serde_json::json;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn list_apps_returns_empty_when_connectors_disabled() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;

    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_apps_list_request(AppsListParams {
            limit: Some(50),
            cursor: None,
            thread_id: None,
            force_refetch: false,
        })
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;

    let AppsListResponse { data, next_cursor } = to_response(response)?;

    assert!(data.is_empty());
    assert!(next_cursor.is_none());
    Ok(())
}

#[tokio::test]
async fn list_apps_uses_thread_feature_flag_when_thread_id_is_provided() -> Result<()> {
    let connectors = vec![AppInfo {
        id: "beta".to_string(),
        name: "Beta".to_string(),
        description: Some("Beta connector".to_string()),
        logo_url: None,
        logo_url_dark: None,
        distribution_channel: None,
        install_url: None,
        is_accessible: false,
    }];
    let tools = vec![connector_tool("beta", "Beta App")?];
    let (server_url, server_handle) =
        start_apps_server_with_delays(connectors, tools, Duration::ZERO, Duration::ZERO).await?;

    let codex_home = TempDir::new()?;
    write_connectors_config(codex_home.path(), &server_url)?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let start_request = mcp
        .send_thread_start_request(ThreadStartParams::default())
        .await?;
    let start_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(start_request)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response(start_response)?;

    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            r#"
chatgpt_base_url = "{server_url}"
mcp_oauth_credentials_store = "file"

[features]
connectors = false
"#
        ),
    )?;

    let global_request = mcp
        .send_apps_list_request(AppsListParams {
            limit: None,
            cursor: None,
            thread_id: None,
            force_refetch: false,
        })
        .await?;
    let global_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(global_request)),
    )
    .await??;
    let AppsListResponse {
        data: global_data,
        next_cursor: global_next_cursor,
    } = to_response(global_response)?;
    assert!(global_data.is_empty());
    assert!(global_next_cursor.is_none());

    let thread_request = mcp
        .send_apps_list_request(AppsListParams {
            limit: None,
            cursor: None,
            thread_id: Some(thread.id),
            force_refetch: false,
        })
        .await?;
    let thread_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_request)),
    )
    .await??;
    let AppsListResponse {
        data: thread_data,
        next_cursor: thread_next_cursor,
    } = to_response(thread_response)?;
    assert!(thread_data.iter().any(|app| app.id == "beta"));
    assert!(thread_next_cursor.is_none());

    server_handle.abort();
    let _ = server_handle.await;
    Ok(())
}

#[tokio::test]
async fn list_apps_emits_updates_and_returns_after_both_lists_load() -> Result<()> {
    let connectors = vec![
        AppInfo {
            id: "alpha".to_string(),
            name: "Alpha".to_string(),
            description: Some("Alpha connector".to_string()),
            logo_url: Some("https://example.com/alpha.png".to_string()),
            logo_url_dark: None,
            distribution_channel: None,
            install_url: None,
            is_accessible: false,
        },
        AppInfo {
            id: "beta".to_string(),
            name: "beta".to_string(),
            description: None,
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            install_url: None,
            is_accessible: false,
        },
    ];

    let tools = vec![connector_tool("beta", "Beta App")?];
    let (server_url, server_handle) = start_apps_server_with_delays(
        connectors.clone(),
        tools,
        Duration::from_millis(300),
        Duration::ZERO,
    )
    .await?;

    let codex_home = TempDir::new()?;
    write_connectors_config(codex_home.path(), &server_url)?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_apps_list_request(AppsListParams {
            limit: None,
            cursor: None,
            thread_id: None,
            force_refetch: false,
        })
        .await?;

    let expected_accessible = vec![AppInfo {
        id: "beta".to_string(),
        name: "Beta App".to_string(),
        description: None,
        logo_url: None,
        logo_url_dark: None,
        distribution_channel: None,
        install_url: Some("https://chatgpt.com/apps/beta-app/beta".to_string()),
        is_accessible: true,
    }];

    let first_update = read_app_list_updated_notification(&mut mcp).await?;
    assert_eq!(first_update.data, expected_accessible);

    let expected_merged = vec![
        AppInfo {
            id: "beta".to_string(),
            name: "Beta App".to_string(),
            description: None,
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            install_url: Some("https://chatgpt.com/apps/beta/beta".to_string()),
            is_accessible: true,
        },
        AppInfo {
            id: "alpha".to_string(),
            name: "Alpha".to_string(),
            description: Some("Alpha connector".to_string()),
            logo_url: Some("https://example.com/alpha.png".to_string()),
            logo_url_dark: None,
            distribution_channel: None,
            install_url: Some("https://chatgpt.com/apps/alpha/alpha".to_string()),
            is_accessible: false,
        },
    ];

    let second_update = read_app_list_updated_notification(&mut mcp).await?;
    assert_eq!(second_update.data, expected_merged);

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;

    let AppsListResponse {
        data: response_data,
        next_cursor,
    } = to_response(response)?;
    assert_eq!(response_data, expected_merged);
    assert!(next_cursor.is_none());

    server_handle.abort();
    let _ = server_handle.await;
    Ok(())
}

#[tokio::test]
async fn list_apps_returns_connectors_with_accessible_flags() -> Result<()> {
    let connectors = vec![
        AppInfo {
            id: "alpha".to_string(),
            name: "Alpha".to_string(),
            description: Some("Alpha connector".to_string()),
            logo_url: Some("https://example.com/alpha.png".to_string()),
            logo_url_dark: None,
            distribution_channel: None,
            install_url: None,
            is_accessible: false,
        },
        AppInfo {
            id: "beta".to_string(),
            name: "beta".to_string(),
            description: None,
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            install_url: None,
            is_accessible: false,
        },
    ];

    let tools = vec![connector_tool("beta", "Beta App")?];
    let (server_url, server_handle) = start_apps_server_with_delays(
        connectors.clone(),
        tools,
        Duration::ZERO,
        Duration::from_millis(300),
    )
    .await?;

    let codex_home = TempDir::new()?;
    write_connectors_config(codex_home.path(), &server_url)?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_apps_list_request(AppsListParams {
            limit: None,
            cursor: None,
            thread_id: None,
            force_refetch: false,
        })
        .await?;

    let first_update = read_app_list_updated_notification(&mut mcp).await?;
    assert_eq!(
        first_update.data,
        vec![
            AppInfo {
                id: "alpha".to_string(),
                name: "Alpha".to_string(),
                description: Some("Alpha connector".to_string()),
                logo_url: Some("https://example.com/alpha.png".to_string()),
                logo_url_dark: None,
                distribution_channel: None,
                install_url: Some("https://chatgpt.com/apps/alpha/alpha".to_string()),
                is_accessible: false,
            },
            AppInfo {
                id: "beta".to_string(),
                name: "beta".to_string(),
                description: None,
                logo_url: None,
                logo_url_dark: None,
                distribution_channel: None,
                install_url: Some("https://chatgpt.com/apps/beta/beta".to_string()),
                is_accessible: false,
            },
        ]
    );

    let expected = vec![
        AppInfo {
            id: "beta".to_string(),
            name: "Beta App".to_string(),
            description: None,
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            install_url: Some("https://chatgpt.com/apps/beta/beta".to_string()),
            is_accessible: true,
        },
        AppInfo {
            id: "alpha".to_string(),
            name: "Alpha".to_string(),
            description: Some("Alpha connector".to_string()),
            logo_url: Some("https://example.com/alpha.png".to_string()),
            logo_url_dark: None,
            distribution_channel: None,
            install_url: Some("https://chatgpt.com/apps/alpha/alpha".to_string()),
            is_accessible: false,
        },
    ];

    let second_update = read_app_list_updated_notification(&mut mcp).await?;
    assert_eq!(second_update.data, expected);

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let AppsListResponse { data, next_cursor } = to_response(response)?;
    assert_eq!(data, expected);
    assert!(next_cursor.is_none());

    server_handle.abort();
    Ok(())
}

#[tokio::test]
async fn list_apps_paginates_results() -> Result<()> {
    let connectors = vec![
        AppInfo {
            id: "alpha".to_string(),
            name: "Alpha".to_string(),
            description: Some("Alpha connector".to_string()),
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            install_url: None,
            is_accessible: false,
        },
        AppInfo {
            id: "beta".to_string(),
            name: "beta".to_string(),
            description: None,
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            install_url: None,
            is_accessible: false,
        },
    ];

    let tools = vec![connector_tool("beta", "Beta App")?];
    let (server_url, server_handle) = start_apps_server_with_delays(
        connectors.clone(),
        tools,
        Duration::ZERO,
        Duration::from_millis(300),
    )
    .await?;

    let codex_home = TempDir::new()?;
    write_connectors_config(codex_home.path(), &server_url)?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let first_request = mcp
        .send_apps_list_request(AppsListParams {
            limit: Some(1),
            cursor: None,
            thread_id: None,
            force_refetch: false,
        })
        .await?;
    let first_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(first_request)),
    )
    .await??;
    let AppsListResponse {
        data: first_page,
        next_cursor: first_cursor,
    } = to_response(first_response)?;

    let expected_first = vec![AppInfo {
        id: "beta".to_string(),
        name: "Beta App".to_string(),
        description: None,
        logo_url: None,
        logo_url_dark: None,
        distribution_channel: None,
        install_url: Some("https://chatgpt.com/apps/beta/beta".to_string()),
        is_accessible: true,
    }];

    assert_eq!(first_page, expected_first);
    let next_cursor = first_cursor.ok_or_else(|| anyhow::anyhow!("missing cursor"))?;

    loop {
        let update = read_app_list_updated_notification(&mut mcp).await?;
        if update.data.len() == 2 && update.data.iter().any(|connector| connector.is_accessible) {
            break;
        }
    }

    let second_request = mcp
        .send_apps_list_request(AppsListParams {
            limit: Some(1),
            cursor: Some(next_cursor),
            thread_id: None,
            force_refetch: false,
        })
        .await?;
    let second_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(second_request)),
    )
    .await??;
    let AppsListResponse {
        data: second_page,
        next_cursor: second_cursor,
    } = to_response(second_response)?;

    let expected_second = vec![AppInfo {
        id: "alpha".to_string(),
        name: "Alpha".to_string(),
        description: Some("Alpha connector".to_string()),
        logo_url: None,
        logo_url_dark: None,
        distribution_channel: None,
        install_url: Some("https://chatgpt.com/apps/alpha/alpha".to_string()),
        is_accessible: false,
    }];

    assert_eq!(second_page, expected_second);
    assert!(second_cursor.is_none());

    server_handle.abort();
    Ok(())
}

#[tokio::test]
async fn list_apps_force_refetch_preserves_previous_cache_on_failure() -> Result<()> {
    let connectors = vec![AppInfo {
        id: "beta".to_string(),
        name: "Beta App".to_string(),
        description: Some("Beta connector".to_string()),
        logo_url: None,
        logo_url_dark: None,
        distribution_channel: None,
        install_url: None,
        is_accessible: false,
    }];
    let tools = vec![connector_tool("beta", "Beta App")?];
    let (server_url, server_handle) =
        start_apps_server_with_delays(connectors, tools, Duration::ZERO, Duration::ZERO).await?;

    let codex_home = TempDir::new()?;
    write_connectors_config(codex_home.path(), &server_url)?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let initial_request = mcp
        .send_apps_list_request(AppsListParams {
            limit: None,
            cursor: None,
            thread_id: None,
            force_refetch: false,
        })
        .await?;
    let initial_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(initial_request)),
    )
    .await??;
    let AppsListResponse {
        data: initial_data,
        next_cursor: initial_next_cursor,
    } = to_response(initial_response)?;
    assert!(initial_next_cursor.is_none());
    assert_eq!(initial_data.len(), 1);
    assert!(initial_data.iter().all(|app| app.is_accessible));

    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token-invalid")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;

    let refetch_request = mcp
        .send_apps_list_request(AppsListParams {
            limit: None,
            cursor: None,
            thread_id: None,
            force_refetch: true,
        })
        .await?;
    let refetch_error: JSONRPCError = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(refetch_request)),
    )
    .await??;
    assert!(refetch_error.error.message.contains("failed to"));

    let cached_request = mcp
        .send_apps_list_request(AppsListParams {
            limit: None,
            cursor: None,
            thread_id: None,
            force_refetch: false,
        })
        .await?;
    let cached_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(cached_request)),
    )
    .await??;
    let AppsListResponse {
        data: cached_data,
        next_cursor: cached_next_cursor,
    } = to_response(cached_response)?;

    assert_eq!(cached_data, initial_data);
    assert!(cached_next_cursor.is_none());
    server_handle.abort();
    Ok(())
}

async fn read_app_list_updated_notification(
    mcp: &mut McpProcess,
) -> Result<AppListUpdatedNotification> {
    let notification = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("app/list/updated"),
    )
    .await??;
    let parsed: ServerNotification = notification.try_into()?;
    let ServerNotification::AppListUpdated(payload) = parsed else {
        bail!("unexpected notification variant");
    };
    Ok(payload)
}

#[derive(Clone)]
struct AppsServerState {
    expected_bearer: String,
    expected_account_id: String,
    response: serde_json::Value,
    directory_delay: Duration,
}

#[derive(Clone)]
struct AppListMcpServer {
    tools: Arc<Vec<Tool>>,
    tools_delay: Duration,
}

impl AppListMcpServer {
    fn new(tools: Arc<Vec<Tool>>, tools_delay: Duration) -> Self {
        Self { tools, tools_delay }
    }
}

impl ServerHandler for AppListMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..ServerInfo::default()
        }
    }

    fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, rmcp::ErrorData>> + Send + '_
    {
        let tools = self.tools.clone();
        let tools_delay = self.tools_delay;
        async move {
            if tools_delay > Duration::ZERO {
                tokio::time::sleep(tools_delay).await;
            }
            Ok(ListToolsResult {
                tools: (*tools).clone(),
                next_cursor: None,
                meta: None,
            })
        }
    }
}

async fn start_apps_server_with_delays(
    connectors: Vec<AppInfo>,
    tools: Vec<Tool>,
    directory_delay: Duration,
    tools_delay: Duration,
) -> Result<(String, JoinHandle<()>)> {
    let state = AppsServerState {
        expected_bearer: "Bearer chatgpt-token".to_string(),
        expected_account_id: "account-123".to_string(),
        response: json!({ "apps": connectors, "next_token": null }),
        directory_delay,
    };
    let state = Arc::new(state);
    let tools = Arc::new(tools);

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    let mcp_service = StreamableHttpService::new(
        {
            let tools = tools.clone();
            move || Ok(AppListMcpServer::new(tools.clone(), tools_delay))
        },
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );

    let router = Router::new()
        .route("/connectors/directory/list", get(list_directory_connectors))
        .route(
            "/connectors/directory/list_workspace",
            get(list_directory_connectors),
        )
        .with_state(state)
        .nest_service("/api/codex/apps", mcp_service);

    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });

    Ok((format!("http://{addr}"), handle))
}

async fn list_directory_connectors(
    State(state): State<Arc<AppsServerState>>,
    headers: HeaderMap,
) -> Result<impl axum::response::IntoResponse, StatusCode> {
    if state.directory_delay > Duration::ZERO {
        tokio::time::sleep(state.directory_delay).await;
    }

    let bearer_ok = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == state.expected_bearer);
    let account_ok = headers
        .get("chatgpt-account-id")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == state.expected_account_id);

    if bearer_ok && account_ok {
        Ok(Json(state.response.clone()))
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

fn connector_tool(connector_id: &str, connector_name: &str) -> Result<Tool> {
    let schema: JsonObject = serde_json::from_value(json!({
        "type": "object",
        "additionalProperties": false
    }))?;
    let mut tool = Tool::new(
        Cow::Owned(format!("connector_{connector_id}")),
        Cow::Borrowed("Connector test tool"),
        Arc::new(schema),
    );
    tool.annotations = Some(ToolAnnotations::new().read_only(true));

    let mut meta = Meta::new();
    meta.0
        .insert("connector_id".to_string(), json!(connector_id));
    meta.0
        .insert("connector_name".to_string(), json!(connector_name));
    tool.meta = Some(meta);
    Ok(tool)
}

fn write_connectors_config(codex_home: &std::path::Path, base_url: &str) -> std::io::Result<()> {
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
chatgpt_base_url = "{base_url}"
mcp_oauth_credentials_store = "file"

[features]
connectors = true
"#
        ),
    )
}
