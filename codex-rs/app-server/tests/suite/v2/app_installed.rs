use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::TestAppServer;
use app_test_support::to_response;
use app_test_support::write_chatgpt_auth;
use axum::Json;
use axum::Router;
use axum::routing::get;
use codex_app_server_protocol::AppInfo;
use codex_app_server_protocol::AppsInstalledParams;
use codex_app_server_protocol::AppsInstalledResponse;
use codex_app_server_protocol::AppsListParams;
use codex_app_server_protocol::AppsListResponse;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_config::types::AuthCredentialsStoreMode;
use pretty_assertions::assert_eq;
use rmcp::handler::server::ServerHandler;
use rmcp::model::ListToolsResult;
use rmcp::model::ServerCapabilities;
use rmcp::model::ServerInfo;
use rmcp::model::Tool;
use rmcp::transport::StreamableHttpServerConfig;
use rmcp::transport::StreamableHttpService;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use serde_json::json;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio::time::timeout;

use super::app_list::connector_tool;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

#[tokio::test]
async fn installed_apps_is_method_not_found_when_feature_is_disabled() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut app_server = TestAppServer::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, app_server.initialize()).await??;

    let request_id = app_server
        .send_apps_installed_request(AppsInstalledParams::default())
        .await?;
    let error: JSONRPCError = timeout(
        DEFAULT_TIMEOUT,
        app_server.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(error.error.code, -32601);
    assert_eq!(
        error.error.message,
        "app/installed is not enabled for this app-server"
    );
    Ok(())
}

#[tokio::test]
async fn installed_apps_reload_commits_tool_derived_ids_and_cached_read_does_not_fetch()
-> Result<()> {
    let fixture = InstalledAppsFixture::start().await?;
    let codex_home = configured_codex_home(fixture.base_url())?;
    let mut app_server = TestAppServer::new_without_managed_config(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, app_server.initialize()).await??;

    let initially_empty = send_installed_request(&mut app_server, /*reload*/ false).await?;
    assert_eq!(initially_empty, AppsInstalledResponse { apps: Vec::new() });
    assert_eq!(fixture.list_tools_calls(), 0);
    assert_eq!(fixture.workspace_settings_calls(), 0);

    let refreshed = send_installed_request(&mut app_server, /*reload*/ true).await?;
    assert_eq!(
        refreshed.apps,
        vec![
            codex_app_server_protocol::InstalledApp {
                id: "alpha".to_string(),
                enabled: true,
                callable: true,
            },
            codex_app_server_protocol::InstalledApp {
                id: "blocked".to_string(),
                enabled: true,
                callable: false,
            },
            codex_app_server_protocol::InstalledApp {
                id: "disabled".to_string(),
                enabled: false,
                callable: false,
            },
        ]
    );
    assert_eq!(fixture.list_tools_calls(), 1);
    assert_eq!(fixture.workspace_settings_calls(), 1);

    let cached = send_installed_request(&mut app_server, /*reload*/ false).await?;
    assert_eq!(cached, refreshed);
    assert_eq!(fixture.list_tools_calls(), 1);
    assert_eq!(fixture.workspace_settings_calls(), 1);
    fixture.shutdown();
    Ok(())
}

#[tokio::test]
async fn installed_apps_reload_commits_empty_tools_result_and_cached_read_does_not_fetch()
-> Result<()> {
    let fixture = InstalledAppsFixture::start().await?;
    fixture.set_tools(Vec::new());
    let codex_home = configured_codex_home(fixture.base_url())?;
    let mut app_server = TestAppServer::new_without_managed_config(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, app_server.initialize()).await??;

    let refreshed = send_installed_request(&mut app_server, /*reload*/ true).await?;
    assert_eq!(refreshed, AppsInstalledResponse { apps: Vec::new() });
    assert_eq!(fixture.list_tools_calls(), 1);

    let cached = send_installed_request(&mut app_server, /*reload*/ false).await?;
    assert_eq!(cached, refreshed);
    assert_eq!(fixture.list_tools_calls(), 1);
    fixture.shutdown();
    Ok(())
}

#[tokio::test]
async fn installed_apps_does_not_treat_synthetic_link_as_installed_identity() -> Result<()> {
    let fixture = InstalledAppsFixture::start().await?;
    let mut synthetic_link = connector_tool("link-only", "Link Only")?;
    synthetic_link
        .meta
        .as_mut()
        .expect("connector tool should have metadata")
        .0
        .insert("_codex_apps".to_string(), json!({ "synthetic_link": true }));
    fixture.set_tools(vec![
        synthetic_link,
        connector_tool("alpha", "Alpha Tool Name")?,
    ]);
    let codex_home = configured_codex_home(fixture.base_url())?;
    let mut app_server = TestAppServer::new_without_managed_config(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, app_server.initialize()).await??;

    let response = send_installed_request(&mut app_server, /*reload*/ true).await?;
    assert_eq!(
        response.apps,
        vec![codex_app_server_protocol::InstalledApp {
            id: "alpha".to_string(),
            enabled: true,
            callable: true,
        }]
    );

    fixture.shutdown();
    Ok(())
}

#[tokio::test]
async fn legacy_app_list_force_refetch_still_works_with_runtime_refactor_enabled() -> Result<()> {
    let fixture = InstalledAppsFixture::start().await?;
    fixture.set_tools(vec![connector_tool("alpha", "Alpha Tool Name")?]);
    let directory_app = AppInfo {
        id: "alpha".to_string(),
        name: "Alpha Directory".to_string(),
        description: Some("Directory metadata v1".to_string()),
        logo_url: None,
        logo_url_dark: None,
        icon_assets: None,
        icon_dark_assets: None,
        distribution_channel: None,
        branding: None,
        app_metadata: None,
        labels: None,
        install_url: None,
        is_accessible: false,
        is_enabled: true,
        plugin_display_names: Vec::new(),
    };
    fixture.set_directory_apps(vec![directory_app.clone()]);
    let codex_home = configured_codex_home(fixture.base_url())?;
    let mut app_server = TestAppServer::new_without_managed_config(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, app_server.initialize()).await??;

    let installed = send_installed_request(&mut app_server, /*reload*/ true).await?;
    assert_eq!(
        installed.apps,
        vec![codex_app_server_protocol::InstalledApp {
            id: "alpha".to_string(),
            enabled: true,
            callable: true,
        }]
    );
    assert_eq!(fixture.directory_calls(), 0);

    let initial_request_id = app_server
        .send_apps_list_request(AppsListParams {
            limit: None,
            cursor: None,
            thread_id: None,
            force_refetch: false,
        })
        .await?;
    let initial_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(initial_request_id)),
    )
    .await??;
    let AppsListResponse {
        data: initial_data,
        next_cursor: initial_next_cursor,
    } = to_response(initial_response)?;
    assert_eq!(initial_data.len(), 1);
    assert_eq!(initial_data[0].id, "alpha");
    assert_eq!(
        initial_data[0].description,
        Some("Directory metadata v1".to_string())
    );
    assert!(initial_data[0].is_accessible);
    assert!(initial_next_cursor.is_none());
    let initial_directory_calls = fixture.directory_calls();
    assert!(initial_directory_calls > 0);

    let mut updated_directory_app = directory_app;
    updated_directory_app.description = Some("Directory metadata v2".to_string());
    fixture.set_directory_apps(vec![updated_directory_app]);
    let refresh_request_id = app_server
        .send_apps_list_request(AppsListParams {
            limit: None,
            cursor: None,
            thread_id: None,
            force_refetch: true,
        })
        .await?;
    let refresh_response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(refresh_request_id)),
    )
    .await??;
    let wire_response = serde_json::to_value(&refresh_response)?;
    assert!(wire_response["result"]["data"].is_array());
    assert!(wire_response["result"].get("nextCursor").is_some());
    assert!(wire_response["result"].get("apps").is_none());
    let AppsListResponse {
        data: refreshed_data,
        next_cursor: refreshed_next_cursor,
    } = to_response(refresh_response)?;
    assert_eq!(refreshed_data.len(), 1);
    assert_eq!(refreshed_data[0].id, "alpha");
    assert_eq!(
        refreshed_data[0].description,
        Some("Directory metadata v2".to_string())
    );
    assert!(refreshed_data[0].is_accessible);
    assert!(refreshed_next_cursor.is_none());
    assert!(fixture.directory_calls() > initial_directory_calls);

    fixture.shutdown();
    Ok(())
}

#[tokio::test]
async fn installed_apps_workspace_policy_retains_identities_as_disabled() -> Result<()> {
    let fixture = InstalledAppsFixture::start().await?;
    let codex_home = configured_codex_home(fixture.base_url())?;
    {
        let mut app_server = TestAppServer::new_without_managed_config(codex_home.path()).await?;
        timeout(DEFAULT_TIMEOUT, app_server.initialize()).await??;
        let committed = send_installed_request(&mut app_server, /*reload*/ true).await?;
        assert_eq!(committed.apps.len(), 3);
    }

    fixture.set_workspace_plugins_enabled(false);
    let mut app_server = TestAppServer::new_without_managed_config(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, app_server.initialize()).await??;
    let cold_cached = send_installed_request(&mut app_server, /*reload*/ false).await?;
    assert_eq!(cold_cached.apps.len(), 3);
    assert!(
        cold_cached
            .apps
            .iter()
            .all(|app| !app.enabled && !app.callable)
    );
    assert_eq!(fixture.workspace_settings_calls(), 1);

    let blocked = send_installed_request(&mut app_server, /*reload*/ true).await?;
    assert_eq!(blocked.apps.len(), 3);
    assert!(blocked.apps.iter().all(|app| !app.enabled && !app.callable));
    assert_eq!(fixture.list_tools_calls(), 1);
    assert_eq!(fixture.workspace_settings_calls(), 2);
    fixture.shutdown();
    Ok(())
}

#[tokio::test]
async fn installed_apps_global_disable_retains_tool_derived_identities() -> Result<()> {
    let fixture = InstalledAppsFixture::start().await?;
    let codex_home = configured_codex_home(fixture.base_url())?;
    {
        let mut app_server = TestAppServer::new_without_managed_config(codex_home.path()).await?;
        timeout(DEFAULT_TIMEOUT, app_server.initialize()).await??;
        let committed = send_installed_request(&mut app_server, /*reload*/ true).await?;
        assert_eq!(committed.apps.len(), 3);
    }

    let config_path = codex_home.path().join("config.toml");
    let config = std::fs::read_to_string(&config_path)?;
    std::fs::write(&config_path, config.replace("apps = true", "apps = false"))?;
    let mut app_server = TestAppServer::new_without_managed_config(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, app_server.initialize()).await??;

    let cached = send_installed_request(&mut app_server, /*reload*/ false).await?;
    assert_eq!(cached.apps.len(), 3);
    assert!(cached.apps.iter().all(|app| !app.enabled && !app.callable));
    let reload = send_installed_request(&mut app_server, /*reload*/ true).await?;
    assert_eq!(reload, cached);
    assert_eq!(fixture.list_tools_calls(), 1);
    assert_eq!(fixture.workspace_settings_calls(), 1);

    fixture.shutdown();
    Ok(())
}

#[tokio::test]
async fn installed_apps_thread_id_uses_effective_thread_config() -> Result<()> {
    let fixture = InstalledAppsFixture::start().await?;
    let codex_home = configured_codex_home(fixture.base_url())?;
    let mut app_server = TestAppServer::new_without_managed_config(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, app_server.initialize()).await??;
    send_installed_request(&mut app_server, /*reload*/ true).await?;

    let request_id = app_server
        .send_thread_start_request(ThreadStartParams {
            config: Some(HashMap::from([(
                "apps.alpha.enabled".to_string(),
                json!(false),
            )])),
            ..Default::default()
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response(response)?;

    let request_id = app_server
        .send_apps_installed_request(AppsInstalledParams {
            thread_id: Some(thread.id),
            reload: false,
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: AppsInstalledResponse = to_response(response)?;
    assert_eq!(
        response.apps,
        vec![
            codex_app_server_protocol::InstalledApp {
                id: "alpha".to_string(),
                enabled: false,
                callable: false,
            },
            codex_app_server_protocol::InstalledApp {
                id: "blocked".to_string(),
                enabled: true,
                callable: false,
            },
            codex_app_server_protocol::InstalledApp {
                id: "disabled".to_string(),
                enabled: false,
                callable: false,
            },
        ]
    );

    fixture.shutdown();
    Ok(())
}

#[tokio::test]
async fn installed_apps_failed_reload_retains_previous_snapshot() -> Result<()> {
    let fixture = InstalledAppsFixture::start().await?;
    let codex_home = configured_codex_home(fixture.base_url())?;
    let mut app_server = TestAppServer::new_without_managed_config(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, app_server.initialize()).await??;

    let committed = send_installed_request(&mut app_server, /*reload*/ true).await?;
    fixture.fail_next_list_tools();
    let request_id = app_server
        .send_apps_installed_request(AppsInstalledParams {
            thread_id: None,
            reload: true,
        })
        .await?;
    let error: JSONRPCError = timeout(
        DEFAULT_TIMEOUT,
        app_server.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(error.error.code, -32603);

    let retained = send_installed_request(&mut app_server, /*reload*/ false).await?;
    assert_eq!(retained, committed);
    assert_eq!(fixture.list_tools_calls(), 2);
    fixture.shutdown();
    Ok(())
}

#[tokio::test]
async fn installed_apps_concurrent_reloads_are_serialized() -> Result<()> {
    let fixture = InstalledAppsFixture::start().await?;
    let codex_home = configured_codex_home(fixture.base_url())?;
    let mut app_server = TestAppServer::new_without_managed_config(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, app_server.initialize()).await??;

    let first_id = app_server
        .send_apps_installed_request(AppsInstalledParams {
            thread_id: None,
            reload: true,
        })
        .await?;
    let second_id = app_server
        .send_apps_installed_request(AppsInstalledParams {
            thread_id: None,
            reload: true,
        })
        .await?;
    for request_id in [first_id, second_id] {
        let response: JSONRPCResponse = timeout(
            DEFAULT_TIMEOUT,
            app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
        )
        .await??;
        let response: AppsInstalledResponse = to_response(response)?;
        assert_eq!(response.apps.len(), 3);
    }

    assert_eq!(fixture.list_tools_calls(), 2);
    assert_eq!(fixture.max_in_flight(), 1);
    fixture.shutdown();
    Ok(())
}

async fn send_installed_request(
    app_server: &mut TestAppServer,
    reload: bool,
) -> Result<AppsInstalledResponse> {
    let request_id = app_server
        .send_apps_installed_request(AppsInstalledParams {
            thread_id: None,
            reload,
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    to_response(response)
}

fn configured_codex_home(base_url: &str) -> Result<TempDir> {
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            r#"
chatgpt_base_url = "{base_url}"
mcp_oauth_credentials_store = "file"

[features]
apps = true
apps_runtime_state_refactor = true

[apps.blocked]
default_tools_enabled = false

[apps.disabled]
enabled = false
"#,
        ),
    )?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123")
            .plan_type("team"),
        AuthCredentialsStoreMode::File,
    )?;
    Ok(codex_home)
}

#[derive(Clone)]
struct InstalledAppsMcpServer {
    state: Arc<InstalledAppsServerState>,
}

impl ServerHandler for InstalledAppsMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
    }

    fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, rmcp::ErrorData>> + Send + '_
    {
        let state = Arc::clone(&self.state);
        async move {
            state.list_tools_calls.fetch_add(1, Ordering::SeqCst);
            let in_flight = state.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
            state.max_in_flight.fetch_max(in_flight, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(100)).await;
            let should_fail = state.fail_next.swap(false, Ordering::SeqCst);
            state.in_flight.fetch_sub(1, Ordering::SeqCst);
            if should_fail {
                return Err(rmcp::ErrorData::internal_error(
                    "injected tools/list failure",
                    None,
                ));
            }

            Ok(ListToolsResult {
                meta: None,
                next_cursor: None,
                tools: state
                    .tools
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone(),
            })
        }
    }
}

struct InstalledAppsServerState {
    tools: Mutex<Vec<Tool>>,
    directory_apps: Mutex<Vec<AppInfo>>,
    list_tools_calls: AtomicUsize,
    directory_calls: AtomicUsize,
    workspace_settings_calls: AtomicUsize,
    workspace_plugins_enabled: AtomicBool,
    in_flight: AtomicUsize,
    max_in_flight: AtomicUsize,
    fail_next: AtomicBool,
}

struct InstalledAppsFixture {
    base_url: String,
    state: Arc<InstalledAppsServerState>,
    handle: JoinHandle<()>,
}

impl InstalledAppsFixture {
    async fn start() -> Result<Self> {
        let state = Arc::new(InstalledAppsServerState {
            tools: Mutex::new(vec![
                connector_tool("alpha", "Alpha Tool Name")?,
                connector_tool("blocked", "Policy Blocked Tool Name")?,
                connector_tool("disabled", "Locally Disabled Tool Name")?,
                connector_tool("alpha", "Duplicate Alpha Tool Name")?,
                connector_tool("", "Empty Connector ID")?,
                Tool::new(
                    "missing_connector_id",
                    "Missing connector id",
                    Arc::new(Default::default()),
                ),
            ]),
            directory_apps: Mutex::new(Vec::new()),
            list_tools_calls: AtomicUsize::new(0),
            directory_calls: AtomicUsize::new(0),
            workspace_settings_calls: AtomicUsize::new(0),
            workspace_plugins_enabled: AtomicBool::new(true),
            in_flight: AtomicUsize::new(0),
            max_in_flight: AtomicUsize::new(0),
            fail_next: AtomicBool::new(false),
        });
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let mcp_service = StreamableHttpService::new(
            {
                let state = Arc::clone(&state);
                move || {
                    Ok(InstalledAppsMcpServer {
                        state: Arc::clone(&state),
                    })
                }
            },
            Arc::new(LocalSessionManager::default()),
            StreamableHttpServerConfig::default(),
        );
        let workspace_state = Arc::clone(&state);
        let directory_state = Arc::clone(&state);
        let workspace_directory_state = Arc::clone(&state);
        let router = Router::new()
            .route(
                "/connectors/directory/list",
                get(move || {
                    let state = Arc::clone(&directory_state);
                    async move {
                        state.directory_calls.fetch_add(1, Ordering::SeqCst);
                        let apps = state
                            .directory_apps
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .clone();
                        Json(json!({ "apps": apps, "next_token": null }))
                    }
                }),
            )
            .route(
                "/connectors/directory/list_workspace",
                get(move || {
                    let state = Arc::clone(&workspace_directory_state);
                    async move {
                        state.directory_calls.fetch_add(1, Ordering::SeqCst);
                        let apps = state
                            .directory_apps
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .clone();
                        Json(json!({ "apps": apps, "next_token": null }))
                    }
                }),
            )
            .route(
                "/accounts/account-123/settings",
                get(move || {
                    let state = Arc::clone(&workspace_state);
                    async move {
                        state
                            .workspace_settings_calls
                            .fetch_add(1, Ordering::SeqCst);
                        let enabled = state.workspace_plugins_enabled.load(Ordering::SeqCst);
                        Json(json!({
                            "beta_settings": { "enable_plugins": enabled }
                        }))
                    }
                }),
            )
            .nest_service("/api/codex/ps/mcp", mcp_service);
        let handle = tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });
        Ok(Self {
            base_url: format!("http://{address}"),
            state,
            handle,
        })
    }

    fn base_url(&self) -> &str {
        &self.base_url
    }

    fn list_tools_calls(&self) -> usize {
        self.state.list_tools_calls.load(Ordering::SeqCst)
    }

    fn set_tools(&self, tools: Vec<Tool>) {
        *self
            .state
            .tools
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = tools;
    }

    fn set_directory_apps(&self, apps: Vec<AppInfo>) {
        *self
            .state
            .directory_apps
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = apps;
    }

    fn directory_calls(&self) -> usize {
        self.state.directory_calls.load(Ordering::SeqCst)
    }

    fn max_in_flight(&self) -> usize {
        self.state.max_in_flight.load(Ordering::SeqCst)
    }

    fn workspace_settings_calls(&self) -> usize {
        self.state.workspace_settings_calls.load(Ordering::SeqCst)
    }

    fn set_workspace_plugins_enabled(&self, enabled: bool) {
        self.state
            .workspace_plugins_enabled
            .store(enabled, Ordering::SeqCst);
    }

    fn fail_next_list_tools(&self) {
        self.state.fail_next.store(true, Ordering::SeqCst);
    }

    fn shutdown(self) {
        self.handle.abort();
    }
}
