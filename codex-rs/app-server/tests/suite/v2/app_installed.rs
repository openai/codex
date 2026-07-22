use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::TestAppServer;
use app_test_support::write_chatgpt_auth;
use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use codex_app_server_protocol::AppsInstalledParams;
use codex_app_server_protocol::AppsInstalledResponse;
use codex_app_server_protocol::InstalledApp;
use codex_app_server_protocol::JSONRPCError;
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
async fn installed_apps_force_refresh_only_refreshes_tools_snapshot() -> Result<()> {
    let fixture = InstalledAppsFixture::start().await?;
    let codex_home = configured_codex_home(fixture.base_url())?;
    let mut app_server = start_app_server(codex_home.path()).await?;

    let initially_empty = send_installed_request(&mut app_server, /*force_refresh*/ false).await?;
    assert_eq!(initially_empty, AppsInstalledResponse { apps: Vec::new() });
    assert_eq!(fixture.list_tools_calls(), 0);
    assert_eq!(fixture.workspace_settings_calls(), 1);

    let refreshed = send_installed_request(&mut app_server, /*force_refresh*/ true).await?;
    assert_eq!(
        refreshed.apps,
        vec![
            InstalledApp {
                id: "alpha".to_string(),
                runtime_name: Some("Alpha Tool Name".to_string()),
                enabled: true,
                callable: true,
            },
            InstalledApp {
                id: "blocked".to_string(),
                runtime_name: Some("Policy Blocked Tool Name".to_string()),
                enabled: true,
                callable: false,
            },
            InstalledApp {
                id: "disabled".to_string(),
                runtime_name: Some("Locally Disabled Tool Name".to_string()),
                enabled: false,
                callable: false,
            },
        ]
    );
    assert_eq!(fixture.list_tools_calls(), 1);
    assert_eq!(fixture.workspace_settings_calls(), 1);

    let cached = send_installed_request(&mut app_server, /*force_refresh*/ false).await?;
    assert_eq!(cached, refreshed);
    assert_eq!(fixture.list_tools_calls(), 1);
    assert_eq!(fixture.workspace_settings_calls(), 1);

    fixture.set_tools(Vec::new());
    let empty = send_installed_request(&mut app_server, /*force_refresh*/ true).await?;
    assert_eq!(empty, AppsInstalledResponse { apps: Vec::new() });
    assert_eq!(fixture.list_tools_calls(), 2);

    let cached_empty = send_installed_request(&mut app_server, /*force_refresh*/ false).await?;
    assert_eq!(cached_empty, empty);
    assert_eq!(fixture.list_tools_calls(), 2);
    assert_eq!(fixture.workspace_settings_calls(), 1);
    assert_eq!(fixture.directory_calls(), 0);
    Ok(())
}

#[tokio::test]
async fn installed_apps_workspace_policy_retains_identities_as_disabled() -> Result<()> {
    let fixture = InstalledAppsFixture::start().await?;
    let codex_home = configured_codex_home(fixture.base_url())?;
    let committed = {
        let mut app_server = start_app_server(codex_home.path()).await?;
        send_installed_request(&mut app_server, /*force_refresh*/ true).await?
    };
    let mut expected_disabled = committed;
    for app in &mut expected_disabled.apps {
        app.enabled = false;
        app.callable = false;
    }

    fixture.set_workspace_plugins_enabled(/*enabled*/ false);
    let mut app_server = start_app_server(codex_home.path()).await?;
    let cold_cached = send_installed_request(&mut app_server, /*force_refresh*/ false).await?;
    assert_eq!(cold_cached, expected_disabled);
    assert_eq!(fixture.workspace_settings_calls(), 2);
    let workspace_settings_calls = fixture.workspace_settings_calls();

    let blocked = send_installed_request(&mut app_server, /*force_refresh*/ true).await?;
    assert_eq!(blocked, expected_disabled);
    assert_eq!(fixture.list_tools_calls(), 1);
    assert_eq!(fixture.workspace_settings_calls(), workspace_settings_calls);
    Ok(())
}

#[tokio::test]
async fn installed_apps_workspace_policy_failure_does_not_block_force_refresh() -> Result<()> {
    let fixture = InstalledAppsFixture::start().await?;
    fixture
        .state
        .fail_workspace_settings
        .store(true, Ordering::SeqCst);
    fixture.set_tools(vec![connector_tool("alpha", "Alpha Tool Name")?]);
    let codex_home = configured_codex_home(fixture.base_url())?;
    let mut app_server = start_app_server(codex_home.path()).await?;

    let refreshed = send_installed_request(&mut app_server, /*force_refresh*/ true).await?;
    assert_eq!(
        refreshed,
        AppsInstalledResponse {
            apps: vec![InstalledApp {
                id: "alpha".to_string(),
                runtime_name: Some("Alpha Tool Name".to_string()),
                enabled: true,
                callable: true,
            }],
        }
    );
    assert_eq!(fixture.workspace_settings_calls(), 1);
    assert_eq!(fixture.list_tools_calls(), 1);
    Ok(())
}

#[tokio::test]
async fn installed_apps_global_disable_retains_tool_derived_identities() -> Result<()> {
    let fixture = InstalledAppsFixture::start().await?;
    let codex_home = configured_codex_home(fixture.base_url())?;
    let committed = {
        let mut app_server = start_app_server(codex_home.path()).await?;
        send_installed_request(&mut app_server, /*force_refresh*/ true).await?
    };
    let mut expected_disabled = committed;
    for app in &mut expected_disabled.apps {
        app.enabled = false;
        app.callable = false;
    }

    let config_path = codex_home.path().join("config.toml");
    let config = std::fs::read_to_string(&config_path)?;
    std::fs::write(&config_path, config.replace("apps = true", "apps = false"))?;
    let mut app_server = start_app_server(codex_home.path()).await?;

    let cached = send_installed_request(&mut app_server, /*force_refresh*/ false).await?;
    assert_eq!(cached, expected_disabled);
    let force_refresh = send_installed_request(&mut app_server, /*force_refresh*/ true).await?;
    assert_eq!(force_refresh, cached);
    assert_eq!(fixture.list_tools_calls(), 1);
    assert_eq!(fixture.workspace_settings_calls(), 1);

    Ok(())
}

#[tokio::test]
async fn installed_apps_thread_id_uses_effective_thread_config() -> Result<()> {
    let fixture = InstalledAppsFixture::start().await?;
    let codex_home = configured_codex_home(fixture.base_url())?;
    let mut app_server = start_app_server(codex_home.path()).await?;
    let mut expected = send_installed_request(&mut app_server, /*force_refresh*/ true).await?;

    let request_id = app_server
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            config: Some(HashMap::from([(
                "apps.alpha.enabled".to_string(),
                json!(false),
            )])),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_TIMEOUT, app_server.read_response(request_id)).await??;

    let request_id = app_server
        .send_apps_installed_request(AppsInstalledParams {
            thread_id: Some(thread.id),
            force_refresh: false,
        })
        .await?;
    let response: AppsInstalledResponse =
        timeout(DEFAULT_TIMEOUT, app_server.read_response(request_id)).await??;
    let alpha = expected
        .apps
        .iter_mut()
        .find(|app| app.id == "alpha")
        .expect("alpha app should be installed");
    alpha.enabled = false;
    alpha.callable = false;
    assert_eq!(response, expected);

    Ok(())
}

#[tokio::test]
async fn installed_apps_failed_force_refresh_retains_previous_snapshot() -> Result<()> {
    let fixture = InstalledAppsFixture::start().await?;
    let codex_home = configured_codex_home(fixture.base_url())?;
    let mut app_server = start_app_server(codex_home.path()).await?;

    let committed = send_installed_request(&mut app_server, /*force_refresh*/ true).await?;
    fixture.fail_next_list_tools();
    let request_id = app_server
        .send_apps_installed_request(AppsInstalledParams {
            thread_id: None,
            force_refresh: true,
        })
        .await?;
    let error: JSONRPCError = timeout(
        DEFAULT_TIMEOUT,
        app_server.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;
    assert_eq!(error.error.code, -32603);

    let retained = send_installed_request(&mut app_server, /*force_refresh*/ false).await?;
    assert_eq!(retained, committed);
    assert_eq!(fixture.list_tools_calls(), 2);
    Ok(())
}

async fn start_app_server(codex_home: &Path) -> Result<TestAppServer> {
    TestAppServer::builder()
        .with_codex_home(codex_home)
        .without_managed_config()
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await
}

async fn send_installed_request(
    app_server: &mut TestAppServer,
    force_refresh: bool,
) -> Result<AppsInstalledResponse> {
    let request_id = app_server
        .send_apps_installed_request(AppsInstalledParams {
            thread_id: None,
            force_refresh,
        })
        .await?;
    timeout(DEFAULT_TIMEOUT, app_server.read_response(request_id)).await?
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
            let should_fail = state.fail_next.swap(false, Ordering::SeqCst);
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
    list_tools_calls: AtomicUsize,
    directory_calls: AtomicUsize,
    workspace_settings_calls: AtomicUsize,
    workspace_plugins_enabled: AtomicBool,
    fail_workspace_settings: AtomicBool,
    fail_next: AtomicBool,
}

struct InstalledAppsFixture {
    base_url: String,
    state: Arc<InstalledAppsServerState>,
    handle: JoinHandle<()>,
}

impl InstalledAppsFixture {
    async fn start() -> Result<Self> {
        let mut synthetic_link = connector_tool("link-only", "Link Only")?;
        synthetic_link
            .meta
            .as_mut()
            .expect("connector tool should have metadata")
            .0
            .insert("_codex_apps".to_string(), json!({ "synthetic_link": true }));
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
                synthetic_link,
            ]),
            list_tools_calls: AtomicUsize::new(0),
            directory_calls: AtomicUsize::new(0),
            workspace_settings_calls: AtomicUsize::new(0),
            workspace_plugins_enabled: AtomicBool::new(true),
            fail_workspace_settings: AtomicBool::new(false),
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
        let router = Router::new()
            .route("/connectors/directory/list", get(list_directory_apps))
            .route(
                "/connectors/directory/list_workspace",
                get(list_directory_apps),
            )
            .route("/accounts/account-123/settings", get(workspace_settings))
            .nest_service("/api/codex/ps/mcp", mcp_service)
            .with_state(Arc::clone(&state));
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

    fn directory_calls(&self) -> usize {
        self.state.directory_calls.load(Ordering::SeqCst)
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
}

impl Drop for InstalledAppsFixture {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

async fn list_directory_apps(
    State(state): State<Arc<InstalledAppsServerState>>,
) -> Json<serde_json::Value> {
    state.directory_calls.fetch_add(1, Ordering::SeqCst);
    Json(json!({ "apps": [], "next_token": null }))
}

async fn workspace_settings(
    State(state): State<Arc<InstalledAppsServerState>>,
) -> (StatusCode, Json<serde_json::Value>) {
    state
        .workspace_settings_calls
        .fetch_add(1, Ordering::SeqCst);
    let enabled = state.workspace_plugins_enabled.load(Ordering::SeqCst);
    let status = if state.fail_workspace_settings.load(Ordering::SeqCst) {
        StatusCode::INTERNAL_SERVER_ERROR
    } else {
        StatusCode::OK
    };
    (
        status,
        Json(json!({
            "beta_settings": { "enable_plugins": enabled }
        })),
    )
}
