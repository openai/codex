use std::collections::HashMap;
use std::ffi::OsString;
use std::io;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use anyhow::anyhow;
use codex_otel::metrics;
use futures::FutureExt;
use mcp_types::CallToolRequestParams;
use mcp_types::CallToolResult;
use mcp_types::InitializeRequestParams;
use mcp_types::InitializeResult;
use mcp_types::ListToolsRequestParams;
use mcp_types::ListToolsResult;
use rmcp::model::CallToolRequestParam;
use rmcp::model::InitializeRequestParam;
use rmcp::model::PaginatedRequestParam;
use rmcp::service::RoleClient;
use rmcp::service::RunningService;
use rmcp::service::{self};
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::auth::AuthClient;
use rmcp::transport::auth::OAuthState;
use rmcp::transport::child_process::TokioChildProcess;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time;
use tokio::time::MissedTickBehavior;
use tracing::{event, info, warn};

use crate::load_oauth_tokens;
use crate::logging_client_handler::LoggingClientHandler;
use crate::oauth::OAuthCredentialsStoreMode;
use crate::oauth::OAuthPersistor;
use crate::oauth::StoredOAuthTokens;
use crate::utils::convert_call_tool_result;
use crate::utils::convert_to_mcp;
use crate::utils::convert_to_rmcp;
use crate::utils::create_env_for_mcp_server;
use crate::utils::run_with_timeout;

enum PendingTransport {
    ChildProcess(TokioChildProcess),
    StreamableHttp {
        transport: StreamableHttpClientTransport<reqwest::Client>,
    },
    StreamableHttpWithOAuth {
        transport: StreamableHttpClientTransport<AuthClient<reqwest::Client>>,
        oauth_persistor: OAuthPersistor,
    },
}

enum ClientState {
    Connecting {
        transport: Option<PendingTransport>,
    },
    Ready {
        service: Arc<RunningService<RoleClient, LoggingClientHandler>>,
        oauth: Option<OAuthPersistor>,
    },
}

/// MCP client implemented on top of the official `rmcp` SDK.
/// https://github.com/modelcontextprotocol/rust-sdk
pub struct RmcpClient {
    state: Mutex<ClientState>,
    last_activity: Arc<AtomicU64>,
    keepalive_handle: StdMutex<Option<JoinHandle<()>>>,
}

impl RmcpClient {
    pub async fn new_stdio_client(
        program: OsString,
        args: Vec<OsString>,
        env: Option<HashMap<String, String>>,
    ) -> io::Result<Self> {
        let program_name = program.to_string_lossy().into_owned();
        let mut command = Command::new(&program);
        command
            .kill_on_drop(true)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .env_clear()
            .envs(create_env_for_mcp_server(env))
            .args(&args);

        let (transport, stderr) = TokioChildProcess::builder(command)
            .stderr(Stdio::piped())
            .spawn()?;

        if let Some(stderr) = stderr {
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr).lines();
                loop {
                    match reader.next_line().await {
                        Ok(Some(line)) => {
                            info!("MCP server stderr ({program_name}): {line}");
                        }
                        Ok(None) => break,
                        Err(error) => {
                            warn!("Failed to read MCP server stderr ({program_name}): {error}");
                            break;
                        }
                    }
                }
            });
        }

        Ok(Self {
            state: Mutex::new(ClientState::Connecting {
                transport: Some(PendingTransport::ChildProcess(transport)),
            }),
            last_activity: Arc::new(AtomicU64::new(now_millis())),
            keepalive_handle: StdMutex::new(None),
        })
    }

    pub async fn new_streamable_http_client(
        server_name: &str,
        url: &str,
        bearer_token: Option<String>,
        store_mode: OAuthCredentialsStoreMode,
    ) -> Result<Self> {
        let initial_oauth_tokens = match bearer_token {
            Some(_) => None,
            None => match load_oauth_tokens(server_name, url, store_mode) {
                Ok(tokens) => tokens,
                Err(err) => {
                    warn!("failed to read tokens for server `{server_name}`: {err}");
                    None
                }
            },
        };
        let transport = if let Some(initial_tokens) = initial_oauth_tokens.clone() {
            let (transport, oauth_persistor) =
                create_oauth_transport_and_runtime(server_name, url, initial_tokens, store_mode)
                    .await?;
            PendingTransport::StreamableHttpWithOAuth {
                transport,
                oauth_persistor,
            }
        } else {
            let mut http_config = StreamableHttpClientTransportConfig::with_uri(url.to_string());
            if let Some(bearer_token) = bearer_token {
                http_config = http_config.auth_header(bearer_token);
            }

            let transport = StreamableHttpClientTransport::from_config(http_config);
            PendingTransport::StreamableHttp { transport }
        };
        Ok(Self {
            state: Mutex::new(ClientState::Connecting {
                transport: Some(transport),
            }),
            last_activity: Arc::new(AtomicU64::new(now_millis())),
            keepalive_handle: StdMutex::new(None),
        })
    }

    /// Perform the initialization handshake with the MCP server.
    /// https://modelcontextprotocol.io/specification/2025-06-18/basic/lifecycle#initialization
    pub async fn initialize(
        &self,
        params: InitializeRequestParams,
        timeout: Option<Duration>,
    ) -> Result<InitializeResult> {
        let rmcp_params: InitializeRequestParam = convert_to_rmcp(params.clone())?;
        let client_handler = LoggingClientHandler::new(rmcp_params);

        let (transport, oauth_persistor) = {
            let mut guard = self.state.lock().await;
            match &mut *guard {
                ClientState::Connecting { transport } => match transport.take() {
                    Some(PendingTransport::ChildProcess(transport)) => (
                        service::serve_client(client_handler.clone(), transport).boxed(),
                        None,
                    ),
                    Some(PendingTransport::StreamableHttp { transport }) => (
                        service::serve_client(client_handler.clone(), transport).boxed(),
                        None,
                    ),
                    Some(PendingTransport::StreamableHttpWithOAuth {
                        transport,
                        oauth_persistor,
                    }) => (
                        service::serve_client(client_handler.clone(), transport).boxed(),
                        Some(oauth_persistor),
                    ),
                    None => return Err(anyhow!("client already initializing")),
                },
                ClientState::Ready { .. } => return Err(anyhow!("client already initialized")),
            }
        };

        let service = match timeout {
            Some(duration) => time::timeout(duration, transport)
                .await
                .map_err(|_| anyhow!("timed out handshaking with MCP server after {duration:?}"))?
                .map_err(|err| anyhow!("handshaking with MCP server failed: {err}"))?,
            None => transport
                .await
                .map_err(|err| anyhow!("handshaking with MCP server failed: {err}"))?,
        };

        let initialize_result_rmcp = service
            .peer()
            .peer_info()
            .ok_or_else(|| anyhow!("handshake succeeded but server info was missing"))?;
        let initialize_result = convert_to_mcp(initialize_result_rmcp)?;

        {
            let mut guard = self.state.lock().await;
            *guard = ClientState::Ready {
                service: Arc::new(service),
                oauth: oauth_persistor.clone(),
            };
        }

        if let Some(runtime) = oauth_persistor
            && let Err(error) = runtime.persist_if_needed().await
        {
            warn!("failed to persist OAuth tokens after initialize: {error}");
        }

        self.record_activity();

        Ok(initialize_result)
    }

    pub async fn list_tools(
        &self,
        params: Option<ListToolsRequestParams>,
        timeout: Option<Duration>,
    ) -> Result<ListToolsResult> {
        let service = self.service().await?;
        let rmcp_params = params
            .map(convert_to_rmcp::<_, PaginatedRequestParam>)
            .transpose()?;

        let fut = service.list_tools(rmcp_params);
        let result = run_with_timeout(fut, timeout, "tools/list").await?;
        let converted = convert_to_mcp(result)?;
        self.persist_oauth_tokens().await;
        self.record_activity();
        Ok(converted)
    }

    pub async fn call_tool(
        &self,
        name: String,
        arguments: Option<serde_json::Value>,
        timeout: Option<Duration>,
    ) -> Result<CallToolResult> {
        let service = self.service().await?;
        let params = CallToolRequestParams { arguments, name };
        let rmcp_params: CallToolRequestParam = convert_to_rmcp(params)?;
        let fut = service.call_tool(rmcp_params);
        let rmcp_result = run_with_timeout(fut, timeout, "tools/call").await?;
        let converted = convert_call_tool_result(rmcp_result)?;
        self.persist_oauth_tokens().await;
        self.record_activity();
        Ok(converted)
    }

    async fn service(&self) -> Result<Arc<RunningService<RoleClient, LoggingClientHandler>>> {
        let guard = self.state.lock().await;
        match &*guard {
            ClientState::Ready { service, .. } => Ok(Arc::clone(service)),
            ClientState::Connecting { .. } => Err(anyhow!("MCP client not initialized")),
        }
    }

    async fn oauth_persistor(&self) -> Option<OAuthPersistor> {
        let guard = self.state.lock().await;
        match &*guard {
            ClientState::Ready {
                oauth: Some(runtime),
                service: _,
            } => Some(runtime.clone()),
            _ => None,
        }
    }

    async fn persist_oauth_tokens(&self) {
        if let Some(runtime) = self.oauth_persistor().await
            && let Err(error) = runtime.persist_if_needed().await
        {
            warn!("failed to persist OAuth tokens: {error}");
        }
    }

    pub fn enable_keepalive(self: &Arc<Self>, interval: Duration, transport_label: &'static str) {
        if interval.is_zero() {
            return;
        }

        let weak = Arc::downgrade(self);
        let interval_ms = interval.as_millis() as u64;
        let label = transport_label;
        let handle = tokio::spawn(async move {
            let mut ticker = time::interval(interval);
            ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
            loop {
                ticker.tick().await;

                let Some(client) = weak.upgrade() else {
                    break;
                };

                let elapsed_ms = client.millis_since_last_activity();
                if elapsed_ms < interval_ms {
                    continue;
                }

                event!(
                    tracing::Level::DEBUG,
                    event.name = "codex.rmcp_keepalive",
                    status = "emit",
                    elapsed_ms = elapsed_ms,
                );
                metrics::record_heartbeat(label, "emit", Some(elapsed_ms));

                match client.list_tools(None, Some(Duration::from_secs(5))).await {
                    Ok(_) => {
                        client.record_activity();
                        event!(
                            tracing::Level::DEBUG,
                            event.name = "codex.rmcp_keepalive",
                            status = "sent",
                            interval_ms = interval_ms,
                        );
                        metrics::record_heartbeat(label, "sent", Some(interval_ms));
                    }
                    Err(error) => {
                        event!(
                            tracing::Level::WARN,
                            event.name = "codex.rmcp_keepalive",
                            status = "error",
                            error.message = %error,
                        );
                        metrics::record_heartbeat(label, "error", Some(elapsed_ms));
                    }
                }
            }
        });

        if let Ok(mut guard) = self.keepalive_handle.lock()
            && let Some(existing) = guard.replace(handle)
        {
            existing.abort();
        }
    }

    fn record_activity(&self) {
        Self::record_activity_atomic(&self.last_activity);
    }

    fn millis_since_last_activity(&self) -> u64 {
        now_millis().saturating_sub(self.last_activity.load(Ordering::Relaxed))
    }

    fn record_activity_atomic(activity: &AtomicU64) {
        activity.store(now_millis(), Ordering::Relaxed);
    }
}

async fn create_oauth_transport_and_runtime(
    server_name: &str,
    url: &str,
    initial_tokens: StoredOAuthTokens,
    credentials_store: OAuthCredentialsStoreMode,
) -> Result<(
    StreamableHttpClientTransport<AuthClient<reqwest::Client>>,
    OAuthPersistor,
)> {
    let http_client = reqwest::Client::builder().build()?;
    let mut oauth_state = OAuthState::new(url.to_string(), Some(http_client.clone())).await?;

    oauth_state
        .set_credentials(
            &initial_tokens.client_id,
            initial_tokens.token_response.0.clone(),
        )
        .await?;

    let manager = match oauth_state {
        OAuthState::Authorized(manager) => manager,
        OAuthState::Unauthorized(manager) => manager,
        OAuthState::Session(_) | OAuthState::AuthorizedHttpClient(_) => {
            return Err(anyhow!("unexpected OAuth state during client setup"));
        }
    };

    let auth_client = AuthClient::new(http_client, manager);
    let auth_manager = auth_client.auth_manager.clone();

    let transport = StreamableHttpClientTransport::with_client(
        auth_client,
        StreamableHttpClientTransportConfig::with_uri(url.to_string()),
    );

    let runtime = OAuthPersistor::new(
        server_name.to_string(),
        url.to_string(),
        auth_manager,
        credentials_store,
        Some(initial_tokens),
    );

    Ok((transport, runtime))
}

impl Drop for RmcpClient {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.keepalive_handle.lock()
            && let Some(handle) = guard.take()
        {
            handle.abort();
        }
    }
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
