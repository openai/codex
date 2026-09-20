//! Exercises independent event connections through the real MCP HTTP transport.
#![allow(clippy::unwrap_used)]

use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

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
        let (opened_tx, opened) = mpsc::unbounded_channel();
        let router = Router::new().fallback(post(
            move |headers: HeaderMap, Json(request): Json<Value>| {
                let opened = opened_tx.clone();
                async move {
                    let result = match request["method"].as_str() {
                        Some("initialize") => json!({
                            "protocolVersion": "2025-06-18", "capabilities": {},
                            "serverInfo": {"name": "test", "version": "1"},
                        }),
                        Some("tools/list") => json!({"tools": []}),
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
                    Json(json!({"jsonrpc": "2.0", "id": request["id"], "result": result}))
                        .into_response()
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
        let mut catalog = ResolvedMcpCatalog::builder();
        catalog.register(McpServerRegistration::from_hosted_apps(
            "test",
            /*contribution_order*/ 0,
            server_config.clone(),
        ));
        config.mcp_server_catalog = catalog.build();
        let config = Arc::new(config);
        let runtime_input = || McpRuntimeInput {
            startup_policy: McpStartupPolicy::Eager,
            config: Arc::clone(&config),
            plugins_available: false,
            ready_selected_capability_roots: Vec::new(),
            mcp_servers: HashMap::from([(
                CODEX_APPS_MCP_SERVER_NAME.into(),
                EffectiveMcpServer::configured(server_config.clone()),
            )]),
            submit_id: "test".into(),
            tx_event: None,
            startup_cancellation_token: CancellationToken::new(),
            runtime_context: McpRuntimeContext::new(
                Arc::new(environment_manager_without_environments()),
                home.path().into(),
            ),
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
        assert!(
            runtime
                .latest_wait_for_server_ready(CODEX_APPS_MCP_SERVER_NAME, WAIT)
                .await
        );
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
