//! Exercises independent event connections through the real MCP HTTP transport.
#![allow(clippy::unwrap_used)]

use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use axum::Json;
use axum::Router;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::sse::Event;
use axum::response::sse::Sse;
use axum::routing::post;
use codex_config::McpServerTransportConfig;
use codex_core::config::ConfigBuilder;
use codex_core::plugins_manager_for_config;
use codex_exec_server_test_support::environment_manager_without_environments;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_login::ExternalAuth;
use codex_login::ExternalAuthFuture;
use codex_login::ExternalAuthRefreshContext;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::CodexAppsResourceListParams;
use codex_mcp::EffectiveMcpServer;
use codex_mcp::McpEventNotification;
use codex_mcp::McpResourceClient;
use codex_mcp::McpRuntime;
use codex_mcp::McpRuntimeContext;
use codex_mcp::McpRuntimeInput;
use codex_mcp::McpServerRegistration;
use codex_mcp::McpStartupPolicy;
use codex_mcp::ResolvedMcpCatalog;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tokio::sync::Notify;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

pub(super) const WAIT: Duration = Duration::from_secs(/*secs*/ 5);

pub(super) struct TestStream {
    pub request: Value,
    pub headers: HeaderMap,
    pub notifications: mpsc::UnboundedSender<Value>,
}

impl TestStream {
    pub fn notify(&self, method: &str) -> McpEventNotification {
        let params = json!({
            "data": {"text": "hello"},
            "_meta": {"io.modelcontextprotocol/subscriptionId": self.request["id"], "connector": "test"},
        });
        self.notifications
            .send(json!({"jsonrpc": "2.0", "method": method, "params": params}))
            .unwrap();
        McpEventNotification {
            method: method.into(),
            params: Some(params),
        }
    }
}

#[derive(Default)]
struct BlockedTools {
    started: Notify,
    release: Notify,
    handshakes: AtomicUsize,
    inventories: AtomicUsize,
    inventory_returned: AtomicBool,
    inventory_error: Option<&'static str>,
    stalled_initialize_timeout: Option<Duration>,
    modern_with_sharing: bool,
    held_mime_type: Option<&'static str>,
    session_id: Option<&'static str>,
    resource_started: Notify,
    release_resource: Notify,
}

pub(super) struct Fixture {
    _home: tempfile::TempDir,
    _server: tokio_util::task::AbortOnDropHandle<std::io::Result<()>>,
    pub runtime: Arc<McpRuntime>,
    pub runtime_input: McpRuntimeInput,
    pub auth: Arc<AuthManager>,
    pub opened: mpsc::UnboundedReceiver<TestStream>,
}

impl Fixture {
    pub async fn new() -> Result<Self> {
        let fixture = Self::start(/*blocked_tools*/ None, /*early_resources*/ false).await?;
        assert!(
            fixture
                .runtime
                .latest_wait_for_server_ready(CODEX_APPS_MCP_SERVER_NAME, WAIT)
                .await
        );
        Ok(fixture)
    }

    async fn start(
        blocked_tools: Option<Arc<BlockedTools>>,
        early_resources: bool,
    ) -> Result<Self> {
        let (opened_tx, opened) = mpsc::unbounded_channel();
        let modern_with_sharing = blocked_tools
            .as_ref()
            .is_some_and(|blocked| blocked.modern_with_sharing);
        let stalled_initialize_timeout = blocked_tools
            .as_ref()
            .and_then(|blocked| blocked.stalled_initialize_timeout);
        let router = Router::new().fallback(post(
            move |headers: HeaderMap, Json(request): Json<Value>| {
                let opened = opened_tx.clone();
                let blocked_tools = blocked_tools.clone();
                async move {
                    let method = request["method"].as_str();
                    let session_id = blocked_tools.as_ref().and_then(|blocked| blocked.session_id);
                    if let Some(session_id) = session_id && method != Some("initialize") {
                        assert_eq!(headers["mcp-session-id"], session_id);
                    }
                    let mut result = match method {
                        Some("server/discover") if modern_with_sharing => {
                            blocked_tools.as_ref().unwrap().handshakes.fetch_add(1, Ordering::SeqCst);
                            json!({
                                "supportedVersions": ["2026-07-28"],
                                "capabilities": {"tools": {}, "resources": {}},
                                "_meta": {"io.modelcontextprotocol/serverInfo": {"name": "test", "version": "1"}},
                                "ttlMs": 0, "cacheScope": "private",
                            })
                        }
                        Some("initialize") if !modern_with_sharing => {
                            if let Some(blocked) = &blocked_tools {
                                blocked.handshakes.fetch_add(1, Ordering::SeqCst);
                                if stalled_initialize_timeout.is_some() {
                                    blocked.started.notify_one();
                                    blocked.release.notified().await;
                                }
                            }
                            json!({
                                "protocolVersion": "2025-06-18", "capabilities": {"tools": {}, "resources": {}},
                                "serverInfo": {"name": "test", "version": "1"},
                            })
                        }
                        Some("tools/list") => {
                            if let Some(blocked) = &blocked_tools {
                                blocked.inventories.fetch_add(1, Ordering::SeqCst);
                                blocked.started.notify_one();
                                blocked.release.notified().await;
                                blocked.inventory_returned.store(true, Ordering::SeqCst);
                                if let Some(message) = blocked.inventory_error {
                                    return Json(json!({
                                        "jsonrpc": "2.0", "id": request["id"],
                                        "error": {"code": -32603, "message": message}
                                    })).into_response();
                                }
                            }
                            json!({"tools": []})
                        }
                        Some("resources/list") => {
                            if let Some(blocked) = &blocked_tools {
                                assert_eq!(blocked.inventory_returned.load(Ordering::SeqCst), !early_resources,
                                    "unexpected catalog request ordering relative to the initial inventory");
                            }
                            let mime_type = request["params"]["mimeType"].as_str().unwrap();
                            let cursor = match mime_type {
                                "mcp/plugin" => "plugin-page-two",
                                "mcp/skill" => "skill-page-two",
                                other => panic!("unexpected resource type: {other}"),
                            };
                            if let Some(blocked) = &blocked_tools && blocked.held_mime_type == Some(mime_type) {
                                blocked.resource_started.notify_one();
                                blocked.release_resource.notified().await;
                            }
                            json!({"resources": [], "nextCursor": cursor})
                        }
                        Some("notifications/initialized") => {
                            return StatusCode::ACCEPTED.into_response();
                        }
                        Some("events/stream") => {
                            let (notifications, receive) = mpsc::unbounded_channel();
                            opened
                                .send(TestStream {
                                    request,
                                    headers,
                                    notifications,
                                })
                                .unwrap();
                            let events = futures::stream::unfold(receive, |mut receive| async {
                                receive.recv().await.map(|value| (value, receive))
                            })
                            .map(|value: Value| {
                                Ok::<_, Infallible>(
                                    Event::default().event("message").data(value.to_string()),
                                )
                            });
                            return Sse::new(events).into_response();
                        }
                        method => panic!("unexpected request: {method:?}"),
                    };
                    if modern_with_sharing {
                        result["resultType"] = "complete".into();
                    }
                    let response = Json(json!({"jsonrpc": "2.0", "id": request["id"], "result": result}));
                    if method == Some("initialize") && let Some(session_id) = session_id {
                        return ([("mcp-session-id", session_id)], response).into_response();
                    }
                    response.into_response()
                }
            },
        ));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let url = format!("http://{}", listener.local_addr()?);
        let server = tokio::spawn(async move { axum::serve(listener, router).await });
        let home = tempfile::tempdir()?;
        let auth_value = CodexAuth::from_external_chatgpt_tokens(
            "header.e30.first",
            "account",
            /*chatgpt_plan_type*/ None,
        )?;
        let auth =
            AuthManager::from_auth_for_testing_with_home(auth_value.clone(), home.path().into());
        let config = ConfigBuilder::default()
            .codex_home(home.path().into())
            .fallback_cwd(Some(home.path().into()))
            .cli_overrides(vec![
                (
                    "features.apps_early_resource_discovery".into(),
                    early_resources.into(),
                ),
                (
                    "features.mcp_connection_sharing".into(),
                    modern_with_sharing.into(),
                ),
                (
                    "features.codex_apps_mcp_2026_07_28".into(),
                    modern_with_sharing.into(),
                ),
            ])
            .build()
            .await?;
        let plugins = plugins_manager_for_config(&config, Arc::clone(&auth));
        let mut config = config.to_mcp_config(&plugins).await;
        let mut server_config = codex_mcp::hosted_plugin_runtime_mcp_server_config(
            &url, /*apps_mcp_product_sku*/ None, /*originator*/ None,
        );
        let McpServerTransportConfig::StreamableHttp {
            bearer_token_env_var,
            ..
        } = &mut server_config.transport
        else {
            panic!("HTTP server");
        };
        *bearer_token_env_var = None;
        if let Some(timeout) = stalled_initialize_timeout {
            server_config.startup_timeout_sec = Some(timeout);
        }
        let mut catalog = ResolvedMcpCatalog::builder();
        catalog.register(McpServerRegistration::from_hosted_apps(
            "test",
            /*contribution_order*/ 0,
            server_config.clone(),
        ));
        config.mcp_server_catalog = catalog.build();
        let config = Arc::new(config);
        let startup_cancellation_token = CancellationToken::new();
        let runtime_input = || McpRuntimeInput {
            network_policy_metadata: None,
            startup_policy: McpStartupPolicy::Eager,
            config: Arc::clone(&config),
            plugins_available: false,
            ready_selected_capability_roots: Vec::new(),
            mcp_servers: HashMap::from([(
                CODEX_APPS_MCP_SERVER_NAME.into(),
                EffectiveMcpServer::from_host_config(server_config.clone()),
            )]),
            submit_id: "test".into(),
            tx_event: None,
            startup_cancellation_token: startup_cancellation_token.clone(),
            runtime_context: McpRuntimeContext::new(
                Arc::new(environment_manager_without_environments()),
                home.path().into(),
            )
            .with_connection_pool(Default::default()),
            codex_apps_tools_cache: Default::default(),
            tool_catalog_cache: Default::default(),
            codex_apps_tools_cache_key: codex_mcp::codex_apps_tools_cache_key(Some(&auth_value)),
            client_mcp_extensions: Default::default(),
            auth: Some(auth_value.clone()),
            auth_manager: Some(Arc::clone(&auth)),
            elicitation_reviewer: None,
            elicitation_lifecycle: None,
        };
        let runtime = Arc::new(McpRuntime::new(runtime_input()).await);
        let runtime_input = runtime_input();
        Ok(Self {
            _home: home,
            _server: tokio_util::task::AbortOnDropHandle::new(server),
            runtime,
            runtime_input,
            auth,
            opened,
        })
    }
}

#[tokio::test]
async fn apps_resource_discovery_during_initial_tools_list() -> Result<()> {
    for (early_resources, modern_with_sharing) in [(false, false), (true, false), (true, true)] {
        let blocked = Arc::new(BlockedTools {
            modern_with_sharing,
            ..Default::default()
        });
        let fixture = Fixture::start(Some(Arc::clone(&blocked)), early_resources).await?;
        timeout(WAIT, blocked.started.notified())
            .await
            .context("initial tools/list did not start")?;
        let reader = McpResourceClient::new(Arc::clone(&fixture.runtime));
        let mut page = Box::pin(
            reader.list_codex_apps_resources(CodexAppsResourceListParams {
                mime_type: "mcp/plugin".into(),
                cursor: None,
            }),
        );
        if early_resources {
            assert_eq!(
                timeout(WAIT, &mut page)
                    .await
                    .context("gated catalog read waited for the blocked inventory")??
                    .next_cursor
                    .as_deref(),
                Some("plugin-page-two")
            );
        } else {
            assert!(futures::poll!(&mut page).is_pending());
            // A read already using the early client would now fail its generation check.
            // A read waiting for startup can still fetch the page after inventory returns.
            fixture.runtime.invalidate_resource_caches();
        }
        let mut ready = Box::pin(
            fixture
                .runtime
                .latest_wait_for_server_ready(CODEX_APPS_MCP_SERVER_NAME, WAIT),
        );
        assert!(futures::poll!(&mut ready).is_pending());
        blocked.release.notify_one();
        assert!(ready.await);
        if !early_resources {
            assert_eq!(
                timeout(WAIT, page).await??.next_cursor.as_deref(),
                Some("plugin-page-two")
            );
        }
        assert_eq!(blocked.handshakes.load(Ordering::SeqCst), 1);
        assert_eq!(blocked.inventories.load(Ordering::SeqCst), 1);
    }
    Ok(())
}

#[tokio::test]
async fn apps_resource_discovery_times_out_during_stalled_initialization() -> Result<()> {
    let startup_timeout = Duration::from_secs(/*secs*/ 1);
    let blocked = Arc::new(BlockedTools {
        stalled_initialize_timeout: Some(startup_timeout),
        ..Default::default()
    });
    let fixture = Fixture::start(Some(Arc::clone(&blocked)), /*early_resources*/ true).await?;
    let reader = McpResourceClient::new(Arc::clone(&fixture.runtime));
    let mut page = Box::pin(
        reader.list_codex_apps_resources(CodexAppsResourceListParams {
            mime_type: "mcp/plugin".into(),
            cursor: None,
        }),
    );
    assert!(futures::poll!(&mut page).is_pending());
    timeout(WAIT, blocked.started.notified())
        .await
        .context("initialize did not reach the server")?;
    let error = timeout(WAIT, page)
        .await
        .context("reader outlived its startup deadline")?
        .unwrap_err();
    // The handshake and early borrow each enforce the configured startup timeout.
    let message = format!("{error:#}");
    assert!(message.contains("timed out"), "{message}");
    assert_eq!(blocked.inventories.load(Ordering::SeqCst), 0);
    blocked.release.notify_one();
    Ok(())
}

#[tokio::test]
async fn apps_resource_discovery_multiplexes_catalogs_while_tools_list_is_pending() -> Result<()> {
    let blocked = Arc::new(BlockedTools {
        held_mime_type: Some("mcp/plugin"),
        session_id: Some("apps-session"),
        ..Default::default()
    });
    let fixture = Fixture::start(Some(Arc::clone(&blocked)), /*early_resources*/ true).await?;
    timeout(WAIT, blocked.started.notified()).await?;

    let reader = McpResourceClient::new(Arc::clone(&fixture.runtime));
    let plugin_reader = reader.clone();
    let plugin = tokio::spawn(async move {
        plugin_reader
            .list_codex_apps_resources(CodexAppsResourceListParams {
                mime_type: "mcp/plugin".into(),
                cursor: None,
            })
            .await
    });
    timeout(WAIT, blocked.resource_started.notified())
        .await
        .context("plugin request did not reach the server while tools/list was pending")?;
    let skill = timeout(
        WAIT,
        reader.list_codex_apps_resources(CodexAppsResourceListParams {
            mime_type: "mcp/skill".into(),
            cursor: None,
        }),
    )
    .await
    .context("skill request queued behind the pending plugin or inventory request")??;
    assert_eq!(skill.next_cursor.as_deref(), Some("skill-page-two"));
    assert!(!plugin.is_finished());

    blocked.release_resource.notify_one();
    let plugin = timeout(WAIT, plugin).await???;
    assert_eq!(plugin.next_cursor.as_deref(), Some("plugin-page-two"));

    let mut ready = Box::pin(
        fixture
            .runtime
            .latest_wait_for_server_ready(CODEX_APPS_MCP_SERVER_NAME, WAIT),
    );
    assert!(futures::poll!(&mut ready).is_pending());
    blocked.release.notify_one();
    assert!(ready.await);
    assert_eq!(blocked.handshakes.load(Ordering::SeqCst), 1);
    assert_eq!(blocked.inventories.load(Ordering::SeqCst), 1);
    Ok(())
}

#[tokio::test]
async fn apps_resource_discovery_propagates_initial_tools_list_failure() -> Result<()> {
    const INVENTORY_ERROR: &str = "initial inventory unavailable";
    let blocked = Arc::new(BlockedTools {
        held_mime_type: Some("mcp/plugin"),
        inventory_error: Some(INVENTORY_ERROR),
        ..Default::default()
    });
    let fixture = Fixture::start(Some(Arc::clone(&blocked)), /*early_resources*/ true).await?;
    timeout(WAIT, blocked.started.notified()).await?;
    let reader = McpResourceClient::new(Arc::clone(&fixture.runtime));
    let mut page = Box::pin(
        reader.list_codex_apps_resources(CodexAppsResourceListParams {
            mime_type: "mcp/plugin".into(),
            cursor: None,
        }),
    );
    tokio::select! {
        result = &mut page => panic!("catalog returned before inventory failed: {result:?}"),
        result = timeout(WAIT, blocked.resource_started.notified()) => result?,
    }

    blocked.release.notify_one();
    let error = timeout(WAIT, page)
        .await
        .context("catalog reader kept waiting after tools/list failed")?
        .unwrap_err()
        .to_string();
    assert!(error.starts_with("MCP startup failed:"), "{error}");
    assert!(error.contains(INVENTORY_ERROR), "{error}");
    assert!(
        !fixture
            .runtime_input
            .startup_cancellation_token
            .is_cancelled()
    );
    blocked.release_resource.notify_one();
    Ok(())
}

#[tokio::test]
async fn apps_resource_discovery_observes_cancellation_after_inventory_finishes() -> Result<()> {
    let blocked = Arc::new(BlockedTools {
        held_mime_type: Some("mcp/plugin"),
        ..Default::default()
    });
    let fixture = Fixture::start(Some(Arc::clone(&blocked)), /*early_resources*/ true).await?;
    timeout(WAIT, blocked.started.notified()).await?;
    let reader = McpResourceClient::new(Arc::clone(&fixture.runtime));
    let mut page = Box::pin(
        reader.list_codex_apps_resources(CodexAppsResourceListParams {
            mime_type: "mcp/plugin".into(),
            cursor: None,
        }),
    );
    tokio::select! {
        result = &mut page => panic!("catalog returned before release: {result:?}"),
        result = timeout(WAIT, blocked.resource_started.notified()) => result?,
    }
    blocked.release.notify_one();
    assert!(
        fixture
            .runtime
            .latest_wait_for_server_ready(CODEX_APPS_MCP_SERVER_NAME, WAIT)
            .await
    );
    assert!(futures::poll!(&mut page).is_pending());
    fixture.runtime_input.startup_cancellation_token.cancel();
    let error = timeout(WAIT, page)
        .await
        .context("catalog ignored cancellation after startup")?
        .unwrap_err();
    assert!(error.to_string().to_lowercase().contains("cancel"));
    blocked.release_resource.notify_one();
    Ok(())
}

#[tokio::test]
async fn apps_resource_discovery_discards_invalidated_page() -> Result<()> {
    for cancel_startup in [false, true] {
        let blocked = Arc::new(BlockedTools {
            held_mime_type: Some("mcp/skill"),
            ..Default::default()
        });
        let fixture = Fixture::start(Some(Arc::clone(&blocked)), /*early_resources*/ true).await?;
        timeout(WAIT, blocked.started.notified()).await?;
        let reader = McpResourceClient::new(Arc::clone(&fixture.runtime));
        let page = tokio::spawn(async move {
            reader
                .list_codex_apps_resources(CodexAppsResourceListParams {
                    mime_type: "mcp/skill".into(),
                    cursor: None,
                })
                .await
        });
        timeout(WAIT, blocked.resource_started.notified()).await?;
        let expected = if cancel_startup {
            fixture.runtime.cancel_startup();
            "cancel"
        } else {
            fixture.runtime.invalidate_resource_caches();
            blocked.release_resource.notify_one();
            "changed during catalog discovery"
        };
        let error = timeout(WAIT, page).await??.unwrap_err();
        assert!(error.to_string().to_lowercase().contains(expected));
        blocked.release.notify_one();
        blocked.release_resource.notify_one();
    }
    Ok(())
}

#[tokio::test]
async fn apps_resource_discovery_discards_page_after_account_switch() -> Result<()> {
    let blocked = Arc::new(BlockedTools {
        held_mime_type: Some("mcp/skill"),
        ..Default::default()
    });
    let mut fixture = Fixture::start(Some(Arc::clone(&blocked)), /*early_resources*/ true).await?;
    timeout(WAIT, blocked.started.notified()).await?;
    let reader = McpResourceClient::new(Arc::clone(&fixture.runtime));
    let mut page = Box::pin(
        reader.list_codex_apps_resources(CodexAppsResourceListParams {
            mime_type: "mcp/skill".into(),
            cursor: None,
        }),
    );
    tokio::select! {
        result = &mut page => panic!("catalog returned before account switch: {result:?}"),
        result = timeout(WAIT, blocked.resource_started.notified()) => result?,
    }

    let previous_auth = reader.auth_cache_key_for_server(CODEX_APPS_MCP_SERVER_NAME);
    let next_auth = CodexAuth::from_external_chatgpt_tokens(
        "header.e30.second",
        "second-account",
        /*chatgpt_plan_type*/ None,
    )?;
    fixture
        .auth
        .set_external_auth(Arc::new(StaticAuth(next_auth.clone())))
        .await?;
    fixture.runtime_input.codex_apps_tools_cache_key =
        codex_mcp::codex_apps_tools_cache_key(Some(&next_auth));
    fixture.runtime_input.auth = Some(next_auth);
    timeout(WAIT, fixture.runtime.replace(fixture.runtime_input)).await?;
    assert!(previous_auth != reader.auth_cache_key_for_server(CODEX_APPS_MCP_SERVER_NAME));
    blocked.release_resource.notify_one();
    let error = timeout(WAIT, page).await?.unwrap_err();
    assert_eq!(
        error.to_string(),
        "Apps connection changed during catalog discovery"
    );
    Ok(())
}

pub(super) struct StaticAuth(pub CodexAuth);

impl ExternalAuth for StaticAuth {
    fn resolve(&self) -> ExternalAuthFuture<'_, CodexAuth> {
        Box::pin(async { Ok(self.0.clone()) })
    }
    fn refresh(&self, _: ExternalAuthRefreshContext) -> ExternalAuthFuture<'_, CodexAuth> {
        self.resolve()
    }
}

#[tokio::test]
async fn event_connections_open_after_runtime_shutdown_with_current_auth() -> Result<()> {
    let mut fixture = Fixture::new().await?;
    let opener = McpResourceClient::new(Arc::clone(&fixture.runtime)).event_stream_opener()?;
    fixture.runtime.shutdown().await;
    drop(fixture.runtime);

    fixture
        .auth
        .set_external_auth(Arc::new(StaticAuth(
            CodexAuth::from_external_chatgpt_tokens(
                "header.e30.refreshed",
                "account",
                /*chatgpt_plan_type*/ None,
            )?,
        )))
        .await?;
    let mut stream = opener
        .open("test.event", &json!({}), /*request_meta*/ None)
        .await?;
    let remote = timeout(WAIT, fixture.opened.recv()).await?.unwrap();
    assert_eq!(
        remote.headers["authorization"],
        "Bearer header.e30.refreshed"
    );
    let expected = remote.notify("notifications/events/active");
    assert_eq!(timeout(WAIT, stream.recv()).await??, Some(expected));
    drop(stream);
    timeout(WAIT, remote.notifications.closed()).await?;
    Ok(())
}
