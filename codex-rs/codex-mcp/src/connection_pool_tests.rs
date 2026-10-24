//! Tests for shared MCP connection ownership and protocol negotiation.

use super::*;
use crate::binding_clients::McpBindingClients;
use crate::connection_manager::tests::create_ready_async_managed_client;
use crate::connection_manager::tests::create_test_tool;
use crate::connection_manager::tests::reusable_server_identity;
use crate::runtime::McpRuntimeContext;
use codex_config::McpServerConfig;
use codex_exec_server::EnvironmentManager;
use codex_login::CodexAuth;
use codex_rmcp_client::Elicitation;
use codex_rmcp_client::InProcessTransportFactory;
use codex_rmcp_client::RmcpClient;
use futures::future::BoxFuture;
use pretty_assertions::assert_eq;
use rmcp::ServerHandler;
use rmcp::ServiceExt;
use rmcp::model::ClientCapabilities;
use rmcp::model::Implementation;
use rmcp::model::InitializeRequestParams;
use rmcp::model::ListResourceTemplatesResult;
use rmcp::model::ListResourcesResult;
use rmcp::model::ListToolsResult;
use rmcp::model::PaginatedRequestParams;
use rmcp::model::ProtocolVersion;
use rmcp::model::ServerCapabilities;
use rmcp::model::ServerConfig;
use rmcp::model::Tool;
use rmcp::service::RequestContext;
use rmcp::service::RoleServer;
use serde_json::json;
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use tokio::sync::Notify;
use tokio::sync::Semaphore;

#[derive(Clone)]
pub(crate) struct TestServer {
    tools: Arc<Mutex<Vec<Tool>>>,
    block_listing: Arc<AtomicBool>,
    started_listings: Arc<Semaphore>,
    cancelled_listings: Arc<Semaphore>,
}

impl Default for TestServer {
    fn default() -> Self {
        Self {
            tools: Arc::new(Mutex::new(vec![
                create_test_tool("test_server", "test").tool,
            ])),
            block_listing: Arc::default(),
            started_listings: Arc::new(Semaphore::new(/*permits*/ 0)),
            cancelled_listings: Arc::new(Semaphore::new(/*permits*/ 0)),
        }
    }
}

impl ServerHandler for TestServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
    }

    async fn call_tool(
        &self,
        _request: rmcp::model::CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<rmcp::model::CallToolResponse, rmcp::ErrorData> {
        Ok(rmcp::model::CallToolResult::success(Vec::new()).into())
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, rmcp::ErrorData> {
        if self.block_listing.load(Ordering::Acquire) {
            context.ct.cancelled().await;
            self.cancelled_listings.add_permits(1);
            return Err(rmcp::ErrorData::internal_error(
                "listing cancelled",
                /*data*/ None,
            ));
        }
        Ok(ListToolsResult::with_all_items(
            self.tools.lock().unwrap().clone(),
        ))
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, rmcp::ErrorData> {
        if self.block_listing.load(Ordering::Acquire) {
            self.started_listings.add_permits(1);
            context.ct.cancelled().await;
            self.cancelled_listings.add_permits(1);
        }
        Ok(ListResourcesResult::with_all_items(Vec::new()))
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, rmcp::ErrorData> {
        if self.block_listing.load(Ordering::Acquire) {
            self.started_listings.add_permits(1);
            context.ct.cancelled().await;
            self.cancelled_listings.add_permits(1);
        }
        Ok(ListResourceTemplatesResult::with_all_items(Vec::new()))
    }
}

impl InProcessTransportFactory for TestServer {
    fn open(&self) -> BoxFuture<'static, std::io::Result<tokio::io::DuplexStream>> {
        let server = self.clone();
        async move {
            let (client, transport) = tokio::io::duplex(4096);
            tokio::spawn(async move {
                server
                    .serve(transport)
                    .await
                    .unwrap()
                    .waiting()
                    .await
                    .unwrap();
            });
            Ok(client)
        }
        .boxed()
    }
}

pub(crate) fn key(context: &McpRuntimeContext, config: &McpServerConfig) -> ConnectionKey {
    ConnectionKey {
        server_name: "test_server".to_string(),
        identity: reusable_server_identity("test_server", config, context),
        account: ConnectorRuntimeContextKey::personal(
            /*account_id*/ None, /*chatgpt_user_id*/ None,
        ),
        auth_state: None,
        startup_timeout: Duration::from_secs(30),
        catalog_item_limit: 100,
    }
}

pub(crate) fn context() -> McpRuntimeContext {
    McpRuntimeContext::new(
        Arc::new(EnvironmentManager::default_for_tests()),
        std::env::temp_dir(),
    )
}

pub(crate) fn config() -> McpServerConfig {
    serde_json::from_value(json!({"command": "test-server", "args": ["server.mjs"]})).unwrap()
}

pub(crate) fn startup(
    sharing: ConnectionSharing,
    starts: Arc<AtomicUsize>,
    first_started: Arc<Notify>,
    release_first: Arc<Notify>,
    server: TestServer,
) -> impl Fn(CancellationToken, Option<CancellationToken>) -> ManagedClientFuture {
    move |cancellation, _sharing_declined| {
        let sharing = sharing.clone();
        let starts = Arc::clone(&starts);
        let first_started = Arc::clone(&first_started);
        let release_first = Arc::clone(&release_first);
        let server = server.clone();
        async move {
            async {
                if starts.fetch_add(1, Ordering::SeqCst) == 0 {
                    first_started.notify_one();
                    release_first.notified().await;
                }
                let mut client = create_ready_async_managed_client(Vec::new())
                    .await
                    .client()
                    .await?;
                client.client = Arc::new(
                    RmcpClient::new_in_process_client(Arc::new(server))
                        .await
                        .expect("create test MCP client"),
                );
                client
                    .client
                    .initialize(
                        InitializeRequestParams::new(
                            ClientCapabilities::default(),
                            Implementation::new("test", "1"),
                        )
                        .with_protocol_version(ProtocolVersion::V_2025_06_18),
                        Some(Duration::from_secs(30)),
                        Box::new(|_, _| {
                            futures::future::ready(Ok(ElicitationResponse {
                                action: ElicitationAction::Decline,
                                content: None,
                                meta: None,
                            }))
                            .boxed()
                        }),
                    )
                    .await?;
                let (tools, meta) = list_tools_for_client_uncached(
                    "test_server",
                    /*is_codex_apps_mcp_server*/ false,
                    "test",
                    &client.client,
                    /*timeout*/ None,
                    /*catalog_item_limit*/ 100,
                    /*server_instructions*/ None,
                )
                .await?;
                client.tool_catalog =
                    Arc::new(ClientToolCatalog::new(tools, meta, /*updates*/ None));
                client.connection_sharing = sharing;
                Ok(client)
            }
            .or_cancel(&cancellation)
            .await
            .unwrap_or(Err(StartupOutcomeError::Cancelled))
        }
        .boxed()
        .shared()
    }
}

#[tokio::test]
async fn shared_startup_survives_first_chat_cancellation_and_rejects_released_bindings() {
    let pool = McpConnectionPool::default();
    let key = key(&context(), &config());
    let server = TestServer::default();
    let starts = Arc::new(AtomicUsize::new(0));
    let started = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let launch = startup(
        ConnectionSharing::Negotiated,
        Arc::clone(&starts),
        Arc::clone(&started),
        Arc::clone(&release),
        server.clone(),
    );
    let first_cancel = CancellationToken::new();
    let first = pool.connect(
        Arc::default(),
        key.clone(),
        Arc::default(),
        /*tool_catalog_cache_context*/ None,
        first_cancel.clone(),
        &launch,
    );
    let second = pool.connect(
        Arc::default(),
        key.clone(),
        Arc::default(),
        /*tool_catalog_cache_context*/ None,
        CancellationToken::new(),
        &launch,
    );
    tokio::pin!(first, second);
    assert!(futures::poll!(&mut first).is_pending());
    assert!(futures::poll!(&mut second).is_pending());
    started.notified().await;
    first_cancel.cancel();
    assert!(matches!(first.await, Err(StartupOutcomeError::Cancelled)));
    release.notify_one();
    let client = second.await.unwrap();
    assert_eq!(starts.load(Ordering::SeqCst), 1);
    assert!(!client.client.is_closed().await);

    let other = pool
        .connect(
            Arc::default(),
            key,
            Arc::default(),
            /*tool_catalog_cache_context*/ None,
            CancellationToken::new(),
            &launch,
        )
        .await
        .unwrap();
    let mut config = crate::mcp::tests::test_mcp_config(std::env::temp_dir());
    config
        .server_permission_profiles
        .insert("test_server".to_string(), config.permission_profile.clone());
    let prepared = crate::binding::PreparedMcpCall::new(
        Arc::new(crate::connection_manager::McpConnectionSet::empty(
            /*prefix_mcp_tool_names*/ true,
        )),
        Arc::new(client.clone()),
        Arc::new(config),
        client.tool_catalog.read(Arc::new).await,
        create_test_tool("test_server", "test"),
        crate::server::McpServerMetadata {
            environment_id: "local".to_string(),
            pollutes_memory: false,
            origin: None,
            supports_parallel_tool_calls: true,
            default_tools_approval_mode: None,
            tool_approval_modes: Default::default(),
        },
        /*plugin_id*/ None,
        /*selected_plugin_server*/ false,
    )
    .unwrap();
    let bindings = McpBindingClients::new(HashMap::from([(
        "test_server".to_string(),
        Arc::new(client.clone()),
    )]));
    server.block_listing.store(true, Ordering::Release);
    let waiting = async {
        tokio::join!(
            bindings.list_resources("test_server", /*params*/ None),
            bindings.list_resource_templates("test_server", /*params*/ None),
        )
    };
    tokio::pin!(waiting);
    tokio::select! {
        _ = &mut waiting => panic!("resource listings must wait for cancellation"),
        permits = server.started_listings.acquire_many(2) => permits.unwrap().forget(),
    }
    let mut owned = create_ready_async_managed_client(Vec::new()).await;
    owned.client = futures::future::ready(Ok(client.clone())).boxed().shared();
    owned.shutdown().await;
    let (resources, templates) = waiting.await;
    assert!(resources.is_err());
    assert!(templates.is_err());
    tokio::time::timeout(
        Duration::from_secs(1),
        server.cancelled_listings.acquire_many(2),
    )
    .await
    .expect("released resource listings must be cancelled")
    .unwrap()
    .forget();
    let prepared_error = prepared
        .call_with_preparation(/*requested_timeout*/ None, || async {
            panic!("released bindings must not perform irreversible preparation")
        })
        .await
        .unwrap_err();
    assert!(
        prepared_error
            .to_string()
            .contains("lease has been released")
    );
    other
        .call_tool(
            "test".to_string(),
            /*arguments*/ None,
            |_| Ok(None),
            /*timeout*/ None,
        )
        .await
        .unwrap();
    owned.client = futures::future::ready(Ok(other)).boxed().shared();
    owned.shutdown().await;
}

#[tokio::test]
async fn callback_during_startup_releases_waiting_chats_without_sharing_authority() {
    let pool = McpConnectionPool::default();
    let key = key(&context(), &config());
    let starts = Arc::new(AtomicUsize::new(0));
    let started = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let second_callback = Arc::new(Notify::new());
    let release_second = Arc::new(Notify::new());
    let callbacks = Arc::new(Mutex::new(Vec::new()));
    let launch = |name| {
        let launch = startup(
            ConnectionSharing::Private,
            Arc::clone(&starts),
            Arc::clone(&started),
            Arc::clone(&release),
            TestServer::default(),
        );
        let callbacks = Arc::clone(&callbacks);
        let second_callback = Arc::clone(&second_callback);
        let release_second = Arc::clone(&release_second);
        move |cancellation, sharing_declined: Option<CancellationToken>| {
            let startup = launch(cancellation, sharing_declined.clone());
            let callbacks = Arc::clone(&callbacks);
            let second_callback = Arc::clone(&second_callback);
            let release_second = Arc::clone(&release_second);
            async move {
                let mut client = startup.await?;
                let (negotiation, callback) = SharingNegotiation::new(
                    Box::new(move |_, _| {
                        callbacks.lock().unwrap().push(name);
                        futures::future::ready(Ok(ElicitationResponse {
                            action: ElicitationAction::Decline,
                            content: None,
                            meta: None,
                        }))
                        .boxed()
                    }),
                    sharing_declined.unwrap_or_default(),
                );
                callback(
                    rmcp::model::RequestId::Number(0),
                    Elicitation::OpenAiForm {
                        meta: None,
                        message: "startup prompt".to_string(),
                        requested_schema: json!({}),
                    },
                )
                .await
                .unwrap();
                if name == "first" {
                    second_callback.notified().await;
                } else {
                    second_callback.notify_one();
                    release_second.notified().await;
                }
                client.connection_sharing = negotiation.finish(&ProtocolVersion::V_2026_07_28);
                Ok(client)
            }
            .boxed()
            .shared()
        }
    };
    let first = pool.connect(
        Arc::default(),
        key.clone(),
        Arc::default(),
        /*tool_catalog_cache_context*/ None,
        CancellationToken::new(),
        launch("first"),
    );
    let second = pool.connect(
        Arc::default(),
        key,
        Arc::default(),
        /*tool_catalog_cache_context*/ None,
        CancellationToken::new(),
        launch("second"),
    );
    tokio::pin!(first, second);
    assert!(futures::poll!(&mut first).is_pending());
    assert!(futures::poll!(&mut second).is_pending());
    started.notified().await;
    release.notify_one();
    let first = tokio::time::timeout(Duration::from_secs(10), async {
        tokio::select! {
            first = &mut first => first,
            _ = &mut second => panic!("the joining chat should still be starting"),
        }
    })
    .await
    .expect("the joining chat must start before the first finishes")
    .unwrap();
    first
        .call_tool(
            "test".to_string(),
            /*arguments*/ None,
            |_| Ok(None),
            /*timeout*/ None,
        )
        .await
        .unwrap();
    let mut owner = create_ready_async_managed_client(Vec::new()).await;
    owner.client = futures::future::ready(Ok(first.clone())).boxed().shared();
    tokio::time::timeout(Duration::from_secs(10), owner.shutdown())
        .await
        .expect("private owner shutdown must not wait for the joining chat");
    assert!(first.client.is_closed().await);

    release_second.notify_one();
    let second = second.await.unwrap();
    assert_eq!(starts.load(Ordering::SeqCst), 2);
    assert_eq!(*callbacks.lock().unwrap(), ["first", "second"]);
    second
        .call_tool(
            "test".to_string(),
            /*arguments*/ None,
            |_| Ok(None),
            /*timeout*/ None,
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn connection_key_changes_with_the_owner_but_survives_credential_refresh() {
    let manager =
        AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    let mut original = key(&context(), &config());
    original.auth_state = Some((manager, AuthChangeState::default()));
    let mut refreshed = original.clone();
    refreshed.auth_state.as_mut().unwrap().1.generation += 1;
    assert!(original == refreshed);

    refreshed.auth_state.as_mut().unwrap().1.owner_generation += 1;
    assert!(original != refreshed);
}

#[tokio::test(start_paused = true)]
async fn new_chat_relists_tools_without_invalidating_or_waiting_for_existing_calls() {
    let pool = McpConnectionPool::default();
    let key = key(&context(), &config());
    let server = TestServer::default();
    let starts = Arc::new(AtomicUsize::new(0));
    let release = Arc::new(Notify::new());
    release.notify_one();
    let launch = startup(
        ConnectionSharing::Negotiated,
        Arc::clone(&starts),
        Arc::default(),
        release,
        server.clone(),
    );
    let first = pool
        .connect(
            Arc::default(),
            key.clone(),
            Arc::default(),
            /*tool_catalog_cache_context*/ None,
            CancellationToken::new(),
            &launch,
        )
        .await
        .unwrap();
    let before = first.tool_catalog.read(|snapshot| snapshot).await;
    let release_call = Notify::new();
    let call = first
        .tool_catalog
        .run_with_snapshot(&before, || release_call.notified());
    tokio::pin!(call);
    assert!(futures::poll!(&mut call).is_pending());
    for tool_name in ["test", "new"] {
        *server.tools.lock().unwrap() = vec![create_test_tool("test_server", tool_name).tool];
        let second = tokio::time::timeout(
            Duration::from_secs(1),
            pool.connect(
                Arc::default(),
                key.clone(),
                Arc::default(),
                /*tool_catalog_cache_context*/ None,
                CancellationToken::new(),
                &launch,
            ),
        )
        .await
        .expect("new chats must not wait for existing calls")
        .unwrap();
        assert_eq!(starts.load(Ordering::SeqCst), 1);
        assert_eq!(
            second
                .tool_catalog
                .read(|snapshot| snapshot.tools[0].tool.name.to_string())
                .await,
            tool_name
        );
        tokio::time::timeout(
            Duration::from_secs(1),
            second.tool_catalog.refresh(
                || async { Ok((vec![create_test_tool("test_server", "refreshed")], None, ())) },
                |_, _, ()| {},
            ),
        )
        .await
        .expect("one chat's refresh must not wait for another chat's calls")
        .unwrap();
        assert_eq!(
            first
                .tool_catalog
                .read(|snapshot| snapshot.tools[0].tool.name.to_string())
                .await,
            "test"
        );
    }
    release_call.notify_one();
    assert_eq!(call.await, Some(()));

    server.block_listing.store(true, Ordering::Release);
    let failed = pool
        .connect(
            Arc::default(),
            key,
            Arc::default(),
            /*tool_catalog_cache_context*/ None,
            CancellationToken::new(),
            &launch,
        )
        .await;
    assert!(failed.is_err());
    let _cancelled =
        tokio::time::timeout(Duration::from_secs(1), server.cancelled_listings.acquire())
            .await
            .expect("timed-out listings must be cancelled")
            .unwrap();
    first
        .call_tool(
            "test".to_string(),
            /*arguments*/ None,
            |_| Ok(None),
            /*timeout*/ None,
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn negotiation_requires_the_stateless_protocol_and_detaches_startup_callbacks() {
    for (protocol_version, expected_sharing, expected_action) in [
        (
            ProtocolVersion::V_2025_06_18,
            false,
            ElicitationAction::Decline,
        ),
        (
            ProtocolVersion::V_2026_07_28,
            true,
            ElicitationAction::Cancel,
        ),
    ] {
        let (negotiation, callback) = SharingNegotiation::new(
            Box::new(|_, _| {
                futures::future::ready(Ok(ElicitationResponse {
                    action: ElicitationAction::Decline,
                    content: None,
                    meta: None,
                }))
                .boxed()
            }),
            CancellationToken::new(),
        );
        let sharing = negotiation.finish(&protocol_version);
        assert_eq!(
            matches!(sharing, ConnectionSharing::Negotiated),
            expected_sharing
        );
        let response = callback(
            rmcp::model::RequestId::Number(0),
            Elicitation::OpenAiForm {
                meta: None,
                message: "unexpected".to_string(),
                requested_schema: json!({}),
            },
        )
        .await
        .unwrap();
        assert_eq!(response.action, expected_action);
    }
}
