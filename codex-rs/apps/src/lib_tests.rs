use std::collections::HashMap;
use std::collections::HashSet;
use std::io;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use axum::Router;
use codex_api::AuthProvider;
use codex_config::Constrained;
use codex_config::McpServerTransportConfig;
use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
#[cfg(unix)]
use codex_exec_server::CODEX_FS_HELPER_ARG1;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::HttpClient;
use codex_exec_server::ReqwestHttpClient;
use codex_mcp::EffectiveMcpServer;
use codex_mcp::MCP_SANDBOX_STATE_META_CAPABILITY;
use codex_mcp::McpConnectionManager;
use codex_mcp::McpConnectionManagerInput;
use codex_mcp::McpRuntimeContext;
use codex_mcp::SandboxState;
use codex_mcp::ToolPluginProvenance;
use codex_protocol::ToolName;
use codex_protocol::mcp::MCP_APPROVAL_CONTEXT_CONNECTED_ACCOUNT_EMAIL_KEY;
use codex_protocol::mcp::MCP_APPROVAL_CONTEXT_META_KEY;
use codex_protocol::mcp::MCP_ERROR_CODE_META_KEY;
use codex_protocol::models::PermissionProfile;
#[cfg(unix)]
use codex_protocol::permissions::FileSystemAccessMode;
#[cfg(unix)]
use codex_protocol::permissions::FileSystemPath;
#[cfg(unix)]
use codex_protocol::permissions::FileSystemSandboxEntry;
#[cfg(unix)]
use codex_protocol::permissions::FileSystemSandboxPolicy;
#[cfg(unix)]
use codex_protocol::permissions::FileSystemSpecialPath;
#[cfg(unix)]
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_rmcp_client::ElicitationResponse;
use codex_utils_path_uri::PathUri;
use futures::FutureExt;
use pretty_assertions::assert_eq;
use reqwest::StatusCode;
use reqwest::header::ORIGIN;
use rmcp::ServerHandler;
use rmcp::model::AnnotateAble;
use rmcp::model::CallToolRequestParams;
use rmcp::model::CallToolResult;
use rmcp::model::ClientCapabilities;
use rmcp::model::ClientResult;
use rmcp::model::Content;
use rmcp::model::CreateElicitationRequest;
use rmcp::model::CreateElicitationRequestParams;
use rmcp::model::CustomRequest;
use rmcp::model::ElicitationAction;
use rmcp::model::ElicitationSchema;
use rmcp::model::GetMeta;
use rmcp::model::Implementation;
use rmcp::model::InitializeRequestParams;
use rmcp::model::InitializeResult;
use rmcp::model::JsonObject;
use rmcp::model::ListResourceTemplatesResult;
use rmcp::model::ListResourcesResult;
use rmcp::model::ListToolsResult;
use rmcp::model::Meta;
use rmcp::model::ProtocolVersion;
use rmcp::model::RawResource;
use rmcp::model::RawResourceTemplate;
use rmcp::model::ReadResourceRequestParams;
use rmcp::model::ReadResourceResult;
use rmcp::model::Resource;
use rmcp::model::ResourceContents;
use rmcp::model::ServerCapabilities;
use rmcp::model::ServerInfo;
use rmcp::model::ServerRequest;
use rmcp::model::Tool;
use rmcp::service::RequestContext;
use rmcp::service::RoleServer;
use rmcp::transport::StreamableHttpServerConfig;
use rmcp::transport::StreamableHttpService;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use serde_json::json;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tokio_util::task::AbortOnDropHandle;

#[cfg(unix)]
use codex_test_binary_support::TestBinaryDispatchGuard;
#[cfg(unix)]
use codex_test_binary_support::TestBinaryDispatchMode;
#[cfg(unix)]
use codex_test_binary_support::configure_test_binary_dispatch;
#[cfg(unix)]
use ctor::ctor;

use super::auth_elicitation::MCP_TOOL_CODEX_APPS_META_KEY;
use super::connector_server::META_CONNECTED_ACCOUNT_EMAIL;
use super::connector_server::move_connected_account_to_approval_context;
use super::file_upload::AppsFileSupport;
use super::file_upload::META_OPENAI_FILE_PARAMS;
use super::file_upload::rewrite_arguments_for_openai_files;
use super::file_upload::rewrite_tool_schema_for_local_file_paths;
use super::*;

const TEST_TIMEOUT: Duration = Duration::from_secs(10);

#[cfg(unix)]
#[ctor]
static TEST_BINARY_DISPATCH_GUARD: Option<TestBinaryDispatchGuard> = {
    configure_test_binary_dispatch("codex-apps-tests", |exe_name, argv1| {
        if argv1 == Some(CODEX_FS_HELPER_ARG1) || exe_name == "codex-linux-sandbox" {
            TestBinaryDispatchMode::DispatchArg0Only
        } else {
            TestBinaryDispatchMode::InstallAliases
        }
    })
};

async fn wait_for_background_refresh(apps: &CodexApps) {
    let background_refresh = apps.background_refresh.lock().await.take();
    if let Some(task) = background_refresh {
        tokio::time::timeout(TEST_TIMEOUT, task)
            .await
            .expect("Codex Apps background refresh timed out")
            .expect("Codex Apps background refresh task");
    }
}

async fn abort_server<T>(server: JoinHandle<T>) {
    server.abort();
    let _ = tokio::time::timeout(TEST_TIMEOUT, server)
        .await
        .expect("test MCP server shutdown timed out");
}

#[test]
fn connect_config_normalizes_supported_upstream_urls() {
    let connect_config = |base_url: &str| {
        CodexAppsConnectConfig::new(
            base_url.to_string(),
            /*product_sku*/ None,
            OAuthCredentialsStoreMode::File,
            AuthKeyringBackendKind::Direct,
        )
    };
    assert_eq!(
        connect_config("https://chatgpt.com").upstream_url(),
        "https://chatgpt.com/backend-api/ps/mcp"
    );
    assert_eq!(
        connect_config("https://chat.openai.com/backend-api/").upstream_url(),
        "https://chat.openai.com/backend-api/ps/mcp"
    );
    assert_eq!(
        connect_config("http://127.0.0.1:1234").upstream_url(),
        "http://127.0.0.1:1234/api/codex/ps/mcp"
    );
    assert_eq!(
        connect_config("https://example.com/api/codex").upstream_url(),
        "https://example.com/api/codex/ps/mcp"
    );
}

#[test]
fn standard_upstream_auth_elicitation_capabilities_are_explicitly_gated() {
    let disabled =
        AppsElicitationBridge::upstream_capabilities(/*auth_elicitation_enabled*/ false);
    assert!(disabled.elicitation.is_none());
    assert!(
        disabled
            .extensions
            .as_ref()
            .is_some_and(|extensions| extensions.contains_key("openai/form"))
    );

    let enabled =
        AppsElicitationBridge::upstream_capabilities(/*auth_elicitation_enabled*/ true);
    let elicitation = enabled.elicitation.expect("elicitation capability");
    assert!(elicitation.form.is_some());
    assert!(elicitation.url.is_some());
    assert!(
        enabled
            .extensions
            .as_ref()
            .is_some_and(|extensions| extensions.contains_key("openai/form"))
    );
}

#[derive(Debug)]
struct EmptyAuthProvider;

impl AuthProvider for EmptyAuthProvider {
    fn add_auth_headers(&self, _headers: &mut ::http::HeaderMap) {}
}

struct HostedUpstream {
    config: CodexAppsConnectConfig,
    server: AbortOnDropHandle<io::Result<()>>,
}

pub(crate) struct HostedCodexApps {
    apps: CodexApps,
    _upstream_server: AbortOnDropHandle<io::Result<()>>,
}

impl std::ops::Deref for HostedCodexApps {
    type Target = CodexApps;

    fn deref(&self) -> &Self::Target {
        &self.apps
    }
}

async fn start_hosted_upstream<S>(server: S) -> HostedUpstream
where
    S: ServerHandler + Clone + Send + Sync + 'static,
{
    let mcp_service = StreamableHttpService::new(
        move || Ok::<_, io::Error>(server.clone()),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default().with_json_response(true),
    );
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind hosted Apps upstream");
    let addr = listener.local_addr().expect("hosted Apps upstream address");
    let router = Router::new().nest_service("/api/codex/ps/mcp", mcp_service);
    let server =
        AbortOnDropHandle::new(tokio::spawn(
            async move { axum::serve(listener, router).await },
        ));
    HostedUpstream {
        config: CodexAppsConnectConfig::new(
            format!("http://{addr}"),
            /*product_sku*/ None,
            OAuthCredentialsStoreMode::File,
            AuthKeyringBackendKind::Direct,
        ),
        server,
    }
}

async fn connect_hosted_apps<S>(server: S) -> HostedCodexApps
where
    S: ServerHandler + Clone + Send + Sync + 'static,
{
    let upstream = start_hosted_upstream(server).await;
    let apps = CodexApps::connect(&upstream.config, Arc::new(EmptyAuthProvider))
        .await
        .expect("connect hosted Apps test runtime");
    HostedCodexApps {
        apps,
        _upstream_server: upstream.server,
    }
}

async fn connect_hosted_apps_with_elicitation<S>(server: S) -> HostedCodexApps
where
    S: ServerHandler + Clone + Send + Sync + 'static,
{
    let upstream = start_hosted_upstream(server).await;
    let config = upstream.config.with_auth_elicitation(/*enabled*/ true);
    let apps = CodexApps::connect(&config, Arc::new(EmptyAuthProvider))
        .await
        .expect("connect hosted Apps elicitation test runtime");
    HostedCodexApps {
        apps,
        _upstream_server: upstream.server,
    }
}

#[tokio::test]
async fn connect_initializes_the_hosted_http_runtime() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let tools = Arc::from(vec![connector_tool(
        Some("calendar"),
        Some("Calendar"),
        "SearchEvents",
    )]);
    let mcp_service = StreamableHttpService::new(
        {
            let calls = Arc::clone(&calls);
            move || {
                Ok(TestServer {
                    tools: Arc::clone(&tools),
                    calls: Arc::clone(&calls),
                    call_gate: None,
                    resource_gate: None,
                })
            }
        },
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default().with_json_response(true),
    );
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind hosted Apps test server");
    let addr = listener.local_addr().expect("hosted Apps test address");
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().nest_service("/api/codex/ps/mcp", mcp_service),
        )
        .await
    });
    let config = CodexAppsConnectConfig::new(
        format!("http://{addr}"),
        /*product_sku*/ None,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::Direct,
    );

    let apps = CodexApps::connect(&config, Arc::new(EmptyAuthProvider))
        .await
        .expect("connect hosted Apps runtime");

    assert_eq!(
        apps.snapshot()
            .apps()
            .iter()
            .map(CodexApp::id)
            .collect::<Vec<_>>(),
        vec!["calendar"]
    );
    let servers = apps.snapshot().effective_mcp_servers();
    let connector = servers
        .get("codex_apps__calendar")
        .expect("calendar MCP server");
    let McpServerTransportConfig::StreamableHttp { url, .. } = &connector.config().transport else {
        panic!("connector should use HTTP MCP");
    };
    assert_ne!(
        reqwest::Url::parse(url).expect("loopback URL").origin(),
        reqwest::Url::parse(&format!("http://{addr}"))
            .expect("upstream URL")
            .origin()
    );

    let manager = mcp_manager_for_servers(&servers).await;
    assert_eq!(
        manager.server_origin("codex_apps__calendar"),
        Some(format!("http://{addr}").as_str())
    );
    let tool = manager
        .list_all_tools()
        .await
        .into_iter()
        .next()
        .expect("calendar tool");
    assert_eq!(tool.server_origin, Some(format!("http://{addr}")));
    assert_eq!(tool.namespace_title.as_deref(), Some("Calendar"));
    assert_eq!(
        tool.namespace_description.as_deref(),
        Some("Tools for working with Calendar.")
    );
    manager.shutdown().await;
    apps.shutdown().await;
    abort_server(server).await;
}

#[tokio::test]
async fn connector_http_sessions_do_not_block_each_other_upstream() {
    let slow_call = CallGate::default();
    let tools = Arc::from(vec![
        connector_tool(Some("gmail"), Some("Gmail"), "GmailSlow"),
        connector_tool(Some("gmail"), Some("Gmail"), "GmailFast"),
    ]);
    let mcp_service = StreamableHttpService::new(
        {
            let slow_call = slow_call.clone();
            move || {
                Ok(SessionIsolationTestServer {
                    tools: Arc::clone(&tools),
                    slow_call: slow_call.clone(),
                })
            }
        },
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default().with_json_response(true),
    );
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind hosted Apps session isolation server");
    let addr = listener
        .local_addr()
        .expect("hosted Apps session isolation address");
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().nest_service("/api/codex/ps/mcp", mcp_service),
        )
        .await
    });
    let config = CodexAppsConnectConfig::new(
        format!("http://{addr}"),
        /*product_sku*/ None,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::Direct,
    );
    let apps = CodexApps::connect(&config, Arc::new(EmptyAuthProvider))
        .await
        .expect("connect hosted Apps session isolation runtime");
    let snapshot = apps.snapshot();
    let slow_manager = Arc::new(mcp_manager_for_snapshot(&snapshot).await);
    let fast_manager = mcp_manager_for_snapshot(&snapshot).await;

    let slow_started = slow_call.started.notified();
    tokio::pin!(slow_started);
    let slow_task = {
        let manager = Arc::clone(&slow_manager);
        tokio::spawn(async move {
            manager
                .call_tool(
                    "codex_apps__gmail",
                    "slow",
                    /*arguments*/ None,
                    /*meta*/ None,
                )
                .await
        })
    };
    tokio::time::timeout(TEST_TIMEOUT, &mut slow_started)
        .await
        .expect("slow connector call did not reach the upstream");

    let fast_result = tokio::time::timeout(
        TEST_TIMEOUT,
        fast_manager.call_tool(
            "codex_apps__gmail",
            "fast",
            /*arguments*/ None,
            /*meta*/ None,
        ),
    )
    .await
    .expect("an independent HTTP MCP session must not wait for the slow call")
    .expect("fast connector call");
    assert_eq!(fast_result.content[0]["text"], json!("GmailFast"));

    slow_call.release.notify_one();
    let slow_result = tokio::time::timeout(TEST_TIMEOUT, slow_task)
        .await
        .expect("slow connector completion timeout")
        .expect("slow connector task")
        .expect("slow connector call");
    assert_eq!(slow_result.content[0]["text"], json!("GmailSlow"));

    slow_manager.shutdown().await;
    fast_manager.shutdown().await;
    apps.shutdown().await;
    abort_server(server).await;
}

#[derive(Clone)]
struct CachedStartupTestServer {
    state: Arc<CachedStartupTestState>,
}

struct CachedStartupTestState {
    tools: Mutex<Vec<Tool>>,
    initialize_started: tokio::sync::Notify,
    initialize_release: tokio::sync::Semaphore,
    fail_list_tools: AtomicBool,
    list_tools_calls: AtomicUsize,
    calls: Mutex<Vec<String>>,
}

impl CachedStartupTestState {
    fn new(tools: Vec<Tool>) -> Arc<Self> {
        Arc::new(Self {
            tools: Mutex::new(tools),
            initialize_started: tokio::sync::Notify::new(),
            initialize_release: tokio::sync::Semaphore::new(0),
            fail_list_tools: AtomicBool::new(false),
            list_tools_calls: AtomicUsize::new(0),
            calls: Mutex::new(Vec::new()),
        })
    }

    fn set_tools(&self, tools: Vec<Tool>) {
        *self
            .tools
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = tools;
    }
}

impl ServerHandler for CachedStartupTestServer {
    async fn initialize(
        &self,
        request: InitializeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, rmcp::ErrorData> {
        self.state.initialize_started.notify_one();
        self.state
            .initialize_release
            .acquire()
            .await
            .expect("test initialization release semaphore")
            .forget();
        context.peer.set_peer_info(request);
        Ok(self.get_info())
    }

    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, rmcp::ErrorData> {
        self.state.list_tools_calls.fetch_add(1, Ordering::AcqRel);
        if self.state.fail_list_tools.load(Ordering::Acquire) {
            return Err(rmcp::ErrorData::internal_error(
                "injected live tools/list failure",
                None,
            ));
        }
        Ok(ListToolsResult {
            tools: self
                .state
                .tools
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone(),
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        self.state
            .calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(request.name.to_string());
        if request.name == "GmailRequiresAuth" {
            let mut result = CallToolResult::error(vec![Content::text("sign in required")]);
            result.meta = Some(Meta(
                json!({
                    MCP_TOOL_CODEX_APPS_META_KEY: {
                        "connector_auth_failure": {
                            "is_auth_failure": true,
                            "connector_id": "gmail",
                            "auth_reason": "missing_link",
                            "error_code": "AUTH_REQUIRED",
                        }
                    }
                })
                .as_object()
                .expect("auth failure metadata")
                .clone(),
            ));
            return Ok(result);
        }
        Ok(CallToolResult::success(vec![Content::text("forwarded")]))
    }
}

async fn start_cached_startup_test_server(
    state: Arc<CachedStartupTestState>,
) -> (String, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind cached Apps test server");
    let addr = listener.local_addr().expect("cached Apps test address");
    let server = start_cached_startup_test_server_on(listener, state);
    (format!("http://{addr}"), server)
}

fn start_cached_startup_test_server_on(
    listener: TcpListener,
    state: Arc<CachedStartupTestState>,
) -> tokio::task::JoinHandle<()> {
    let mcp_service = StreamableHttpService::new(
        move || {
            Ok(CachedStartupTestServer {
                state: Arc::clone(&state),
            })
        },
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default().with_json_response(true),
    );
    tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            Router::new().nest_service("/api/codex/ps/mcp", mcp_service),
        )
        .await;
    })
}

fn test_cache_context(home: &tempfile::TempDir) -> CodexAppsCacheContext {
    CodexAppsCacheContext::new(
        home.path(),
        CodexAppsCacheIdentity::default()
            .with_account_id(Some("account-123".to_string()))
            .with_chatgpt_user_id(Some("user-123".to_string())),
    )
}

fn cached_connect_config(base_url: String, home: &tempfile::TempDir) -> CodexAppsConnectConfig {
    CodexAppsConnectConfig::new(
        base_url,
        /*product_sku*/ None,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::Direct,
    )
    .with_cache_context(test_cache_context(home))
}

#[tokio::test]
async fn debug_bearer_token_bypasses_and_does_not_rewrite_disk_cache() {
    let home = tempfile::TempDir::new().expect("cache home");
    let cached_tool = connector_tool(Some("calendar"), Some("Calendar"), "CalendarCached");
    let live_tool = connector_tool(Some("calendar"), Some("Calendar"), "CalendarLive");
    let state = CachedStartupTestState::new(vec![live_tool]);
    state.initialize_release.add_permits(1);
    let (base_url, server) = start_cached_startup_test_server(Arc::clone(&state)).await;
    let config = cached_connect_config(base_url, &home);
    let cache_context = config.scoped_cache_context().expect("scoped cache context");
    cache_context
        .write_tools(std::slice::from_ref(&cached_tool))
        .expect("seed Apps cache");

    let apps = CodexApps::connect_inner_with_bearer_token(
        &config,
        Some("debug-token".to_string()),
        Arc::new(EmptyAuthProvider),
        /*file_support*/ None,
        Arc::new(|| {}),
        CodexAppsAccessGuard::default(),
    )
    .await
    .expect("debug-token connection must fetch live inventory");

    assert!(apps.snapshot().is_live_inventory());
    let manager = mcp_manager(&apps).await;
    let tools = manager.list_all_tools().await;
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].tool.name.as_ref(), "live");
    assert_eq!(
        cache_context
            .load_tools()
            .expect("load cache")
            .expect("cache hit"),
        vec![cached_tool],
        "a debug-token connection must neither consume nor rewrite shared cache state"
    );

    manager.shutdown().await;
    apps.shutdown().await;
    abort_server(server).await;
}

#[tokio::test]
async fn warm_cache_returns_immediately_and_live_refresh_publishes_a_new_generation() {
    let home = tempfile::TempDir::new().expect("cache home");
    let mut cached_tool = connector_tool(Some("calendar"), Some("Calendar"), "CalendarCached");
    cached_tool
        .meta
        .as_mut()
        .expect("cached tool metadata")
        .insert(
            MCP_TOOL_CODEX_APPS_META_KEY.to_string(),
            json!({ META_CONNECTED_ACCOUNT_EMAIL: "stale@example.com" }),
        );
    let mut live_tool = connector_tool(Some("calendar"), Some("Calendar"), "CalendarLive");
    live_tool.meta.as_mut().expect("live tool metadata").insert(
        MCP_TOOL_CODEX_APPS_META_KEY.to_string(),
        json!({ META_CONNECTED_ACCOUNT_EMAIL: "live@example.com" }),
    );
    let state = CachedStartupTestState::new(vec![live_tool]);
    let (base_url, server) = start_cached_startup_test_server(Arc::clone(&state)).await;
    let config = cached_connect_config(base_url, &home);
    let cache_context = config.scoped_cache_context().expect("scoped cache context");
    cache_context
        .write_tools(&[cached_tool])
        .expect("seed Apps cache");

    let apps = tokio::time::timeout(
        Duration::from_secs(1),
        CodexApps::connect(&config, Arc::new(EmptyAuthProvider)),
    )
    .await
    .expect("warm cache should not wait for upstream initialize")
    .expect("connect from warm cache");
    assert!(!apps.snapshot().is_live_inventory());
    let manager = Arc::new(mcp_manager(&apps).await);
    let cached_tools = manager.list_all_tools().await;
    assert_eq!(cached_tools[0].tool.name.as_ref(), "cached");
    assert!(
        cached_tools[0]
            .tool
            .meta
            .as_ref()
            .is_none_or(|meta| meta.get(MCP_APPROVAL_CONTEXT_META_KEY).is_none()),
        "cached account identity must not be trusted before live discovery"
    );

    let call = manager.call_tool(
        "codex_apps__calendar",
        "cached",
        /*arguments*/ None,
        /*meta*/ None,
    );
    tokio::pin!(call);
    assert!(
        futures::poll!(call.as_mut()).is_pending(),
        "cached tool calls must wait for their session upstream to initialize"
    );
    assert!(
        state
            .calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty(),
        "a pinned cached call must not reach the upstream before initialization"
    );

    state.initialize_release.add_permits(2);
    let result = tokio::time::timeout(Duration::from_secs(5), call)
        .await
        .expect("gated cached call completion")
        .expect("cached call result");
    assert_eq!(result.content[0]["text"], json!("forwarded"));
    wait_for_background_refresh(&apps).await;
    assert!(apps.snapshot().is_live_inventory());

    let retained_tools = manager.list_all_tools().await;
    assert_eq!(retained_tools.len(), 1);
    assert_eq!(retained_tools[0].tool.name.as_ref(), "cached");
    let refreshed_manager = mcp_manager_for_snapshot(&apps.snapshot()).await;
    let live_tools = refreshed_manager.list_all_tools().await;
    assert_eq!(live_tools.len(), 1);
    assert_eq!(live_tools[0].tool.name.as_ref(), "live");
    assert_eq!(
        live_tools[0]
            .tool
            .meta
            .as_ref()
            .and_then(|meta| meta.get(MCP_APPROVAL_CONTEXT_META_KEY)),
        Some(&json!({
            MCP_APPROVAL_CONTEXT_CONNECTED_ACCOUNT_EMAIL_KEY: "live@example.com",
        }))
    );
    assert_eq!(
        cache_context
            .load_tools()
            .expect("load refreshed cache")
            .expect("refreshed cache hit")[0]
            .name
            .as_ref(),
        "CalendarLive"
    );
    assert!(
        cache_context
            .load_tools()
            .expect("load refreshed cache")
            .expect("refreshed cache hit")[0]
            .meta
            .as_ref()
            .is_none_or(|meta| {
                meta.get(MCP_APPROVAL_CONTEXT_META_KEY).is_none()
                    && meta
                        .get(MCP_TOOL_CODEX_APPS_META_KEY)
                        .and_then(serde_json::Value::as_object)
                        .is_none_or(|source| source.get(META_CONNECTED_ACCOUNT_EMAIL).is_none())
            }),
        "cache must omit volatile private account identity"
    );
    assert_eq!(
        state
            .calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_slice(),
        &["CalendarCached".to_string()]
    );

    refreshed_manager.shutdown().await;
    manager.shutdown().await;
    apps.shutdown().await;
    abort_server(server).await;
}

#[tokio::test]
async fn dropping_warm_cache_owner_aborts_only_its_startup_refresh() {
    let home = tempfile::TempDir::new().expect("cache home");
    let state = CachedStartupTestState::new(vec![connector_tool(
        Some("calendar"),
        Some("Calendar"),
        "CalendarLive",
    )]);
    let (base_url, server) = start_cached_startup_test_server(Arc::clone(&state)).await;
    let config = cached_connect_config(base_url, &home);
    config
        .scoped_cache_context()
        .expect("scoped cache context")
        .write_tools(&[connector_tool(
            Some("calendar"),
            Some("Calendar"),
            "CalendarCached",
        )])
        .expect("seed Apps cache");

    let apps = CodexApps::connect(&config, Arc::new(EmptyAuthProvider))
        .await
        .expect("connect from warm cache");
    let snapshot = apps.snapshot();
    let addr = reqwest::Url::parse(&virtual_server_url(&snapshot, "codex_apps__calendar"))
        .expect("virtual server URL")
        .socket_addrs(|| None)
        .expect("virtual server address")[0];
    let manager = mcp_manager_for_snapshot(&snapshot).await;
    drop(snapshot);

    tokio::time::timeout(TEST_TIMEOUT, state.initialize_started.notified())
        .await
        .expect("background refresh reaches gated initialization");
    let background_refresh = apps
        .background_refresh
        .lock()
        .await
        .as_ref()
        .expect("warm-cache background refresh")
        .abort_handle();
    assert!(!background_refresh.is_finished());

    drop(apps);
    tokio::time::timeout(TEST_TIMEOUT, async {
        while !background_refresh.is_finished() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("dropping Apps aborts its gated background refresh");
    // Let the abandoned HTTP request unwind before the retained manager starts a new upstream
    // initialization. The background task itself has already stopped.
    state.initialize_release.add_permits(1);

    assert!(tokio::net::TcpStream::connect(addr).await.is_ok());
    let cached_tools = manager.list_all_tools().await;
    assert_eq!(cached_tools.len(), 1);
    assert_eq!(cached_tools[0].tool.name.as_ref(), "cached");

    let result = {
        let call = manager.call_tool(
            "codex_apps__calendar",
            "cached",
            /*arguments*/ None,
            /*meta*/ None,
        );
        tokio::pin!(call);
        tokio::time::timeout(TEST_TIMEOUT, async {
            tokio::select! {
                () = state.initialize_started.notified() => {}
                result = &mut call => panic!("cached call completed before initialization was released: {result:?}"),
            }
        })
        .await
        .expect("retained manager retries gated upstream initialization");
        state.initialize_release.add_permits(1);
        tokio::time::timeout(TEST_TIMEOUT, call)
            .await
            .expect("cached call completion")
            .expect("cached call result")
    };
    assert_eq!(result.content[0]["text"], json!("forwarded"));

    manager.shutdown().await;
    drop(manager);
    tokio::time::timeout(TEST_TIMEOUT, async {
        while tokio::net::TcpStream::connect(addr).await.is_ok() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("dropping the retained manager closes the cached generation listener");
    abort_server(server).await;
}

#[tokio::test]
async fn first_ensure_live_joins_the_cached_startup_refresh() {
    let home = tempfile::TempDir::new().expect("cache home");
    let state = CachedStartupTestState::new(vec![connector_tool(
        Some("calendar"),
        Some("Calendar"),
        "CalendarLive",
    )]);
    let (base_url, server) = start_cached_startup_test_server(Arc::clone(&state)).await;
    let config = cached_connect_config(base_url, &home);
    config
        .scoped_cache_context()
        .expect("scoped cache context")
        .write_tools(&[connector_tool(
            Some("calendar"),
            Some("Calendar"),
            "CalendarCached",
        )])
        .expect("seed Apps cache");
    let apps = Arc::new(
        CodexApps::connect(&config, Arc::new(EmptyAuthProvider))
            .await
            .expect("connect from warm cache"),
    );
    assert!(!apps.snapshot().is_live_inventory());
    tokio::time::timeout(TEST_TIMEOUT, state.initialize_started.notified())
        .await
        .expect("background initialization starts");

    let ensure_live = apps.ensure_live();
    tokio::pin!(ensure_live);
    assert!(
        futures::poll!(ensure_live.as_mut()).is_pending(),
        "ensure-live must wait for the in-flight startup refresh"
    );
    state.initialize_release.add_permits(1);
    let snapshot = tokio::time::timeout(TEST_TIMEOUT, ensure_live)
        .await
        .expect("ensure-live completes")
        .expect("live inventory");
    assert!(snapshot.is_live_inventory());
    assert_eq!(
        state.list_tools_calls.load(Ordering::Acquire),
        1,
        "force freshness should join the startup refresh, not issue a duplicate tools/list"
    );

    apps.shutdown().await;
    abort_server(server).await;
}

#[tokio::test]
async fn invalid_cached_generation_falls_back_to_live_inventory() {
    let home = tempfile::TempDir::new().expect("cache home");
    let live_tool = connector_tool(Some("calendar"), Some("Calendar"), "CalendarLive");
    let state = CachedStartupTestState::new(vec![live_tool.clone()]);
    let (base_url, server) = start_cached_startup_test_server(Arc::clone(&state)).await;
    let config = cached_connect_config(base_url, &home);
    let cache_context = config.scoped_cache_context().expect("scoped cache context");
    cache_context
        .write_tools(&[
            connector_tool(Some("same-id"), Some("Calendar"), "CalendarCached"),
            connector_tool(Some("same-id"), Some("Gmail"), "GmailCached"),
        ])
        .expect("seed structurally inconsistent cache");
    state.initialize_release.add_permits(1);

    let apps = CodexApps::connect(&config, Arc::new(EmptyAuthProvider))
        .await
        .expect("invalid cache should fall back to live discovery");
    let snapshot = apps.snapshot();
    assert!(snapshot.is_live_inventory());
    assert_eq!(snapshot.apps().len(), 1);
    assert_eq!(snapshot.apps()[0].id(), "calendar");
    assert_eq!(
        cache_context.load_tools().expect("load repaired cache"),
        Some(vec![live_tool])
    );

    apps.shutdown().await;
    abort_server(server).await;
}

#[tokio::test]
async fn synthetic_cached_generation_stays_consistent_when_live_tools_are_published() {
    let home = tempfile::TempDir::new().expect("cache home");
    let state = CachedStartupTestState::new(vec![connector_tool(
        Some("calendar"),
        Some("Calendar"),
        "CalendarLive",
    )]);
    let (base_url, server) = start_cached_startup_test_server(Arc::clone(&state)).await;
    let config = cached_connect_config(base_url, &home);
    config
        .scoped_cache_context()
        .as_ref()
        .expect("scoped cache context")
        .write_tools(&[synthetic_connector_tool(
            "calendar",
            "Calendar",
            "CalendarLink",
        )])
        .expect("seed synthetic Apps cache");
    let apps = CodexApps::connect(&config, Arc::new(EmptyAuthProvider))
        .await
        .expect("connect from synthetic cache");
    let cached_snapshot = apps.snapshot();
    assert!(cached_snapshot.apps().is_empty());
    assert!(
        cached_snapshot
            .effective_mcp_servers()
            .contains_key("codex_apps__calendar")
    );
    let manager = mcp_manager_for_snapshot(&cached_snapshot).await;

    state.initialize_release.add_permits(1);
    wait_for_background_refresh(&apps).await;
    let cached_tools = manager.list_all_tools().await;
    assert_eq!(cached_tools.len(), 1);
    assert_eq!(cached_tools[0].tool.name.as_ref(), "link");
    assert!(cached_snapshot.apps().is_empty());

    let live_snapshot = apps.snapshot();
    let live_manager = mcp_manager_for_snapshot(&live_snapshot).await;
    let live_tools = live_manager.list_all_tools().await;
    assert_eq!(live_tools.len(), 1);
    assert_eq!(live_tools[0].tool.name.as_ref(), "live");
    assert_eq!(live_snapshot.apps()[0].id(), "calendar");

    live_manager.shutdown().await;
    manager.shutdown().await;
    apps.shutdown().await;
    abort_server(server).await;
}

#[tokio::test]
async fn failed_live_fetch_keeps_the_cached_inventory_callable() {
    let home = tempfile::TempDir::new().expect("cache home");
    let cached_tool = connector_tool(Some("calendar"), Some("Calendar"), "CalendarCached");
    let state = CachedStartupTestState::new(vec![connector_tool(
        Some("calendar"),
        Some("Calendar"),
        "CalendarLive",
    )]);
    state.fail_list_tools.store(true, Ordering::Release);
    let (base_url, server) = start_cached_startup_test_server(Arc::clone(&state)).await;
    let config = cached_connect_config(base_url, &home);
    let cache_context = config.scoped_cache_context().expect("scoped cache context");
    cache_context
        .write_tools(std::slice::from_ref(&cached_tool))
        .expect("seed Apps cache");
    let apps = CodexApps::connect(&config, Arc::new(EmptyAuthProvider))
        .await
        .expect("connect from warm cache");
    assert!(!apps.snapshot().is_live_inventory());

    state.initialize_release.add_permits(1);
    wait_for_background_refresh(&apps).await;
    assert!(!apps.snapshot().is_live_inventory());
    let manager = mcp_manager(&apps).await;
    assert_eq!(
        manager.list_all_tools().await[0].tool.name.as_ref(),
        "cached"
    );
    state.initialize_release.add_permits(1);
    manager
        .call_tool(
            "codex_apps__calendar",
            "cached",
            /*arguments*/ None,
            /*meta*/ None,
        )
        .await
        .expect("last-good cached tool should remain callable");
    assert_eq!(
        cache_context.load_tools().expect("load cache"),
        Some(vec![cached_tool])
    );

    manager.shutdown().await;
    apps.shutdown().await;
    abort_server(server).await;
}

#[tokio::test]
async fn transient_warm_cache_connection_failure_can_be_retried() {
    let home = tempfile::TempDir::new().expect("cache home");
    let reserved_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("reserve retry test address");
    let addr = reserved_listener
        .local_addr()
        .expect("retry test listener address");
    drop(reserved_listener);

    let config = cached_connect_config(format!("http://{addr}"), &home);
    let cache_context = config.scoped_cache_context().expect("scoped cache context");
    cache_context
        .write_tools(&[connector_tool(
            Some("calendar"),
            Some("Calendar"),
            "CalendarCached",
        )])
        .expect("seed Apps cache");
    let apps = CodexApps::connect(&config, Arc::new(EmptyAuthProvider))
        .await
        .expect("warm cache starts without upstream");

    wait_for_background_refresh(&apps).await;
    assert_eq!(apps.snapshot().apps()[0].name(), "Calendar");

    let state = CachedStartupTestState::new(vec![connector_tool(
        Some("calendar"),
        Some("Calendar"),
        "CalendarLive",
    )]);
    let listener = TcpListener::bind(addr)
        .await
        .expect("rebind retry test address");
    let server = start_cached_startup_test_server_on(listener, Arc::clone(&state));
    state.initialize_release.add_permits(1);

    let refreshed = apps
        .refresh()
        .await
        .expect("retry connects and refreshes inventory");
    let manager = mcp_manager_for_snapshot(&refreshed).await;
    assert_eq!(manager.list_all_tools().await[0].tool.name.as_ref(), "live");
    assert_eq!(
        cache_context
            .load_tools()
            .expect("load refreshed cache")
            .expect("refreshed cache hit")[0]
            .name
            .as_ref(),
        "CalendarLive"
    );

    manager.shutdown().await;
    apps.shutdown().await;
    abort_server(server).await;
}

#[tokio::test]
async fn accepted_auth_elicitation_atomically_publishes_exact_new_registrations() {
    let home = tempfile::TempDir::new().expect("cache home");
    let state = CachedStartupTestState::new(vec![connector_tool(
        Some("gmail"),
        Some("Gmail"),
        "GmailRequiresAuth",
    )]);
    let (base_url, server) = start_cached_startup_test_server(Arc::clone(&state)).await;
    let config = cached_connect_config(base_url, &home);
    let cache_context = config.scoped_cache_context().expect("scoped cache context");
    let revision = Arc::new(AtomicUsize::new(0));
    let on_change: Arc<dyn Fn() + Send + Sync> = {
        let revision = Arc::clone(&revision);
        Arc::new(move || {
            revision.fetch_add(1, Ordering::AcqRel);
        })
    };
    state.initialize_release.add_permits(1);
    let apps = CodexApps::connect_with_environment(
        &config,
        Arc::new(EmptyAuthProvider),
        Arc::new(EnvironmentManager::without_environments()),
        on_change,
        CodexAppsAccessGuard::default(),
    )
    .await
    .expect("connect Apps with empty cache");
    let original_snapshot = apps.snapshot();
    let original_servers = original_snapshot.effective_mcp_servers();
    let (manager, events) = mcp_manager_for_servers_with_events(&original_servers).await;
    let manager = Arc::new(manager);
    assert_eq!(
        manager.list_all_tools().await[0].tool.name.as_ref(),
        "requiresauth"
    );

    let connector_id = "connector_76869538009648d5b282a4bb21c3d157";
    let mut unlocked = connector_tool(Some(connector_id), Some("GitHub"), "GitHubAddComment");
    unlocked.title = Some("GitHub_add_comment_to_issue".to_string());
    state.set_tools(vec![
        unlocked,
        connector_tool(Some("calendar"), Some("Calendar"), "CalendarList"),
    ]);
    state.initialize_release.add_permits(1);
    let call_manager = Arc::clone(&manager);
    let call = tokio::spawn(async move {
        call_manager
            .call_tool(
                "codex_apps__gmail",
                "requiresauth",
                /*arguments*/ None,
                /*meta*/ None,
            )
            .await
    });
    let request = recv_elicitation_request(&events, Duration::from_secs(5))
        .await
        .expect("Apps auth elicitation");
    assert_eq!(request.server_name, "codex_apps__gmail");
    manager
        .resolve_elicitation(
            request.server_name,
            rmcp_request_id(request.id),
            ElicitationResponse {
                action: ElicitationAction::Accept,
                content: Some(json!({})),
                meta: None,
            },
        )
        .await
        .expect("accept Apps auth elicitation");
    let result = tokio::time::timeout(Duration::from_secs(5), call)
        .await
        .expect("auth refresh completion")
        .expect("auth call join")
        .expect("auth call result");
    assert_eq!(
        result.content[0]["text"],
        json!("Authentication for Gmail was requested and accepted. Retry this tool call now.")
    );

    assert_eq!(revision.load(Ordering::Acquire), 1);
    let old_tools = manager.list_all_tools().await;
    assert_eq!(old_tools.len(), 1);
    assert_eq!(old_tools[0].tool.name.as_ref(), "requiresauth");
    assert_eq!(
        original_snapshot
            .apps()
            .iter()
            .map(CodexApp::id)
            .collect::<Vec<_>>(),
        vec!["gmail"]
    );

    let published = apps.snapshot();
    assert_eq!(
        published
            .apps()
            .iter()
            .map(CodexApp::id)
            .collect::<Vec<_>>(),
        vec!["calendar", connector_id]
    );
    let published_servers = published.effective_mcp_servers();
    assert!(!published_servers.contains_key("codex_apps__gmail"));
    assert!(published_servers.contains_key("codex_apps__calendar"));
    let published_manager = mcp_manager_for_servers(&published_servers).await;
    let metadata = published_manager
        .tool_runtime_metadata("codex_apps__github", "addcomment")
        .expect("new tool runtime metadata");
    assert!(metadata.approval_persistence().is_none());
    let presentation = metadata
        .approval_presentation()
        .expect("new tool approval presentation");
    assert_eq!(
        presentation.question(),
        "Allow GitHub to add a comment to a pull request?"
    );
    assert_eq!(
        presentation
            .parameter_labels()
            .iter()
            .map(|parameter| (parameter.name(), parameter.label()))
            .collect::<Vec<_>>(),
        vec![
            ("pr_number", "Pull request"),
            ("repo_full_name", "Repository"),
            ("comment", "Comment"),
        ]
    );
    assert_eq!(
        published_manager
            .list_all_tools()
            .await
            .into_iter()
            .map(|tool| tool.canonical_tool_name())
            .collect::<HashSet<_>>(),
        HashSet::from([
            ToolName::namespaced("mcp__codex_apps__calendar", "list"),
            ToolName::namespaced("mcp__codex_apps__github", "addcomment"),
        ])
    );
    assert_eq!(
        cache_context
            .load_tools()
            .expect("load auth-refreshed cache")
            .expect("auth-refreshed cache hit")
            .into_iter()
            .map(|tool| tool.name.to_string())
            .collect::<Vec<_>>(),
        vec!["GitHubAddComment", "CalendarList"]
    );

    published_manager.shutdown().await;
    manager.shutdown().await;
    apps.shutdown().await;
    abort_server(server).await;
}

#[tokio::test]
async fn cache_miss_blocks_until_upstream_inventory_is_ready() {
    let state = CachedStartupTestState::new(vec![connector_tool(
        Some("calendar"),
        Some("Calendar"),
        "CalendarLive",
    )]);
    let (base_url, server) = start_cached_startup_test_server(Arc::clone(&state)).await;
    let config = CodexAppsConnectConfig::new(
        base_url,
        /*product_sku*/ None,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::Direct,
    );
    let connect =
        tokio::spawn(async move { CodexApps::connect(&config, Arc::new(EmptyAuthProvider)).await });
    tokio::time::timeout(TEST_TIMEOUT, state.initialize_started.notified())
        .await
        .expect("upstream initialization did not start");
    assert!(
        !connect.is_finished(),
        "a cache miss must preserve blocking startup"
    );
    state.initialize_release.add_permits(1);
    let apps = tokio::time::timeout(TEST_TIMEOUT, connect)
        .await
        .expect("cache-miss connect timed out")
        .expect("cache-miss connect join")
        .expect("cache-miss connect");
    assert!(apps.snapshot().is_live_inventory());
    assert_eq!(apps.snapshot().apps()[0].id(), "calendar");

    apps.shutdown().await;
    abort_server(server).await;
}

#[tokio::test]
async fn resource_proxy_uses_the_shared_upstream_and_exposes_no_tools() {
    let (apps, _) = apps_with_tools(vec![connector_tool(
        Some("calendar"),
        Some("Calendar"),
        "CalendarList",
    )])
    .await;
    let snapshot = apps.snapshot();
    let resource_server = snapshot.resource_mcp_server();
    let config = resource_server.config();
    assert_eq!(config.enabled_tools, Some(Vec::new()));
    let McpServerTransportConfig::StreamableHttp { url, .. } = &config.transport else {
        panic!("resource proxy should use ordinary HTTP MCP");
    };
    assert!(url.starts_with("http://127.0.0.1:"));
    assert!(url.ends_with("/mcp/codex_apps"));
    let connector_url = virtual_server_url(&snapshot, "codex_apps__calendar");
    assert_eq!(
        reqwest::Url::parse(url)
            .expect("resource proxy URL")
            .origin(),
        reqwest::Url::parse(&connector_url)
            .expect("connector proxy URL")
            .origin(),
        "resources and connector tools should share one loopback host"
    );

    let manager = mcp_manager_for_servers(&HashMap::from([(
        CODEX_APPS_RESOURCE_MCP_SERVER_NAME.to_string(),
        resource_server,
    )]))
    .await;
    assert!(manager.list_all_tools().await.is_empty());
    let resources = manager
        .list_resources(CODEX_APPS_RESOURCE_MCP_SERVER_NAME, /*params*/ None)
        .await
        .expect("list proxied resources");
    assert_eq!(resources.resources.len(), 1);
    assert_eq!(resources.resources[0].uri, "test://apps/shared");
    let templates = manager
        .list_resource_templates(CODEX_APPS_RESOURCE_MCP_SERVER_NAME, /*params*/ None)
        .await
        .expect("list proxied resource templates");
    assert_eq!(templates.resource_templates.len(), 1);
    assert_eq!(
        templates.resource_templates[0].uri_template,
        "test://apps/{slug}"
    );
    let read = manager
        .read_resource(
            CODEX_APPS_RESOURCE_MCP_SERVER_NAME,
            ReadResourceRequestParams::new("test://apps/shared"),
        )
        .await
        .expect("read proxied resource");
    assert_eq!(
        read.contents,
        vec![ResourceContents::text(
            "shared upstream resource",
            "test://apps/shared",
        )]
    );
    let unlisted = manager
        .read_resource(
            CODEX_APPS_RESOURCE_MCP_SERVER_NAME,
            ReadResourceRequestParams::new("test://apps/unlisted"),
        )
        .await
        .expect("dedicated resource proxy forwards arbitrary upstream URIs");
    assert_eq!(
        unlisted.contents,
        vec![ResourceContents::text(
            "shared upstream resource",
            "test://apps/unlisted",
        )]
    );

    manager.shutdown().await;
    apps.shutdown().await;
}

#[tokio::test]
async fn connector_http_server_only_reads_resources_declared_by_its_tools() {
    let mut gmail_tool = connector_tool(Some("gmail"), Some("Gmail"), "GmailSearch");
    gmail_tool
        .meta
        .as_mut()
        .expect("connector metadata")
        .insert(
            "ui".to_string(),
            json!({"resourceUri": "ui://gmail/search.html"}),
        );
    let mut calendar_tool = connector_tool(Some("calendar"), Some("Calendar"), "CalendarList");
    calendar_tool
        .meta
        .as_mut()
        .expect("connector metadata")
        .insert(
            "ui".to_string(),
            json!({"resourceUri": "ui://calendar/list.html"}),
        );
    let (apps, _) = apps_with_tools(vec![gmail_tool, calendar_tool]).await;
    let manager = mcp_manager(&apps).await;
    let server_name = "codex_apps__gmail";
    let resource_uri = "ui://gmail/search.html";

    assert!(
        manager
            .list_resources(server_name, /*params*/ None)
            .await
            .expect("list connector-local resources")
            .resources
            .is_empty()
    );
    assert!(
        manager
            .list_resource_templates(server_name, /*params*/ None)
            .await
            .expect("list connector-local resource templates")
            .resource_templates
            .is_empty()
    );
    assert_eq!(
        manager
            .read_resource(server_name, ReadResourceRequestParams::new(resource_uri),)
            .await
            .expect("read connector tool UI resource"),
        ReadResourceResult::new(vec![ResourceContents::text(
            "shared upstream resource",
            resource_uri,
        )])
    );
    for forbidden_uri in ["ui://calendar/list.html", "test://apps/shared"] {
        let error = manager
            .read_resource(server_name, ReadResourceRequestParams::new(forbidden_uri))
            .await
            .expect_err("connector route must reject undeclared resources");
        let protocol_error = codex_rmcp_client::mcp_error_data(&error)
            .expect("connector rejection should remain an MCP protocol error");
        assert_eq!(
            protocol_error.code,
            rmcp::model::ErrorCode::RESOURCE_NOT_FOUND
        );
        assert!(
            protocol_error
                .message
                .contains("is not declared by this MCP server"),
            "unexpected error for {forbidden_uri}: {protocol_error:?}"
        );
    }

    manager.shutdown().await;
    apps.shutdown().await;
}

#[tokio::test]
async fn resource_proxy_does_not_apply_the_inventory_startup_timeout() {
    let resource_gate = CallGate::default();
    let apps = connect_hosted_apps(TestServer {
        tools: Arc::from(Vec::new()),
        calls: Arc::new(Mutex::new(Vec::new())),
        call_gate: None,
        resource_gate: Some(resource_gate.clone()),
    })
    .await;
    let manager = Arc::new(
        mcp_manager_for_servers(&HashMap::from([(
            CODEX_APPS_RESOURCE_MCP_SERVER_NAME.to_string(),
            apps.snapshot().resource_mcp_server(),
        )]))
        .await,
    );
    assert!(manager.list_all_tools().await.is_empty());

    tokio::time::pause();
    let call_manager = Arc::clone(&manager);
    let call = tokio::spawn(async move {
        call_manager
            .list_resources(CODEX_APPS_RESOURCE_MCP_SERVER_NAME, /*params*/ None)
            .await
    });
    resource_gate.started.notified().await;
    tokio::time::advance(CODEX_APPS_LOAD_TIMEOUT + Duration::from_secs(1)).await;
    tokio::task::yield_now().await;
    assert!(
        !call.is_finished(),
        "resource requests must not inherit the 30-second inventory timeout"
    );
    resource_gate.release.notify_one();
    let resources = call
        .await
        .expect("resource task")
        .expect("resource request should still complete");
    tokio::time::resume();

    assert_eq!(resources.resources[0].uri, "test://apps/shared");
    manager.shutdown().await;
    apps.shutdown().await;
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RecordedCall {
    name: String,
    arguments: Option<serde_json::Value>,
    meta: serde_json::Value,
}

#[derive(Clone)]
struct TestServer {
    tools: Arc<[Tool]>,
    calls: Arc<Mutex<Vec<RecordedCall>>>,
    call_gate: Option<CallGate>,
    resource_gate: Option<CallGate>,
}

#[derive(Clone, Default)]
struct CallGate {
    started: Arc<tokio::sync::Notify>,
    release: Arc<tokio::sync::Notify>,
}

#[derive(Clone)]
struct SessionIsolationTestServer {
    tools: Arc<[Tool]>,
    slow_call: CallGate,
}

impl ServerHandler for SessionIsolationTestServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, rmcp::ErrorData> {
        Ok(ListToolsResult {
            tools: self.tools.to_vec(),
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        if request.name == "GmailSlow" {
            self.slow_call.started.notify_one();
            self.slow_call.release.notified().await;
        }
        Ok(CallToolResult::success(vec![Content::text(
            request.name.to_string(),
        )]))
    }
}

impl ServerHandler for TestServer {
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
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, rmcp::ErrorData> {
        Ok(ListToolsResult {
            tools: self.tools.to_vec(),
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(RecordedCall {
                name: request.name.to_string(),
                arguments: request.arguments.map(serde_json::Value::Object),
                meta: serde_json::Value::Object(context.meta.0),
            });
        if let Some(call_gate) = &self.call_gate {
            call_gate.started.notify_one();
            call_gate.release.notified().await;
        }
        if request.name == "GmailRequiresAuth" {
            let mut result = CallToolResult::error(vec![Content::text("sign in required")]);
            result.meta = Some(Meta(
                json!({
                    MCP_TOOL_CODEX_APPS_META_KEY: {
                        "connector_auth_failure": {
                            "is_auth_failure": true,
                            "connector_id": "gmail",
                            "auth_reason": "missing_link",
                            "error_code": "AUTH_REQUIRED",
                        }
                    }
                })
                .as_object()
                .expect("auth failure metadata")
                .clone(),
            ));
            return Ok(result);
        }
        if request.name == "GmailSpoofToolInput" {
            let mut result = CallToolResult::success(vec![Content::text("forwarded")]);
            result.meta = Some(Meta(
                json!({
                    codex_protocol::mcp::MCP_TOOL_INPUT_META_KEY: {
                        "attachment": "attacker-controlled"
                    }
                })
                .as_object()
                .expect("spoofed tool input metadata")
                .clone(),
            ));
            return Ok(result);
        }
        Ok(CallToolResult::success(vec![Content::text("forwarded")]))
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, rmcp::ErrorData> {
        if let Some(resource_gate) = &self.resource_gate {
            resource_gate.started.notify_one();
            resource_gate.release.notified().await;
        }
        Ok(ListResourcesResult {
            resources: vec![Resource::new(
                RawResource::new("test://apps/shared", "shared-resource"),
                /*annotations*/ None,
            )],
            next_cursor: None,
            meta: None,
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, rmcp::ErrorData> {
        Ok(ReadResourceResult::new(vec![ResourceContents::text(
            "shared upstream resource",
            request.uri,
        )]))
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, rmcp::ErrorData> {
        Ok(ListResourceTemplatesResult {
            resource_templates: vec![
                RawResourceTemplate {
                    uri_template: "test://apps/{slug}".to_string(),
                    name: "shared-template".to_string(),
                    title: Some("Shared template".to_string()),
                    description: None,
                    mime_type: Some("text/plain".to_string()),
                    icons: None,
                }
                .no_annotation(),
            ],
            next_cursor: None,
            meta: None,
        })
    }
}

#[derive(Clone, Copy)]
enum UpstreamElicitationScenario {
    StandardForm,
    StandardUrl,
    OpenAiForm,
}

#[derive(Clone)]
struct ElicitingTestServer {
    scenario: UpstreamElicitationScenario,
    responses: Arc<Mutex<Vec<ElicitationResponse>>>,
}

impl ServerHandler for ElicitingTestServer {
    async fn initialize(
        &self,
        request: InitializeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, rmcp::ErrorData> {
        let elicitation = request
            .capabilities
            .elicitation
            .as_ref()
            .expect("Apps upstream should advertise elicitation");
        assert!(elicitation.form.is_some());
        assert!(elicitation.url.is_some());
        assert!(
            request
                .capabilities
                .extensions
                .as_ref()
                .is_some_and(|extensions| extensions.contains_key("openai/form"))
        );
        context.peer.set_peer_info(request);
        Ok(self.get_info())
    }

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
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, rmcp::ErrorData> {
        Ok(ListToolsResult {
            tools: vec![
                connector_tool(Some("calendar"), Some("Calendar"), "CalendarConfirm"),
                connector_tool(Some("gmail"), Some("Gmail"), "GmailConfirm"),
            ],
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        _request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let request = match self.scenario {
            UpstreamElicitationScenario::StandardForm => {
                let requested_schema = serde_json::from_value::<ElicitationSchema>(json!({
                    "type": "object",
                    "properties": { "confirmed": { "type": "boolean" } },
                    "required": ["confirmed"]
                }))
                .map_err(|error| rmcp::ErrorData::internal_error(error.to_string(), None))?;
                ServerRequest::CreateElicitationRequest(CreateElicitationRequest::new(
                    CreateElicitationRequestParams::FormElicitationParams {
                        meta: Some(Meta(
                            json!({ "requestKind": "standard" })
                                .as_object()
                                .expect("metadata object")
                                .clone(),
                        )),
                        message: "Confirm the calendar action".to_string(),
                        requested_schema,
                    },
                ))
            }
            UpstreamElicitationScenario::StandardUrl => {
                ServerRequest::CreateElicitationRequest(CreateElicitationRequest::new(
                    CreateElicitationRequestParams::UrlElicitationParams {
                        meta: Some(Meta(
                            json!({ "requestKind": "url" })
                                .as_object()
                                .expect("metadata object")
                                .clone(),
                        )),
                        message: "Connect the calendar".to_string(),
                        url: "https://example.com/connect/calendar".to_string(),
                        elicitation_id: "calendar-connect".to_string(),
                    },
                ))
            }
            UpstreamElicitationScenario::OpenAiForm => {
                let mut request = CustomRequest::new(
                    "openai/form",
                    Some(json!({
                        "message": "Select a calendar",
                        "requestedSchema": {
                            "type": "object",
                            "properties": { "calendar": { "type": "string" } },
                            "required": ["calendar"]
                        }
                    })),
                );
                request
                    .get_meta_mut()
                    .insert("requestKind".to_string(), json!("openai"));
                ServerRequest::CustomRequest(request)
            }
        };
        let response = context
            .peer
            .send_request(request)
            .await
            .map_err(|error| rmcp::ErrorData::internal_error(error.to_string(), None))?;
        let response = test_elicitation_response(response)?;
        self.responses
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(response.clone());
        Ok(CallToolResult::success(vec![Content::text(
            match response.action {
                ElicitationAction::Accept => "accepted",
                ElicitationAction::Decline => "declined",
                ElicitationAction::Cancel => "cancelled",
            },
        )]))
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, rmcp::ErrorData> {
        let requested_schema = serde_json::from_value::<ElicitationSchema>(json!({
            "type": "object",
            "properties": { "confirmed": { "type": "boolean" } },
            "required": ["confirmed"]
        }))
        .map_err(|error| rmcp::ErrorData::internal_error(error.to_string(), None))?;
        let response = context
            .peer
            .send_request(ServerRequest::CreateElicitationRequest(
                CreateElicitationRequest::new(
                    CreateElicitationRequestParams::FormElicitationParams {
                        meta: Some(Meta(
                            json!({ "requestKind": "resource" })
                                .as_object()
                                .expect("metadata object")
                                .clone(),
                        )),
                        message: "Allow shared Apps resources".to_string(),
                        requested_schema,
                    },
                ),
            ))
            .await
            .map_err(|error| rmcp::ErrorData::internal_error(error.to_string(), None))?;
        self.responses
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(test_elicitation_response(response)?);
        Ok(ListResourcesResult {
            resources: vec![Resource::new(
                RawResource::new("test://apps/elicited", "elicited-resource"),
                /*annotations*/ None,
            )],
            next_cursor: None,
            meta: None,
        })
    }
}

fn test_elicitation_response(result: ClientResult) -> Result<ElicitationResponse, rmcp::ErrorData> {
    match result {
        ClientResult::CreateElicitationResult(result) => Ok(ElicitationResponse {
            action: result.action,
            content: result.content,
            meta: result.meta.map(|meta| serde_json::Value::Object(meta.0)),
        }),
        ClientResult::CustomResult(result) => serde_json::from_value(result.0)
            .map_err(|error| rmcp::ErrorData::internal_error(error.to_string(), None)),
        result => Err(rmcp::ErrorData::internal_error(
            format!("unexpected elicitation response: {result:?}"),
            None,
        )),
    }
}

#[derive(Clone)]
struct RefreshableTestServer {
    state: Arc<RefreshableTestState>,
}

#[derive(Default)]
struct RefreshableTestState {
    pages: Mutex<Vec<Vec<Tool>>>,
    page_delay: Mutex<Duration>,
    next_list_tools_gate: Mutex<Option<CallGate>>,
    fail_list_tools: AtomicBool,
    requested_cursors: Mutex<Vec<Option<String>>>,
}

impl RefreshableTestState {
    fn set_pages(&self, pages: Vec<Vec<Tool>>) {
        *self
            .pages
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = pages;
    }

    fn set_list_failure(&self, fail: bool) {
        self.fail_list_tools.store(fail, Ordering::Release);
    }

    fn set_page_delay(&self, delay: Duration) {
        *self
            .page_delay
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = delay;
    }

    fn gate_next_list_tools(&self, gate: CallGate) {
        *self
            .next_list_tools_gate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(gate);
    }

    fn requested_cursors(&self) -> Vec<Option<String>> {
        self.requested_cursors
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl ServerHandler for RefreshableTestServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
    }

    async fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, rmcp::ErrorData> {
        if self.state.fail_list_tools.load(Ordering::Acquire) {
            return Err(rmcp::ErrorData::internal_error(
                "injected tools/list failure",
                None,
            ));
        }
        let page_delay = *self
            .state
            .page_delay
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        tokio::time::sleep(page_delay).await;
        let cursor = request.and_then(|request| request.cursor);
        self.state
            .requested_cursors
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(cursor.clone());
        let page_index = match cursor.as_deref() {
            None => 0,
            Some(cursor) => cursor
                .strip_prefix("page-")
                .and_then(|index| index.parse::<usize>().ok())
                .ok_or_else(|| rmcp::ErrorData::invalid_params("invalid test cursor", None))?,
        };
        let (tools, next_cursor) = {
            let pages = self
                .state
                .pages
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            (
                pages.get(page_index).cloned().unwrap_or_default(),
                (page_index + 1 < pages.len()).then(|| format!("page-{}", page_index + 1)),
            )
        };
        let gate = self
            .state
            .next_list_tools_gate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(gate) = gate {
            gate.started.notify_one();
            gate.release.notified().await;
        }
        Ok(ListToolsResult {
            tools,
            next_cursor,
            meta: None,
        })
    }
}

pub(crate) fn connector_tool(
    connector_id: Option<&str>,
    connector_name: Option<&str>,
    name: &str,
) -> Tool {
    let mut tool = Tool::new(name.to_string(), "test tool", Arc::new(JsonObject::new()));
    let mut meta = JsonObject::new();
    if let Some(connector_id) = connector_id {
        meta.insert("connector_id".to_string(), json!(connector_id));
    }
    if let Some(connector_name) = connector_name {
        meta.insert("connector_name".to_string(), json!(connector_name));
        tool.title = Some(format!("{connector_name}_Title"));
    }
    tool.meta = Some(Meta(meta));
    tool
}

fn synthetic_connector_tool(connector_id: &str, connector_name: &str, name: &str) -> Tool {
    let mut tool = connector_tool(Some(connector_id), Some(connector_name), name);
    tool.meta
        .as_mut()
        .expect("connector metadata")
        .insert("_codex_apps".to_string(), json!({"synthetic_link": true}));
    tool
}

async fn initialized_http_client(config: &CodexAppsConnectConfig) -> Arc<RmcpClient> {
    let client = Arc::new(
        RmcpClient::new_streamable_http_client(
            "codex-apps-test",
            &config.upstream_url(),
            /*bearer_token*/ None,
            /*http_headers*/ None,
            /*env_http_headers*/ None,
            config.oauth_credentials_store_mode,
            config.auth_keyring_backend_kind,
            Arc::new(ReqwestHttpClient) as Arc<dyn HttpClient>,
            /*auth_provider*/ None,
        )
        .await
        .expect("HTTP client"),
    );
    client
        .initialize(
            InitializeRequestParams::new(
                ClientCapabilities::default(),
                Implementation::new("codex-apps-test", "1"),
            )
            .with_protocol_version(ProtocolVersion::V_2025_06_18),
            /*timeout*/ None,
            Box::new(|_, _| {
                async {
                    Ok(codex_rmcp_client::ElicitationResponse {
                        action: ElicitationAction::Cancel,
                        content: None,
                        meta: None,
                    })
                }
                .boxed()
            }),
        )
        .await
        .expect("initialize client");
    client
}

pub(crate) async fn apps_with_tools(
    tools: Vec<Tool>,
) -> (HostedCodexApps, Arc<Mutex<Vec<RecordedCall>>>) {
    apps_with_tools_and_gate(tools, /*call_gate*/ None).await
}

async fn apps_with_tools_and_gate(
    tools: Vec<Tool>,
    call_gate: Option<CallGate>,
) -> (HostedCodexApps, Arc<Mutex<Vec<RecordedCall>>>) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let apps = connect_hosted_apps(TestServer {
        tools: Arc::from(tools),
        calls: Arc::clone(&calls),
        call_gate,
        resource_gate: None,
    })
    .await;
    (apps, calls)
}

async fn apps_with_refreshable_pages(
    pages: Vec<Vec<Tool>>,
) -> (HostedCodexApps, Arc<RefreshableTestState>) {
    let state = Arc::new(RefreshableTestState::default());
    state.set_pages(pages);
    let apps = connect_hosted_apps(RefreshableTestServer {
        state: Arc::clone(&state),
    })
    .await;
    (apps, state)
}

async fn list_refreshable_pages(
    pages: Vec<Vec<Tool>>,
) -> (Result<Vec<Tool>>, Arc<RefreshableTestState>) {
    list_refreshable_pages_with_timeout(
        pages,
        /*page_delay*/ Duration::ZERO,
        CODEX_APPS_LOAD_TIMEOUT,
    )
    .await
}

async fn list_refreshable_pages_with_timeout(
    pages: Vec<Vec<Tool>>,
    page_delay: Duration,
    load_timeout: Duration,
) -> (Result<Vec<Tool>>, Arc<RefreshableTestState>) {
    let state = Arc::new(RefreshableTestState::default());
    state.set_pages(pages);
    state.set_page_delay(page_delay);
    let hosted_upstream = start_hosted_upstream(RefreshableTestServer {
        state: Arc::clone(&state),
    })
    .await;
    let upstream = initialized_http_client(&hosted_upstream.config).await;
    let result = list_all_upstream_tools_with_timeout(&upstream, load_timeout).await;
    upstream.shutdown().await;
    (result, state)
}

async fn list_refreshable_pages_with_inventory_limit(
    pages: Vec<Vec<Tool>>,
    max_inventory_bytes: usize,
) -> Result<Vec<Tool>> {
    let state = Arc::new(RefreshableTestState::default());
    state.set_pages(pages);
    let hosted_upstream = start_hosted_upstream(RefreshableTestServer { state }).await;
    let upstream = initialized_http_client(&hosted_upstream.config).await;
    let list_client = Arc::clone(&upstream);
    let result = list_all_upstream_tools_with_lister_and_inventory_limit(
        CODEX_APPS_LOAD_TIMEOUT,
        max_inventory_bytes,
        move |params, remaining| {
            let list_client = Arc::clone(&list_client);
            Box::pin(async move { list_client.list_tools(params, Some(remaining)).await })
        },
    )
    .await;
    upstream.shutdown().await;
    result
}

#[tokio::test]
async fn shutdown_closes_endpoints_even_with_an_active_tool_call() {
    let call_gate = CallGate::default();
    let (apps, _) = apps_with_tools_and_gate(
        vec![connector_tool(
            Some("gmail"),
            Some("Gmail"),
            "GmailSearchMessages",
        )],
        Some(call_gate.clone()),
    )
    .await;
    let addr = reqwest::Url::parse(&virtual_server_url(&apps.snapshot(), "codex_apps__gmail"))
        .expect("virtual server URL")
        .socket_addrs(|| None)
        .expect("virtual server address")[0];
    let manager = Arc::new(mcp_manager(&apps).await);
    let call_task = {
        let manager = Arc::clone(&manager);
        tokio::spawn(async move {
            manager
                .call_tool(
                    "codex_apps__gmail",
                    "searchmessages",
                    /*arguments*/ None,
                    /*meta*/ None,
                )
                .await
        })
    };
    tokio::time::timeout(TEST_TIMEOUT, call_gate.started.notified())
        .await
        .expect("upstream tool call did not start");

    tokio::time::timeout(Duration::from_secs(1), apps.shutdown())
        .await
        .expect("shutdown should stop the active HTTP request");
    assert!(tokio::net::TcpStream::connect(addr).await.is_err());
    call_gate.release.notify_waiters();
    abort_server(call_task).await;
    manager.shutdown().await;
}

async fn mcp_manager(apps: &CodexApps) -> McpConnectionManager {
    mcp_manager_for_snapshot(&apps.snapshot()).await
}

async fn mcp_manager_for_snapshot(snapshot: &CodexAppsSnapshot) -> McpConnectionManager {
    let servers = snapshot.effective_mcp_servers();
    mcp_manager_for_servers(&servers).await
}

async fn mcp_manager_for_servers(
    servers: &HashMap<String, EffectiveMcpServer>,
) -> McpConnectionManager {
    let (manager, rx_event) = mcp_manager_for_servers_with_events(servers).await;
    drop(rx_event);
    manager
}

async fn mcp_manager_for_servers_with_events(
    servers: &HashMap<String, EffectiveMcpServer>,
) -> (McpConnectionManager, async_channel::Receiver<Event>) {
    mcp_manager_for_servers_with_events_and_openai_form(
        servers, /*supports_openai_form_elicitation*/ false,
    )
    .await
}

async fn mcp_manager_for_servers_with_events_and_openai_form(
    servers: &HashMap<String, EffectiveMcpServer>,
    supports_openai_form_elicitation: bool,
) -> (McpConnectionManager, async_channel::Receiver<Event>) {
    mcp_manager_for_servers_with_events_and_capabilities(
        servers,
        rmcp::model::ElicitationCapability {
            form: Some(rmcp::model::FormElicitationCapability::default()),
            url: Some(rmcp::model::UrlElicitationCapability::default()),
        },
        supports_openai_form_elicitation,
    )
    .await
}

async fn mcp_manager_for_servers_with_events_and_capabilities(
    servers: &HashMap<String, EffectiveMcpServer>,
    elicitation_capability: rmcp::model::ElicitationCapability,
    supports_openai_form_elicitation: bool,
) -> (McpConnectionManager, async_channel::Receiver<Event>) {
    let (tx_event, rx_event) = async_channel::unbounded();
    let manager = McpConnectionManager::new(
        servers,
        McpConnectionManagerInput {
            store_mode: OAuthCredentialsStoreMode::default(),
            keyring_backend_kind: AuthKeyringBackendKind::default(),
            auth_entries: HashMap::new(),
            approval_policy: &Constrained::allow_any(AskForApproval::OnRequest),
            submit_id: String::new(),
            tx_event,
            startup_cancellation_token: CancellationToken::new(),
            initial_permission_profile: PermissionProfile::default(),
            runtime_context: McpRuntimeContext::new(
                Arc::new(EnvironmentManager::without_environments()),
                std::env::temp_dir(),
            ),
            prefix_mcp_tool_names: true,
            client_elicitation_capability: elicitation_capability,
            supports_openai_form_elicitation,
            tool_plugin_provenance: ToolPluginProvenance::default(),
            auth_snapshot: codex_mcp::McpAuthSnapshot::new(/*auth*/ None, /*revision*/ 0),
            elicitation_reviewer: None,
        },
    )
    .await;
    (manager, rx_event)
}

#[tokio::test]
async fn standard_upstream_elicitation_round_trips_through_connector_http_mcp() {
    upstream_elicitation_round_trip(UpstreamElicitationScenario::StandardForm).await;
}

#[tokio::test]
async fn url_upstream_elicitation_round_trips_through_connector_http_mcp() {
    upstream_elicitation_round_trip(UpstreamElicitationScenario::StandardUrl).await;
}

#[tokio::test]
async fn openai_form_upstream_elicitation_round_trips_through_connector_http_mcp() {
    upstream_elicitation_round_trip(UpstreamElicitationScenario::OpenAiForm).await;
}

#[tokio::test]
async fn unsupported_openai_form_elicitation_is_cancelled_at_the_bridge() {
    let responses = Arc::new(Mutex::new(Vec::new()));
    let apps = connect_hosted_apps_with_elicitation(ElicitingTestServer {
        scenario: UpstreamElicitationScenario::OpenAiForm,
        responses: Arc::clone(&responses),
    })
    .await;
    let (manager, events) =
        mcp_manager_for_servers_with_events(&apps.snapshot().effective_mcp_servers()).await;

    let result = manager
        .call_tool(
            "codex_apps__calendar",
            "confirm",
            /*arguments*/ None,
            /*meta*/ None,
        )
        .await
        .expect("unsupported elicitation should cancel the upstream request");
    assert_eq!(result.content[0]["text"], json!("cancelled"));
    assert_no_queued_elicitation_request(
        &events,
        "unsupported openai/form must not be sent to the downstream client",
    );
    assert_eq!(
        responses
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_slice(),
        &[ElicitationResponse {
            action: ElicitationAction::Cancel,
            content: None,
            meta: None,
        }]
    );

    manager.shutdown().await;
    apps.shutdown().await;
}

#[tokio::test]
async fn unsupported_standard_elicitation_is_cancelled_at_the_bridge() {
    let responses = Arc::new(Mutex::new(Vec::new()));
    let apps = connect_hosted_apps_with_elicitation(ElicitingTestServer {
        scenario: UpstreamElicitationScenario::StandardForm,
        responses: Arc::clone(&responses),
    })
    .await;
    let (manager, events) = mcp_manager_for_servers_with_events_and_capabilities(
        &apps.snapshot().effective_mcp_servers(),
        rmcp::model::ElicitationCapability::default(),
        /*supports_openai_form_elicitation*/ false,
    )
    .await;

    let result = manager
        .call_tool(
            "codex_apps__calendar",
            "confirm",
            /*arguments*/ None,
            /*meta*/ None,
        )
        .await
        .expect("unsupported elicitation should cancel the upstream request");
    assert_eq!(result.content[0]["text"], json!("cancelled"));
    assert_no_queued_elicitation_request(
        &events,
        "unsupported standard elicitation must not be sent to the downstream client",
    );
    assert_eq!(
        responses
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_slice(),
        &[ElicitationResponse {
            action: ElicitationAction::Cancel,
            content: None,
            meta: None,
        }]
    );

    manager.shutdown().await;
    apps.shutdown().await;
}

#[tokio::test]
async fn upstream_resource_elicitation_round_trips_through_resource_http_mcp() {
    let responses = Arc::new(Mutex::new(Vec::new()));
    let apps = connect_hosted_apps_with_elicitation(ElicitingTestServer {
        scenario: UpstreamElicitationScenario::StandardForm,
        responses: Arc::clone(&responses),
    })
    .await;
    let resource_server = apps.snapshot().resource_mcp_server();
    let (manager, events) = mcp_manager_for_servers_with_events(&HashMap::from([(
        CODEX_APPS_RESOURCE_MCP_SERVER_NAME.to_string(),
        resource_server,
    )]))
    .await;
    let manager = Arc::new(manager);
    let call_manager = Arc::clone(&manager);
    let call = tokio::spawn(async move {
        call_manager
            .list_resources(CODEX_APPS_RESOURCE_MCP_SERVER_NAME, /*params*/ None)
            .await
    });

    let request = loop {
        let event = tokio::time::timeout(Duration::from_secs(5), events.recv())
            .await
            .expect("resource elicitation event timeout")
            .expect("resource elicitation event channel");
        if let EventMsg::ElicitationRequest(request) = event.msg {
            break request;
        }
    };
    assert_eq!(request.server_name, CODEX_APPS_RESOURCE_MCP_SERVER_NAME);
    assert_eq!(
        request.request,
        codex_protocol::approvals::ElicitationRequest::Form {
            meta: Some(json!({ "requestKind": "resource" })),
            message: "Allow shared Apps resources".to_string(),
            requested_schema: json!({
                "type": "object",
                "properties": { "confirmed": { "type": "boolean" } },
                "required": ["confirmed"]
            }),
        }
    );
    manager
        .resolve_elicitation(
            CODEX_APPS_RESOURCE_MCP_SERVER_NAME.to_string(),
            rmcp_request_id(request.id),
            ElicitationResponse {
                action: ElicitationAction::Accept,
                content: Some(json!({ "confirmed": true })),
                meta: Some(json!({ "responseSource": "resource-test" })),
            },
        )
        .await
        .expect("resolve resource elicitation");
    let result = tokio::time::timeout(Duration::from_secs(5), call)
        .await
        .expect("resource completion timeout")
        .expect("resource join")
        .expect("resource result");
    assert_eq!(result.resources[0].uri, "test://apps/elicited");
    assert_eq!(
        responses
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_slice(),
        &[ElicitationResponse {
            action: ElicitationAction::Accept,
            content: Some(json!({ "confirmed": true })),
            meta: Some(json!({ "responseSource": "resource-test" })),
        }]
    );

    manager.shutdown().await;
    apps.shutdown().await;
}

async fn upstream_elicitation_round_trip(scenario: UpstreamElicitationScenario) {
    let responses = Arc::new(Mutex::new(Vec::new()));
    let apps = connect_hosted_apps_with_elicitation(ElicitingTestServer {
        scenario,
        responses: Arc::clone(&responses),
    })
    .await;
    let (manager, events) = mcp_manager_for_servers_with_events_and_openai_form(
        &apps.snapshot().effective_mcp_servers(),
        matches!(scenario, UpstreamElicitationScenario::OpenAiForm),
    )
    .await;
    let manager = Arc::new(manager);
    let call_manager = Arc::clone(&manager);
    let call = tokio::spawn(async move {
        call_manager
            .call_tool(
                "codex_apps__calendar",
                "confirm",
                /*arguments*/ None,
                /*meta*/ None,
            )
            .await
    });

    let request = loop {
        let event = tokio::time::timeout(Duration::from_secs(5), events.recv())
            .await
            .expect("upstream elicitation event timeout")
            .expect("upstream elicitation event channel");
        if let EventMsg::ElicitationRequest(request) = event.msg {
            break request;
        }
    };
    assert_eq!(request.server_name, "codex_apps__calendar");
    match (scenario, request.request) {
        (
            UpstreamElicitationScenario::StandardForm,
            codex_protocol::approvals::ElicitationRequest::Form {
                meta,
                message,
                requested_schema,
            },
        ) => {
            assert_eq!(meta, Some(json!({ "requestKind": "standard" })));
            assert_eq!(message, "Confirm the calendar action");
            assert_eq!(requested_schema["required"], json!(["confirmed"]));
        }
        (
            UpstreamElicitationScenario::OpenAiForm,
            codex_protocol::approvals::ElicitationRequest::OpenAiForm {
                meta,
                message,
                requested_schema,
            },
        ) => {
            assert_eq!(meta, Some(json!({ "requestKind": "openai" })));
            assert_eq!(message, "Select a calendar");
            assert_eq!(requested_schema["required"], json!(["calendar"]));
        }
        (
            UpstreamElicitationScenario::StandardUrl,
            codex_protocol::approvals::ElicitationRequest::Url {
                meta,
                message,
                url,
                elicitation_id,
            },
        ) => {
            assert_eq!(meta, Some(json!({ "requestKind": "url" })));
            assert_eq!(message, "Connect the calendar");
            assert_eq!(url, "https://example.com/connect/calendar");
            assert_eq!(elicitation_id, "calendar-connect");
        }
        _ => panic!("unexpected elicitation scenario or request mode"),
    }
    let response_content = (!matches!(scenario, UpstreamElicitationScenario::StandardUrl))
        .then(|| json!({ "confirmed": true, "calendar": "work" }));
    manager
        .resolve_elicitation(
            "codex_apps__calendar".to_string(),
            rmcp_request_id(request.id),
            ElicitationResponse {
                action: ElicitationAction::Accept,
                content: response_content.clone(),
                meta: Some(json!({ "responseSource": "apps-test" })),
            },
        )
        .await
        .expect("resolve upstream elicitation");
    let result = tokio::time::timeout(Duration::from_secs(5), call)
        .await
        .expect("upstream tool completion timeout")
        .expect("upstream tool join")
        .expect("upstream tool result");
    assert_eq!(result.content[0]["text"], json!("accepted"));
    assert_eq!(
        responses
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_slice(),
        &[ElicitationResponse {
            action: ElicitationAction::Accept,
            content: response_content,
            meta: Some(json!({ "responseSource": "apps-test" })),
        }]
    );

    manager.shutdown().await;
    apps.shutdown().await;
}

fn rmcp_request_id(id: codex_protocol::mcp::RequestId) -> rmcp::model::RequestId {
    match id {
        codex_protocol::mcp::RequestId::String(value) => {
            rmcp::model::RequestId::String(value.into())
        }
        codex_protocol::mcp::RequestId::Integer(value) => rmcp::model::RequestId::Number(value),
    }
}

async fn recv_elicitation_request(
    events: &async_channel::Receiver<Event>,
    timeout: Duration,
) -> Option<codex_protocol::approvals::ElicitationRequestEvent> {
    tokio::time::timeout(timeout, async {
        loop {
            let event = events.recv().await.ok()?;
            if let EventMsg::ElicitationRequest(request) = event.msg {
                return Some(request);
            }
        }
    })
    .await
    .ok()
    .flatten()
}

fn assert_no_queued_elicitation_request(events: &async_channel::Receiver<Event>, message: &str) {
    while let Ok(event) = events.try_recv() {
        assert!(
            !matches!(event.msg, EventMsg::ElicitationRequest(_)),
            "{message}"
        );
    }
}

#[tokio::test]
async fn zero_connectors_ignores_tools_without_complete_identity_or_name() {
    let (apps, _) = apps_with_tools(vec![
        connector_tool(Some("mail"), /*connector_name*/ None, "Search"),
        connector_tool(Some("mail"), Some("Mail"), "   "),
    ])
    .await;
    let snapshot = apps.snapshot();
    assert!(snapshot.apps().is_empty());
    assert!(snapshot.effective_mcp_servers().is_empty());
    apps.shutdown().await;
}

#[tokio::test]
async fn apps_runtime_metadata_maps_approval_presentation_and_source() {
    let connector_id = "connector_76869538009648d5b282a4bb21c3d157";
    let mut tool = connector_tool(Some(connector_id), Some("GitHub"), "GitHubAddComment");
    tool.title = Some("GitHub_add_comment_to_issue".to_string());
    let (apps, _) = apps_with_tools(vec![tool]).await;
    let manager = mcp_manager_for_servers(&apps.snapshot().effective_mcp_servers()).await;
    let metadata = manager
        .tool_runtime_metadata("codex_apps__github", "addcomment")
        .expect("GitHub runtime tool metadata");
    assert!(metadata.approval_persistence().is_none());
    let presentation = metadata
        .approval_presentation()
        .expect("approval presentation");
    assert_eq!(metadata.approval_header(), Some("Approve app tool call?"));
    let approval_source = metadata.approval_source().expect("Apps approval source");
    assert_eq!(approval_source.id(), connector_id);
    assert_eq!(approval_source.name(), "GitHub");
    assert_eq!(approval_source.description(), None);
    assert_eq!(
        metadata.metric_labels(),
        &[
            ("connector_id".to_string(), connector_id.to_string()),
            ("connector_name".to_string(), "GitHub".to_string()),
        ]
    );
    let telemetry_identity = metadata
        .telemetry_identity()
        .expect("Apps telemetry identity");
    assert_eq!(telemetry_identity.server_name(), "codex_apps");
    assert_eq!(telemetry_identity.tool_name(), "GitHubAddComment");
    assert_eq!(
        metadata.approval_form_metadata(),
        json!({
            "source": "connector",
            "connector_id": connector_id,
            "connector_name": "GitHub",
        })
        .as_object()
        .expect("approval metadata object")
    );
    assert_eq!(
        presentation.question(),
        "Allow GitHub to add a comment to a pull request?"
    );
    assert_eq!(
        presentation
            .parameter_labels()
            .iter()
            .map(|parameter| (parameter.name(), parameter.label()))
            .collect::<Vec<_>>(),
        vec![
            ("pr_number", "Pull request"),
            ("repo_full_name", "Repository"),
            ("comment", "Comment"),
        ]
    );

    manager.shutdown().await;
    apps.shutdown().await;
}

#[tokio::test]
async fn apps_tool_metadata_projects_identity_from_the_private_apps_envelope() {
    let mut tool = connector_tool(Some("calendar"), Some("Calendar"), "CalendarCreateEvent");
    let meta = tool.meta.as_mut().expect("connector metadata");
    meta.insert("template_id".to_string(), json!("spoofed-template"));
    meta.insert("resource_uri".to_string(), json!("/spoofed/action"));
    meta.insert(
        MCP_TOOL_CODEX_APPS_META_KEY.to_string(),
        json!({
            "template_id": "calendar-template",
            "resource_uri": "/calendar/link/create_event/",
        }),
    );

    let (apps, _) = apps_with_tools(vec![tool]).await;
    let snapshot = apps.snapshot();
    let metadata = snapshot
        .tool_metadata("codex_apps__calendar", "createevent")
        .expect("Apps-owned tool metadata");

    assert_eq!(metadata.connector_name(), "Calendar");
    assert_eq!(metadata.template_id(), Some("calendar-template"));
    assert_eq!(metadata.action_name(), Some("create_event"));

    apps.shutdown().await;
}

#[tokio::test]
async fn live_tools_replace_spoofed_approval_context_with_authenticated_account_identity() {
    let mut tool = connector_tool(Some("drive"), Some("Google Drive"), "DriveUpload");
    let meta = tool.meta.as_mut().expect("connector metadata");
    meta.insert(
        MCP_TOOL_CODEX_APPS_META_KEY.to_string(),
        json!({
            META_CONNECTED_ACCOUNT_EMAIL: "  owner@example.com  ",
            "retained": true,
        }),
    );
    meta.insert(
        MCP_APPROVAL_CONTEXT_META_KEY.to_string(),
        json!({
            MCP_APPROVAL_CONTEXT_CONNECTED_ACCOUNT_EMAIL_KEY: "spoofed@example.com",
            "spoofed": true,
        }),
    );
    let (apps, _) = apps_with_tools(vec![tool]).await;
    let server = apps
        .snapshot()
        .effective_mcp_servers()
        .remove("codex_apps__google_drive")
        .expect("Drive virtual server");
    let manager = mcp_manager_for_servers(&HashMap::from([(
        "codex_apps__google_drive".to_string(),
        server,
    )]))
    .await;
    assert!(manager.server_trusts_approval_context("codex_apps__google_drive"));
    let listed = manager.list_all_tools().await;
    let meta = listed[0].tool.meta.as_ref().expect("listed tool metadata");
    assert_eq!(
        meta.get(MCP_APPROVAL_CONTEXT_META_KEY),
        Some(&json!({
            MCP_APPROVAL_CONTEXT_CONNECTED_ACCOUNT_EMAIL_KEY: "owner@example.com",
        }))
    );
    let source = meta
        .get(MCP_TOOL_CODEX_APPS_META_KEY)
        .and_then(serde_json::Value::as_object)
        .expect("Apps source metadata");
    assert_eq!(source.get("retained"), Some(&json!(true)));
    assert!(source.get(META_CONNECTED_ACCOUNT_EMAIL).is_none());

    manager.shutdown().await;
    apps.shutdown().await;
}

#[test]
fn invalid_account_identity_removes_inbound_approval_context() {
    let too_long = format!("{}@example.com", "a".repeat(320));
    for email in [
        "",
        "not-an-email",
        "owner @example.com",
        "owner@exam\0ple.com",
        too_long.as_str(),
    ] {
        let mut tool = connector_tool(Some("drive"), Some("Drive"), "DriveUpload");
        let meta = tool.meta.as_mut().expect("connector metadata");
        meta.insert(
            MCP_TOOL_CODEX_APPS_META_KEY.to_string(),
            json!({ META_CONNECTED_ACCOUNT_EMAIL: email }),
        );
        meta.insert(
            MCP_APPROVAL_CONTEXT_META_KEY.to_string(),
            json!({ MCP_APPROVAL_CONTEXT_CONNECTED_ACCOUNT_EMAIL_KEY: "spoofed@example.com" }),
        );

        move_connected_account_to_approval_context(&mut tool);

        let meta = tool.meta.as_ref().expect("stamped metadata");
        assert!(
            meta.get(MCP_APPROVAL_CONTEXT_META_KEY).is_none(),
            "{email:?}"
        );
        assert!(
            meta.get(MCP_TOOL_CODEX_APPS_META_KEY)
                .and_then(serde_json::Value::as_object)
                .is_some_and(|source| source.get(META_CONNECTED_ACCOUNT_EMAIL).is_none()),
            "{email:?}"
        );
    }
}

#[tokio::test]
async fn one_connector_is_an_ordinary_mcp_server_with_legacy_model_name() {
    let mut upstream_tool = connector_tool(Some("gmail"), Some("Gmail"), "GmailSearchMessages");
    upstream_tool
        .meta
        .as_mut()
        .expect("connector metadata")
        .insert(
            "_codex_apps".to_string(),
            json!({
                "synthetic_link": false,
                "upstream_tool_name": "spoofed",
            }),
        );
    let upstream_meta = upstream_tool.meta.as_mut().expect("connector metadata");
    upstream_meta.insert(
        "connector_description".to_string(),
        json!("Search and organize Gmail messages."),
    );
    upstream_meta.insert("link_id".to_string(), json!("link_gmail"));
    upstream_meta.insert(
        "ui".to_string(),
        json!({"resourceUri": "ui://gmail/search.html"}),
    );
    let (apps, calls) = apps_with_tools(vec![upstream_tool]).await;
    let snapshot = apps.snapshot();
    let apps_inventory = snapshot.apps();
    let [app] = apps_inventory else {
        panic!("expected one app")
    };
    assert_eq!(app.id(), "gmail");
    assert_eq!(app.name(), "Gmail");
    assert_eq!(
        app.description(),
        Some("Search and organize Gmail messages.")
    );
    assert_eq!(app.mcp_server_name(), "codex_apps__gmail");
    let server = snapshot
        .effective_mcp_servers()
        .remove("codex_apps__gmail")
        .expect("Gmail MCP server");
    assert!(format!("{server:?}").contains("[REDACTED]"));
    let config = server.config();
    let McpServerTransportConfig::StreamableHttp {
        url, http_headers, ..
    } = &config.transport
    else {
        panic!("virtual app server should use streamable HTTP");
    };
    assert!(url.starts_with("http://127.0.0.1:"));
    assert!(url.ends_with("/mcp/codex_apps__gmail"));
    assert_eq!(http_headers, &None);
    let client = reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("HTTP client");
    assert_eq!(
        client
            .post(url.as_str())
            .send()
            .await
            .expect("missing auth")
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        client
            .post(url.as_str())
            .header("Authorization", "Bearer wrong")
            .send()
            .await
            .expect("wrong auth")
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        client
            .post(url.as_str())
            .header(ORIGIN, "https://example.com")
            .send()
            .await
            .expect("browser origin")
            .status(),
        StatusCode::FORBIDDEN
    );
    let manager = mcp_manager(&apps).await;
    assert_eq!(
        manager.server_sandbox_state_source("codex_apps__gmail"),
        codex_mcp::McpSandboxStateSource::PrimaryTurnEnvironment
    );
    let listed = manager.list_all_tools().await;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].server_name, "codex_apps__gmail");
    assert_eq!(
        listed[0].canonical_tool_name(),
        ToolName::namespaced("mcp__codex_apps__gmail", "searchmessages")
    );
    assert_eq!(listed[0].tool.name.as_ref(), "searchmessages");
    assert_eq!(listed[0].tool.title.as_deref(), Some("Title"));
    let trusted_metadata = snapshot
        .tool_metadata("codex_apps__gmail", "searchmessages")
        .expect("Apps-owned tool metadata");
    assert_eq!(trusted_metadata.connector_id(), "gmail");
    assert_eq!(trusted_metadata.connector_name(), "Gmail");
    assert_eq!(
        trusted_metadata.connector_description(),
        Some("Search and organize Gmail messages.")
    );
    assert_eq!(trusted_metadata.upstream_tool_name(), "GmailSearchMessages");
    assert_eq!(trusted_metadata.link_id(), Some("link_gmail"));
    assert_eq!(
        trusted_metadata.mcp_app_resource_uri(),
        Some("ui://gmail/search.html")
    );
    assert!(
        snapshot
            .tool_metadata("codex_apps__gmail", "SearchMessages")
            .is_none(),
        "snapshot lookup must use exact protocol-routing names"
    );
    assert_eq!(
        listed[0]
            .tool
            .meta
            .as_deref()
            .and_then(|meta| meta.get("_codex_apps"))
            .and_then(serde_json::Value::as_object),
        Some(&serde_json::Map::from_iter([
            ("synthetic_link".to_string(), json!(false)),
            ("upstream_tool_name".to_string(), json!("spoofed")),
        ]))
    );
    manager
        .call_tool(
            "codex_apps__gmail",
            "searchmessages",
            Some(json!({"query": "rust"})),
            Some(json!({
                "threadId": "thread-1",
                "codex/toolCallId": "model-call-123",
                "_codex_apps": {
                    "call_id": "downstream-call-id",
                    "connector_id": "downstream-connector",
                    "connector_name": "Downstream Connector",
                    "upstream_tool_name": "downstream-tool",
                    "downstream_private_field": true,
                },
            })),
        )
        .await
        .expect("call virtual app tool");
    {
        let calls = calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let [call] = calls.as_slice() else {
            panic!("expected one forwarded call")
        };
        assert_eq!(call.name, "GmailSearchMessages");
        assert_eq!(call.arguments, Some(json!({"query": "rust"})));
        assert!(
            call.meta["progressToken"].is_number(),
            "the virtual MCP request id should be forwarded as a progress token"
        );
        assert_eq!(call.meta["threadId"], json!("thread-1"));
        assert_eq!(
            call.meta["_codex_apps"],
            json!({
                "synthetic_link": false,
                "upstream_tool_name": "spoofed",
                "call_id": "model-call-123",
            })
        );
        assert!(call.meta.get("codex/toolCallId").is_none());
    }
    manager.shutdown().await;

    let manager = mcp_manager(&apps).await;
    assert_eq!(manager.list_all_tools().await.len(), 1);
    manager.shutdown().await;

    let addr = reqwest::Url::parse(url)
        .expect("virtual server URL")
        .socket_addrs(|| None)
        .expect("virtual server address")[0];
    apps.shutdown().await;
    assert!(tokio::net::TcpStream::connect(addr).await.is_err());
}

#[tokio::test]
async fn matching_tool_names_route_through_distinct_connector_http_namespaces() {
    let (apps, calls) = apps_with_tools(vec![
        connector_tool(Some("gmail"), Some("Gmail"), "GmailSearch"),
        connector_tool(Some("calendar"), Some("Calendar"), "CalendarSearch"),
    ])
    .await;
    let manager = mcp_manager(&apps).await;
    let listed = manager.list_all_tools().await;
    assert_eq!(listed.len(), 2);
    let listed = listed
        .into_iter()
        .map(|tool| tool.canonical_tool_name())
        .collect::<HashSet<_>>();
    assert_eq!(
        listed,
        HashSet::from([
            ToolName::namespaced("mcp__codex_apps__gmail", "search"),
            ToolName::namespaced("mcp__codex_apps__calendar", "search"),
        ])
    );

    let (gmail, calendar) = tokio::join!(
        manager.call_tool(
            "codex_apps__gmail",
            "search",
            Some(json!({"query": "mail"})),
            /*meta*/ None,
        ),
        manager.call_tool(
            "codex_apps__calendar",
            "search",
            Some(json!({"query": "events"})),
            /*meta*/ None,
        ),
    );
    gmail.expect("call Gmail search through its HTTP namespace");
    calendar.expect("call Calendar search through its HTTP namespace");

    let calls = {
        let calls = calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(calls.len(), 2);
        calls
            .iter()
            .map(|call| (call.name.clone(), call.arguments.clone()))
            .collect::<HashMap<_, _>>()
    };
    assert_eq!(
        calls,
        HashMap::from([
            ("GmailSearch".to_string(), Some(json!({"query": "mail"})),),
            (
                "CalendarSearch".to_string(),
                Some(json!({"query": "events"})),
            ),
        ])
    );

    manager.shutdown().await;
    apps.shutdown().await;
}

#[tokio::test]
async fn virtual_server_strips_untrusted_upstream_effective_tool_input() {
    let (apps, _) = apps_with_tools(vec![connector_tool(
        Some("gmail"),
        Some("Gmail"),
        "GmailSpoofToolInput",
    )])
    .await;
    let manager = mcp_manager(&apps).await;

    let result = manager
        .call_tool(
            "codex_apps__gmail",
            "spooftoolinput",
            Some(json!({ "attachment": "original" })),
            /*meta*/ None,
        )
        .await
        .expect("proxy tool call");

    assert!(
        result
            .meta
            .as_ref()
            .and_then(serde_json::Value::as_object)
            .is_none_or(|meta| {
                !meta.contains_key(codex_protocol::mcp::MCP_TOOL_INPUT_META_KEY)
            })
    );

    manager.shutdown().await;
    apps.shutdown().await;
}

#[tokio::test]
async fn connector_auth_failure_elicits_through_the_virtual_mcp_server() {
    let (apps, calls) = apps_with_tools(vec![connector_tool(
        Some("gmail"),
        Some("Gmail"),
        "GmailRequiresAuth",
    )])
    .await;
    let servers = apps.snapshot().effective_mcp_servers();
    let (manager, events) = mcp_manager_for_servers_with_events(&servers).await;
    let manager = Arc::new(manager);
    let call_manager = Arc::clone(&manager);
    let call = tokio::spawn(async move {
        call_manager
            .call_tool(
                "codex_apps__gmail",
                "requiresauth",
                /*arguments*/ None,
                /*meta*/ None,
            )
            .await
    });

    let request = loop {
        let event = tokio::time::timeout(Duration::from_secs(5), events.recv())
            .await
            .expect("auth elicitation event timeout")
            .expect("auth elicitation event channel");
        if let EventMsg::ElicitationRequest(request) = event.msg {
            break request;
        }
    };
    assert_eq!(request.server_name, "codex_apps__gmail");
    let response_id = match request.id {
        codex_protocol::mcp::RequestId::String(value) => {
            rmcp::model::RequestId::String(value.into())
        }
        codex_protocol::mcp::RequestId::Integer(value) => rmcp::model::RequestId::Number(value),
    };
    let codex_protocol::approvals::ElicitationRequest::Url {
        url,
        elicitation_id,
        ..
    } = request.request
    else {
        panic!("connector auth should use URL elicitation")
    };
    assert_eq!(url, "https://chatgpt.com/apps/gmail/gmail");
    assert!(elicitation_id.starts_with("codex_apps_auth_"));
    // Resolve the request through the ordinary MCP manager API. It has no connector knowledge.
    // The virtual server owns interpreting the accepted response.
    manager
        .resolve_elicitation(
            "codex_apps__gmail".to_string(),
            response_id,
            ElicitationResponse {
                action: ElicitationAction::Accept,
                content: Some(json!({})),
                meta: None,
            },
        )
        .await
        .expect("resolve connector auth elicitation");
    let result = tokio::time::timeout(Duration::from_secs(5), call)
        .await
        .expect("connector auth completion timeout")
        .expect("connector auth call task")
        .expect("connector auth call result");
    assert_eq!(result.is_error, Some(true));
    assert_eq!(
        result.content[0]["text"],
        json!("Authentication for Gmail was requested and accepted. Retry this tool call now.")
    );
    assert_eq!(result.structured_content, None);
    assert_eq!(
        result
            .meta
            .as_ref()
            .and_then(|meta| meta.get(MCP_ERROR_CODE_META_KEY)),
        Some(&json!("AUTH_REQUIRED")),
    );
    {
        let calls = calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(calls.len(), 1);
        assert!(calls[0].meta["_codex_apps"].get("connector_id").is_none());
        assert!(calls[0].meta["_codex_apps"]["call_id"].is_string());
    }
    manager.shutdown().await;
    apps.shutdown().await;
}

#[tokio::test]
async fn connector_auth_failure_preserves_the_tool_error_without_url_capability() {
    let (apps, _) = apps_with_tools(vec![connector_tool(
        Some("gmail"),
        Some("Gmail"),
        "GmailRequiresAuth",
    )])
    .await;
    let (manager, events) = mcp_manager_for_servers_with_events_and_capabilities(
        &apps.snapshot().effective_mcp_servers(),
        rmcp::model::ElicitationCapability::default(),
        /*supports_openai_form_elicitation*/ false,
    )
    .await;

    let result = manager
        .call_tool(
            "codex_apps__gmail",
            "requiresauth",
            /*arguments*/ None,
            /*meta*/ None,
        )
        .await
        .expect("auth failure result");
    assert_eq!(result.is_error, Some(true));
    assert_eq!(result.content[0]["text"], json!("sign in required"));
    assert_eq!(result.structured_content, None);
    assert_eq!(
        result
            .meta
            .as_ref()
            .and_then(|meta| meta.get(MCP_ERROR_CODE_META_KEY)),
        Some(&json!("AUTH_REQUIRED")),
    );
    assert_no_queued_elicitation_request(
        &events,
        "auth URL must not be sent without downstream URL capability",
    );

    manager.shutdown().await;
    apps.shutdown().await;
}

#[tokio::test]
async fn apps_refresh_preserves_upstream_file_schemas_until_upload_is_proxied() {
    let upload_tool = || {
        let mut tool = connector_tool(Some("gmail"), Some("Gmail"), "GmailUpload");
        tool.input_schema = Arc::new(
            json!({
                "type": "object",
                "properties": {
                    "attachment": {
                        "type": "object",
                        "description": "Upload me."
                    },
                    "caption": {
                        "type": "string"
                    }
                }
            })
            .as_object()
            .expect("input schema object")
            .clone(),
        );
        tool.meta
            .as_mut()
            .expect("connector metadata")
            .insert("openai/fileParams".to_string(), json!(["attachment"]));
        tool
    };
    let (apps, state) = apps_with_refreshable_pages(vec![vec![upload_tool()]]).await;
    let startup_snapshot = apps.snapshot();
    let startup_manager = mcp_manager_for_snapshot(&startup_snapshot).await;

    let startup_tools = startup_manager.list_all_tools().await;
    state.set_pages(vec![vec![upload_tool()]]);
    let refreshed_snapshot = apps.refresh().await.expect("refresh Apps inventory");
    let refreshed_manager = mcp_manager_for_snapshot(&refreshed_snapshot).await;
    let refreshed_tools = refreshed_manager.list_all_tools().await;
    let [startup_tool] = startup_tools.as_slice() else {
        panic!("expected one startup tool")
    };
    let [refreshed_tool] = refreshed_tools.as_slice() else {
        panic!("expected one refreshed tool")
    };
    assert_eq!(refreshed_tool.tool.name.as_ref(), "upload");
    assert_eq!(
        *refreshed_tool.tool.input_schema,
        json!({
            "type": "object",
            "properties": {
                "attachment": {
                    "type": "object",
                    "description": "Upload me."
                },
                "caption": {
                    "type": "string"
                }
            }
        })
        .as_object()
        .expect("expected input schema object")
        .clone()
    );
    assert_eq!(
        refreshed_tool.tool.input_schema,
        startup_tool.tool.input_schema
    );
    let metadata = refreshed_snapshot
        .tool_metadata("codex_apps__gmail", "upload")
        .expect("refreshed Apps-owned tool metadata");
    assert_eq!(metadata.upstream_tool_name(), "GmailUpload");

    startup_manager.shutdown().await;
    refreshed_manager.shutdown().await;
    apps.shutdown().await;
}

#[test]
fn local_file_array_schema_preserves_cardinality_constraints() {
    let mut tool = connector_tool(Some("gmail"), Some("Gmail"), "GmailUpload");
    tool.input_schema = Arc::new(
        json!({
            "type": "object",
            "properties": {
                "attachments": {
                    "type": "array",
                    "items": { "type": "object" },
                    "minItems": 1,
                    "maxItems": 3,
                    "uniqueItems": true,
                    "description": "Upload these."
                }
            }
        })
        .as_object()
        .expect("schema object")
        .clone(),
    );

    rewrite_tool_schema_for_local_file_paths(&mut tool, &["attachments".to_string()]);

    assert_eq!(
        tool.input_schema["properties"]["attachments"],
        json!({
            "type": "array",
            "items": { "type": "string" },
            "minItems": 1,
            "maxItems": 3,
            "uniqueItems": true,
            "description": concat!(
                "Upload these. This parameter expects an absolute local file path. ",
                "If you want to upload a file, provide the absolute path to that file here."
            )
        })
    );
}

#[tokio::test]
async fn virtual_server_uploads_from_a_pinned_replaced_environment() {
    use tempfile::tempdir;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::body_json;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    let files = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/backend-api/files"))
        .and(body_json(json!({
            "file_name": "report.csv",
            "file_size": 5,
            "use_case": "codex",
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "file_id": "file_123",
            "upload_url": format!("{}/upload/file_123", files.uri()),
        })))
        .expect(1)
        .mount(&files)
        .await;
    Mock::given(method("PUT"))
        .and(path("/upload/file_123"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&files)
        .await;
    Mock::given(method("POST"))
        .and(path("/backend-api/files/file_123/uploaded"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success",
            "download_url": format!("{}/download/file_123", files.uri()),
            "file_name": "report.csv",
            "mime_type": "text/csv",
            "file_size_bytes": 5,
        })))
        .expect(1)
        .mount(&files)
        .await;

    let dir = tempdir().expect("temp dir");
    let sandbox_dir = dir.path().join("allowed");
    let outside_dir = dir.path().join("outside");
    tokio::fs::create_dir_all(&sandbox_dir)
        .await
        .expect("create sandbox directory");
    tokio::fs::create_dir_all(&outside_dir)
        .await
        .expect("create outside directory");
    tokio::fs::write(sandbox_dir.join("report.csv"), b"hello")
        .await
        .expect("write test file");
    tokio::fs::write(outside_dir.join("secret.csv"), b"nope")
        .await
        .expect("write denied file");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside_dir, sandbox_dir.join("outside-link"))
        .expect("create escaping symlink");
    #[cfg(unix)]
    let codex_linux_sandbox_exe = TEST_BINARY_DISPATCH_GUARD
        .as_ref()
        .and_then(|guard| guard.paths().codex_linux_sandbox_exe.clone());
    #[cfg(not(unix))]
    let codex_linux_sandbox_exe = None;
    let runtime_paths = codex_exec_server::ExecServerRuntimePaths::new(
        std::env::current_exe().expect("current exe"),
        codex_linux_sandbox_exe,
    )
    .expect("runtime paths");
    let environment_manager = Arc::new(
        EnvironmentManager::create_for_tests_with_local(
            /*exec_server_url*/ None,
            runtime_paths,
        )
        .await,
    );
    let pinned_environment = environment_manager
        .get_environment("local")
        .expect("local environment");
    let environment_instance_id = pinned_environment.instance_id().to_string();

    let mut upload_tool = connector_tool(Some("gmail"), Some("Gmail"), "GmailUpload");
    upload_tool.input_schema = Arc::new(
        json!({
            "type": "object",
            "properties": {
                "attachment": { "type": "object", "description": "Upload me." },
                "attachments": {
                    "type": "array",
                    "items": { "type": "object" },
                    "description": "Upload these."
                }
            }
        })
        .as_object()
        .expect("schema object")
        .clone(),
    );
    upload_tool.meta.as_mut().expect("tool metadata").insert(
        META_OPENAI_FILE_PARAMS.to_string(),
        json!(["attachment", "attachments"]),
    );
    let calls = Arc::new(Mutex::new(Vec::new()));
    let upstream = start_hosted_upstream(TestServer {
        tools: Arc::from(vec![upload_tool]),
        calls: Arc::clone(&calls),
        call_gate: None,
        resource_gate: None,
    })
    .await;
    let auth_provider: codex_api::SharedAuthProvider = Arc::new(EmptyAuthProvider);
    let apps = CodexApps::connect_inner(
        &upstream.config,
        Arc::clone(&auth_provider),
        Some(Arc::new(AppsFileSupport {
            chatgpt_base_url: format!("{}/backend-api", files.uri()),
            auth_provider,
            environment_manager: Arc::clone(&environment_manager),
        })),
        Arc::new(|| {}),
        CodexAppsAccessGuard::default(),
    )
    .await
    .expect("connect hosted Apps with file support");
    let apps = HostedCodexApps {
        apps,
        _upstream_server: upstream.server,
    };
    let snapshot = apps.snapshot();
    let manager = mcp_manager_for_snapshot(&snapshot).await;
    let listed_tools = manager.list_all_tools().await;
    let [listed] = listed_tools.as_slice() else {
        panic!("expected one virtual tool")
    };
    assert_eq!(
        listed.tool.input_schema["properties"]["attachment"]["type"],
        "string"
    );
    assert_eq!(
        listed.tool.input_schema["properties"]["attachments"]["items"]["type"],
        "string"
    );

    environment_manager
        .upsert_environment(
            "local".to_string(),
            "ws://127.0.0.1:1".to_string(),
            Some(Duration::from_millis(1)),
        )
        .expect("replace named environment after the turn pinned it");
    assert_ne!(
        environment_manager
            .get_environment("local")
            .expect("replacement environment")
            .instance_id(),
        pinned_environment.instance_id(),
    );

    let sandbox_cwd = PathUri::from_host_native_path(&sandbox_dir).expect("absolute temp path");
    #[cfg(unix)]
    let permission_profile = PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
            },
            access: FileSystemAccessMode::Read,
        }]),
        NetworkSandboxPolicy::Restricted,
    );
    #[cfg(not(unix))]
    let permission_profile = PermissionProfile::Disabled;
    let sandbox_state = SandboxState {
        environment_id: "local".to_string(),
        environment_instance_id: Some(environment_instance_id),
        permission_profile,
        codex_linux_sandbox_exe: None,
        sandbox_cwd,
        use_legacy_landlock: false,
    };
    let upload = manager
        .call_tool(
            "codex_apps__gmail",
            "upload",
            Some(json!({ "attachment": "report.csv" })),
            Some(json!({
                (MCP_SANDBOX_STATE_META_CAPABILITY): sandbox_state.clone(),
                (codex_protocol::mcp::MCP_TOOL_CALL_ID_META_KEY): "upload-call-1",
            })),
        )
        .await;
    #[cfg(unix)]
    let mut sandbox_warning: Option<String> = None;
    let result = match upload {
        Ok(result) => result,
        Err(error) => {
            #[cfg(unix)]
            if format!("{error:#}").contains("fs sandbox helper failed") {
                let warning = format!("{error:#}");
                eprintln!("managed file-upload sandbox is unavailable: {warning}");
                sandbox_warning = Some(warning);
                let sandbox_state = SandboxState {
                    permission_profile: PermissionProfile::Disabled,
                    ..sandbox_state.clone()
                };
                manager
                    .call_tool(
                        "codex_apps__gmail",
                        "upload",
                        Some(json!({ "attachment": "report.csv" })),
                        Some(json!({
                            (MCP_SANDBOX_STATE_META_CAPABILITY): sandbox_state.clone(),
                            (codex_protocol::mcp::MCP_TOOL_CALL_ID_META_KEY): "upload-call-1",
                        })),
                    )
                    .await
                    .expect("retry upload without unavailable platform sandbox")
            } else {
                panic!("call upload tool: {error:#}")
            }
            #[cfg(not(unix))]
            panic!("call upload tool: {error:#}")
        }
    };

    assert_eq!(
        result
            .meta
            .as_ref()
            .and_then(|meta| meta.get(codex_protocol::mcp::MCP_TOOL_INPUT_META_KEY)),
        Some(&json!({
            "attachment": {
                "download_url": format!("{}/download/file_123", files.uri()),
                "file_id": "file_123",
                "mime_type": "text/csv",
                "file_name": "report.csv",
                "uri": "sediment://file_123",
                "file_size_bytes": 5,
            }
        }))
    );
    {
        let calls = calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].meta["_codex_apps"]["call_id"], "upload-call-1");
        assert!(
            calls[0]
                .meta
                .get(MCP_SANDBOX_STATE_META_CAPABILITY)
                .is_none()
        );
        assert_eq!(
            calls[0].arguments,
            result
                .meta
                .as_ref()
                .and_then(|meta| meta.get(codex_protocol::mcp::MCP_TOOL_INPUT_META_KEY))
                .cloned()
        );
    }

    let storage_request_count = files
        .received_requests()
        .await
        .expect("file API requests")
        .len();
    assert_eq!(storage_request_count, 3, "one upload uses three requests");
    #[cfg(unix)]
    {
        if let Some(warning) = sandbox_warning.as_deref() {
            eprintln!("skipping managed file-upload sandbox assertions: {warning}");
        } else {
            let denied_paths = [
                outside_dir
                    .join("secret.csv")
                    .to_string_lossy()
                    .into_owned(),
                "../outside/secret.csv".to_string(),
                "outside-link/secret.csv".to_string(),
            ];
            for denied_path in denied_paths {
                let error = manager
                    .call_tool(
                        "codex_apps__gmail",
                        "upload",
                        Some(json!({ "attachment": denied_path })),
                        Some(json!({
                            (MCP_SANDBOX_STATE_META_CAPABILITY): sandbox_state.clone(),
                        })),
                    )
                    .await
                    .expect_err("sandbox escape must be rejected");
                assert!(
                    format!("{error:#}").contains("failed to upload"),
                    "unexpected sandbox error: {error}"
                );
            }
        }
    }

    let malformed = manager
        .call_tool(
            "codex_apps__gmail",
            "upload",
            Some(json!({ "attachments": ["report.csv", 7] })),
            Some(json!({
                (MCP_SANDBOX_STATE_META_CAPABILITY): sandbox_state,
            })),
        )
        .await
        .expect_err("mixed file array must be rejected before upload");
    assert!(
        format!("{malformed:#}").contains("expected a local file path string"),
        "unexpected malformed array error: {malformed}"
    );
    assert_eq!(
        files
            .received_requests()
            .await
            .expect("file API requests after rejected calls")
            .len(),
        storage_request_count,
        "rejected paths and malformed arrays must not reach file storage"
    );
    {
        let calls = calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(
            calls.len(),
            1,
            "rejected paths and malformed arrays must not reach the hosted tool"
        );
    }

    manager.shutdown().await;
    apps.shutdown().await;
}

struct GatedFileUploadFixture {
    apps: HostedCodexApps,
    calls: Arc<Mutex<Vec<RecordedCall>>>,
    sandbox_state: SandboxState,
    upload_started: Arc<tokio::sync::Notify>,
    upload_server: JoinHandle<Result<(), std::io::Error>>,
    _dir: tempfile::TempDir,
}

async fn gated_file_upload_fixture() -> GatedFileUploadFixture {
    let upload_started = Arc::new(tokio::sync::Notify::new());
    let upload_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind gated file API");
    let upload_addr = upload_listener
        .local_addr()
        .expect("gated file API address");
    let upload_server = tokio::spawn({
        let upload_started = Arc::clone(&upload_started);
        async move {
            let router = Router::new().route(
                "/backend-api/files",
                axum::routing::post(move || {
                    let upload_started = Arc::clone(&upload_started);
                    async move {
                        upload_started.notify_one();
                        std::future::pending::<axum::response::Response>().await
                    }
                }),
            );
            axum::serve(upload_listener, router).await
        }
    });

    let dir = tempfile::tempdir().expect("temp dir");
    tokio::fs::write(dir.path().join("report.csv"), b"hello")
        .await
        .expect("write test file");
    let runtime_paths = codex_exec_server::ExecServerRuntimePaths::new(
        std::env::current_exe().expect("current exe"),
        /*codex_linux_sandbox_exe*/ None,
    )
    .expect("runtime paths");
    let environment_manager = Arc::new(
        EnvironmentManager::create_for_tests_with_local(
            /*exec_server_url*/ None,
            runtime_paths,
        )
        .await,
    );
    let environment_instance_id = environment_manager
        .get_environment("local")
        .expect("local environment")
        .instance_id()
        .to_string();

    let mut upload_tool = connector_tool(Some("gmail"), Some("Gmail"), "GmailUpload");
    upload_tool.input_schema = Arc::new(
        json!({
            "type": "object",
            "properties": {
                "attachment": { "type": "object", "description": "Upload me." }
            }
        })
        .as_object()
        .expect("schema object")
        .clone(),
    );
    upload_tool
        .meta
        .as_mut()
        .expect("tool metadata")
        .insert(META_OPENAI_FILE_PARAMS.to_string(), json!(["attachment"]));
    let calls = Arc::new(Mutex::new(Vec::new()));
    let upstream = start_hosted_upstream(TestServer {
        tools: Arc::from(vec![
            upload_tool,
            connector_tool(Some("gmail"), Some("Gmail"), "GmailPing"),
        ]),
        calls: Arc::clone(&calls),
        call_gate: None,
        resource_gate: None,
    })
    .await;
    let auth_provider: codex_api::SharedAuthProvider = Arc::new(EmptyAuthProvider);
    let apps = CodexApps::connect_inner(
        &upstream.config,
        Arc::clone(&auth_provider),
        Some(Arc::new(AppsFileSupport {
            chatgpt_base_url: format!("http://{upload_addr}/backend-api"),
            auth_provider,
            environment_manager,
        })),
        Arc::new(|| {}),
        CodexAppsAccessGuard::default(),
    )
    .await
    .expect("connect hosted Apps with file support");
    let apps = HostedCodexApps {
        apps,
        _upstream_server: upstream.server,
    };
    let sandbox_state = SandboxState {
        environment_id: "local".to_string(),
        environment_instance_id: Some(environment_instance_id),
        permission_profile: PermissionProfile::Disabled,
        codex_linux_sandbox_exe: None,
        sandbox_cwd: PathUri::from_host_native_path(dir.path()).expect("absolute temp path"),
        use_legacy_landlock: false,
    };

    GatedFileUploadFixture {
        apps,
        calls,
        sandbox_state,
        upload_started,
        upload_server,
        _dir: dir,
    }
}

#[tokio::test]
async fn shutdown_cancels_file_upload_before_forwarding() {
    let fixture = gated_file_upload_fixture().await;
    let manager = Arc::new(mcp_manager_for_snapshot(&fixture.apps.snapshot()).await);
    let call = tokio::spawn({
        let manager = Arc::clone(&manager);
        let sandbox_state = fixture.sandbox_state.clone();
        async move {
            manager
                .call_tool(
                    "codex_apps__gmail",
                    "upload",
                    Some(json!({ "attachment": "report.csv" })),
                    Some(json!({
                        (MCP_SANDBOX_STATE_META_CAPABILITY): sandbox_state,
                    })),
                )
                .await
        }
    });
    tokio::time::timeout(TEST_TIMEOUT, fixture.upload_started.notified())
        .await
        .expect("file upload did not reach the gated API");

    tokio::time::timeout(Duration::from_secs(1), fixture.apps.shutdown())
        .await
        .expect("generation shutdown should cancel the blocked file upload");
    assert!(
        fixture
            .calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty(),
        "a cancelled upload must not reach the hosted tool"
    );

    abort_server(call).await;
    manager.shutdown().await;
    abort_server(fixture.upload_server).await;
}

fn loopback_mcp_post(
    client: &reqwest::Client,
    url: &str,
    bearer_token: &str,
    session_id: Option<&str>,
) -> reqwest::RequestBuilder {
    let mut request = client
        .post(url)
        .bearer_auth(bearer_token)
        .header("accept", "application/json, text/event-stream")
        .header("mcp-protocol-version", "2025-06-18");
    if let Some(session_id) = session_id {
        request = request.header("mcp-session-id", session_id);
    }
    request
}

#[tokio::test]
async fn request_cancellation_stops_file_upload_and_keeps_generation_usable() {
    let fixture = gated_file_upload_fixture().await;
    let snapshot = fixture.apps.snapshot();
    let server_name = "codex_apps__gmail";
    let url = virtual_server_url(&snapshot, server_name);
    let bearer_token = snapshot.loopback_bearer_token_for_test(server_name);
    let client = reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("loopback MCP client");

    let initialize = loopback_mcp_post(&client, &url, &bearer_token, /*session_id*/ None)
        .json(&json!({
            "jsonrpc": "2.0",
            "id": "initialize",
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "cancellation-test", "version": "1" },
            },
        }))
        .send()
        .await
        .expect("initialize loopback MCP session");
    assert_eq!(initialize.status(), StatusCode::OK);
    let session_id = initialize
        .headers()
        .get("mcp-session-id")
        .expect("loopback MCP session id")
        .to_str()
        .expect("valid loopback MCP session id")
        .to_string();
    let initialize_body = initialize.text().await.expect("initialize response body");
    assert!(initialize_body.contains("\"id\":\"initialize\""));

    let initialized = loopback_mcp_post(&client, &url, &bearer_token, Some(session_id.as_str()))
        .json(&json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
        }))
        .send()
        .await
        .expect("acknowledge loopback MCP initialization");
    assert_eq!(initialized.status(), StatusCode::ACCEPTED);

    let upload_request = tokio::spawn({
        let client = client.clone();
        let url = url.clone();
        let bearer_token = bearer_token.clone();
        let session_id = session_id.clone();
        let sandbox_state = fixture.sandbox_state.clone();
        async move {
            let response =
                loopback_mcp_post(&client, &url, &bearer_token, Some(session_id.as_str()))
                    .json(&json!({
                        "jsonrpc": "2.0",
                        "id": "upload-request",
                        "method": "tools/call",
                        "params": {
                            "name": "upload",
                            "arguments": { "attachment": "report.csv" },
                            "_meta": {
                                (MCP_SANDBOX_STATE_META_CAPABILITY): sandbox_state,
                            },
                        },
                    }))
                    .send()
                    .await
                    .expect("start file-upload MCP call");
            let status = response.status();
            response
                .bytes()
                .await
                .expect("consume file-upload MCP response");
            status
        }
    });
    tokio::time::timeout(TEST_TIMEOUT, fixture.upload_started.notified())
        .await
        .expect("file upload did not reach the gated API");

    let cancelled = loopback_mcp_post(&client, &url, &bearer_token, Some(session_id.as_str()))
        .json(&json!({
            "jsonrpc": "2.0",
            "method": "notifications/cancelled",
            "params": {
                "requestId": "upload-request",
                "reason": "test request cancellation",
            },
        }))
        .send()
        .await
        .expect("cancel file-upload MCP call");
    assert_eq!(cancelled.status(), StatusCode::ACCEPTED);

    let status = tokio::time::timeout(Duration::from_secs(1), upload_request)
        .await
        .expect("cancelled file-upload MCP call should resolve promptly")
        .expect("file-upload MCP task");
    assert_eq!(status, StatusCode::OK);
    assert!(
        fixture
            .calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty(),
        "a request-cancelled upload must not reach the hosted tool"
    );

    let ping = loopback_mcp_post(&client, &url, &bearer_token, Some(session_id.as_str()))
        .json(&json!({
            "jsonrpc": "2.0",
            "id": "ping-request",
            "method": "tools/call",
            "params": { "name": "ping" },
        }))
        .send()
        .await
        .expect("call loopback MCP after cancellation");
    assert_eq!(ping.status(), StatusCode::OK);
    let ping_body = ping.text().await.expect("post-cancellation MCP response");
    assert!(ping_body.contains("forwarded"));
    {
        let calls = fixture
            .calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "GmailPing");
    }

    fixture.apps.shutdown().await;
    abort_server(fixture.upload_server).await;
}

#[tokio::test]
async fn file_upload_rejects_missing_environment_instance_id() {
    let file_support = AppsFileSupport {
        chatgpt_base_url: "http://127.0.0.1:1/backend-api".to_string(),
        auth_provider: Arc::new(EmptyAuthProvider),
        environment_manager: Arc::new(EnvironmentManager::default_for_tests()),
    };
    let sandbox_state = SandboxState {
        environment_id: "local".to_string(),
        environment_instance_id: None,
        permission_profile: PermissionProfile::Disabled,
        codex_linux_sandbox_exe: None,
        sandbox_cwd: PathUri::parse("file:///tmp").expect("sandbox cwd"),
        use_legacy_landlock: false,
    };

    let error = rewrite_arguments_for_openai_files(
        &file_support,
        Some(&sandbox_state),
        Some(json!({ "attachment": "report.csv" })),
        &["attachment".to_string()],
    )
    .await
    .expect_err("missing environment identity must fail closed");

    assert_eq!(
        error,
        "failed to upload `report.csv` for `attachment`: sandbox state is missing an environment instance id"
    );
}

#[tokio::test]
async fn file_upload_rejects_unpinned_replaced_environment() {
    let environment_manager = Arc::new(EnvironmentManager::without_environments());
    environment_manager
        .upsert_environment(
            "workspace".to_string(),
            "ws://127.0.0.1:1".to_string(),
            Some(Duration::from_millis(1)),
        )
        .expect("original environment");
    let original_instance_id = environment_manager
        .get_environment("workspace")
        .expect("original environment")
        .instance_id()
        .to_string();
    environment_manager
        .upsert_environment(
            "workspace".to_string(),
            "ws://127.0.0.1:2".to_string(),
            Some(Duration::from_millis(1)),
        )
        .expect("replacement environment");
    let replacement_instance_id = environment_manager
        .get_environment("workspace")
        .expect("replacement environment")
        .instance_id()
        .to_string();
    assert_ne!(original_instance_id, replacement_instance_id);

    let file_support = AppsFileSupport {
        chatgpt_base_url: "http://127.0.0.1:1/backend-api".to_string(),
        auth_provider: Arc::new(EmptyAuthProvider),
        environment_manager,
    };
    let sandbox_state = SandboxState {
        environment_id: "workspace".to_string(),
        environment_instance_id: Some(original_instance_id),
        permission_profile: PermissionProfile::Disabled,
        codex_linux_sandbox_exe: None,
        sandbox_cwd: PathUri::parse("file:///tmp").expect("sandbox cwd"),
        use_legacy_landlock: false,
    };

    let error = rewrite_arguments_for_openai_files(
        &file_support,
        Some(&sandbox_state),
        Some(json!({ "attachment": "report.csv" })),
        &["attachment".to_string()],
    )
    .await
    .expect_err("replaced environment identity must fail closed");

    assert_eq!(
        error,
        "failed to upload `report.csv` for `attachment`: environment `workspace` was replaced after the sandbox state was captured"
    );
}

#[tokio::test]
async fn collisions_use_legacy_identity_hashes() {
    let (apps, _) = apps_with_tools(vec![
        connector_tool(Some("drive-one"), Some("Drive!"), "DriveList"),
        connector_tool(Some("drive-two"), Some("Drive?"), "DriveGet"),
        connector_tool(Some("gmail"), Some("Gmail"), "GmailFoo-Bar"),
        connector_tool(Some("gmail"), Some("Gmail"), "GmailFoo_Bar"),
    ])
    .await;
    let snapshot = apps.snapshot();
    assert_eq!(snapshot.apps().len(), 3);
    assert_eq!(
        snapshot.apps()[0].mcp_server_name(),
        "codex_apps__drive_99a0d4a4035d"
    );
    assert_eq!(
        snapshot.apps()[1].mcp_server_name(),
        "codex_apps__drive_b469ba67a2f2"
    );
    let manager = mcp_manager(&apps).await;
    let names = manager
        .list_all_tools()
        .await
        .into_iter()
        .map(|tool| tool.canonical_tool_name())
        .collect::<HashSet<_>>();
    assert_eq!(
        names,
        HashSet::from([
            ToolName::namespaced("mcp__codex_apps__drive_99a0d4a4035d", "list"),
            ToolName::namespaced("mcp__codex_apps__drive_b469ba67a2f2", "get"),
            ToolName::namespaced("mcp__codex_apps__gmail", "foo_bar_7362b7bd5a54"),
            ToolName::namespaced("mcp__codex_apps__gmail", "foo_bar_8919b3893acb"),
        ])
    );
    manager.shutdown().await;
    apps.shutdown().await;
}

#[tokio::test]
async fn approval_identity_preserves_distinct_raw_tool_names() {
    let (apps, _) = apps_with_tools(vec![
        connector_tool(Some("gmail"), Some("Gmail"), "GmailTrim"),
        connector_tool(Some("gmail"), Some("Gmail"), " GmailTrim "),
    ])
    .await;
    let snapshot = apps.snapshot();
    let server = snapshot
        .effective_mcp_servers()
        .remove("codex_apps__gmail")
        .expect("Gmail MCP server");
    let identities = snapshot
        .tools()
        .map(|(_, tool_name, _)| {
            server
                .runtime_metadata()
                .tool(tool_name)
                .and_then(codex_mcp::McpToolRuntimeMetadata::approval_identity)
                .expect("stable approval identity")
                .tool_name()
                .to_string()
        })
        .collect::<HashSet<_>>();
    assert_eq!(
        identities,
        HashSet::from(["GmailTrim".to_string(), " GmailTrim ".to_string()])
    );

    apps.shutdown().await;
}

#[tokio::test]
async fn natural_connector_name_wins_when_it_matches_a_generated_name() {
    let base_server_name = codex_connectors::metadata::connector_mcp_server_name("Drive!");
    let drive_one_identity = format!(
        "{}\0{base_server_name}\0drive-one",
        codex_connectors::metadata::CODEX_APPS_MCP_SERVER_NAME
    );
    let preferred_suffix = codex_utils_string::sha1_12_hex_suffix(&drive_one_identity);
    let natural_connector_name = format!("Drive{preferred_suffix}");
    let preferred_name = format!("{base_server_name}{preferred_suffix}");
    assert_eq!(
        codex_connectors::metadata::connector_mcp_server_name(&natural_connector_name),
        preferred_name
    );
    let tools = vec![
        connector_tool(Some("drive-one"), Some("Drive!"), "DriveList"),
        connector_tool(Some("drive-two"), Some("Drive?"), "DriveGet"),
        connector_tool(
            Some("natural"),
            Some(&natural_connector_name),
            "NaturalSearch",
        ),
    ];
    let (apps, _) = apps_with_tools(tools).await;
    let snapshot = apps.snapshot();
    let names = snapshot
        .apps()
        .iter()
        .map(|app| (app.id().to_string(), app.mcp_server_name().to_string()))
        .collect::<HashMap<_, _>>();
    assert_eq!(names["natural"], preferred_name);
    assert_eq!(
        names["drive-one"],
        format!(
            "{base_server_name}{}",
            codex_utils_string::sha1_12_hex_suffix(&format!("{drive_one_identity}\0{}", 1))
        )
    );
    assert_eq!(names.values().collect::<HashSet<_>>().len(), names.len());

    apps.shutdown().await;
}

#[tokio::test]
async fn natural_tool_name_wins_when_it_matches_a_generated_name() {
    let raw_namespace_identity = format!(
        "{}\0{}\0gmail",
        codex_connectors::metadata::CODEX_APPS_MCP_SERVER_NAME,
        codex_connectors::metadata::connector_mcp_server_name("Gmail")
    );
    let first_upstream_name = "GmailFoo-Bar";
    let second_upstream_name = "GmailFoo_Bar";
    let base_callable = codex_connectors::metadata::connector_tool_name(
        first_upstream_name,
        Some("gmail"),
        Some("Gmail"),
    );
    let first_identity =
        format!("{raw_namespace_identity}\0{base_callable}\0{first_upstream_name}");
    let preferred_suffix = codex_utils_string::sha1_12_hex_suffix(&first_identity);
    let natural_upstream_name = format!("GmailFoo-Bar{preferred_suffix}");
    let natural_name = codex_connectors::metadata::connector_tool_name(
        &natural_upstream_name,
        Some("gmail"),
        Some("Gmail"),
    );
    assert_eq!(natural_name, format!("{base_callable}{preferred_suffix}"));
    let second_identity =
        format!("{raw_namespace_identity}\0{base_callable}\0{second_upstream_name}");
    let first_name = format!(
        "{base_callable}{}",
        codex_utils_string::sha1_12_hex_suffix(&format!("{first_identity}\0{}", 1))
    );
    let second_name = format!(
        "{base_callable}{}",
        codex_utils_string::sha1_12_hex_suffix(&second_identity)
    );
    let tools = vec![
        connector_tool(Some("gmail"), Some("Gmail"), first_upstream_name),
        connector_tool(Some("gmail"), Some("Gmail"), &natural_upstream_name),
        connector_tool(Some("gmail"), Some("Gmail"), second_upstream_name),
    ];
    let (apps, _) = apps_with_tools(tools).await;
    let snapshot = apps.snapshot();
    let names_by_upstream = snapshot
        .tools()
        .map(|(_, exposed_name, metadata)| {
            (
                metadata.upstream_tool_name().to_string(),
                exposed_name.to_string(),
            )
        })
        .collect::<HashMap<_, _>>();
    assert_eq!(
        names_by_upstream,
        HashMap::from([
            (first_upstream_name.to_string(), first_name),
            (natural_upstream_name, natural_name),
            (second_upstream_name.to_string(), second_name),
        ])
    );

    apps.shutdown().await;
}

#[tokio::test]
async fn paginated_inventory_excludes_synthetic_only_connectors_from_apps() {
    let mut gmail_tool = connector_tool(Some("gmail"), Some("Gmail"), "GmailSearch");
    gmail_tool
        .meta
        .as_mut()
        .expect("connector metadata")
        .insert("connector_description".to_string(), json!("Search Gmail"));
    let (apps, state) = apps_with_refreshable_pages(vec![
        vec![gmail_tool],
        vec![synthetic_connector_tool(
            "calendar",
            "Calendar",
            "CalendarLink",
        )],
    ])
    .await;

    assert_eq!(
        state.requested_cursors(),
        vec![None, Some("page-1".to_string())]
    );
    let snapshot = apps.snapshot();
    let apps_inventory = snapshot.apps();
    let [app] = apps_inventory else {
        panic!("expected one non-synthetic app")
    };
    assert_eq!(app.id(), "gmail");
    assert_eq!(app.name(), "Gmail");
    assert_eq!(app.description(), Some("Search Gmail"));
    assert_eq!(app.mcp_server_name(), "codex_apps__gmail");
    assert_eq!(
        snapshot
            .all_connectors()
            .iter()
            .map(CodexApp::id)
            .collect::<Vec<_>>(),
        vec!["calendar", "gmail"]
    );
    assert_eq!(
        snapshot
            .effective_mcp_servers()
            .into_keys()
            .collect::<HashSet<_>>(),
        HashSet::from([
            "codex_apps__calendar".to_string(),
            "codex_apps__gmail".to_string(),
        ])
    );
    assert!(snapshot.effective_mcp_servers().values().all(|server| {
        !server
            .runtime_metadata()
            .records_physical_tools_list_metric()
    }));
    assert!(
        !snapshot
            .resource_mcp_server()
            .runtime_metadata()
            .records_physical_tools_list_metric()
    );

    apps.shutdown().await;
}

#[tokio::test]
async fn upstream_tool_limit_accepts_boundary_and_rejects_overflow() {
    let bare_tool = connector_tool(
        /*connector_id*/ None, /*connector_name*/ None, "BareTool",
    );
    let (at_limit, _) =
        list_refreshable_pages(vec![vec![bare_tool.clone(); MAX_CODEX_APPS_TOOLS]]).await;
    assert_eq!(
        at_limit.expect("tool limit should be inclusive").len(),
        MAX_CODEX_APPS_TOOLS
    );

    let (over_limit, state) = list_refreshable_pages(vec![
        vec![bare_tool.clone(); MAX_CODEX_APPS_TOOLS],
        vec![bare_tool],
    ])
    .await;
    let error = over_limit.expect_err("one tool over the limit must fail");
    assert!(
        error.to_string().contains("exceeded the 4096-tool limit"),
        "unexpected tools/list error: {error:#}"
    );
    assert_eq!(state.requested_cursors().len(), 2);
}

#[tokio::test]
async fn upstream_inventory_byte_limit_is_cumulative_and_inclusive() {
    let first = connector_tool(
        /*connector_id*/ None, /*connector_name*/ None, "First",
    );
    let second = connector_tool(
        /*connector_id*/ None, /*connector_name*/ None, "Second",
    );
    let expected = vec![first.clone(), second.clone()];
    let serialized_bytes = serde_json::to_vec(&expected)
        .expect("serialize expected inventory")
        .len();

    let at_limit = list_refreshable_pages_with_inventory_limit(
        vec![vec![first.clone()], vec![second.clone()]],
        serialized_bytes,
    )
    .await
    .expect("serialized inventory limit should be inclusive");
    assert_eq!(at_limit, expected);

    let error = list_refreshable_pages_with_inventory_limit(
        vec![vec![first], vec![second]],
        serialized_bytes - 1,
    )
    .await
    .expect_err("one byte over the serialized inventory limit must fail");
    assert!(
        error.to_string().contains("serialized inventory limit"),
        "unexpected tools/list error: {error:#}"
    );
}

#[tokio::test]
async fn upstream_page_limit_accepts_boundary_and_stops_before_overflow_request() {
    let (at_limit, state) =
        list_refreshable_pages(vec![Vec::new(); MAX_CODEX_APPS_TOOL_PAGES]).await;
    assert!(at_limit.expect("page limit should be inclusive").is_empty());
    assert_eq!(state.requested_cursors().len(), MAX_CODEX_APPS_TOOL_PAGES);

    let (over_limit, state) =
        list_refreshable_pages(vec![Vec::new(); MAX_CODEX_APPS_TOOL_PAGES + 1]).await;
    let error = over_limit.expect_err("one page over the limit must fail");
    assert!(
        error.to_string().contains("exceeded the 128-page limit"),
        "unexpected tools/list error: {error:#}"
    );
    assert_eq!(state.requested_cursors().len(), MAX_CODEX_APPS_TOOL_PAGES);
}

#[tokio::test(start_paused = true)]
async fn paginated_inventory_uses_one_overall_load_timeout() {
    let requested_cursors = Arc::new(Mutex::new(Vec::new()));
    let result = list_all_upstream_tools_with_lister(Duration::from_secs(50), {
        let requested_cursors = Arc::clone(&requested_cursors);
        move |request, remaining| {
            let requested_cursors = Arc::clone(&requested_cursors);
            Box::pin(async move {
                tokio::time::timeout(remaining, tokio::time::sleep(Duration::from_secs(30)))
                    .await
                    .map_err(|_| anyhow::anyhow!("page deadline elapsed"))?;
                let cursor = request.and_then(|request| request.cursor);
                requested_cursors
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push(cursor.clone());
                Ok(ListToolsResult {
                    tools: Vec::new(),
                    next_cursor: cursor.is_none().then(|| "page-1".to_string()),
                    meta: None,
                })
            })
        }
    })
    .await;

    let error = result.expect_err("the second page must exhaust the overall timeout");
    assert!(
        error
            .to_string()
            .contains("failed to list Codex Apps tools"),
        "unexpected tools/list timeout: {error:#}"
    );
    assert_eq!(
        *requested_cursors
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        vec![None],
    );
}

#[tokio::test]
async fn repeated_refreshes_publish_new_immutable_generations() {
    let (apps, state) = apps_with_refreshable_pages(vec![vec![connector_tool(
        Some("gmail"),
        Some("Gmail"),
        "GmailSearch",
    )]])
    .await;
    let original = apps.snapshot();
    let manager = mcp_manager_for_snapshot(&original).await;

    state.set_pages(vec![vec![connector_tool(
        Some("gmail"),
        Some("Gmail"),
        "GmailArchive",
    )]]);
    let first = apps.refresh().await.expect("first Apps refresh");
    assert_eq!(
        manager
            .list_all_tools()
            .await
            .into_iter()
            .map(|tool| tool.tool.name.to_string())
            .collect::<Vec<_>>(),
        vec!["search"]
    );
    let first_manager = mcp_manager_for_snapshot(&first).await;
    assert_eq!(
        first_manager
            .list_all_tools()
            .await
            .into_iter()
            .map(|tool| tool.tool.name.to_string())
            .collect::<Vec<_>>(),
        vec!["archive"]
    );

    state.set_pages(vec![vec![connector_tool(
        Some("gmail"),
        Some("Gmail"),
        "GmailDelete",
    )]]);
    let second = apps.refresh().await.expect("second Apps refresh");
    assert_eq!(
        manager
            .list_all_tools()
            .await
            .into_iter()
            .map(|tool| tool.tool.name.to_string())
            .collect::<Vec<_>>(),
        vec!["search"]
    );
    assert_eq!(
        second
            .tool_metadata("codex_apps__gmail", "delete")
            .expect("new generation metadata")
            .upstream_tool_name(),
        "GmailDelete"
    );
    assert!(
        original
            .tool_metadata("codex_apps__gmail", "delete")
            .is_none()
    );
    assert!(first.tool_metadata("codex_apps__gmail", "delete").is_none());

    first_manager.shutdown().await;
    manager.shutdown().await;
    apps.shutdown().await;
}

#[tokio::test]
async fn approval_identity_survives_namespace_collision_churn() {
    let (apps, state) = apps_with_refreshable_pages(vec![vec![connector_tool(
        Some("drive-one"),
        Some("Drive!"),
        "DriveList",
    )]])
    .await;
    let approval_route = |snapshot: &CodexAppsSnapshot, connector_id: &str| {
        let (server_name, tool_name, _) = snapshot
            .tools()
            .find(|(_, _, metadata)| metadata.connector_id() == connector_id)
            .expect("connector tool metadata");
        let identity = snapshot
            .effective_mcp_servers()
            .get(server_name)
            .and_then(|server| server.runtime_metadata().tool(tool_name))
            .and_then(codex_mcp::McpToolRuntimeMetadata::approval_identity)
            .cloned()
            .expect("stable approval identity");
        (server_name.to_string(), tool_name.to_string(), identity)
    };

    let original = apps.snapshot();
    let original_route = approval_route(&original, "drive-one");
    assert_eq!(
        (original_route.0.as_str(), original_route.1.as_str()),
        ("codex_apps__drive", "list")
    );

    state.set_pages(vec![vec![
        connector_tool(Some("drive-one"), Some("Drive!"), "DriveList"),
        connector_tool(Some("drive-two"), Some("Drive?"), "DriveList"),
    ]]);
    let colliding = apps.refresh().await.expect("publish colliding generation");
    let colliding_first = approval_route(&colliding, "drive-one");
    let colliding_second = approval_route(&colliding, "drive-two");
    assert_ne!(colliding_first.0, colliding_second.0);
    assert_eq!(colliding_first.2, original_route.2);
    assert_ne!(colliding_first.2, colliding_second.2);

    state.set_pages(vec![vec![connector_tool(
        Some("drive-two"),
        Some("Drive?"),
        "DriveList",
    )]]);
    let replacement = apps
        .refresh()
        .await
        .expect("publish replacement generation");
    let replacement_route = approval_route(&replacement, "drive-two");
    assert_eq!(
        (&replacement_route.0, &replacement_route.1),
        (&original_route.0, &original_route.1),
        "the second source inherits the first source's routed namespace"
    );
    assert_ne!(
        replacement_route.2, original_route.2,
        "session approval identity must not follow the routed namespace"
    );
    assert_eq!(
        replacement_route.2.server_name(),
        codex_connectors::metadata::CODEX_APPS_MCP_SERVER_NAME
    );
    assert_eq!(replacement_route.2.source_id(), "drive-two");
    assert_eq!(replacement_route.2.tool_name(), "DriveList");

    apps.shutdown().await;
}

#[tokio::test]
async fn waiting_explicit_refresh_fetches_inventory_after_in_flight_refresh() {
    let (apps, state) = apps_with_refreshable_pages(vec![vec![connector_tool(
        Some("gmail"),
        Some("Gmail"),
        "GmailInitial",
    )]])
    .await;
    let apps = Arc::new(apps);
    state.set_pages(vec![vec![connector_tool(
        Some("gmail"),
        Some("Gmail"),
        "GmailStale",
    )]]);
    let first_refresh_gate = CallGate::default();
    state.gate_next_list_tools(first_refresh_gate.clone());

    let first_refresh = {
        let apps = Arc::clone(&apps);
        tokio::spawn(async move { apps.refresh().await })
    };
    tokio::time::timeout(TEST_TIMEOUT, first_refresh_gate.started.notified())
        .await
        .expect("first refresh reaches gated tools/list");

    state.set_pages(vec![vec![connector_tool(
        Some("gmail"),
        Some("Gmail"),
        "GmailFresh",
    )]]);
    let second_refresh = {
        let apps = Arc::clone(&apps);
        tokio::spawn(async move { apps.refresh().await })
    };
    tokio::task::yield_now().await;
    assert!(
        !second_refresh.is_finished(),
        "the second refresh must wait for the refresh permit"
    );

    first_refresh_gate.release.notify_one();
    let first = tokio::time::timeout(TEST_TIMEOUT, first_refresh)
        .await
        .expect("first refresh completion timeout")
        .expect("first refresh task")
        .expect("first refresh");
    let second = tokio::time::timeout(TEST_TIMEOUT, second_refresh)
        .await
        .expect("second refresh completion timeout")
        .expect("second refresh task")
        .expect("second refresh");

    assert!(first.tool_metadata("codex_apps__gmail", "stale").is_some());
    assert!(second.tool_metadata("codex_apps__gmail", "fresh").is_some());
    assert_eq!(state.requested_cursors(), vec![None, None, None]);

    apps.shutdown().await;
}

#[tokio::test]
async fn refresh_keeps_pinned_servers_consistent_until_their_registration_is_dropped() {
    let (apps, state) = apps_with_refreshable_pages(vec![vec![connector_tool(
        Some("gmail"),
        Some("Gmail"),
        "GmailSearch",
    )]])
    .await;
    let pinned = apps.snapshot();
    let old_url = virtual_server_url(&pinned, "codex_apps__gmail");
    let old_addr = reqwest::Url::parse(&old_url)
        .expect("old virtual server URL")
        .socket_addrs(|| None)
        .expect("old virtual server address")[0];

    state.set_pages(vec![vec![connector_tool(
        Some("calendar"),
        Some("Calendar"),
        "CalendarList",
    )]]);
    let refreshed = apps.refresh().await.expect("refresh Apps");

    assert_eq!(
        pinned.apps().iter().map(CodexApp::id).collect::<Vec<_>>(),
        vec!["gmail"]
    );
    assert_eq!(
        refreshed
            .apps()
            .iter()
            .map(CodexApp::id)
            .collect::<Vec<_>>(),
        vec!["calendar"]
    );
    assert_eq!(
        apps.snapshot()
            .apps()
            .iter()
            .map(CodexApp::id)
            .collect::<Vec<_>>(),
        vec!["calendar"]
    );

    let old_servers = pinned.effective_mcp_servers();
    drop(pinned);
    assert!(tokio::net::TcpStream::connect(old_addr).await.is_ok());
    let old_manager = mcp_manager_for_servers(&old_servers).await;
    drop(old_servers);
    assert_eq!(
        old_manager.list_all_tools().await[0].tool.name.as_ref(),
        "search"
    );
    old_manager.shutdown().await;
    drop(old_manager);
    tokio::time::timeout(TEST_TIMEOUT, async {
        while tokio::net::TcpStream::connect(old_addr).await.is_ok() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("unpinned old generation should stop its listener");

    apps.shutdown().await;
}

#[tokio::test]
async fn failed_refresh_keeps_the_last_good_generation() {
    let (apps, state) = apps_with_refreshable_pages(vec![vec![connector_tool(
        Some("gmail"),
        Some("Gmail"),
        "GmailSearch",
    )]])
    .await;
    let original = apps.snapshot();
    state.set_list_failure(/*fail*/ true);

    let error = apps.refresh().await.err().expect("refresh should fail");
    assert!(
        error
            .to_string()
            .contains("failed to list Codex Apps tools")
    );
    assert_eq!(
        apps.snapshot()
            .apps()
            .iter()
            .map(CodexApp::id)
            .collect::<Vec<_>>(),
        vec!["gmail"]
    );
    let manager = mcp_manager_for_snapshot(&original).await;
    assert_eq!(manager.list_all_tools().await.len(), 1);
    manager.shutdown().await;

    apps.shutdown().await;
}

#[tokio::test]
async fn inventory_change_notifier_runs_once_per_successful_refresh_publication() {
    let state = Arc::new(RefreshableTestState::default());
    state.set_pages(vec![vec![connector_tool(
        Some("gmail"),
        Some("Gmail"),
        "GmailSearch",
    )]]);
    let upstream = start_hosted_upstream(RefreshableTestServer {
        state: Arc::clone(&state),
    })
    .await;
    let changes = Arc::new(AtomicUsize::new(0));
    let changes_for_notifier = Arc::clone(&changes);
    let apps = CodexApps::connect_inner(
        &upstream.config,
        Arc::new(EmptyAuthProvider),
        /*file_support*/ None,
        Arc::new(move || {
            changes_for_notifier.fetch_add(1, Ordering::Relaxed);
        }),
        CodexAppsAccessGuard::default(),
    )
    .await
    .expect("connect hosted Apps");
    let apps = HostedCodexApps {
        apps,
        _upstream_server: upstream.server,
    };
    assert_eq!(changes.load(Ordering::Relaxed), 0);

    state.set_pages(vec![vec![connector_tool(
        Some("gmail"),
        Some("Gmail"),
        "GmailArchive",
    )]]);
    apps.refresh().await.expect("successful refresh");
    assert_eq!(changes.load(Ordering::Relaxed), 1);

    state.set_list_failure(/*fail*/ true);
    assert!(apps.refresh().await.is_err());
    assert_eq!(changes.load(Ordering::Relaxed), 1);

    apps.shutdown().await;
}

#[tokio::test]
async fn shutdown_joins_pinned_replaced_generation_listeners() {
    let (apps, state) = apps_with_refreshable_pages(vec![vec![connector_tool(
        Some("gmail"),
        Some("Gmail"),
        "GmailSearch",
    )]])
    .await;
    let pinned = apps.snapshot();
    let pinned_addr = reqwest::Url::parse(&virtual_server_url(&pinned, "codex_apps__gmail"))
        .expect("pinned generation URL")
        .socket_addrs(|| None)
        .expect("pinned generation address")[0];

    state.set_pages(vec![vec![connector_tool(
        Some("calendar"),
        Some("Calendar"),
        "CalendarList",
    )]]);
    let current = apps.refresh().await.expect("refresh Apps generation");
    let current_addr = reqwest::Url::parse(&virtual_server_url(&current, "codex_apps__calendar"))
        .expect("current generation URL")
        .socket_addrs(|| None)
        .expect("current generation address")[0];
    assert!(tokio::net::TcpStream::connect(pinned_addr).await.is_ok());
    assert!(tokio::net::TcpStream::connect(current_addr).await.is_ok());

    tokio::time::timeout(TEST_TIMEOUT, apps.shutdown())
        .await
        .expect("shutdown must join every pinned generation listener");

    assert!(tokio::net::TcpStream::connect(pinned_addr).await.is_err());
    assert!(tokio::net::TcpStream::connect(current_addr).await.is_err());
    drop((pinned, current));
}

#[tokio::test]
async fn refresh_rotates_loopback_bearers_between_generations() {
    let (apps, state) = apps_with_refreshable_pages(vec![vec![connector_tool(
        Some("gmail"),
        Some("Gmail"),
        "GmailSearch",
    )]])
    .await;
    let original = apps.snapshot();
    let server_name = "codex_apps__gmail";
    let original_url = virtual_server_url(&original, server_name);
    let original_bearer = original.loopback_bearer_token_for_test(server_name);

    state.set_pages(vec![vec![connector_tool(
        Some("gmail"),
        Some("Gmail"),
        "GmailArchive",
    )]]);
    let refreshed = apps.refresh().await.expect("refresh Apps generation");
    let refreshed_url = virtual_server_url(&refreshed, server_name);
    let refreshed_bearer = refreshed.loopback_bearer_token_for_test(server_name);
    assert_ne!(original_url, refreshed_url);
    assert_ne!(original_bearer, refreshed_bearer);

    let client = reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("loopback HTTP client");
    for (url, stale_bearer) in [
        (&refreshed_url, &original_bearer),
        (&original_url, &refreshed_bearer),
    ] {
        assert_eq!(
            client
                .post(url)
                .bearer_auth(stale_bearer)
                .send()
                .await
                .expect("cross-generation bearer request")
                .status(),
            StatusCode::UNAUTHORIZED,
        );
    }

    apps.shutdown().await;
}

#[tokio::test]
async fn auth_revision_rejects_new_requests_without_cancelling_an_already_forwarded_call() {
    let call_gate = CallGate::default();
    let upstream = start_hosted_upstream(TestServer {
        tools: Arc::from(vec![connector_tool(
            Some("gmail"),
            Some("Gmail"),
            "GmailSearch",
        )]),
        calls: Arc::new(Mutex::new(Vec::new())),
        call_gate: Some(call_gate.clone()),
        resource_gate: None,
    })
    .await;
    let auth_revision = Arc::new(AtomicUsize::new(0));
    let expected_revision = Arc::clone(&auth_revision);
    let apps = CodexApps::connect_inner(
        &upstream.config,
        Arc::new(EmptyAuthProvider),
        /*file_support*/ None,
        Arc::new(|| {}),
        CodexAppsAccessGuard::new(move || expected_revision.load(Ordering::Acquire) == 0),
    )
    .await
    .expect("connect guarded Apps runtime");
    let apps = HostedCodexApps {
        apps,
        _upstream_server: upstream.server,
    };
    let snapshot = apps.snapshot();
    let server_name = "codex_apps__gmail";
    let url = virtual_server_url(&snapshot, server_name);
    let bearer = snapshot.loopback_bearer_token_for_test(server_name);
    let manager = Arc::new(mcp_manager_for_snapshot(&snapshot).await);
    let call = tokio::spawn({
        let manager = Arc::clone(&manager);
        async move {
            manager
                .call_tool(
                    server_name,
                    "search",
                    /*arguments*/ None,
                    /*meta*/ None,
                )
                .await
        }
    });
    tokio::time::timeout(TEST_TIMEOUT, call_gate.started.notified())
        .await
        .expect("tool call must reach the upstream before auth changes");

    auth_revision.store(1, Ordering::Release);
    let client = reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("loopback HTTP client");
    assert_eq!(
        client
            .post(url)
            .bearer_auth(bearer)
            .send()
            .await
            .expect("request after auth revision")
            .status(),
        StatusCode::UNAUTHORIZED,
    );

    call_gate.release.notify_one();
    let result = tokio::time::timeout(TEST_TIMEOUT, call)
        .await
        .expect("forwarded call completion timeout")
        .expect("forwarded call task")
        .expect("a call authorized before forwarding remains valid");
    assert_eq!(result.content[0]["text"], json!("forwarded"));

    manager.shutdown().await;
    apps.shutdown().await;
}

fn virtual_server_url(snapshot: &CodexAppsSnapshot, server_name: &str) -> String {
    let servers = snapshot.effective_mcp_servers();
    let server = servers.get(server_name).expect("virtual MCP server");
    let config = server.config();
    let McpServerTransportConfig::StreamableHttp { url, .. } = &config.transport else {
        panic!("virtual app server should use streamable HTTP");
    };
    url.clone()
}
