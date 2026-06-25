use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use axum::Router;
use codex_api::AuthProvider;
use codex_apps::CodexApps;
use codex_apps::CodexAppsAccessGuard;
use codex_apps::CodexAppsConnectConfig;
use codex_config::Constrained;
use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::EnvironmentManager;
use codex_mcp::EffectiveMcpServer;
use codex_mcp::McpConnectionManager;
use codex_mcp::McpConnectionManagerInput;
use codex_mcp::McpRuntimeContext;
use codex_mcp::ToolPluginProvenance;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::Event;
use rmcp::ServerHandler;
use rmcp::model::CallToolRequestParams;
use rmcp::model::CallToolResult;
use rmcp::model::Content;
use rmcp::model::JsonObject;
use rmcp::model::ListToolsResult;
use rmcp::model::Meta;
use rmcp::model::PaginatedRequestParams;
use rmcp::model::ServerCapabilities;
use rmcp::model::ServerInfo;
use rmcp::model::Tool;
use rmcp::model::ToolAnnotations;
use rmcp::service::RequestContext;
use rmcp::service::RoleServer;
use rmcp::transport::StreamableHttpServerConfig;
use rmcp::transport::StreamableHttpService;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
struct ToolServer {
    tools: Arc<[Tool]>,
    calls: Arc<AtomicUsize>,
    reject_tools: Option<Arc<AtomicBool>>,
    list_calls: Option<Arc<AtomicUsize>>,
    list_gate: Option<Arc<tokio::sync::Semaphore>>,
}

impl ServerHandler for ToolServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, rmcp::ErrorData> {
        if let Some(list_calls) = &self.list_calls {
            list_calls.fetch_add(1, Ordering::AcqRel);
        }
        if let Some(list_gate) = &self.list_gate {
            let permit = list_gate
                .acquire()
                .await
                .map_err(|_| rmcp::ErrorData::internal_error("Apps inventory gate closed", None))?;
            permit.forget();
        }
        if self
            .reject_tools
            .as_ref()
            .is_some_and(|reject_tools| reject_tools.load(Ordering::Acquire))
        {
            return Err(rmcp::ErrorData::internal_error(
                "injected Apps inventory failure",
                None,
            ));
        }
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
        self.calls.fetch_add(1, Ordering::Relaxed);
        Ok(CallToolResult::success(vec![Content::text(format!(
            "called {}",
            request.name
        ))]))
    }
}

pub(super) async fn start_gated_http_apps_server(
    tools: Vec<Tool>,
) -> (String, Arc<AtomicBool>, JoinHandle<std::io::Result<()>>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind hosted Apps test server");
    let address = listener.local_addr().expect("hosted Apps test address");
    let reject_tools = Arc::new(AtomicBool::new(true));
    let server_reject_tools = Arc::clone(&reject_tools);
    let service = StreamableHttpService::new(
        move || {
            Ok(ToolServer {
                tools: Arc::from(tools.clone()),
                calls: Arc::new(AtomicUsize::new(0)),
                reject_tools: Some(Arc::clone(&server_reject_tools)),
                list_calls: None,
                list_gate: None,
            })
        },
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default().with_json_response(true),
    );
    let router = Router::new().nest_service("/api/codex/ps/mcp", service);
    let server = tokio::spawn(async move { axum::serve(listener, router).await });
    (format!("http://{address}"), reject_tools, server)
}

pub(super) async fn start_blocked_http_apps_server(
    tools: Vec<Tool>,
) -> (
    String,
    Arc<AtomicUsize>,
    Arc<tokio::sync::Semaphore>,
    JoinHandle<std::io::Result<()>>,
) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind blocked Apps test server");
    let address = listener.local_addr().expect("blocked Apps test address");
    let list_calls = Arc::new(AtomicUsize::new(0));
    let list_gate = Arc::new(tokio::sync::Semaphore::new(0));
    let service = StreamableHttpService::new(
        {
            let list_calls = Arc::clone(&list_calls);
            let list_gate = Arc::clone(&list_gate);
            move || {
                Ok(ToolServer {
                    tools: Arc::from(tools.clone()),
                    calls: Arc::new(AtomicUsize::new(0)),
                    reject_tools: None,
                    list_calls: Some(Arc::clone(&list_calls)),
                    list_gate: Some(Arc::clone(&list_gate)),
                })
            }
        },
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default().with_json_response(true),
    );
    let router = Router::new().nest_service("/api/codex/ps/mcp", service);
    let server = tokio::spawn(async move { axum::serve(listener, router).await });
    (format!("http://{address}"), list_calls, list_gate, server)
}

pub(super) fn connector_tool(
    connector_id: &str,
    connector_name: &str,
    name: &str,
    destructive: bool,
) -> Tool {
    let mut tool = Tool::new(name.to_string(), "test tool", Arc::new(JsonObject::new()));
    tool.annotations = Some(ToolAnnotations::new().destructive(destructive));
    tool.meta = Some(Meta(serde_json::Map::from_iter([
        ("connector_id".to_string(), serde_json::json!(connector_id)),
        (
            "connector_name".to_string(),
            serde_json::json!(connector_name),
        ),
    ])));
    tool
}

pub(super) fn gmail_tool(name: &str, destructive: bool) -> Tool {
    connector_tool("gmail", "Gmail", name, destructive)
}

pub(super) async fn test_apps(tools: Vec<Tool>) -> Arc<CodexApps> {
    test_apps_with_access_guard(tools, CodexAppsAccessGuard::default())
        .await
        .0
}

pub(super) async fn test_apps_with_access_guard(
    tools: Vec<Tool>,
    access_guard: CodexAppsAccessGuard,
) -> (Arc<CodexApps>, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind hosted Apps test server");
    let address = listener.local_addr().expect("hosted Apps test address");
    let service = StreamableHttpService::new(
        {
            let calls = Arc::clone(&calls);
            move || {
                Ok(ToolServer {
                    tools: Arc::from(tools.clone()),
                    calls: Arc::clone(&calls),
                    reject_tools: None,
                    list_calls: None,
                    list_gate: None,
                })
            }
        },
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default().with_json_response(true),
    );
    let _server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().nest_service("/api/codex/ps/mcp", service),
        )
        .await
    });
    let config = CodexAppsConnectConfig::new(
        format!("http://{address}"),
        /*product_sku*/ None,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::Direct,
    );
    (
        Arc::new(
            CodexApps::connect_with_environment(
                &config,
                Arc::new(EmptyAuthProvider),
                Arc::new(EnvironmentManager::without_environments()),
                Arc::new(|| {}),
                access_guard,
            )
            .await
            .expect("connect hosted Apps test server"),
        ),
        calls,
    )
}

#[derive(Debug)]
struct EmptyAuthProvider;

impl AuthProvider for EmptyAuthProvider {
    fn add_auth_headers(&self, _headers: &mut axum::http::HeaderMap) {}
}

pub(super) async fn mcp_manager_for_servers(
    servers: &HashMap<String, EffectiveMcpServer>,
) -> McpConnectionManager {
    let (tx_event, rx_event) = async_channel::unbounded::<Event>();
    drop(rx_event);
    McpConnectionManager::new(
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
            client_elicitation_capability: Default::default(),
            supports_openai_form_elicitation: false,
            tool_plugin_provenance: ToolPluginProvenance::default(),
            auth_snapshot: codex_mcp::McpAuthSnapshot::new(/*auth*/ None, /*revision*/ 0),
            elicitation_reviewer: None,
        },
    )
    .await
}
