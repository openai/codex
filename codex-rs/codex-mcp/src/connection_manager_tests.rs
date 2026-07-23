use super::*;
use crate::McpBinding;
use crate::elicitation::ElicitationLifecycle;
use crate::elicitation::ElicitationRequestManager;
use crate::elicitation::ElicitationRequestRouter;
use crate::elicitation::elicitation_is_rejected_by_policy;
use crate::rmcp_client::AsyncManagedClient;
use crate::rmcp_client::CODEX_APPS_RECONNECT_INITIAL_BACKOFF;
use crate::rmcp_client::CodexAppsStartupReconnect;
use crate::rmcp_client::ManagedClient;
use crate::rmcp_client::ManagedClientFuture;
use crate::rmcp_client::StartupOutcomeError;
use crate::rmcp_client::list_tools_for_client_uncached;
use crate::runtime::McpRuntimeContext;
use crate::server::EffectiveMcpServer;
use crate::server::McpServerMetadata;
use crate::server::McpServerOrigin;
use crate::tool_catalog_cache::McpToolCatalogCache;
use crate::tools::ToolFilter;
use crate::tools::ToolInfo;
use crate::tools::filter_tools;
use crate::tools::normalize_tools_for_model_with_prefix;
use codex_config::AppToolApproval;
use codex_config::Constrained;
use codex_config::McpServerConfig;
use codex_config::McpServerEnvVar;
use codex_config::McpServerToolConfig;
use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_connectors::ConnectorRuntimeContext;
use codex_connectors::ConnectorRuntimeContextKey;
use codex_connectors::ConnectorRuntimeFetchSource;
use codex_connectors::ConnectorRuntimeManager;
use codex_exec_server::EnvironmentManager;
use codex_login::CodexAuth;
use codex_protocol::ToolName;
use codex_protocol::mcp::McpServerInfo;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::GranularApprovalConfig;
use codex_protocol::protocol::McpStartupFailureReason;
use codex_rmcp_client::ElicitationResponse;
use codex_rmcp_client::InProcessTransportFactory;
use codex_rmcp_client::McpAuthState;
use codex_rmcp_client::McpLoginRequirement;
use codex_rmcp_client::RmcpClient;
use futures::FutureExt;
use futures::future::BoxFuture;
use pretty_assertions::assert_eq;
use rmcp::ErrorData as McpError;
use rmcp::RoleServer;
use rmcp::ServerHandler;
use rmcp::ServiceExt;
use rmcp::model::ClientCapabilities;
use rmcp::model::CreateElicitationRequestParams;
use rmcp::model::ElicitationAction;
use rmcp::model::ElicitationCapability;
use rmcp::model::Implementation;
use rmcp::model::InitializeRequestParams;
use rmcp::model::JsonObject;
use rmcp::model::ListToolsResult;
use rmcp::model::NumberOrString;
use rmcp::model::PaginatedRequestParams;
use rmcp::model::ProtocolVersion;
use rmcp::model::ServerCapabilities;
use rmcp::model::ServerInfo;
use rmcp::model::Tool;
use rmcp::service::RequestContext;
use std::collections::HashMap;
use std::collections::HashSet;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use tempfile::tempdir;
use tokio::io::DuplexStream;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

impl McpConnectionSet {
    fn new_uninitialized(
        approval_policy: &Constrained<AskForApproval>,
        permission_profile: &Constrained<PermissionProfile>,
        prefix_mcp_tool_names: bool,
    ) -> Self {
        Self {
            servers: HashMap::new(),
            required_servers: Vec::new(),
            tool_catalog_revision: Arc::new(RwLock::new(0)),
            codex_apps_tools_override: RwLock::new(None),
            codex_apps_refresh_lock: Mutex::new(()),
            tool_plugin_provenance: Arc::new(ToolPluginProvenance::default()),
            prefix_mcp_tool_names,
            non_prefixed_mcp_tool_servers: Vec::new(),
            elicitation_requests: ElicitationRequestManager::new(
                approval_policy.value(),
                permission_profile.get().clone(),
                /*reviewer*/ None,
                /*lifecycle*/ None,
                ElicitationRequestRouter::default(),
            ),
        }
    }

    fn insert_test_client(&mut self, name: impl Into<String>, client: AsyncManagedClient) {
        let name = name.into();
        self.servers.insert(
            name,
            McpServerView {
                tool_filter: ToolFilter::default(),
                connection: Arc::new(McpServerConnection {
                    identity: None,
                    client,
                }),
                metadata: McpServerMetadata {
                    environment_id: String::new(),
                    pollutes_memory: true,
                    origin: None,
                    supports_parallel_tool_calls: false,
                    default_tools_approval_mode: None,
                    tool_approval_modes: HashMap::new(),
                },
                tool_timeout: None,
            },
        );
    }

    fn test_client(&self, name: &str) -> &AsyncManagedClient {
        &self.servers[name].connection.client
    }

    fn set_test_server_metadata(&mut self, name: &str, metadata: McpServerMetadata) {
        self.servers
            .get_mut(name)
            .expect("test server exists")
            .metadata = metadata;
    }

    fn shares_test_connection_with(&self, other: &Self, name: &str) -> bool {
        let Some(left) = self.servers.get(name) else {
            return false;
        };
        let Some(right) = other.servers.get(name) else {
            return false;
        };
        Arc::ptr_eq(&left.connection, &right.connection)
    }
}

fn create_test_tool(server_name: &str, tool_name: &str) -> ToolInfo {
    ToolInfo {
        server_name: server_name.to_string(),
        supports_parallel_tool_calls: false,
        server_origin: None,
        callable_name: tool_name.to_string(),
        callable_namespace: server_name.to_string(),
        namespace_description: None,
        tool: Tool::new(
            tool_name.to_string(),
            format!("Test tool: {tool_name}"),
            Arc::new(JsonObject::default()),
        ),
        openai_file_input_optional_fields: Default::default(),
        connector_id: None,
        connector_name: None,
        plugin_display_names: Vec::new(),
    }
}

fn create_codex_apps_tools_cache_context(
    codex_home: PathBuf,
    account_id: Option<&str>,
    chatgpt_user_id: Option<&str>,
) -> ConnectorRuntimeContext<ToolInfo> {
    ConnectorRuntimeManager::<ToolInfo>::default().context(
        codex_home,
        ConnectorRuntimeContextKey::personal(
            account_id.map(ToOwned::to_owned),
            chatgpt_user_id.map(ToOwned::to_owned),
        ),
    )
}

fn store_current_tools(cache_context: &ConnectorRuntimeContext<ToolInfo>, tools: Vec<ToolInfo>) {
    let _ = cache_context.publish_if_newest_accepted(
        cache_context.begin_fetch(ConnectorRuntimeFetchSource::HardRefresh),
        &create_test_server_info("Codex Apps"),
        tools,
    );
}

async fn capture_binding(manager: &Arc<McpConnectionSet>) -> McpBinding {
    manager
        .capture_binding_with_metadata(
            Arc::new(crate::mcp::tests::test_mcp_config(std::env::temp_dir())),
            /*plugins_available*/ false,
        )
        .await
}

fn create_test_server_info(title: &str) -> McpServerInfo {
    McpServerInfo {
        name: "codex-apps".to_string(),
        title: Some(title.to_string()),
        version: "1.0.0".to_string(),
        description: None,
        icons: None,
        website_url: None,
    }
}

struct TestInProcessTransportFactory;

impl InProcessTransportFactory for TestInProcessTransportFactory {
    fn open(&self) -> BoxFuture<'static, io::Result<DuplexStream>> {
        async {
            let (client_stream, _server_stream) = tokio::io::duplex(1);
            Ok(client_stream)
        }
        .boxed()
    }
}

#[derive(Clone)]
struct RefreshTestTransportFactory {
    tool: Tool,
    list_started: Option<Arc<Notify>>,
    release_list: Option<Arc<Notify>>,
}

impl ServerHandler for RefreshTestTransportFactory {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        if let Some(list_started) = &self.list_started {
            list_started.notify_one();
        }
        if let Some(release_list) = &self.release_list {
            release_list.notified().await;
        }
        Ok(ListToolsResult {
            tools: vec![self.tool.clone()],
            next_cursor: None,
            meta: None,
        })
    }
}

impl InProcessTransportFactory for RefreshTestTransportFactory {
    fn open(&self) -> BoxFuture<'static, io::Result<DuplexStream>> {
        let server = self.clone();
        async move {
            let (client_stream, server_stream) = tokio::io::duplex(4096);
            tokio::spawn(async move {
                let server = server
                    .serve(server_stream)
                    .await
                    .expect("serve test MCP server");
                server.waiting().await.expect("wait for test MCP server");
            });
            Ok(client_stream)
        }
        .boxed()
    }
}

#[derive(Clone)]
struct MutableToolsServer {
    tools: Arc<tokio::sync::RwLock<Vec<Tool>>>,
    block_tool_listing: Arc<AtomicBool>,
}

impl ServerHandler for MutableToolsServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
    }

    async fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, rmcp::ErrorData> {
        if self.block_tool_listing.load(Ordering::Acquire) {
            std::future::pending::<()>().await;
        }
        Ok(ListToolsResult {
            tools: self.tools.read().await.clone(),
            ..Default::default()
        })
    }
}

struct MutableToolsTransportFactory {
    server: MutableToolsServer,
}

impl InProcessTransportFactory for MutableToolsTransportFactory {
    fn open(&self) -> BoxFuture<'static, io::Result<DuplexStream>> {
        let server = self.server.clone();
        async move {
            let (client_stream, server_stream) = tokio::io::duplex(4096);
            tokio::spawn(async move {
                server
                    .serve(server_stream)
                    .await
                    .expect("serve mutable MCP tools")
                    .waiting()
                    .await
                    .expect("mutable MCP tools server completes");
            });
            Ok(client_stream)
        }
        .boxed()
    }
}

struct DisconnectingToolsTransportFactory {
    server: MutableToolsServer,
    disconnect: CancellationToken,
}

impl InProcessTransportFactory for DisconnectingToolsTransportFactory {
    fn open(&self) -> BoxFuture<'static, io::Result<DuplexStream>> {
        let server = self.server.clone();
        let disconnect = self.disconnect.clone();
        async move {
            let (client_stream, server_stream) = tokio::io::duplex(4096);
            tokio::spawn(async move {
                let server = server
                    .serve(server_stream)
                    .await
                    .expect("serve disconnecting MCP tools");
                let cancellation = server.cancellation_token();
                tokio::select! {
                    () = disconnect.cancelled() => cancellation.cancel(),
                    result = server.waiting() => {
                        result.expect("disconnecting MCP server should complete");
                    }
                }
            });
            Ok(client_stream)
        }
        .boxed()
    }
}

async fn create_test_managed_client(tools: Vec<ToolInfo>) -> ManagedClient {
    ManagedClient {
        client: Arc::new(
            RmcpClient::new_in_process_client(Arc::new(TestInProcessTransportFactory))
                .await
                .expect("create in-process RMCP client"),
        ),
        server_info: create_test_server_info("Ready"),
        tools,
        tool_timeout: None,
        server_instructions: None,
        server_supports_sandbox_state_meta_capability: false,
        codex_apps_tools_cache_context: None,
    }
}

async fn create_ready_async_managed_client(tools: Vec<ToolInfo>) -> AsyncManagedClient {
    AsyncManagedClient {
        client: futures::future::ready::<Result<ManagedClient, StartupOutcomeError>>(Ok(
            create_test_managed_client(tools).await,
        ))
        .boxed()
        .shared(),
        is_codex_apps_mcp_server: false,
        cached_server_info: None,
        codex_apps_tools_cache_context: None,
        tool_catalog_cache_context: None,
        startup_complete: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        startup_reconnect: None,
        cancel_token: CancellationToken::new(),
    }
}

async fn create_test_manager_with_ready_apps_client(
    cache_context: ConnectorRuntimeContext<ToolInfo>,
    tool_name: &str,
    list_started: Option<Arc<Notify>>,
    release_list: Option<Arc<Notify>>,
) -> anyhow::Result<Arc<McpConnectionSet>> {
    let tool = create_test_tool(CODEX_APPS_MCP_SERVER_NAME, tool_name);
    let client = Arc::new(
        RmcpClient::new_in_process_client(Arc::new(RefreshTestTransportFactory {
            tool: tool.tool.clone(),
            list_started,
            release_list,
        }))
        .await?,
    );
    client
        .initialize(
            InitializeRequestParams::new(
                ClientCapabilities::default(),
                Implementation::new("codex-test", "0.0.0-test"),
            )
            .with_protocol_version(ProtocolVersion::V_2025_06_18),
            Some(Duration::from_secs(5)),
            Box::new(|_, _| async { Err(anyhow!("unexpected elicitation")) }.boxed()),
        )
        .await?;

    let managed_client = ManagedClient {
        client,
        server_info: create_test_server_info("Codex Apps"),
        tools: vec![tool],
        tool_timeout: Some(Duration::from_secs(5)),
        server_instructions: None,
        server_supports_sandbox_state_meta_capability: false,
        codex_apps_tools_cache_context: Some(cache_context.clone()),
    };
    let approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    let permission_profile = Constrained::allow_any(PermissionProfile::default());
    let mut manager = McpConnectionSet::new_uninitialized(
        &approval_policy,
        &permission_profile,
        /*prefix_mcp_tool_names*/ true,
    );
    manager.insert_test_client(
        CODEX_APPS_MCP_SERVER_NAME.to_string(),
        AsyncManagedClient {
            client: futures::future::ready::<Result<ManagedClient, StartupOutcomeError>>(Ok(
                managed_client,
            ))
            .boxed()
            .shared(),
            is_codex_apps_mcp_server: true,
            cached_server_info: Some(create_test_server_info("Codex Apps")),
            codex_apps_tools_cache_context: Some(cache_context),
            tool_catalog_cache_context: None,
            startup_complete: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            startup_reconnect: None,
            cancel_token: CancellationToken::new(),
        },
    );
    manager.set_test_server_metadata(
        CODEX_APPS_MCP_SERVER_NAME,
        McpServerMetadata {
            environment_id: codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
            pollutes_memory: false,
            origin: None,
            supports_parallel_tool_calls: false,
            default_tools_approval_mode: None,
            tool_approval_modes: HashMap::new(),
        },
    );
    Ok(Arc::new(manager))
}

fn create_test_manager_with_failed_apps_startup(
    cached_tools: Vec<ToolInfo>,
    reconnect_factory: Arc<dyn Fn() -> ManagedClientFuture + Send + Sync>,
) -> McpConnectionSet {
    let client: ManagedClientFuture = futures::future::ready(Err(StartupOutcomeError::Failed {
        error: "startup failed".to_string(),
        is_authentication_required: false,
    }))
    .boxed()
    .shared();
    let codex_home = tempdir().expect("tempdir");
    let cache_context = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("reconnect-test-account"),
        Some("reconnect-test-user"),
    );
    store_current_tools(&cache_context, cached_tools);
    let approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    let permission_profile = Constrained::allow_any(PermissionProfile::default());
    let mut manager = McpConnectionSet::new_uninitialized(
        &approval_policy,
        &permission_profile,
        /*prefix_mcp_tool_names*/ true,
    );
    manager.insert_test_client(
        CODEX_APPS_MCP_SERVER_NAME.to_string(),
        AsyncManagedClient {
            client,
            is_codex_apps_mcp_server: true,
            cached_server_info: None,
            codex_apps_tools_cache_context: Some(cache_context),
            tool_catalog_cache_context: None,
            startup_complete: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            startup_reconnect: Some(Arc::new(CodexAppsStartupReconnect::new(reconnect_factory))),
            cancel_token: CancellationToken::new(),
        },
    );
    manager
}

fn model_tool_names(tools: &[ToolInfo]) -> HashSet<ToolName> {
    tools
        .iter()
        .map(ToolInfo::canonical_tool_name)
        .collect::<HashSet<_>>()
}

fn model_tool_name_len(name: &ToolName) -> usize {
    name.namespace
        .as_deref()
        .map_or(0, |namespace| namespace.len() + "__".len())
        + name.name.len()
}

fn is_code_mode_compatible_tool_name(name: &ToolName) -> bool {
    name.namespace
        .as_deref()
        .into_iter()
        .chain(std::iter::once(name.name.as_str()))
        .flat_map(str::chars)
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[test]
fn elicitation_granular_policy_defaults_to_prompting() {
    assert!(!elicitation_is_rejected_by_policy(
        AskForApproval::OnRequest
    ));
    assert!(!elicitation_is_rejected_by_policy(
        AskForApproval::UnlessTrusted
    ));
    assert!(elicitation_is_rejected_by_policy(AskForApproval::Granular(
        GranularApprovalConfig {
            sandbox_approval: true,
            rules: true,
            skill_approval: true,
            request_permissions: true,
            mcp_elicitations: false,
        }
    )));
}

#[test]
fn elicitation_granular_policy_respects_never_and_config() {
    assert!(elicitation_is_rejected_by_policy(AskForApproval::Never));
    assert!(elicitation_is_rejected_by_policy(AskForApproval::Granular(
        GranularApprovalConfig {
            sandbox_approval: true,
            rules: true,
            skill_approval: true,
            request_permissions: true,
            mcp_elicitations: false,
        }
    )));
}

#[tokio::test]
async fn disabled_permissions_auto_accept_elicitation_with_empty_form_schema() {
    let manager = ElicitationRequestManager::new(
        AskForApproval::Never,
        PermissionProfile::Disabled,
        /*reviewer*/ None,
        /*lifecycle*/ None,
        ElicitationRequestRouter::default(),
    );
    let (tx_event, _rx_event) = async_channel::bounded(1);
    let sender = manager.make_sender("server".to_string(), Some(tx_event));

    let response = sender(
        NumberOrString::Number(1),
        codex_rmcp_client::Elicitation::Mcp(
            CreateElicitationRequestParams::FormElicitationParams {
                meta: None,
                message: "Confirm?".to_string(),
                requested_schema: rmcp::model::ElicitationSchema::builder()
                    .build()
                    .expect("schema should build"),
            },
        ),
    )
    .await
    .expect("elicitation should auto accept");

    assert_eq!(
        response,
        ElicitationResponse {
            action: ElicitationAction::Accept,
            content: Some(serde_json::json!({})),
            meta: None,
        }
    );
}

#[tokio::test]
async fn disabled_permissions_do_not_auto_accept_elicitation_with_requested_fields() {
    let manager = ElicitationRequestManager::new(
        AskForApproval::Never,
        PermissionProfile::Disabled,
        /*reviewer*/ None,
        /*lifecycle*/ None,
        ElicitationRequestRouter::default(),
    );
    let (tx_event, _rx_event) = async_channel::bounded(1);
    let sender = manager.make_sender("server".to_string(), Some(tx_event));

    let response = sender(
        NumberOrString::Number(1),
        codex_rmcp_client::Elicitation::Mcp(
            CreateElicitationRequestParams::FormElicitationParams {
                meta: None,
                message: "What should I say?".to_string(),
                requested_schema: rmcp::model::ElicitationSchema::builder()
                    .required_property(
                        "message",
                        rmcp::model::PrimitiveSchema::String(rmcp::model::StringSchema::new()),
                    )
                    .build()
                    .expect("schema should build"),
            },
        ),
    )
    .await
    .expect("elicitation should auto decline");

    assert_eq!(
        response,
        ElicitationResponse {
            action: ElicitationAction::Decline,
            content: None,
            meta: None,
        }
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_authority_updates_never_auto_approve_mixed_policy() {
    let manager = ElicitationRequestManager::new(
        AskForApproval::Never,
        PermissionProfile::default(),
        /*reviewer*/ None,
        /*lifecycle*/ None,
        ElicitationRequestRouter::default(),
    );
    let updating_manager = manager.clone();
    let updater = tokio::spawn(async move {
        for _ in 0..1_000 {
            assert!(updating_manager.update(
                AskForApproval::OnRequest,
                PermissionProfile::Disabled,
                /*reviewer*/ None,
                /*lifecycle*/ None,
            ));
            assert!(updating_manager.update(
                AskForApproval::Never,
                PermissionProfile::default(),
                /*reviewer*/ None,
                /*lifecycle*/ None,
            ));
        }
    });
    let sender = manager.make_sender("server".to_string(), /*tx_event*/ None);
    let elicitation = codex_rmcp_client::Elicitation::Mcp(
        CreateElicitationRequestParams::FormElicitationParams {
            meta: None,
            message: "Confirm?".to_string(),
            requested_schema: rmcp::model::ElicitationSchema::builder()
                .build()
                .expect("schema should build"),
        },
    );

    for _ in 0..1_000 {
        let response = sender(NumberOrString::Number(1), elicitation.clone())
            .await
            .expect("elicitation should resolve");
        assert_eq!(
            response,
            ElicitationResponse {
                action: ElicitationAction::Decline,
                content: None,
                meta: None,
            }
        );
    }

    updater.await.expect("authority updates should finish");
}

#[tokio::test]
async fn shared_elicitation_router_targets_the_exact_pending_request() {
    struct Registration(Arc<AtomicUsize>);

    impl Drop for Registration {
        fn drop(&mut self) {
            self.0.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        }
    }

    let router = ElicitationRequestRouter::default();
    let outstanding = Arc::new(AtomicUsize::new(0));
    let lifecycle = ElicitationLifecycle::new({
        let outstanding = outstanding.clone();
        move || {
            outstanding.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Registration(outstanding.clone())
        }
    });
    let manager_a = ElicitationRequestManager::new(
        AskForApproval::OnRequest,
        PermissionProfile::default(),
        /*reviewer*/ None,
        Some(lifecycle.clone()),
        router.clone(),
    );
    let manager_b = ElicitationRequestManager::new(
        AskForApproval::OnRequest,
        PermissionProfile::default(),
        /*reviewer*/ None,
        Some(lifecycle),
        router.clone(),
    );
    let (tx_event, rx_event) = async_channel::bounded(2);
    let sender_a = manager_a.make_sender("server".to_string(), Some(tx_event.clone()));
    let sender_b = manager_b.make_sender("server".to_string(), Some(tx_event));
    let elicitation = codex_rmcp_client::Elicitation::Mcp(
        CreateElicitationRequestParams::FormElicitationParams {
            meta: None,
            message: "Which runtime?".to_string(),
            requested_schema: rmcp::model::ElicitationSchema::builder()
                .required_property(
                    "runtime",
                    rmcp::model::PrimitiveSchema::String(rmcp::model::StringSchema::new()),
                )
                .build()
                .expect("schema should build"),
        },
    );

    let pending_a = tokio::spawn(sender_a(NumberOrString::Number(1), elicitation.clone()));
    let EventMsg::ElicitationRequest(request_a) = rx_event.recv().await.expect("request A").msg
    else {
        panic!("expected elicitation request");
    };
    let pending_b = tokio::spawn(sender_b(NumberOrString::Number(1), elicitation));
    let EventMsg::ElicitationRequest(request_b) = rx_event.recv().await.expect("request B").msg
    else {
        panic!("expected elicitation request");
    };
    assert_eq!(outstanding.load(std::sync::atomic::Ordering::SeqCst), 2);
    let (
        codex_protocol::mcp::RequestId::String(request_a_id),
        codex_protocol::mcp::RequestId::String(request_b_id),
    ) = (request_a.id, request_b.id)
    else {
        panic!("expected Codex-owned string request IDs");
    };
    assert_ne!(request_a_id, request_b_id);

    let response_a = ElicitationResponse {
        action: ElicitationAction::Accept,
        content: Some(serde_json::json!({"runtime": "a"})),
        meta: None,
    };
    router
        .resolve(
            "server".to_string(),
            NumberOrString::String(request_a_id.into()),
            response_a.clone(),
        )
        .await
        .expect("runtime B should route a response to runtime A");
    let response_b = ElicitationResponse {
        action: ElicitationAction::Accept,
        content: Some(serde_json::json!({"runtime": "b"})),
        meta: None,
    };
    router
        .resolve(
            "server".to_string(),
            NumberOrString::String(request_b_id.into()),
            response_b.clone(),
        )
        .await
        .expect("runtime A should route a response to runtime B");

    assert_eq!(
        pending_a
            .await
            .expect("request A task")
            .expect("request A response"),
        response_a
    );
    assert_eq!(
        pending_b
            .await
            .expect("request B task")
            .expect("request B response"),
        response_b
    );
    assert_eq!(outstanding.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[test]
fn test_normalize_tools_short_non_duplicated_names() {
    let tools = vec![
        create_test_tool("server1", "tool1"),
        create_test_tool("server1", "tool2"),
    ];

    let model_tools =
        normalize_tools_for_model_with_prefix(tools, /*prefix_mcp_tool_names*/ true, &[]);

    assert_eq!(
        model_tool_names(&model_tools),
        HashSet::from([
            ToolName::namespaced("mcp__server1", "tool1"),
            ToolName::namespaced("mcp__server1", "tool2")
        ])
    );
}

#[test]
fn test_normalize_tools_omits_prefix_only_for_selected_servers() {
    let tools = vec![
        create_test_tool("history", "search"),
        create_test_tool("notes", "read"),
        create_test_tool("calendar", "list"),
    ];

    let model_tools = normalize_tools_for_model_with_prefix(
        tools,
        /*prefix_mcp_tool_names*/ true,
        &["history".to_string(), "notes".to_string()],
    );

    assert_eq!(
        model_tool_names(&model_tools),
        HashSet::from([
            ToolName::namespaced("history", "search"),
            ToolName::namespaced("notes", "read"),
            ToolName::namespaced("mcp__calendar", "list"),
        ])
    );
}

#[test]
fn test_normalize_tools_selects_raw_server_name() {
    let mut tool = create_test_tool("codex_apps", "search");
    tool.callable_namespace = "codex_apps__calendar".to_string();

    let model_tools = normalize_tools_for_model_with_prefix(
        vec![tool],
        /*prefix_mcp_tool_names*/ true,
        &["codex_apps".to_string()],
    );

    assert_eq!(
        model_tool_names(&model_tools),
        HashSet::from([ToolName::namespaced("codex_apps__calendar", "search")])
    );
}

#[test]
fn test_normalize_tools_global_feature_omits_prefix_for_every_server() {
    let tools = vec![
        create_test_tool("history", "search"),
        create_test_tool("calendar", "list"),
    ];

    let model_tools = normalize_tools_for_model_with_prefix(
        tools,
        /*prefix_mcp_tool_names*/ false,
        &["history".to_string()],
    );

    assert_eq!(
        model_tool_names(&model_tools),
        HashSet::from([
            ToolName::namespaced("history", "search"),
            ToolName::namespaced("calendar", "list"),
        ])
    );
}

#[test]
fn test_normalize_tools_duplicated_names_skipped() {
    let tools = vec![
        create_test_tool("server1", "duplicate_tool"),
        create_test_tool("server1", "duplicate_tool"),
    ];

    let model_tools =
        normalize_tools_for_model_with_prefix(tools, /*prefix_mcp_tool_names*/ true, &[]);

    // Only the first tool should remain, the second is skipped
    assert_eq!(
        model_tool_names(&model_tools),
        HashSet::from([ToolName::namespaced("mcp__server1", "duplicate_tool")])
    );
}

#[test]
fn test_normalize_tools_long_names_same_server() {
    let server_name = "my_server";

    let tools = vec![
        create_test_tool(
            server_name,
            "extremely_lengthy_function_name_that_absolutely_surpasses_all_reasonable_limits",
        ),
        create_test_tool(
            server_name,
            "yet_another_extremely_lengthy_function_name_that_absolutely_surpasses_all_reasonable_limits",
        ),
    ];

    let model_tools =
        normalize_tools_for_model_with_prefix(tools, /*prefix_mcp_tool_names*/ true, &[]);

    assert_eq!(model_tools.len(), 2);

    let names = model_tool_names(&model_tools);

    assert!(names.iter().all(|name| model_tool_name_len(name) == 64));
    assert!(
        names
            .iter()
            .all(|name| name.namespace.as_deref() == Some("mcp__my_server"))
    );
    assert!(
        names.iter().all(is_code_mode_compatible_tool_name),
        "model-visible names must be code-mode compatible: {names:?}"
    );
}

#[test]
fn test_normalize_tools_sanitizes_invalid_characters() {
    let tools = vec![create_test_tool("server.one", "tool.two-three")];

    let model_tools =
        normalize_tools_for_model_with_prefix(tools, /*prefix_mcp_tool_names*/ true, &[]);

    assert_eq!(model_tools.len(), 1);
    let tool = model_tools.into_iter().next().expect("one tool");
    let model_name = tool.canonical_tool_name();
    assert_eq!(
        model_name,
        ToolName::namespaced("mcp__server_one", "tool_two_three")
    );
    assert_eq!(
        ToolName::namespaced(tool.callable_namespace.clone(), tool.callable_name.clone()),
        model_name
    );
    // The callable parts are sanitized for model-visible tool calls, but the raw
    // MCP name is preserved for the actual MCP call.
    assert_eq!(tool.server_name, "server.one");
    assert_eq!(tool.callable_namespace, "mcp__server_one");
    assert_eq!(tool.callable_name, "tool_two_three");
    assert_eq!(tool.tool.name, "tool.two-three");

    assert!(
        is_code_mode_compatible_tool_name(&model_name),
        "model-visible name must be code-mode compatible: {model_name:?}"
    );
}

#[test]
fn test_normalize_tools_keeps_hyphenated_mcp_tools_callable() {
    let tools = vec![create_test_tool("music-studio", "get-strudel-guide")];

    let model_tools =
        normalize_tools_for_model_with_prefix(tools, /*prefix_mcp_tool_names*/ true, &[]);

    assert_eq!(model_tools.len(), 1);
    let tool = model_tools.into_iter().next().expect("one tool");
    assert_eq!(
        tool.canonical_tool_name(),
        ToolName::namespaced("mcp__music_studio", "get_strudel_guide")
    );
    assert_eq!(tool.callable_namespace, "mcp__music_studio");
    assert_eq!(tool.callable_name, "get_strudel_guide");
    assert_eq!(tool.tool.name, "get-strudel-guide");
}

#[test]
fn test_normalize_tools_disambiguates_sanitized_namespace_collisions() {
    let tools = vec![
        create_test_tool("basic-server", "lookup"),
        create_test_tool("basic_server", "query"),
    ];

    let model_tools =
        normalize_tools_for_model_with_prefix(tools, /*prefix_mcp_tool_names*/ true, &[]);

    assert_eq!(model_tools.len(), 2);
    let mut namespaces = model_tools
        .iter()
        .map(|tool| tool.callable_namespace.as_str())
        .collect::<Vec<_>>();
    namespaces.sort();
    namespaces.dedup();
    assert_eq!(namespaces.len(), 2);

    let raw_servers = model_tools
        .iter()
        .map(|tool| tool.server_name.as_str())
        .collect::<HashSet<_>>();
    assert_eq!(raw_servers, HashSet::from(["basic-server", "basic_server"]));
    let model_names = model_tool_names(&model_tools);
    assert!(
        model_names.iter().all(is_code_mode_compatible_tool_name),
        "model-visible names must be code-mode compatible: {model_names:?}"
    );
}

#[test]
fn test_normalize_tools_disambiguates_sanitized_tool_name_collisions() {
    let tools = vec![
        create_test_tool("server", "tool-name"),
        create_test_tool("server", "tool_name"),
    ];

    let model_tools =
        normalize_tools_for_model_with_prefix(tools, /*prefix_mcp_tool_names*/ true, &[]);

    assert_eq!(model_tools.len(), 2);
    let raw_tool_names = model_tools
        .iter()
        .map(|tool| tool.tool.name.to_string())
        .collect::<HashSet<_>>();
    assert_eq!(
        raw_tool_names,
        HashSet::from(["tool-name".to_string(), "tool_name".to_string()])
    );
    let callable_tool_names = model_tools
        .iter()
        .map(|tool| tool.callable_name.as_str())
        .collect::<HashSet<_>>();
    assert_eq!(callable_tool_names.len(), 2);
}

#[test]
fn tool_filter_allows_by_default() {
    let filter = ToolFilter::default();

    assert!(filter.allows("any"));
}

#[test]
fn tool_filter_applies_enabled_list() {
    let filter = ToolFilter {
        enabled: Some(HashSet::from(["allowed".to_string()])),
        disabled: HashSet::new(),
    };

    assert!(filter.allows("allowed"));
    assert!(!filter.allows("denied"));
}

#[test]
fn tool_filter_applies_disabled_list() {
    let filter = ToolFilter {
        enabled: None,
        disabled: HashSet::from(["blocked".to_string()]),
    };

    assert!(!filter.allows("blocked"));
    assert!(filter.allows("open"));
}

#[test]
fn tool_filter_applies_enabled_then_disabled() {
    let filter = ToolFilter {
        enabled: Some(HashSet::from(["keep".to_string(), "remove".to_string()])),
        disabled: HashSet::from(["remove".to_string()]),
    };

    assert!(filter.allows("keep"));
    assert!(!filter.allows("remove"));
    assert!(!filter.allows("unknown"));
}

#[test]
fn filter_tools_applies_per_server_filters() {
    let server1_tools = vec![
        create_test_tool("server1", "tool_a"),
        create_test_tool("server1", "tool_b"),
    ];
    let server2_tools = vec![create_test_tool("server2", "tool_a")];
    let server1_filter = ToolFilter {
        enabled: Some(HashSet::from(["tool_a".to_string(), "tool_b".to_string()])),
        disabled: HashSet::from(["tool_b".to_string()]),
    };
    let server2_filter = ToolFilter {
        enabled: None,
        disabled: HashSet::from(["tool_a".to_string()]),
    };

    let filtered: Vec<_> = filter_tools(server1_tools, &server1_filter)
        .into_iter()
        .chain(filter_tools(server2_tools, &server2_filter))
        .collect();

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].server_name, "server1");
    assert_eq!(filtered[0].callable_name, "tool_a");
}

#[test]
fn codex_apps_env_bearer_token_bypasses_shared_tools_cache() {
    assert!(!should_share_codex_apps_tools_cache(
        CODEX_APPS_MCP_SERVER_NAME,
        /*uses_env_bearer_token*/ true,
    ));
}

#[tokio::test]
async fn list_all_tools_uses_shared_codex_apps_cache_while_client_is_pending() {
    let codex_home = tempdir().expect("tempdir");
    let cache_context = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-one"),
        Some("user-one"),
    );
    store_current_tools(
        &cache_context,
        vec![create_test_tool(
            CODEX_APPS_MCP_SERVER_NAME,
            "calendar_create_event",
        )],
    );
    let pending_client = futures::future::pending::<Result<ManagedClient, StartupOutcomeError>>()
        .boxed()
        .shared();
    let approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    let permission_profile = Constrained::allow_any(PermissionProfile::default());
    let mut manager = McpConnectionSet::new_uninitialized(
        &approval_policy,
        &permission_profile,
        /*prefix_mcp_tool_names*/ true,
    );
    manager.insert_test_client(
        CODEX_APPS_MCP_SERVER_NAME.to_string(),
        AsyncManagedClient {
            client: pending_client,
            is_codex_apps_mcp_server: true,
            cached_server_info: None,
            codex_apps_tools_cache_context: Some(cache_context),
            tool_catalog_cache_context: None,
            startup_complete: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            startup_reconnect: None,
            cancel_token: CancellationToken::new(),
        },
    );

    let tools = manager.list_all_tools().await;
    let tool = tools
        .iter()
        .find(|tool| {
            tool.canonical_tool_name()
                == ToolName::namespaced("mcp__codex_apps", "calendar_create_event")
        })
        .expect("tool from shared cache");
    assert_eq!(tool.server_name, CODEX_APPS_MCP_SERVER_NAME);
    assert_eq!(tool.callable_name, "calendar_create_event");
}

#[tokio::test]
async fn capture_binding_uses_the_ready_clients_own_tools() {
    let codex_home = tempdir().expect("tempdir");
    let cache_context = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-one"),
        Some("user-one"),
    );
    store_current_tools(
        &cache_context,
        vec![create_test_tool(
            CODEX_APPS_MCP_SERVER_NAME,
            "shared_cached_tool",
        )],
    );
    let mut ready_client = create_test_managed_client(vec![
        create_test_tool(CODEX_APPS_MCP_SERVER_NAME, "client_local_tool"),
        create_test_tool(CODEX_APPS_MCP_SERVER_NAME, "client_local_blocked"),
    ])
    .await;
    let tool_filter = ToolFilter {
        enabled: None,
        disabled: HashSet::from(["client_local_blocked".to_string()]),
    };
    ready_client.codex_apps_tools_cache_context = Some(cache_context.clone());
    let approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    let permission_profile = Constrained::allow_any(PermissionProfile::default());
    let mut manager = McpConnectionSet::new_uninitialized(
        &approval_policy,
        &permission_profile,
        /*prefix_mcp_tool_names*/ true,
    );
    manager.insert_test_client(
        CODEX_APPS_MCP_SERVER_NAME.to_string(),
        AsyncManagedClient {
            client: futures::future::ready(Ok(ready_client)).boxed().shared(),
            is_codex_apps_mcp_server: true,
            cached_server_info: None,
            codex_apps_tools_cache_context: Some(cache_context),
            tool_catalog_cache_context: None,
            startup_complete: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            startup_reconnect: None,
            cancel_token: CancellationToken::new(),
        },
    );
    manager
        .servers
        .get_mut(CODEX_APPS_MCP_SERVER_NAME)
        .expect("test server exists")
        .tool_filter = tool_filter;
    manager.set_test_server_metadata(
        CODEX_APPS_MCP_SERVER_NAME,
        McpServerMetadata {
            environment_id: codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
            pollutes_memory: false,
            origin: None,
            supports_parallel_tool_calls: false,
            default_tools_approval_mode: None,
            tool_approval_modes: HashMap::new(),
        },
    );
    let manager = Arc::new(manager);

    assert_eq!(
        manager
            .list_all_tools()
            .await
            .iter()
            .map(|tool| tool.callable_name.as_str())
            .collect::<Vec<_>>(),
        vec!["shared_cached_tool"]
    );
    let step = capture_binding(&manager).await;
    assert_eq!(
        step.tools()
            .iter()
            .map(|tool| tool.callable_name.as_str())
            .collect::<Vec<_>>(),
        vec!["client_local_tool"]
    );
    assert!(
        step.prepare_call(CODEX_APPS_MCP_SERVER_NAME, "client_local_tool")
            .is_some()
    );
    assert!(
        step.prepare_call(CODEX_APPS_MCP_SERVER_NAME, "shared_cached_tool")
            .is_none()
    );
    assert!(
        step.prepare_call(CODEX_APPS_MCP_SERVER_NAME, "client_local_blocked")
            .is_none()
    );
}

#[tokio::test]
async fn hard_refresh_keeps_binding_override_local_when_shared_cache_loses_race()
-> anyhow::Result<()> {
    let codex_home = tempdir()?;
    let shared_cache = ConnectorRuntimeManager::<ToolInfo>::default();
    let cache_key = ConnectorRuntimeContextKey::personal(
        Some("shared-account".to_string()),
        Some("shared-user".to_string()),
    );
    let cache_context_a = shared_cache.context(codex_home.path().to_path_buf(), cache_key.clone());
    let cache_context_b = shared_cache.context(codex_home.path().to_path_buf(), cache_key);
    let list_started = Arc::new(Notify::new());
    let release_list = Arc::new(Notify::new());
    let manager_a = create_test_manager_with_ready_apps_client(
        cache_context_a.clone(),
        "a_only",
        Some(Arc::clone(&list_started)),
        Some(Arc::clone(&release_list)),
    )
    .await?;
    let manager_b = create_test_manager_with_ready_apps_client(
        cache_context_b,
        "b_only",
        /*list_started*/ None,
        /*release_list*/ None,
    )
    .await?;

    let manager_a_for_refresh = Arc::clone(&manager_a);
    let refresh_a = tokio::spawn(async move {
        manager_a_for_refresh
            .hard_refresh_codex_apps_tools_cache()
            .await
    });
    list_started.notified().await;
    let tools_b = manager_b.hard_refresh_codex_apps_tools_cache().await?;
    release_list.notify_one();
    let tools_a = refresh_a.await??;

    assert_eq!(
        tools_b
            .iter()
            .map(|tool| tool.callable_name.as_str())
            .collect::<Vec<_>>(),
        vec!["b_only"]
    );
    assert_eq!(
        tools_a
            .iter()
            .map(|tool| tool.callable_name.as_str())
            .collect::<Vec<_>>(),
        vec!["b_only"]
    );
    assert_eq!(
        cache_context_a
            .current_tools()
            .expect("shared cache tools")
            .iter()
            .map(|tool| tool.callable_name.as_str())
            .collect::<Vec<_>>(),
        vec!["b_only"]
    );
    assert_eq!(
        capture_binding(&manager_a)
            .await
            .tools()
            .iter()
            .map(|tool| tool.callable_name.as_str())
            .collect::<Vec<_>>(),
        vec!["a_only"]
    );
    assert_eq!(
        capture_binding(&manager_b)
            .await
            .tools()
            .iter()
            .map(|tool| tool.callable_name.as_str())
            .collect::<Vec<_>>(),
        vec!["b_only"]
    );
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn tool_catalog_cache_sanitizes_tools_and_tracks_environment_generation() {
    let cache = McpToolCatalogCache::default();
    let environment_manager = Arc::new(EnvironmentManager::without_environments());
    let replace_environment = |url: &str| {
        environment_manager
            .upsert_environment(
                "remote".to_string(),
                url.to_string(),
                /*connect_timeout*/ None,
            )
            .expect("replace environment");
    };
    replace_environment("ws://127.0.0.1:1");
    let runtime_context =
        McpRuntimeContext::new(Arc::clone(&environment_manager), PathBuf::from("/tmp"));
    let config: McpServerConfig = serde_json::from_value(serde_json::json!({
        "command": "docs-mcp",
        "environment_id": "remote"
    }))
    .expect("MCP config");
    let resolve_environment = || {
        runtime_context
            .resolve_server_environment("docs", &config)
            .expect("resolve environment")
            .expect("remote environment")
    };
    let cache_context = |environment: &Arc<codex_exec_server::Environment>| {
        cache
            .context(
                "docs",
                &config,
                &runtime_context,
                Some(environment),
                &ElicitationCapability::default(),
                /*supports_openai_form_elicitation*/ false,
            )
            .expect("cache context")
    };
    let first_environment = resolve_environment();
    let first_environment_weak = Arc::downgrade(&first_environment);
    let first_context = cache_context(&first_environment);
    let mut tool = create_test_tool("docs", "search");
    tool.tool.annotations = Some(rmcp::model::ToolAnnotations::new().read_only(true));
    first_context.publish_if_newest(first_context.begin_fetch(), &[tool]);
    assert_eq!(
        first_context.current_tools().expect("cached tools")[0]
            .tool
            .annotations,
        None
    );

    drop(first_environment);
    replace_environment("ws://127.0.0.1:2");
    assert!(first_environment_weak.upgrade().is_none());
    let replacement_environment = resolve_environment();
    assert!(!cache_context(&replacement_environment).has_tools());

    let older = first_context.begin_fetch();
    let newer = first_context.begin_fetch();
    first_context.publish_if_newest(newer, &[create_test_tool("docs", "new")]);
    first_context.publish_if_newest(older, &[create_test_tool("docs", "old")]);
    assert_eq!(
        first_context.current_tools().expect("cached tools")[0].callable_name,
        "new"
    );

    tokio::time::advance(Duration::from_secs(30 * 60 + 1)).await;
    assert!(!first_context.has_tools());
}

#[test]
fn tool_catalog_cache_bypasses_remote_sourced_environment_variables() {
    let cache = McpToolCatalogCache::default();
    let runtime_context = McpRuntimeContext::new(
        Arc::new(EnvironmentManager::without_environments()),
        PathBuf::from("/tmp"),
    );
    let config: McpServerConfig = serde_json::from_value(serde_json::json!({
        "command": "docs-mcp",
        "env_vars": [McpServerEnvVar::Config {
            name: "DOCS_TOKEN".to_string(),
            source: Some("remote".to_string()),
        }],
    }))
    .expect("MCP config");

    assert!(
        cache
            .context(
                "docs",
                &config,
                &runtime_context,
                /*resolved_environment*/ None,
                &ElicitationCapability::default(),
                /*supports_openai_form_elicitation*/ false,
            )
            .is_none()
    );
}

#[tokio::test]
async fn list_available_server_infos_uses_cache_while_client_is_pending() {
    let pending_client = futures::future::pending::<Result<ManagedClient, StartupOutcomeError>>()
        .boxed()
        .shared();
    let approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    let permission_profile = Constrained::allow_any(PermissionProfile::default());
    let mut manager = McpConnectionSet::new_uninitialized(
        &approval_policy,
        &permission_profile,
        /*prefix_mcp_tool_names*/ true,
    );
    let server_info = create_test_server_info("Codex Apps");
    manager.insert_test_client(
        CODEX_APPS_MCP_SERVER_NAME.to_string(),
        AsyncManagedClient {
            client: pending_client,
            is_codex_apps_mcp_server: true,
            cached_server_info: Some(server_info.clone()),
            codex_apps_tools_cache_context: None,
            tool_catalog_cache_context: None,
            startup_complete: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            startup_reconnect: None,
            cancel_token: CancellationToken::new(),
        },
    );

    let timeout_result = tokio::time::timeout(
        Duration::from_millis(10),
        manager.list_available_server_infos(),
    )
    .await;
    let server_infos = timeout_result.expect("server info lookup should not block on startup");
    assert_eq!(
        server_infos.get(CODEX_APPS_MCP_SERVER_NAME),
        Some(&server_info)
    );
}

#[tokio::test]
async fn list_all_tools_accepts_canonical_namespaced_tool_names() {
    let managed_client =
        create_ready_async_managed_client(vec![create_test_tool("rmcp", "echo")]).await;
    let approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    let permission_profile = Constrained::allow_any(PermissionProfile::default());
    let mut manager = McpConnectionSet::new_uninitialized(
        &approval_policy,
        &permission_profile,
        /*prefix_mcp_tool_names*/ false,
    );
    manager.insert_test_client("rmcp", managed_client);

    let tools = manager.list_all_tools().await;
    let tool = tools
        .iter()
        .find(|tool| tool.canonical_tool_name() == ToolName::namespaced("rmcp", "echo"))
        .expect("split MCP tool namespace and name should resolve");

    let expected = ("rmcp", "rmcp", "echo", "echo");
    assert_eq!(
        (
            tool.server_name.as_str(),
            tool.callable_namespace.as_str(),
            tool.callable_name.as_str(),
            tool.tool.name.as_ref(),
        ),
        expected
    );
}

#[tokio::test]
async fn capture_binding_waits_for_fresh_startup_even_with_cached_tools() {
    let codex_home = tempdir().expect("tempdir");
    let cache_context = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-one"),
        Some("user-one"),
    );
    store_current_tools(
        &cache_context,
        vec![create_test_tool(
            CODEX_APPS_MCP_SERVER_NAME,
            "shared_cached_tool",
        )],
    );
    let startup_complete = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let startup_complete_for_client = Arc::clone(&startup_complete);
    let (startup_started, wait_for_startup) = tokio::sync::oneshot::channel();
    let (release_startup, startup_released) = tokio::sync::oneshot::channel();
    let pending_client = async move {
        startup_started.send(()).expect("signal client startup");
        startup_released.await.expect("release client startup");
        startup_complete_for_client.store(true, std::sync::atomic::Ordering::Release);
        Ok(create_test_managed_client(vec![create_test_tool(
            CODEX_APPS_MCP_SERVER_NAME,
            "client_local_tool",
        )])
        .await)
    }
    .boxed()
    .shared();
    let approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    let permission_profile = Constrained::allow_any(PermissionProfile::default());
    let mut manager = McpConnectionSet::new_uninitialized(
        &approval_policy,
        &permission_profile,
        /*prefix_mcp_tool_names*/ true,
    );
    manager.insert_test_client(
        CODEX_APPS_MCP_SERVER_NAME.to_string(),
        AsyncManagedClient {
            client: pending_client,
            is_codex_apps_mcp_server: true,
            cached_server_info: None,
            codex_apps_tools_cache_context: Some(cache_context),
            tool_catalog_cache_context: None,
            startup_complete,
            startup_reconnect: None,
            cancel_token: CancellationToken::new(),
        },
    );
    manager.set_test_server_metadata(
        CODEX_APPS_MCP_SERVER_NAME,
        McpServerMetadata {
            environment_id: codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
            pollutes_memory: false,
            origin: None,
            supports_parallel_tool_calls: false,
            default_tools_approval_mode: None,
            tool_approval_modes: HashMap::new(),
        },
    );
    let manager = Arc::new(manager);
    let manager_for_capture = Arc::clone(&manager);
    let capture = tokio::spawn(async move { capture_binding(&manager_for_capture).await });

    wait_for_startup.await.expect("client startup should begin");
    assert!(!capture.is_finished());
    release_startup.send(()).expect("release client startup");

    let step = capture.await.expect("capture task");
    assert_eq!(
        step.tools()
            .iter()
            .map(|tool| tool.callable_name.as_str())
            .collect::<Vec<_>>(),
        vec!["client_local_tool"]
    );
}

#[tokio::test]
async fn list_all_tools_applies_legacy_mcp_prefix_by_default() {
    let managed_client =
        create_ready_async_managed_client(vec![create_test_tool("rmcp", "echo")]).await;
    let approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    let permission_profile = Constrained::allow_any(PermissionProfile::default());
    let mut manager = McpConnectionSet::new_uninitialized(
        &approval_policy,
        &permission_profile,
        /*prefix_mcp_tool_names*/ true,
    );
    manager.insert_test_client("rmcp", managed_client);

    let tools = manager.list_all_tools().await;
    let tool = tools
        .iter()
        .find(|tool| tool.canonical_tool_name() == ToolName::namespaced("mcp__rmcp", "echo"))
        .expect("legacy-prefixed MCP tool name should resolve");

    let expected = ("rmcp", "mcp__rmcp", "echo", "echo");
    assert_eq!(
        (
            tool.server_name.as_str(),
            tool.callable_namespace.as_str(),
            tool.callable_name.as_str(),
            tool.tool.name.as_ref(),
        ),
        expected
    );
}

#[tokio::test]
async fn list_all_tools_blocks_while_client_is_pending_without_cached_tools() {
    let pending_client = futures::future::pending::<Result<ManagedClient, StartupOutcomeError>>()
        .boxed()
        .shared();
    let approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    let permission_profile = Constrained::allow_any(PermissionProfile::default());
    let mut manager = McpConnectionSet::new_uninitialized(
        &approval_policy,
        &permission_profile,
        /*prefix_mcp_tool_names*/ true,
    );
    manager.insert_test_client(
        CODEX_APPS_MCP_SERVER_NAME.to_string(),
        AsyncManagedClient {
            client: pending_client,
            is_codex_apps_mcp_server: true,
            cached_server_info: None,
            codex_apps_tools_cache_context: None,
            tool_catalog_cache_context: None,
            startup_complete: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            startup_reconnect: None,
            cancel_token: CancellationToken::new(),
        },
    );

    let timeout_result =
        tokio::time::timeout(Duration::from_millis(10), manager.list_all_tools()).await;
    assert!(timeout_result.is_err());
}

#[tokio::test]
async fn cancelling_startup_does_not_disable_a_ready_client() {
    let client = create_ready_async_managed_client(vec![create_test_tool("ready", "search")]).await;

    client.cancel_token.cancel();

    let managed = client
        .client()
        .await
        .expect("startup cancellation should not disable a ready client");
    assert_eq!(
        model_tool_names(&managed.tools),
        HashSet::from([ToolName::namespaced("ready", "search")])
    );
}

#[tokio::test]
async fn shutdown_cancels_pending_tool_listing() {
    let cancel_token = CancellationToken::new();
    let cancel_token_for_startup = cancel_token.clone();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let pending_client = async move {
        let _ = started_tx.send(());
        cancel_token_for_startup.cancelled().await;
        Err(StartupOutcomeError::Cancelled)
    }
    .boxed()
    .shared();
    let approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    let permission_profile = Constrained::allow_any(PermissionProfile::default());
    let mut manager = McpConnectionSet::new_uninitialized(
        &approval_policy,
        &permission_profile,
        /*prefix_mcp_tool_names*/ true,
    );
    manager.insert_test_client(
        CODEX_APPS_MCP_SERVER_NAME.to_string(),
        AsyncManagedClient {
            client: pending_client,
            is_codex_apps_mcp_server: true,
            cached_server_info: None,
            codex_apps_tools_cache_context: None,
            tool_catalog_cache_context: None,
            startup_complete: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            startup_reconnect: None,
            cancel_token,
        },
    );
    let manager = Arc::new(manager);
    let manager_for_list = Arc::clone(&manager);
    let list_task = tokio::spawn(async move { manager_for_list.list_all_tools().await });

    started_rx.await.expect("tool listing should start");
    tokio::time::timeout(Duration::from_secs(1), manager.shutdown())
        .await
        .expect("shutdown should cancel speculative tool listing");
    let tools = list_task.await.expect("tool listing task should not panic");
    assert!(tools.is_empty());
}

#[tokio::test]
async fn shutdown_continues_after_caller_is_aborted() {
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (completed_tx, completed_rx) = tokio::sync::oneshot::channel();
    let release = Arc::new(tokio::sync::Notify::new());
    let release_for_client = Arc::clone(&release);
    let blocking_client = async move {
        let _ = started_tx.send(());
        release_for_client.notified().await;
        let _ = completed_tx.send(());
        Err(StartupOutcomeError::Cancelled)
    }
    .boxed()
    .shared();
    let approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    let permission_profile = Constrained::allow_any(PermissionProfile::default());
    let mut manager = McpConnectionSet::new_uninitialized(
        &approval_policy,
        &permission_profile,
        /*prefix_mcp_tool_names*/ true,
    );
    manager.insert_test_client(
        CODEX_APPS_MCP_SERVER_NAME.to_string(),
        AsyncManagedClient {
            client: blocking_client,
            is_codex_apps_mcp_server: true,
            cached_server_info: None,
            codex_apps_tools_cache_context: None,
            tool_catalog_cache_context: None,
            startup_complete: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            startup_reconnect: None,
            cancel_token: CancellationToken::new(),
        },
    );
    let manager = Arc::new(manager);
    let shutdown_task = tokio::spawn({
        let manager = Arc::clone(&manager);
        async move { manager.shutdown().await }
    });

    started_rx.await.expect("client shutdown should start");
    shutdown_task.abort();
    let shutdown_error = shutdown_task
        .await
        .expect_err("caller shutdown task should be aborted");
    assert!(shutdown_error.is_cancelled());
    release.notify_one();

    tokio::time::timeout(Duration::from_secs(1), completed_rx)
        .await
        .expect("client shutdown should survive caller cancellation")
        .expect("client shutdown completion sender should stay alive");
}

#[tokio::test]
async fn list_all_tools_does_not_block_when_shared_codex_apps_cache_is_empty() {
    let codex_home = tempdir().expect("tempdir");
    let cache_context = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-one"),
        Some("user-one"),
    );
    store_current_tools(&cache_context, Vec::new());
    let pending_client = futures::future::pending::<Result<ManagedClient, StartupOutcomeError>>()
        .boxed()
        .shared();
    let approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    let permission_profile = Constrained::allow_any(PermissionProfile::default());
    let mut manager = McpConnectionSet::new_uninitialized(
        &approval_policy,
        &permission_profile,
        /*prefix_mcp_tool_names*/ true,
    );
    manager.insert_test_client(
        CODEX_APPS_MCP_SERVER_NAME.to_string(),
        AsyncManagedClient {
            client: pending_client,
            is_codex_apps_mcp_server: true,
            cached_server_info: None,
            codex_apps_tools_cache_context: Some(cache_context),
            tool_catalog_cache_context: None,
            startup_complete: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            startup_reconnect: None,
            cancel_token: CancellationToken::new(),
        },
    );

    let timeout_result =
        tokio::time::timeout(Duration::from_millis(10), manager.list_all_tools()).await;
    let tools = timeout_result.expect("shared empty cache should not block");
    assert!(tools.is_empty());
}

#[tokio::test]
async fn list_all_tools_uses_shared_codex_apps_cache_when_client_startup_fails() {
    let codex_home = tempdir().expect("tempdir");
    let cache_context = create_codex_apps_tools_cache_context(
        codex_home.path().to_path_buf(),
        Some("account-one"),
        Some("user-one"),
    );
    store_current_tools(
        &cache_context,
        vec![create_test_tool(
            CODEX_APPS_MCP_SERVER_NAME,
            "calendar_create_event",
        )],
    );
    let server_info = create_test_server_info("Codex Apps");
    let failed_client = futures::future::ready::<Result<ManagedClient, StartupOutcomeError>>(Err(
        StartupOutcomeError::Failed {
            error: "startup failed".to_string(),
            is_authentication_required: false,
        },
    ))
    .boxed()
    .shared();
    let approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    let permission_profile = Constrained::allow_any(PermissionProfile::default());
    let mut manager = McpConnectionSet::new_uninitialized(
        &approval_policy,
        &permission_profile,
        /*prefix_mcp_tool_names*/ true,
    );
    let startup_complete = Arc::new(std::sync::atomic::AtomicBool::new(true));
    manager.insert_test_client(
        CODEX_APPS_MCP_SERVER_NAME.to_string(),
        AsyncManagedClient {
            client: failed_client,
            is_codex_apps_mcp_server: true,
            cached_server_info: Some(server_info.clone()),
            codex_apps_tools_cache_context: Some(cache_context),
            tool_catalog_cache_context: None,
            startup_complete,
            startup_reconnect: None,
            cancel_token: CancellationToken::new(),
        },
    );

    let tools = manager.list_all_tools().await;
    let tool = tools
        .iter()
        .find(|tool| {
            tool.canonical_tool_name()
                == ToolName::namespaced("mcp__codex_apps", "calendar_create_event")
        })
        .expect("tool from shared cache");
    assert_eq!(tool.server_name, CODEX_APPS_MCP_SERVER_NAME);
    assert_eq!(tool.callable_name, "calendar_create_event");
    assert_eq!(
        manager
            .list_available_server_infos()
            .await
            .get(CODEX_APPS_MCP_SERVER_NAME),
        Some(&server_info)
    );
}

#[tokio::test]
async fn list_all_tools_reconnects_failed_codex_apps_startup_and_reuses_client() {
    let recovered_client = create_test_managed_client(vec![create_test_tool(
        CODEX_APPS_MCP_SERVER_NAME,
        "drive_search",
    )])
    .await;
    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_for_reconnect = Arc::clone(&attempts);
    let reconnect_finished = Arc::new(tokio::sync::Notify::new());
    let reconnect_finished_for_factory = Arc::clone(&reconnect_finished);
    let reconnect_factory = Arc::new(move || {
        attempts_for_reconnect.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let reconnect_finished = Arc::clone(&reconnect_finished_for_factory);
        let recovered_client = recovered_client.clone();
        async move {
            reconnect_finished.notify_one();
            Ok(recovered_client)
        }
        .boxed()
        .shared()
    });
    let mut manager = create_test_manager_with_failed_apps_startup(Vec::new(), reconnect_factory);
    manager
        .servers
        .get_mut(CODEX_APPS_MCP_SERVER_NAME)
        .expect("test server exists")
        .metadata = McpServerMetadata {
        environment_id: codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
        pollutes_memory: false,
        origin: None,
        supports_parallel_tool_calls: false,
        default_tools_approval_mode: None,
        tool_approval_modes: HashMap::new(),
    };
    let manager = Arc::new(manager);

    let reconnect_finished_wait = reconnect_finished.notified();
    let tools = manager.list_all_tools().await;
    assert!(tools.is_empty());
    reconnect_finished_wait.await;

    let tools = manager.list_all_tools().await;
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool.callable_name.as_str())
            .collect::<Vec<_>>(),
        vec!["drive_search"]
    );
    assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 1);

    let step = capture_binding(&manager).await;
    let prepared = step
        .prepare_call(CODEX_APPS_MCP_SERVER_NAME, "drive_search")
        .expect("recovered tool should have a prepared call");
    assert!(
        !prepared
            .server_supports_sandbox_state_meta_capability()
            .await
            .expect("prepared call should use the recovered client")
    );

    let tools = manager.list_all_tools().await;
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool.callable_name.as_str())
            .collect::<Vec<_>>(),
        vec!["drive_search"]
    );
    assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[tokio::test(start_paused = true)]
async fn later_tool_list_retries_after_failed_reconnect_and_keeps_cached_tools() {
    let recovered_client = create_test_managed_client(vec![create_test_tool(
        CODEX_APPS_MCP_SERVER_NAME,
        "drive_search",
    )])
    .await;
    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_for_reconnect = Arc::clone(&attempts);
    let reconnect_finished = Arc::new(tokio::sync::Notify::new());
    let reconnect_finished_for_factory = Arc::clone(&reconnect_finished);
    let reconnect_factory = Arc::new(move || {
        let attempt = attempts_for_reconnect.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let reconnect_finished = Arc::clone(&reconnect_finished_for_factory);
        let recovered_client = recovered_client.clone();
        async move {
            let result = if attempt < 2 {
                Err(StartupOutcomeError::Failed {
                    error: "recreated startup failed".to_string(),
                    is_authentication_required: false,
                })
            } else {
                Ok(recovered_client)
            };
            reconnect_finished.notify_one();
            result
        }
        .boxed()
        .shared()
    });
    let manager = create_test_manager_with_failed_apps_startup(
        vec![create_test_tool(
            CODEX_APPS_MCP_SERVER_NAME,
            "cached_drive_search",
        )],
        reconnect_factory,
    );

    let first_reconnect_finished = reconnect_finished.notified();
    let tools = manager.list_all_tools().await;
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool.callable_name.as_str())
            .collect::<Vec<_>>(),
        vec!["cached_drive_search"]
    );
    first_reconnect_finished.await;
    assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 1);

    let tools = manager.list_all_tools().await;
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool.callable_name.as_str())
            .collect::<Vec<_>>(),
        vec!["cached_drive_search"]
    );
    assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 1);

    tokio::time::advance(CODEX_APPS_RECONNECT_INITIAL_BACKOFF).await;
    let second_reconnect_finished = reconnect_finished.notified();
    let tools = manager.list_all_tools().await;
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool.callable_name.as_str())
            .collect::<Vec<_>>(),
        vec!["cached_drive_search"]
    );
    second_reconnect_finished.await;
    assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 2);

    tokio::time::advance(CODEX_APPS_RECONNECT_INITIAL_BACKOFF).await;
    let tools = manager.list_all_tools().await;
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool.callable_name.as_str())
            .collect::<Vec<_>>(),
        vec!["cached_drive_search"]
    );
    assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 2);

    tokio::time::advance(CODEX_APPS_RECONNECT_INITIAL_BACKOFF).await;
    let third_reconnect_finished = reconnect_finished.notified();
    let tools = manager.list_all_tools().await;
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool.callable_name.as_str())
            .collect::<Vec<_>>(),
        vec!["cached_drive_search"]
    );
    third_reconnect_finished.await;
    assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 3);

    let tools = manager.list_all_tools().await;
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool.callable_name.as_str())
            .collect::<Vec<_>>(),
        vec!["drive_search"]
    );
}

#[tokio::test]
async fn tool_lists_do_not_block_and_share_codex_apps_startup_reconnect() {
    let recovered_client = create_test_managed_client(vec![create_test_tool(
        CODEX_APPS_MCP_SERVER_NAME,
        "drive_search",
    )])
    .await;
    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_for_reconnect = Arc::clone(&attempts);
    let reconnect_started = Arc::new(tokio::sync::Notify::new());
    let reconnect_started_for_factory = Arc::clone(&reconnect_started);
    let release_reconnect = Arc::new(tokio::sync::Notify::new());
    let release_reconnect_for_factory = Arc::clone(&release_reconnect);
    let reconnect_factory = Arc::new(move || {
        let recovered_client = recovered_client.clone();
        let attempts = Arc::clone(&attempts_for_reconnect);
        let reconnect_started = Arc::clone(&reconnect_started_for_factory);
        let release_reconnect = Arc::clone(&release_reconnect_for_factory);
        async move {
            attempts.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            reconnect_started.notify_one();
            release_reconnect.notified().await;
            Ok(recovered_client)
        }
        .boxed()
        .shared()
    });
    let mut manager = create_test_manager_with_failed_apps_startup(
        vec![create_test_tool(
            CODEX_APPS_MCP_SERVER_NAME,
            "cached_drive_search",
        )],
        reconnect_factory,
    );
    manager.set_test_server_metadata(
        CODEX_APPS_MCP_SERVER_NAME,
        McpServerMetadata {
            environment_id: codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
            pollutes_memory: false,
            origin: None,
            supports_parallel_tool_calls: false,
            default_tools_approval_mode: None,
            tool_approval_modes: HashMap::new(),
        },
    );
    let manager = Arc::new(manager);
    let reconnect_started_wait = reconnect_started.notified();
    let first_tools = tokio::time::timeout(Duration::from_millis(10), manager.list_all_tools())
        .await
        .expect("cached tools should not wait for reconnect");

    reconnect_started_wait.await;
    let second_tools = tokio::time::timeout(Duration::from_millis(10), manager.list_all_tools())
        .await
        .expect("concurrent cached tools should not wait for reconnect");
    assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(
        first_tools
            .iter()
            .map(|tool| tool.callable_name.as_str())
            .collect::<Vec<_>>(),
        vec!["cached_drive_search"]
    );
    assert_eq!(
        second_tools
            .iter()
            .map(|tool| tool.callable_name.as_str())
            .collect::<Vec<_>>(),
        vec!["cached_drive_search"]
    );
    let pending_step = tokio::time::timeout(Duration::from_millis(10), capture_binding(&manager))
        .await
        .expect("step capture should not wait for reconnect");
    assert!(
        pending_step.tools().is_empty(),
        "a model step must not advertise cached tools without an exact ready client"
    );

    release_reconnect.notify_one();
    tokio::task::yield_now().await;
    let tools = manager.list_all_tools().await;
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool.callable_name.as_str())
            .collect::<Vec<_>>(),
        vec!["drive_search"]
    );
    let recovered_step = capture_binding(&manager).await;
    assert_eq!(
        recovered_step
            .tools()
            .iter()
            .map(|tool| tool.callable_name.as_str())
            .collect::<Vec<_>>(),
        vec!["drive_search"]
    );
    assert!(
        recovered_step
            .prepare_call(CODEX_APPS_MCP_SERVER_NAME, "drive_search")
            .is_some()
    );
    assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[tokio::test]
async fn list_all_tools_adds_server_metadata_to_tools() {
    let server_name = "docs";
    let managed_client =
        create_ready_async_managed_client(vec![create_test_tool(server_name, "search")]).await;
    let approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    let permission_profile = Constrained::allow_any(PermissionProfile::default());
    let mut manager = McpConnectionSet::new_uninitialized(
        &approval_policy,
        &permission_profile,
        /*prefix_mcp_tool_names*/ true,
    );
    manager.insert_test_client(server_name, managed_client);
    manager.set_test_server_metadata(
        server_name,
        McpServerMetadata {
            environment_id: codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
            pollutes_memory: true,
            origin: Some(McpServerOrigin::StreamableHttp(
                "https://docs.example".to_string(),
            )),
            supports_parallel_tool_calls: true,
            default_tools_approval_mode: None,
            tool_approval_modes: HashMap::new(),
        },
    );

    let tools = manager.list_all_tools().await;
    assert_eq!(tools.len(), 1);
    let tool = &tools[0];
    assert_eq!(tool.server_name, server_name);
    assert!(tool.supports_parallel_tool_calls);
    assert_eq!(tool.server_origin.as_deref(), Some("https://docs.example"));
}

#[test]
fn server_metadata_preserves_tool_approval_policy() {
    let mut config = crate::codex_apps_mcp_server_config(
        "https://docs.example",
        /*apps_mcp_product_sku*/ None,
        /*originator*/ None,
    );
    config.environment_id = "remote".to_string();
    config.default_tools_approval_mode = Some(AppToolApproval::Prompt);
    config.tools.insert(
        "search".to_string(),
        McpServerToolConfig {
            approval_mode: Some(AppToolApproval::Approve),
        },
    );
    let metadata = McpServerMetadata::from(&EffectiveMcpServer::configured(config));

    assert_eq!(metadata.environment_id, "remote");
    assert_eq!(metadata.tool_approval_mode("read"), AppToolApproval::Prompt);
    assert_eq!(
        metadata.tool_approval_mode("search"),
        AppToolApproval::Approve
    );
}

#[tokio::test]
async fn no_local_runtime_fails_local_stdio_but_keeps_local_http_server() {
    let codex_home = tempdir().expect("tempdir");
    let mcp_servers = HashMap::from([
        (
            "stdio".to_string(),
            EffectiveMcpServer::configured(McpServerConfig {
                auth: Default::default(),
                transport: McpServerTransportConfig::Stdio {
                    command: "echo".to_string(),
                    args: Vec::new(),
                    env: None,
                    env_vars: Vec::new(),
                    cwd: None,
                },
                environment_id: codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
                enabled: true,
                required: false,
                supports_parallel_tool_calls: false,
                disabled_reason: None,
                startup_timeout_sec: None,
                tool_timeout_sec: None,
                default_tools_approval_mode: None,
                enabled_tools: None,
                disabled_tools: None,
                scopes: None,
                oauth: None,
                oauth_resource: None,
                tools: HashMap::new(),
            }),
        ),
        (
            "http".to_string(),
            EffectiveMcpServer::configured(McpServerConfig {
                auth: Default::default(),
                transport: McpServerTransportConfig::StreamableHttp {
                    url: "http://127.0.0.1:1".to_string(),
                    bearer_token_env_var: None,
                    http_headers: None,
                    env_http_headers: None,
                },
                environment_id: codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
                enabled: true,
                required: false,
                supports_parallel_tool_calls: false,
                disabled_reason: None,
                startup_timeout_sec: None,
                tool_timeout_sec: None,
                default_tools_approval_mode: None,
                enabled_tools: None,
                disabled_tools: None,
                scopes: None,
                oauth: None,
                oauth_resource: None,
                tools: HashMap::new(),
            }),
        ),
    ]);

    let cancel_token = CancellationToken::new();
    let manager = McpConnectionSet::new(
        /*previous*/ None,
        McpPublicationGate::already_published(),
        McpRuntimeInput {
            config: Arc::new(crate::mcp::tests::test_mcp_config(
                codex_home.path().to_path_buf(),
            )),
            plugins_available: false,
            ready_selected_capability_roots: Vec::new(),
            mcp_servers,
            submit_id: String::new(),
            tx_event: None,
            startup_cancellation_token: cancel_token.clone(),
            runtime_context: McpRuntimeContext::new(
                Arc::new(EnvironmentManager::without_environments()),
                PathBuf::from("/tmp"),
            ),
            codex_apps_tools_cache: ConnectorRuntimeManager::<ToolInfo>::default(),
            tool_catalog_cache: McpToolCatalogCache::default(),
            codex_apps_tools_cache_key: ConnectorRuntimeContextKey::personal(
                /*account_id*/ None, /*chatgpt_user_id*/ None,
            ),
            supports_openai_form_elicitation: false,
            auth: None,
            codex_apps_auth_manager: None,
            elicitation_reviewer: None,
            elicitation_lifecycle: None,
        },
        ElicitationRequestRouter::default(),
    )
    .await;

    assert!(manager.contains_server("stdio"));
    assert!(manager.contains_server("http"));
    assert!(
        manager
            .test_client("http")
            .tool_catalog_cache_context
            .is_none()
    );
    assert!(
        !manager
            .wait_for_server_ready("stdio", Duration::from_millis(10))
            .await
    );
    let error = match manager.test_client("stdio").client().await {
        Ok(_) => panic!("local stdio MCP startup should fail"),
        Err(error) => error,
    };
    let StartupOutcomeError::Failed { error, .. } = error else {
        panic!("local stdio MCP startup should fail rather than be cancelled");
    };
    assert_eq!(
        error,
        "local stdio MCP server `stdio` requires a local environment"
    );
    cancel_token.cancel();
}

#[test]
fn elicitation_capability_uses_2025_06_18_shape_for_form_only_support() {
    let capability = Some(ElicitationCapability::default());
    assert_eq!(
        serde_json::to_value(capability).expect("serialize elicitation capability"),
        serde_json::json!({})
    );
}

#[test]
fn elicitation_capability_advertises_url_support_when_enabled() {
    let capability = Some(ElicitationCapability {
        form: Some(rmcp::model::FormElicitationCapability::default()),
        url: Some(rmcp::model::UrlElicitationCapability::default()),
    });
    assert_eq!(
        serde_json::to_value(capability).expect("serialize elicitation capability"),
        serde_json::json!({
            "form": {},
            "url": {},
        })
    );
}

#[test]
fn mcp_init_error_display_prompts_for_github_pat() {
    let server_name = "github";
    let config = McpServerConfig {
        auth: Default::default(),
        transport: McpServerTransportConfig::StreamableHttp {
            url: "https://api.githubcopilot.com/mcp/".to_string(),
            bearer_token_env_var: None,
            http_headers: None,
            env_http_headers: None,
        },
        environment_id: codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
        enabled: true,
        required: false,
        supports_parallel_tool_calls: false,
        disabled_reason: None,
        startup_timeout_sec: None,
        tool_timeout_sec: None,
        default_tools_approval_mode: None,
        enabled_tools: None,
        disabled_tools: None,
        scopes: None,
        oauth: None,
        oauth_resource: None,
        tools: HashMap::new(),
    };
    let err: StartupOutcomeError = anyhow::anyhow!("OAuth is unsupported").into();

    let display = mcp_init_error_display(server_name, Some(&config), &err);

    let expected = format!(
        "GitHub MCP does not support OAuth. Log in by adding a personal access token (https://github.com/settings/personal-access-tokens) to your environment and config.toml:\n[mcp_servers.{server_name}]\nbearer_token_env_var = CODEX_GITHUB_PERSONAL_ACCESS_TOKEN"
    );

    assert_eq!(expected, display);
}

#[test]
fn mcp_init_error_display_prompts_for_login_when_auth_required() {
    let server_name = "example";
    let err: StartupOutcomeError = anyhow::anyhow!("Auth required for server").into();

    let display = mcp_init_error_display(server_name, /*config*/ None, &err);

    let expected = format!(
        "The {server_name} MCP server is not logged in. Run `codex mcp login {server_name}`."
    );

    assert_eq!(expected, display);
}

#[test]
fn mcp_startup_failure_reason_requires_existing_oauth_and_auth_failure() {
    for (auth_state, is_authentication_required, expected) in [
        (
            Some(McpAuthState::LoggedOut(
                McpLoginRequirement::Reauthentication,
            )),
            true,
            Some(McpStartupFailureReason::ReauthenticationRequired),
        ),
        (
            Some(McpAuthState::LoggedOut(
                McpLoginRequirement::Reauthentication,
            )),
            false,
            None,
        ),
        (
            Some(McpAuthState::LoggedOut(McpLoginRequirement::Login)),
            true,
            None,
        ),
        (Some(McpAuthState::Unsupported), true, None),
        (Some(McpAuthState::BearerToken), true, None),
        (Some(McpAuthState::OAuth), true, None),
        (None, true, None),
    ] {
        let error = StartupOutcomeError::Failed {
            error: "startup failed".to_string(),
            is_authentication_required,
        };

        assert_eq!(
            mcp_startup_failure_reason(auth_state, &error),
            expected,
            "auth_state={auth_state:?}, is_authentication_required={is_authentication_required}"
        );
    }
}

#[test]
fn mcp_init_error_display_reports_generic_errors() {
    let server_name = "custom";
    let config = McpServerConfig {
        auth: Default::default(),
        transport: McpServerTransportConfig::StreamableHttp {
            url: "https://example.com".to_string(),
            bearer_token_env_var: Some("TOKEN".to_string()),
            http_headers: None,
            env_http_headers: None,
        },
        environment_id: codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
        enabled: true,
        required: false,
        supports_parallel_tool_calls: false,
        disabled_reason: None,
        startup_timeout_sec: None,
        tool_timeout_sec: None,
        default_tools_approval_mode: None,
        enabled_tools: None,
        disabled_tools: None,
        scopes: None,
        oauth: None,
        oauth_resource: None,
        tools: HashMap::new(),
    };
    let err: StartupOutcomeError = anyhow::anyhow!("boom").into();

    let display = mcp_init_error_display(server_name, Some(&config), &err);

    let expected = format!("MCP client for `{server_name}` failed to start: {err:#}");

    assert_eq!(expected, display);
}

#[test]
fn mcp_init_error_display_includes_startup_timeout_hint() {
    let server_name = "slow";
    for error in [
        "request timed out",
        "MCP client startup timed out after 30s",
    ] {
        let err: StartupOutcomeError = anyhow::anyhow!(error).into();

        let display = mcp_init_error_display(server_name, /*config*/ None, &err);

        assert_eq!(
            "MCP client for `slow` timed out after 30 seconds. Add or adjust `startup_timeout_sec` in your config.toml:\n[mcp_servers.slow]\nstartup_timeout_sec = XX",
            display
        );
    }
}

fn reusable_server_config(url: &str) -> McpServerConfig {
    McpServerConfig {
        auth: Default::default(),
        transport: McpServerTransportConfig::StreamableHttp {
            url: url.to_string(),
            bearer_token_env_var: Some("CODEX_MCP_REUSE_TEST_TOKEN".to_string()),
            http_headers: None,
            env_http_headers: None,
        },
        environment_id: codex_config::DEFAULT_MCP_SERVER_ENVIRONMENT_ID.to_string(),
        enabled: true,
        required: false,
        supports_parallel_tool_calls: false,
        disabled_reason: None,
        startup_timeout_sec: None,
        tool_timeout_sec: None,
        default_tools_approval_mode: None,
        enabled_tools: None,
        disabled_tools: None,
        scopes: None,
        oauth: None,
        oauth_resource: None,
        tools: HashMap::new(),
    }
}

fn reusable_server_runtime_context() -> McpRuntimeContext {
    McpRuntimeContext::new(
        Arc::new(EnvironmentManager::without_environments()),
        PathBuf::from("/tmp"),
    )
}

fn reusable_server_identity(
    config: &McpServerConfig,
    runtime_context: &McpRuntimeContext,
) -> McpServerConnectionIdentity {
    let server = EffectiveMcpServer::configured(config.clone());
    McpServerConnectionIdentity::new(
        "docs",
        &server,
        OAuthCredentialsStoreMode::default(),
        AuthKeyringBackendKind::default(),
        &Ok(None),
        runtime_context,
        /*runtime_auth_provider*/ None,
        /*auth*/ None,
        /*codex_apps_cache_identity*/ None,
        ElicitationCapability::default(),
        /*supports_openai_form_elicitation*/ false,
    )
}

async fn manager_with_reusable_ready_server(
    config: &McpServerConfig,
    runtime_context: &McpRuntimeContext,
    tools: Vec<ToolInfo>,
) -> McpConnectionSet {
    let approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    let permission_profile = Constrained::allow_any(PermissionProfile::default());
    let mut manager = McpConnectionSet::new_uninitialized(
        &approval_policy,
        &permission_profile,
        /*prefix_mcp_tool_names*/ true,
    );
    let server = EffectiveMcpServer::configured(config.clone());
    manager.servers.insert(
        "docs".to_string(),
        McpServerView {
            connection: Arc::new(McpServerConnection {
                identity: Some(reusable_server_identity(config, runtime_context)),
                client: create_ready_async_managed_client(tools).await,
            }),
            metadata: McpServerMetadata::from(&server),
            tool_filter: ToolFilter::from_config(config),
            tool_timeout: Some(config.tool_timeout_sec.unwrap_or(DEFAULT_TOOL_TIMEOUT)),
        },
    );
    manager
}

async fn reconcile_reusable_server(
    previous: &McpConnectionSet,
    config: McpServerConfig,
    runtime_context: McpRuntimeContext,
) -> McpConnectionSet {
    let (tx_event, _rx_event) = async_channel::unbounded();
    let codex_home = tempdir().expect("tempdir");
    McpConnectionSet::new(
        Some(previous),
        McpPublicationGate::already_published(),
        McpRuntimeInput {
            config: Arc::new(crate::mcp::tests::test_mcp_config(
                codex_home.path().to_path_buf(),
            )),
            plugins_available: false,
            ready_selected_capability_roots: Vec::new(),
            mcp_servers: HashMap::from([(
                "docs".to_string(),
                EffectiveMcpServer::configured(config),
            )]),
            submit_id: "refresh".to_string(),
            tx_event: Some(tx_event),
            startup_cancellation_token: CancellationToken::new(),
            runtime_context,
            codex_apps_tools_cache: ConnectorRuntimeManager::default(),
            tool_catalog_cache: McpToolCatalogCache::default(),
            codex_apps_tools_cache_key: ConnectorRuntimeContextKey::personal(
                /*account_id*/ None, /*chatgpt_user_id*/ None,
            ),
            supports_openai_form_elicitation: false,
            auth: None,
            codex_apps_auth_manager: None,
            elicitation_reviewer: None,
            elicitation_lifecycle: None,
        },
        ElicitationRequestRouter::default(),
    )
    .await
}

#[tokio::test]
async fn reconciliation_reuses_connection_without_relisting_regular_tools() -> anyhow::Result<()> {
    let tools = Arc::new(tokio::sync::RwLock::new(vec![Tool::new(
        "old_search",
        "old search",
        Arc::new(JsonObject::default()),
    )]));
    let block_tool_listing = Arc::new(AtomicBool::new(false));
    let client = Arc::new(
        RmcpClient::new_in_process_client(Arc::new(MutableToolsTransportFactory {
            server: MutableToolsServer {
                tools: Arc::clone(&tools),
                block_tool_listing: Arc::clone(&block_tool_listing),
            },
        }))
        .await?,
    );
    let initialize = client
        .initialize(
            InitializeRequestParams::new(
                ClientCapabilities::default(),
                Implementation::new("codex-test", "0.0.0-test"),
            )
            .with_protocol_version(ProtocolVersion::V_2025_06_18),
            /*timeout*/ None,
            Box::new(|_, _| {
                async {
                    Ok(ElicitationResponse {
                        action: ElicitationAction::Decline,
                        content: None,
                        meta: None,
                    })
                }
                .boxed()
            }),
        )
        .await?;
    let initial_tools = list_tools_for_client_uncached(
        "docs",
        /*is_codex_apps_mcp_server*/ false,
        /*codex_apps_refresh_trigger*/ "test",
        &client,
        /*timeout*/ None,
        initialize.instructions.as_deref(),
    )
    .await?;
    let managed_client = ManagedClient {
        client,
        server_info: create_test_server_info("Mutable tools"),
        tools: initial_tools,
        tool_timeout: None,
        server_instructions: initialize.instructions,
        server_supports_sandbox_state_meta_capability: false,
        codex_apps_tools_cache_context: None,
    };
    let runtime_context = reusable_server_runtime_context();
    let config = reusable_server_config("http://127.0.0.1:1");
    let approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    let permission_profile = Constrained::allow_any(PermissionProfile::default());
    let mut previous = McpConnectionSet::new_uninitialized(
        &approval_policy,
        &permission_profile,
        /*prefix_mcp_tool_names*/ true,
    );
    let server = EffectiveMcpServer::configured(config.clone());
    previous.servers.insert(
        "docs".to_string(),
        McpServerView {
            connection: Arc::new(McpServerConnection {
                identity: Some(reusable_server_identity(&config, &runtime_context)),
                client: AsyncManagedClient {
                    client: futures::future::ready(Ok(managed_client)).boxed().shared(),
                    is_codex_apps_mcp_server: false,
                    cached_server_info: None,
                    codex_apps_tools_cache_context: None,
                    tool_catalog_cache_context: None,
                    startup_complete: Arc::new(std::sync::atomic::AtomicBool::new(true)),
                    startup_reconnect: None,
                    cancel_token: CancellationToken::new(),
                },
            }),
            metadata: McpServerMetadata::from(&server),
            tool_filter: ToolFilter::from_config(&config),
            tool_timeout: Some(config.tool_timeout_sec.unwrap_or(DEFAULT_TOOL_TIMEOUT)),
        },
    );
    let previous = Arc::new(previous);
    let old_step = capture_binding(&previous).await;
    *tools.write().await = vec![Tool::new(
        "new_search",
        "new search",
        Arc::new(JsonObject::default()),
    )];
    block_tool_listing.store(true, Ordering::Release);

    let reconciled = Arc::new(
        tokio::time::timeout(
            Duration::from_secs(1),
            reconcile_reusable_server(&previous, config, runtime_context),
        )
        .await
        .expect("connection reuse must not wait for a tool-list request"),
    );
    let new_step = capture_binding(&reconciled).await;

    assert!(previous.shares_test_connection_with(&reconciled, "docs"));
    assert_eq!(
        old_step
            .tools()
            .iter()
            .map(|tool| tool.tool.name.to_string())
            .collect::<Vec<_>>(),
        vec!["old_search".to_string()]
    );
    assert_eq!(
        new_step
            .tools()
            .iter()
            .map(|tool| tool.tool.name.to_string())
            .collect::<Vec<_>>(),
        vec!["old_search".to_string()]
    );
    Ok(())
}

#[tokio::test]
async fn reconciliation_reuses_an_unchanged_ready_server() {
    let runtime_context = reusable_server_runtime_context();
    let config = reusable_server_config("http://127.0.0.1:1");
    let previous = manager_with_reusable_ready_server(
        &config,
        &runtime_context,
        vec![create_test_tool("docs", "search")],
    )
    .await;

    let reconciled = reconcile_reusable_server(&previous, config, runtime_context.clone()).await;

    assert!(previous.shares_test_connection_with(&reconciled, "docs"));
    assert_eq!(
        model_tool_names(&reconciled.list_all_tools().await),
        HashSet::from([ToolName::namespaced("mcp__docs", "search")])
    );
}

#[tokio::test]
async fn reconciliation_updates_elicitation_policy_without_restarting_ready_server() {
    let runtime_context = reusable_server_runtime_context();
    let config = reusable_server_config("http://127.0.0.1:1");
    let previous = manager_with_reusable_ready_server(
        &config,
        &runtime_context,
        vec![create_test_tool("docs", "search")],
    )
    .await;
    {
        let mut authority = previous
            .elicitation_requests
            .authority
            .lock()
            .expect("elicitation authority lock");
        authority.approval_policy = AskForApproval::Never;
        authority.permission_profile = PermissionProfile::Disabled;
    }

    let reconciled = reconcile_reusable_server(&previous, config, runtime_context).await;

    assert!(previous.shares_test_connection_with(&reconciled, "docs"));
    let authority = reconciled
        .elicitation_requests
        .authority
        .lock()
        .expect("elicitation authority lock");
    assert_eq!(authority.approval_policy, AskForApproval::OnRequest);
    assert_eq!(authority.permission_profile, PermissionProfile::default());
}

#[tokio::test]
async fn reconciliation_reuses_ready_server_when_startup_timeout_changes() {
    let runtime_context = reusable_server_runtime_context();
    let mut config = reusable_server_config("http://127.0.0.1:1");
    let previous = manager_with_reusable_ready_server(
        &config,
        &runtime_context,
        vec![create_test_tool("docs", "search")],
    )
    .await;
    config.startup_timeout_sec = Some(Duration::from_secs(30));

    let reconciled = reconcile_reusable_server(&previous, config, runtime_context).await;

    assert!(previous.shares_test_connection_with(&reconciled, "docs"));
}

#[tokio::test]
async fn reconciliation_replaces_closed_connections() -> anyhow::Result<()> {
    let runtime_context = reusable_server_runtime_context();
    let config = reusable_server_config("http://127.0.0.1:1");
    let mut previous = manager_with_reusable_ready_server(
        &config,
        &runtime_context,
        vec![create_test_tool("docs", "search")],
    )
    .await;
    let disconnect = CancellationToken::new();
    let client = Arc::new(
        RmcpClient::new_in_process_client(Arc::new(DisconnectingToolsTransportFactory {
            server: MutableToolsServer {
                tools: Arc::new(tokio::sync::RwLock::new(vec![Tool::new(
                    "search",
                    "search",
                    Arc::new(JsonObject::default()),
                )])),
                block_tool_listing: Arc::new(AtomicBool::new(false)),
            },
            disconnect: disconnect.clone(),
        }))
        .await?,
    );
    client
        .initialize(
            InitializeRequestParams::new(
                ClientCapabilities::default(),
                Implementation::new("codex-test", "0.0.0-test"),
            )
            .with_protocol_version(ProtocolVersion::V_2025_06_18),
            /*timeout*/ None,
            Box::new(|_, _| async { Err(anyhow!("unexpected elicitation")) }.boxed()),
        )
        .await?;
    let view = previous
        .servers
        .get_mut("docs")
        .expect("test server should exist");
    let mut connected_client = view.connection.client().await?;
    connected_client.client = Arc::clone(&client);
    view.connection = Arc::new(McpServerConnection {
        identity: Some(reusable_server_identity(&config, &runtime_context)),
        client: AsyncManagedClient {
            client: futures::future::ready(Ok(connected_client))
                .boxed()
                .shared(),
            is_codex_apps_mcp_server: false,
            cached_server_info: None,
            codex_apps_tools_cache_context: None,
            tool_catalog_cache_context: None,
            startup_complete: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            startup_reconnect: None,
            cancel_token: CancellationToken::new(),
        },
    });

    assert!(!client.is_closed().await);
    disconnect.cancel();
    tokio::time::timeout(Duration::from_secs(2), async {
        while !client.is_closed().await {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("closed MCP transport should be detected");

    let reconciled = reconcile_reusable_server(&previous, config, runtime_context).await;

    assert!(!previous.shares_test_connection_with(&reconciled, "docs"));
    Ok(())
}

#[tokio::test]
async fn reconciliation_reconnects_when_connection_identity_changes() {
    let runtime_context = reusable_server_runtime_context();
    let previous_config = reusable_server_config("http://127.0.0.1:1");
    let previous = manager_with_reusable_ready_server(
        &previous_config,
        &runtime_context,
        vec![create_test_tool("docs", "search")],
    )
    .await;

    let reconciled = reconcile_reusable_server(
        &previous,
        reusable_server_config("http://127.0.0.1:2"),
        runtime_context,
    )
    .await;

    assert!(!previous.shares_test_connection_with(&reconciled, "docs"));
}

#[tokio::test]
async fn connection_identity_distinguishes_accounts_with_the_same_token() -> anyhow::Result<()> {
    let runtime_context = reusable_server_runtime_context();
    let config = reusable_server_config("http://127.0.0.1:1");
    let server = EffectiveMcpServer::configured(config);
    let access_token = "header.e30.same";
    let previous_auth = CodexAuth::from_external_chatgpt_tokens(
        access_token,
        "account-a",
        /*chatgpt_plan_type*/ None,
    )?;
    let changed_auth = CodexAuth::from_external_chatgpt_tokens(
        access_token,
        "account-b",
        /*chatgpt_plan_type*/ None,
    )?;
    let connection_identity = |auth: &CodexAuth| {
        let provider = codex_model_provider::auth_provider_from_auth(auth);
        McpServerConnectionIdentity::new(
            "docs",
            &server,
            OAuthCredentialsStoreMode::default(),
            AuthKeyringBackendKind::default(),
            &Ok(None),
            &runtime_context,
            Some(&provider),
            Some(auth),
            /*codex_apps_cache_identity*/ None,
            ElicitationCapability::default(),
            /*supports_openai_form_elicitation*/ false,
        )
    };

    assert_eq!(previous_auth, changed_auth);
    assert_eq!(previous_auth.get_token()?, changed_auth.get_token()?);
    assert!(
        !connection_identity(&previous_auth)
            .has_same_connection_config(&connection_identity(&changed_auth))
    );
    Ok(())
}

#[tokio::test]
async fn connection_identity_distinguishes_agent_account_runtime_and_task() -> anyhow::Result<()> {
    let runtime_context = reusable_server_runtime_context();
    let config = reusable_server_config("http://127.0.0.1:1");
    let server = EffectiveMcpServer::configured(config);
    let record = codex_login::auth::AgentIdentityAuthRecord {
        agent_runtime_id: "agent-a".to_string(),
        agent_private_key: "MC4CAQAwBQYDK2VwBCIEIJ7kFBaOujmoz1gvBNEC+BeM2IX87FFB0xmISOZ/XO0c"
            .to_string(),
        account_id: "account-a".to_string(),
        chatgpt_user_id: "user-a".to_string(),
        email: Some("agent@example.com".to_string()),
        plan_type: codex_protocol::account::PlanType::Plus,
        chatgpt_account_is_fedramp: false,
        task_id: Some("task-a".to_string()),
    };
    let auth_route_config = codex_login::test_support::transport_default_auth_route_config();
    let previous_auth = CodexAuth::AgentIdentity(
        codex_login::auth::AgentIdentityAuth::from_record(
            record.clone(),
            "https://auth.openai.com/api/accounts",
            &auth_route_config,
        )
        .await?,
    );
    let connection_identity = |auth: &CodexAuth| {
        let provider = codex_model_provider::auth_provider_from_auth(auth);
        McpServerConnectionIdentity::new(
            CODEX_APPS_MCP_SERVER_NAME,
            &server,
            OAuthCredentialsStoreMode::default(),
            AuthKeyringBackendKind::default(),
            &Ok(None),
            &runtime_context,
            Some(&provider),
            Some(auth),
            /*codex_apps_cache_identity*/ None,
            ElicitationCapability::default(),
            /*supports_openai_form_elicitation*/ false,
        )
    };
    let previous_identity = connection_identity(&previous_auth);

    for changed_record in [
        codex_login::auth::AgentIdentityAuthRecord {
            account_id: "account-b".to_string(),
            ..record.clone()
        },
        codex_login::auth::AgentIdentityAuthRecord {
            chatgpt_user_id: "user-b".to_string(),
            ..record.clone()
        },
        codex_login::auth::AgentIdentityAuthRecord {
            chatgpt_account_is_fedramp: true,
            ..record.clone()
        },
        codex_login::auth::AgentIdentityAuthRecord {
            agent_runtime_id: "agent-b".to_string(),
            ..record.clone()
        },
        codex_login::auth::AgentIdentityAuthRecord {
            task_id: Some("task-b".to_string()),
            ..record.clone()
        },
    ] {
        let changed_auth = CodexAuth::AgentIdentity(
            codex_login::auth::AgentIdentityAuth::from_record(
                changed_record,
                "https://auth.openai.com/api/accounts",
                &auth_route_config,
            )
            .await?,
        );
        assert_eq!(previous_auth, changed_auth);
        assert!(!previous_identity.has_same_connection_config(&connection_identity(&changed_auth)));
    }

    Ok(())
}

#[tokio::test]
async fn view_only_changes_reuse_connection_and_preserve_the_old_step() {
    let runtime_context = reusable_server_runtime_context();
    let mut old_config = reusable_server_config("http://127.0.0.1:1");
    old_config.default_tools_approval_mode = Some(AppToolApproval::Prompt);
    let previous = Arc::new(
        manager_with_reusable_ready_server(
            &old_config,
            &runtime_context,
            vec![
                create_test_tool("docs", "search"),
                create_test_tool("docs", "write"),
            ],
        )
        .await,
    );
    let old_step = capture_binding(&previous).await;
    let old_call = old_step
        .prepare_call("docs", "search")
        .expect("old step should prepare search");

    let mut new_config = old_config;
    new_config.enabled_tools = Some(vec!["search".to_string()]);
    new_config.default_tools_approval_mode = Some(AppToolApproval::Approve);
    let reconciled =
        Arc::new(reconcile_reusable_server(previous.as_ref(), new_config, runtime_context).await);
    assert!(previous.shares_test_connection_with(&reconciled, "docs"));

    let new_step = capture_binding(&reconciled).await;
    let new_call = new_step
        .prepare_call("docs", "search")
        .expect("new step should prepare search");
    drop(previous);

    assert_eq!(
        old_step
            .tools()
            .iter()
            .map(|tool| tool.tool.name.to_string())
            .collect::<HashSet<_>>(),
        HashSet::from(["search".to_string(), "write".to_string()])
    );
    assert_eq!(old_call.tool_approval_mode(), AppToolApproval::Prompt);
    assert_eq!(
        new_step
            .tools()
            .iter()
            .map(|tool| tool.tool.name.to_string())
            .collect::<Vec<_>>(),
        vec!["search".to_string()]
    );
    assert_eq!(new_call.tool_approval_mode(), AppToolApproval::Approve);
}
