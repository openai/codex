use std::collections::HashMap;
use std::ffi::OsString;
use std::future::Future;
use std::io;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use anyhow::Result;
use anyhow::anyhow;
use codex_api::SharedAuthProvider;
use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::McpServerEnvVar;
use codex_exec_server::HttpClient;
use codex_keyring_store::DefaultKeyringStore;
use futures::FutureExt;
use futures::future::BoxFuture;
use oauth2::AccessToken;
use oauth2::TokenResponse;
use reqwest::header::AUTHORIZATION;
use reqwest::header::HeaderMap;
use rmcp::model::CallToolRequestParams;
use rmcp::model::CallToolResult;
use rmcp::model::ClientNotification;
use rmcp::model::ClientRequest;
use rmcp::model::CreateElicitationRequestParams;
use rmcp::model::CreateElicitationResult;
use rmcp::model::CustomNotification;
use rmcp::model::CustomRequest;
use rmcp::model::ElicitationAction;
use rmcp::model::Extensions;
use rmcp::model::InitializeRequestParams;
use rmcp::model::InitializeResult;
use rmcp::model::ListResourceTemplatesResult;
use rmcp::model::ListResourcesResult;
use rmcp::model::ListToolsResult;
use rmcp::model::PaginatedRequestParams;
use rmcp::model::ReadResourceRequestParams;
use rmcp::model::ReadResourceResult;
use rmcp::model::RequestId;
use rmcp::model::RequestParamsMeta;
use rmcp::model::ServerResult;
use rmcp::model::Tool;
use rmcp::service::RoleClient;
use rmcp::service::RunningService;
use rmcp::service::{self};
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::auth::AuthClient;
use rmcp::transport::auth::AuthError;
use rmcp::transport::auth::OAuthState;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::streamable_http_client::StreamableHttpError;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use tokio::sync::Mutex;
use tokio::sync::Semaphore;
use tokio::sync::watch;
use tokio::time;
use tracing::instrument;
use tracing::warn;

use crate::elicitation_client_service::ElicitationClientService;
use crate::http_client_adapter::StreamableHttpClientAdapter;
use crate::http_client_adapter::StreamableHttpClientAdapterError;
use crate::in_process_transport::InProcessTransportFactory;
use crate::oauth::OAuthPersistor;
use crate::oauth::ResolvedOAuthCredentialStore;
use crate::oauth::ResolvedOAuthTokens;
use crate::oauth::StoredOAuthTokens;
use crate::oauth::load_oauth_tokens_from_store;
use crate::oauth::request_oauth_token_response;
use crate::oauth::resolve_oauth_tokens;
use crate::oauth_http_client::OAuthHttpClientAdapter;
use crate::oauth_transport::OAuthTransportClient;
use crate::stdio_server_launcher::StdioServerCommand;
use crate::stdio_server_launcher::StdioServerLauncher;
use crate::stdio_server_launcher::StdioServerProcessHandle;
use crate::stdio_server_launcher::StdioServerTransport;
use crate::utils::build_default_headers;
use codex_config::types::OAuthCredentialsStoreMode;

#[path = "streamable_http_retry.rs"]
mod streamable_http_retry;

use self::streamable_http_retry::HandshakeError;
use self::streamable_http_retry::STREAMABLE_HTTP_RETRY_DELAYS_MS;
use self::streamable_http_retry::sleep_with_retry_deadline;

enum PendingTransport {
    InProcess {
        transport: tokio::io::DuplexStream,
    },
    Stdio {
        transport: StdioServerTransport,
    },
    StreamableHttp {
        transport: StreamableHttpClientTransport<StreamableHttpClientAdapter>,
    },
    StreamableHttpWithOAuth {
        transport: StreamableHttpClientTransport<OAuthTransportClient>,
        oauth_persistor: OAuthPersistor,
    },
}

enum ClientState {
    Connecting {
        transport: Option<PendingTransport>,
    },
    Ready {
        service: Arc<RunningService<RoleClient, ElicitationClientService>>,
        oauth: Option<OAuthPersistor>,
    },
    Closed,
}

#[derive(Clone)]
enum TransportRecipe {
    InProcess {
        factory: Arc<dyn InProcessTransportFactory>,
    },
    Stdio {
        command: StdioServerCommand,
        launcher: Arc<dyn StdioServerLauncher>,
    },
    StreamableHttp {
        server_name: String,
        url: String,
        bearer_token: Option<String>,
        http_headers: Option<HashMap<String, String>>,
        env_http_headers: Option<HashMap<String, String>>,
        store_mode: OAuthCredentialsStoreMode,
        keyring_backend_kind: AuthKeyringBackendKind,
        resolved_store: Arc<OnceLock<ResolvedOAuthCredentialStore>>,
        oauth_client: Arc<OnceLock<OAuthTransportClient>>,
        http_client: Arc<dyn HttpClient>,
        auth_provider: Option<SharedAuthProvider>,
    },
}

#[derive(Clone)]
struct InitializeContext {
    timeout: Option<Duration>,
    client_service: ElicitationClientService,
}

#[derive(Clone)]
pub(crate) struct ElicitationPauseState {
    active_count: Arc<AtomicUsize>,
    paused: watch::Sender<bool>,
}

impl ElicitationPauseState {
    fn new() -> Self {
        let (paused, _rx) = watch::channel(false);
        Self {
            active_count: Arc::new(AtomicUsize::new(0)),
            paused,
        }
    }

    pub(crate) fn enter(&self) -> ElicitationPauseGuard {
        if self.active_count.fetch_add(1, Ordering::AcqRel) == 0 {
            self.paused.send_replace(true);
        }
        ElicitationPauseGuard {
            pause_state: self.clone(),
        }
    }

    fn subscribe(&self) -> watch::Receiver<bool> {
        self.paused.subscribe()
    }
}

pub(crate) struct ElicitationPauseGuard {
    pause_state: ElicitationPauseState,
}

impl Drop for ElicitationPauseGuard {
    fn drop(&mut self) {
        if self.pause_state.active_count.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.pause_state.paused.send_replace(false);
        }
    }
}

async fn active_time_timeout<T, Fut>(
    duration: Duration,
    mut pause_state: watch::Receiver<bool>,
    operation: Fut,
) -> std::result::Result<T, ()>
where
    Fut: Future<Output = T>,
{
    let mut remaining = duration;
    tokio::pin!(operation);

    loop {
        if *pause_state.borrow_and_update() {
            tokio::select! {
                result = &mut operation => return Ok(result),
                changed = pause_state.changed() => {
                    if changed.is_err() {
                        return time::timeout(remaining, operation).await.map_err(|_| ());
                    }
                    let _paused = *pause_state.borrow_and_update();
                }
            }
            continue;
        }

        let active_start = Instant::now();
        tokio::select! {
            result = &mut operation => return Ok(result),
            _ = time::sleep(remaining) => {
                return Err(());
            }
            changed = pause_state.changed() => {
                if changed.is_err() {
                    return time::timeout(remaining, operation).await.map_err(|_| ());
                }
                if *pause_state.borrow_and_update() {
                    remaining = remaining.saturating_sub(active_start.elapsed());
                    if remaining.is_zero() {
                        return Err(());
                    }
                }
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
enum ClientOperationError {
    #[error(transparent)]
    Service(#[from] rmcp::service::ServiceError),
    #[error("timed out awaiting {label} after {duration:?}")]
    Timeout { label: String, duration: Duration },
}

fn remaining_operation_timeout(
    label: &str,
    timeout: Option<Duration>,
    deadline: Option<Instant>,
) -> std::result::Result<Option<Duration>, ClientOperationError> {
    let Some(deadline) = deadline else {
        return Ok(None);
    };
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        Err(ClientOperationError::Timeout {
            label: label.to_string(),
            duration: timeout.unwrap_or(remaining),
        })
    } else {
        Ok(Some(remaining))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Elicitation {
    Mcp(CreateElicitationRequestParams),
    OpenAiForm {
        meta: Option<serde_json::Value>,
        message: String,
        requested_schema: serde_json::Value,
    },
}

impl Elicitation {
    pub fn meta(&self) -> Option<&serde_json::Map<String, serde_json::Value>> {
        match self {
            Self::Mcp(request) => request.meta().map(|meta| &meta.0),
            Self::OpenAiForm { meta, .. } => meta.as_ref().and_then(serde_json::Value::as_object),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElicitationResponse {
    pub action: ElicitationAction,
    pub content: Option<serde_json::Value>,
    #[serde(rename = "_meta")]
    pub meta: Option<serde_json::Value>,
}

impl From<CreateElicitationResult> for ElicitationResponse {
    fn from(value: CreateElicitationResult) -> Self {
        Self {
            action: value.action,
            content: value.content,
            meta: None,
        }
    }
}

impl From<ElicitationResponse> for CreateElicitationResult {
    fn from(value: ElicitationResponse) -> Self {
        Self {
            action: value.action,
            content: value.content,
            meta: None,
        }
    }
}

/// Interface for sending elicitation requests to the UI and awaiting a response.
pub type SendElicitation = Box<
    dyn Fn(RequestId, Elicitation) -> BoxFuture<'static, Result<ElicitationResponse>> + Send + Sync,
>;

pub struct ToolWithConnectorId {
    pub tool: Tool,
    pub connector_id: Option<String>,
    pub connector_name: Option<String>,
    pub connector_description: Option<String>,
}

pub struct ListToolsWithConnectorIdResult {
    pub next_cursor: Option<String>,
    pub tools: Vec<ToolWithConnectorId>,
}

/// MCP client implemented on top of the official `rmcp` SDK.
/// https://github.com/modelcontextprotocol/rust-sdk
pub struct RmcpClient {
    state: Mutex<ClientState>,
    stdio_process: Option<StdioServerProcessHandle>,
    transport_recipe: TransportRecipe,
    initialize_context: Mutex<Option<InitializeContext>>,
    session_recovery_lock: Semaphore,
    elicitation_pause_state: ElicitationPauseState,
}

impl RmcpClient {
    pub async fn new_in_process_client(
        factory: Arc<dyn InProcessTransportFactory>,
    ) -> io::Result<Self> {
        let transport_recipe = TransportRecipe::InProcess { factory };
        let transport = Self::create_pending_transport(&transport_recipe)
            .await
            .map_err(io::Error::other)?;

        Ok(Self {
            state: Mutex::new(ClientState::Connecting {
                transport: Some(transport),
            }),
            stdio_process: None,
            transport_recipe,
            initialize_context: Mutex::new(None),
            session_recovery_lock: Semaphore::new(/*permits*/ 1),
            elicitation_pause_state: ElicitationPauseState::new(),
        })
    }

    pub async fn new_stdio_client(
        program: OsString,
        args: Vec<OsString>,
        env: Option<HashMap<OsString, OsString>>,
        env_vars: &[McpServerEnvVar],
        cwd: Option<String>,
        launcher: Arc<dyn StdioServerLauncher>,
    ) -> io::Result<Self> {
        let transport_recipe = TransportRecipe::Stdio {
            command: StdioServerCommand::new(program, args, env, env_vars.to_vec(), cwd),
            launcher,
        };
        let transport = Self::create_pending_transport(&transport_recipe)
            .await
            .map_err(io::Error::other)?;
        let stdio_process = match &transport {
            PendingTransport::Stdio { transport } => Some(transport.process_handle()),
            PendingTransport::InProcess { .. }
            | PendingTransport::StreamableHttp { .. }
            | PendingTransport::StreamableHttpWithOAuth { .. } => None,
        };

        Ok(Self {
            state: Mutex::new(ClientState::Connecting {
                transport: Some(transport),
            }),
            stdio_process,
            transport_recipe,
            initialize_context: Mutex::new(None),
            session_recovery_lock: Semaphore::new(/*permits*/ 1),
            elicitation_pause_state: ElicitationPauseState::new(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn new_streamable_http_client(
        server_name: &str,
        url: &str,
        bearer_token: Option<String>,
        http_headers: Option<HashMap<String, String>>,
        env_http_headers: Option<HashMap<String, String>>,
        store_mode: OAuthCredentialsStoreMode,
        keyring_backend_kind: AuthKeyringBackendKind,
        http_client: Arc<dyn HttpClient>,
        auth_provider: Option<SharedAuthProvider>,
    ) -> Result<Self> {
        let transport_recipe = TransportRecipe::StreamableHttp {
            server_name: server_name.to_string(),
            url: url.to_string(),
            bearer_token,
            http_headers,
            env_http_headers,
            store_mode,
            keyring_backend_kind,
            resolved_store: Arc::new(OnceLock::new()),
            oauth_client: Arc::new(OnceLock::new()),
            http_client,
            auth_provider,
        };
        let transport = Self::create_pending_transport(&transport_recipe).await?;
        Ok(Self {
            state: Mutex::new(ClientState::Connecting {
                transport: Some(transport),
            }),
            stdio_process: None,
            transport_recipe,
            initialize_context: Mutex::new(None),
            session_recovery_lock: Semaphore::new(/*permits*/ 1),
            elicitation_pause_state: ElicitationPauseState::new(),
        })
    }

    /// Perform the initialization handshake with the MCP server.
    /// https://modelcontextprotocol.io/specification/2025-06-18/basic/lifecycle#initialization
    #[instrument(level = "trace", skip_all)]
    pub async fn initialize(
        &self,
        params: InitializeRequestParams,
        timeout: Option<Duration>,
        send_elicitation: SendElicitation,
    ) -> Result<InitializeResult> {
        let client_service = ElicitationClientService::new(
            params.clone(),
            send_elicitation,
            self.elicitation_pause_state.clone(),
        );
        let pending_transport = {
            let mut guard = self.state.lock().await;
            match &mut *guard {
                ClientState::Connecting { transport } => match transport.take() {
                    Some(transport) => transport,
                    None => return Err(anyhow!("client already initializing")),
                },
                ClientState::Ready { .. } => return Err(anyhow!("client already initialized")),
                ClientState::Closed => return Err(anyhow!("MCP client is shut down")),
            }
        };

        let (service, oauth_persistor) = self
            .connect_pending_transport_with_oauth_recovery(
                pending_transport,
                client_service.clone(),
                timeout,
            )
            .await?;

        let initialize_result_rmcp = service
            .peer()
            .peer_info()
            .ok_or_else(|| anyhow!("handshake succeeded but server info was missing"))?;
        let initialize_result = initialize_result_rmcp.as_ref().clone();

        {
            let mut initialize_context = self.initialize_context.lock().await;
            *initialize_context = Some(InitializeContext {
                timeout,
                client_service,
            });
        }

        {
            let mut guard = self.state.lock().await;
            if matches!(*guard, ClientState::Closed) {
                return Err(anyhow!("MCP client is shut down"));
            }
            *guard = ClientState::Ready {
                service,
                oauth: oauth_persistor.clone(),
            };
        }

        Ok(initialize_result)
    }

    pub async fn list_tools(
        &self,
        params: Option<PaginatedRequestParams>,
        timeout: Option<Duration>,
    ) -> Result<ListToolsResult> {
        self.refresh_oauth_if_needed().await?;
        let result = self
            .run_service_operation("tools/list", timeout, move |service| {
                let params = params.clone();
                async move { service.list_tools(params).await }.boxed()
            })
            .await?;
        Ok(result)
    }

    #[instrument(level = "trace", skip_all)]
    pub async fn list_tools_with_connector_ids(
        &self,
        params: Option<PaginatedRequestParams>,
        timeout: Option<Duration>,
    ) -> Result<ListToolsWithConnectorIdResult> {
        self.refresh_oauth_if_needed().await?;
        let result = self
            .run_service_operation("tools/list", timeout, move |service| {
                let params = params.clone();
                async move { service.list_tools(params).await }.boxed()
            })
            .await?;
        let tools = result
            .tools
            .into_iter()
            .map(|tool| {
                let meta = tool.meta.as_ref();
                let connector_id = Self::meta_string(meta, "connector_id");
                let connector_name = Self::meta_string(meta, "connector_name")
                    .or_else(|| Self::meta_string(meta, "connector_display_name"));
                let connector_description = Self::meta_string(meta, "connector_description")
                    .or_else(|| Self::meta_string(meta, "connectorDescription"));
                Ok(ToolWithConnectorId {
                    tool,
                    connector_id,
                    connector_name,
                    connector_description,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(ListToolsWithConnectorIdResult {
            next_cursor: result.next_cursor,
            tools,
        })
    }

    fn meta_string(meta: Option<&rmcp::model::Meta>, key: &str) -> Option<String> {
        meta.and_then(|meta| meta.get(key))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    }

    pub async fn list_resources(
        &self,
        params: Option<PaginatedRequestParams>,
        timeout: Option<Duration>,
    ) -> Result<ListResourcesResult> {
        self.refresh_oauth_if_needed().await?;
        let result = self
            .run_service_operation("resources/list", timeout, move |service| {
                let params = params.clone();
                async move { service.list_resources(params).await }.boxed()
            })
            .await?;
        Ok(result)
    }

    pub async fn list_resource_templates(
        &self,
        params: Option<PaginatedRequestParams>,
        timeout: Option<Duration>,
    ) -> Result<ListResourceTemplatesResult> {
        self.refresh_oauth_if_needed().await?;
        let result = self
            .run_service_operation("resources/templates/list", timeout, move |service| {
                let params = params.clone();
                async move { service.list_resource_templates(params).await }.boxed()
            })
            .await?;
        Ok(result)
    }

    pub async fn read_resource(
        &self,
        params: ReadResourceRequestParams,
        timeout: Option<Duration>,
    ) -> Result<ReadResourceResult> {
        self.refresh_oauth_if_needed().await?;
        let result = self
            .run_service_operation("resources/read", timeout, move |service| {
                let params = params.clone();
                async move { service.read_resource(params).await }.boxed()
            })
            .await?;
        Ok(result)
    }

    pub async fn call_tool(
        &self,
        name: String,
        arguments: Option<serde_json::Value>,
        meta: Option<serde_json::Value>,
        timeout: Option<Duration>,
    ) -> Result<CallToolResult> {
        self.refresh_oauth_if_needed().await?;
        let arguments = match arguments {
            Some(Value::Object(map)) => Some(map),
            Some(other) => {
                return Err(anyhow!(
                    "MCP tool arguments must be a JSON object, got {other}"
                ));
            }
            None => None,
        };
        let meta = match meta {
            Some(Value::Object(map)) => Some(rmcp::model::Meta(map)),
            Some(other) => {
                return Err(anyhow!(
                    "MCP tool request _meta must be a JSON object, got {other}"
                ));
            }
            None => None,
        };
        let mut rmcp_params = CallToolRequestParams::new(name);
        rmcp_params.arguments = arguments;
        let result = self
            .run_service_operation("tools/call", timeout, move |service| {
                let rmcp_params = rmcp_params.clone();
                let meta = meta.clone();
                async move {
                    let mut options = rmcp::service::PeerRequestOptions::no_options();
                    options.meta = meta;
                    let result = service
                        .peer()
                        .send_request_with_option(
                            ClientRequest::CallToolRequest(rmcp::model::CallToolRequest::new(
                                rmcp_params,
                            )),
                            options,
                        )
                        .await?
                        .await_response()
                        .await?;
                    match result {
                        ServerResult::CallToolResult(result) => Ok(result),
                        _ => Err(rmcp::service::ServiceError::UnexpectedResponse),
                    }
                }
                .boxed()
            })
            .await?;
        Ok(result)
    }

    pub async fn send_custom_notification(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<()> {
        self.refresh_oauth_if_needed().await?;
        self.run_service_operation(
            "notifications/custom",
            /*timeout*/ None,
            move |service| {
                let params = params.clone();
                async move {
                    service
                        .send_notification(ClientNotification::CustomNotification(
                            CustomNotification {
                                method: method.to_string(),
                                params,
                                extensions: Extensions::new(),
                            },
                        ))
                        .await
                }
                .boxed()
            },
        )
        .await?;
        Ok(())
    }

    pub async fn send_custom_request(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<ServerResult> {
        self.refresh_oauth_if_needed().await?;
        let response = self
            .run_service_operation("requests/custom", /*timeout*/ None, move |service| {
                let params = params.clone();
                async move {
                    service
                        .send_request(ClientRequest::CustomRequest(CustomRequest::new(
                            method, params,
                        )))
                        .await
                }
                .boxed()
            })
            .await?;
        Ok(response)
    }

    async fn service(&self) -> Result<Arc<RunningService<RoleClient, ElicitationClientService>>> {
        self.service_and_oauth_persistor()
            .await
            .map(|(service, _oauth_persistor)| service)
    }

    async fn service_and_oauth_persistor(
        &self,
    ) -> Result<(
        Arc<RunningService<RoleClient, ElicitationClientService>>,
        Option<OAuthPersistor>,
    )> {
        let guard = self.state.lock().await;
        match &*guard {
            ClientState::Ready { service, oauth } => Ok((Arc::clone(service), oauth.clone())),
            ClientState::Connecting { .. } => Err(anyhow!("MCP client not initialized")),
            ClientState::Closed => Err(anyhow!("MCP client is shut down")),
        }
    }

    async fn oauth_persistor(&self) -> Option<OAuthPersistor> {
        let guard = self.state.lock().await;
        match &*guard {
            ClientState::Ready {
                oauth: Some(runtime),
                ..
            } => Some(runtime.clone()),
            _ => None,
        }
    }

    /// Stop the MCP transport and any stdio server process owned by this client.
    pub async fn shutdown(&self) {
        let previous_state = {
            let mut guard = self.state.lock().await;
            std::mem::replace(&mut *guard, ClientState::Closed)
        };

        if let Some(process) = &self.stdio_process
            && let Err(error) = process.terminate().await
        {
            warn!("failed to terminate MCP stdio server process: {error}");
        }

        drop(previous_state);
    }

    async fn refresh_oauth_if_needed(&self) -> Result<()> {
        if let Some(runtime) = self.oauth_persistor().await {
            runtime.refresh_if_needed().await?;
        }
        Ok(())
    }

    async fn create_pending_transport(
        transport_recipe: &TransportRecipe,
    ) -> Result<PendingTransport> {
        match transport_recipe {
            TransportRecipe::InProcess { factory } => {
                let transport = factory.open().await?;
                Ok(PendingTransport::InProcess { transport })
            }
            TransportRecipe::Stdio { command, launcher } => {
                let transport = launcher.launch(command.clone()).await?;
                Ok(PendingTransport::Stdio { transport })
            }
            TransportRecipe::StreamableHttp {
                server_name,
                url,
                bearer_token,
                http_headers,
                env_http_headers,
                store_mode,
                keyring_backend_kind,
                resolved_store,
                oauth_client,
                http_client,
                auth_provider,
            } => {
                let default_headers =
                    build_default_headers(http_headers.clone(), env_http_headers.clone())?;
                let auth_provider =
                    if bearer_token.is_some() || default_headers.contains_key(AUTHORIZATION) {
                        None
                    } else {
                        auth_provider.clone()
                    };

                // Reuse one OAuth manager and persistor across initialize retries and session
                // reconstruction. This preserves the lifecycle-pinned store and keeps each failed
                // request paired with the manager snapshot that supplied its access token.
                if let Some(oauth_client) = oauth_client.get() {
                    let runtime = oauth_client.persistor();
                    let transport = StreamableHttpClientTransport::with_client(
                        oauth_client.clone(),
                        StreamableHttpClientTransportConfig::with_uri(url.clone()),
                    );
                    return Ok(PendingTransport::StreamableHttpWithOAuth {
                        transport,
                        oauth_persistor: runtime,
                    });
                }

                let resolved_oauth_tokens = if bearer_token.is_none()
                    && auth_provider.is_none()
                    && !default_headers.contains_key(AUTHORIZATION)
                {
                    if let Some(store) = resolved_store.get().copied() {
                        load_oauth_tokens_from_store(&DefaultKeyringStore, server_name, url, store)?
                            .map(|tokens| ResolvedOAuthTokens { tokens, store })
                    } else {
                        match resolve_oauth_tokens(
                            &DefaultKeyringStore,
                            server_name,
                            url,
                            *store_mode,
                            *keyring_backend_kind,
                        ) {
                            Ok(tokens) => {
                                if let Some(resolved) = tokens.as_ref() {
                                    // Transport retries and session recovery are part of the same
                                    // client lifecycle. Pin the first concrete source in memory so
                                    // rebuilding a transport never re-evaluates Auto and adopts a
                                    // possibly stale credential from another store.
                                    resolved_store.set(resolved.store).map_err(|_| {
                                        anyhow!(
                                            "OAuth credential store resolved concurrently for MCP server `{server_name}`"
                                        )
                                    })?;
                                }
                                tokens
                            }
                            Err(err) => {
                                warn!("failed to read tokens for server `{server_name}`: {err}");
                                None
                            }
                        }
                    }
                } else {
                    None
                };

                if let Some(ResolvedOAuthTokens {
                    tokens: initial_tokens,
                    store: credential_store,
                }) = resolved_oauth_tokens
                {
                    match create_oauth_transport_client(
                        server_name,
                        url,
                        initial_tokens.clone(),
                        credential_store,
                        default_headers.clone(),
                        Arc::clone(http_client),
                    )
                    .await
                    {
                        Ok(resolved_oauth_client) => {
                            oauth_client
                                .set(resolved_oauth_client.clone())
                                .map_err(|_| {
                                    anyhow!(
                                        "OAuth client resolved concurrently for MCP server `{server_name}`"
                                    )
                                })?;
                            let oauth_persistor = resolved_oauth_client.persistor();
                            let transport = StreamableHttpClientTransport::with_client(
                                resolved_oauth_client,
                                StreamableHttpClientTransportConfig::with_uri(url.clone()),
                            );
                            Ok(PendingTransport::StreamableHttpWithOAuth {
                                transport,
                                oauth_persistor,
                            })
                        }
                        Err(err)
                            if err.downcast_ref::<AuthError>().is_some_and(|auth_err| {
                                matches!(auth_err, AuthError::NoAuthorizationSupport)
                            }) =>
                        {
                            let access_token = initial_tokens
                                .token_response
                                .0
                                .access_token()
                                .secret()
                                .to_string();
                            warn!(
                                "OAuth metadata discovery is unavailable for MCP server `{server_name}`; falling back to stored bearer token authentication"
                            );
                            let http_config =
                                StreamableHttpClientTransportConfig::with_uri(url.clone())
                                    .auth_header(access_token);
                            let transport = StreamableHttpClientTransport::with_client(
                                StreamableHttpClientAdapter::new(
                                    Arc::clone(http_client),
                                    default_headers,
                                    /*auth_provider*/ None,
                                ),
                                http_config,
                            );
                            Ok(PendingTransport::StreamableHttp { transport })
                        }
                        Err(err) => Err(err),
                    }
                } else {
                    let mut http_config =
                        StreamableHttpClientTransportConfig::with_uri(url.clone());
                    if let Some(bearer_token) = bearer_token.clone() {
                        http_config = http_config.auth_header(bearer_token);
                    }

                    let transport = StreamableHttpClientTransport::with_client(
                        StreamableHttpClientAdapter::new(
                            Arc::clone(http_client),
                            default_headers,
                            auth_provider,
                        ),
                        http_config,
                    );
                    Ok(PendingTransport::StreamableHttp { transport })
                }
            }
        }
    }

    async fn connect_pending_transport(
        pending_transport: PendingTransport,
        client_service: ElicitationClientService,
        timeout: Option<Duration>,
    ) -> Result<(
        Arc<RunningService<RoleClient, ElicitationClientService>>,
        Option<OAuthPersistor>,
    )> {
        let (transport, oauth_persistor) = match pending_transport {
            PendingTransport::InProcess { transport } => (
                service::serve_client(client_service, transport).boxed(),
                None,
            ),
            PendingTransport::Stdio { transport } => (
                service::serve_client(client_service, transport).boxed(),
                None,
            ),
            PendingTransport::StreamableHttp { transport } => (
                service::serve_client(client_service, transport).boxed(),
                None,
            ),
            PendingTransport::StreamableHttpWithOAuth {
                transport,
                oauth_persistor,
            } => (
                service::serve_client(client_service, transport).boxed(),
                Some(oauth_persistor),
            ),
        };

        let service_result = match timeout {
            Some(duration) => match time::timeout(duration, transport).await {
                Ok(result) => {
                    result.map_err(|source| anyhow::Error::from(HandshakeError { source }))
                }
                Err(_elapsed) => Err(anyhow!(
                    "timed out handshaking with MCP server after {duration:?}"
                )),
            },
            None => transport
                .await
                .map_err(|source| anyhow::Error::from(HandshakeError { source })),
        };
        let service = service_result?;

        Ok((Arc::new(service), oauth_persistor))
    }

    async fn run_service_operation<T, F, Fut>(
        &self,
        label: &str,
        timeout: Option<Duration>,
        operation: F,
    ) -> Result<T>
    where
        F: Fn(Arc<RunningService<RoleClient, ElicitationClientService>>) -> Fut,
        Fut: std::future::Future<Output = std::result::Result<T, rmcp::service::ServiceError>>,
    {
        let deadline = timeout.map(|duration| Instant::now() + duration);
        // Keep the OAuth persistor paired with the service that performs this operation. Session
        // recovery can replace both while the request is in flight; rereading only the persistor
        // after a 401 could refresh credentials owned by a different transport lifecycle.
        let (service, oauth_persistor) = self.service_and_oauth_persistor().await?;
        let mut result = Self::run_service_operation_with_transient_retries(
            Arc::clone(&service),
            label,
            timeout,
            deadline,
            self.elicitation_pause_state.clone(),
            &operation,
        )
        .await;

        if let Some(rejected_access_token) = result
            .as_ref()
            .err()
            .and_then(Self::rejected_access_token_from_operation_error)
            && let Some(oauth_persistor) = oauth_persistor
        {
            // Public request/notification recovery stays here rather than in the transport
            // wrapper because this layer owns the caller deadline. RMCP can continue processing a
            // queued transport message after the caller times out; retrying it inside the wrapper
            // could therefore replay a timed-out tool call. The refresh transaction itself is
            // independently owned and completes to its bounded provider timeout if this caller is
            // canceled.
            let remaining = remaining_operation_timeout(label, timeout, deadline)?;
            let refresh = oauth_persistor.refresh_after_unauthorized(rejected_access_token);
            let refresh_result = match remaining {
                Some(remaining) => match time::timeout(remaining, refresh).await {
                    Ok(result) => result,
                    Err(_) => {
                        // `refresh_after_unauthorized` spawns the credential transaction before it
                        // waits. Dropping this caller wait therefore detaches the JoinHandle while
                        // the transaction retains the credential lock and continues through its
                        // own provider/persistence bounds. The public operation still honors the
                        // timeout it advertised and does not replay the rejected request later.
                        return Err(ClientOperationError::Timeout {
                            label: label.to_string(),
                            duration: timeout.unwrap_or(remaining),
                        }
                        .into());
                    }
                },
                None => refresh.await,
            };
            if let Err(error) = refresh_result {
                if let Err(timeout_error) = remaining_operation_timeout(label, timeout, deadline) {
                    return Err(timeout_error.into());
                }
                return Err(error);
            }
            result = Self::run_service_operation_with_transient_retries(
                Arc::clone(&service),
                label,
                timeout,
                deadline,
                self.elicitation_pause_state.clone(),
                &operation,
            )
            .await;
            if result
                .as_ref()
                .err()
                .and_then(Self::rejected_access_token_from_operation_error)
                .is_some()
            {
                // The rejected token is needed only to attribute the first 401. A second 401
                // after the one allowed refresh means this lifecycle needs reauthentication.
                return Err(AuthError::AuthorizationRequired.into());
            }
        }

        if result.as_ref().is_err_and(Self::is_session_expired_404) {
            // Session recovery remains one-shot and runs after the optional OAuth retry, so a 401
            // followed by the old session's 404 still reconstructs the transport before retrying.
            self.reinitialize_after_session_expiry(&service).await?;
            let recovered_service = self.service().await?;
            result = Self::run_service_operation_with_transient_retries(
                recovered_service,
                label,
                timeout,
                deadline,
                self.elicitation_pause_state.clone(),
                &operation,
            )
            .await;
        }

        result.map_err(Into::into)
    }

    async fn run_service_operation_with_transient_retries<T, F, Fut>(
        service: Arc<RunningService<RoleClient, ElicitationClientService>>,
        label: &str,
        timeout: Option<Duration>,
        retry_deadline: Option<Instant>,
        pause_state: ElicitationPauseState,
        operation: &F,
    ) -> std::result::Result<T, ClientOperationError>
    where
        F: Fn(Arc<RunningService<RoleClient, ElicitationClientService>>) -> Fut,
        Fut: std::future::Future<Output = std::result::Result<T, rmcp::service::ServiceError>>,
    {
        for (attempt, retry_delay_ms) in STREAMABLE_HTTP_RETRY_DELAYS_MS
            .iter()
            .copied()
            .map(Some)
            .chain(std::iter::once(None))
            .enumerate()
        {
            let attempt_timeout = remaining_operation_timeout(label, timeout, retry_deadline)?;
            match Self::run_service_operation_once(
                Arc::clone(&service),
                label,
                attempt_timeout,
                pause_state.clone(),
                operation,
            )
            .await
            {
                Ok(result) => return Ok(result),
                Err(error) if Self::is_retryable_tools_list_error(label, &error) => {
                    let Some(retry_delay_ms) = retry_delay_ms else {
                        return Err(error);
                    };
                    let delay = Duration::from_millis(retry_delay_ms);
                    warn!(
                        attempt = attempt + 1,
                        max_attempts = STREAMABLE_HTTP_RETRY_DELAYS_MS.len() + 1,
                        delay_ms = delay.as_millis(),
                        error = %error,
                        "streamable HTTP MCP tools/list failed with a retryable error; retrying"
                    );
                    if !sleep_with_retry_deadline(delay, retry_deadline).await {
                        return Err(ClientOperationError::Timeout {
                            label: label.to_string(),
                            duration: timeout.unwrap_or(delay),
                        });
                    }
                }
                Err(error) => return Err(error),
            }
        }

        unreachable!("service operation retry loop should return on success or final error")
    }

    async fn run_service_operation_once<T, F, Fut>(
        service: Arc<RunningService<RoleClient, ElicitationClientService>>,
        label: &str,
        timeout: Option<Duration>,
        pause_state: ElicitationPauseState,
        operation: &F,
    ) -> std::result::Result<T, ClientOperationError>
    where
        F: Fn(Arc<RunningService<RoleClient, ElicitationClientService>>) -> Fut,
        Fut: std::future::Future<Output = std::result::Result<T, rmcp::service::ServiceError>>,
    {
        match timeout {
            Some(duration) => {
                active_time_timeout(duration, pause_state.subscribe(), operation(service))
                    .await
                    .map_err(|_| ClientOperationError::Timeout {
                        label: label.to_string(),
                        duration,
                    })?
                    .map_err(ClientOperationError::from)
            }
            None => operation(service).await.map_err(ClientOperationError::from),
        }
    }

    fn is_retryable_tools_list_error(label: &str, error: &ClientOperationError) -> bool {
        if label != "tools/list" {
            return false;
        }
        let ClientOperationError::Service(rmcp::service::ServiceError::TransportSend(error)) =
            error
        else {
            return false;
        };

        error
            .error
            .downcast_ref::<StreamableHttpError<StreamableHttpClientAdapterError>>()
            .is_some_and(Self::is_retryable_streamable_http_error)
    }

    fn is_session_expired_404(error: &ClientOperationError) -> bool {
        let ClientOperationError::Service(rmcp::service::ServiceError::TransportSend(error)) =
            error
        else {
            return false;
        };

        error
            .error
            .downcast_ref::<StreamableHttpError<StreamableHttpClientAdapterError>>()
            .is_some_and(|error| {
                matches!(
                    error,
                    StreamableHttpError::Client(
                        StreamableHttpClientAdapterError::SessionExpired404
                    )
                )
            })
    }

    fn rejected_access_token_from_operation_error(
        error: &ClientOperationError,
    ) -> Option<AccessToken> {
        let ClientOperationError::Service(rmcp::service::ServiceError::TransportSend(error)) =
            error
        else {
            return None;
        };

        error
            .error
            .downcast_ref::<StreamableHttpError<StreamableHttpClientAdapterError>>()
            .and_then(Self::rejected_access_token)
    }

    pub(super) fn rejected_access_token(
        error: &StreamableHttpError<StreamableHttpClientAdapterError>,
    ) -> Option<AccessToken> {
        match error {
            StreamableHttpError::Client(
                StreamableHttpClientAdapterError::AccessTokenRejected {
                    rejected_access_token,
                },
            ) => Some(rejected_access_token.clone()),
            _ => None,
        }
    }

    async fn reinitialize_after_session_expiry(
        &self,
        failed_service: &Arc<RunningService<RoleClient, ElicitationClientService>>,
    ) -> Result<()> {
        let _recovery_guard = self
            .session_recovery_lock
            .acquire()
            .await
            .map_err(|_| anyhow!("MCP client recovery semaphore closed"))?;

        {
            let guard = self.state.lock().await;
            match &*guard {
                ClientState::Ready { service, .. } if !Arc::ptr_eq(service, failed_service) => {
                    return Ok(());
                }
                ClientState::Ready { .. } => {}
                ClientState::Connecting { .. } => {
                    return Err(anyhow!("MCP client not initialized"));
                }
                ClientState::Closed => {
                    return Err(anyhow!("MCP client is shut down"));
                }
            }
        }

        let initialize_context = self
            .initialize_context
            .lock()
            .await
            .clone()
            .ok_or_else(|| anyhow!("MCP client cannot recover before initialize succeeds"))?;
        let pending_transport = Self::create_pending_transport(&self.transport_recipe).await?;
        let (service, oauth_persistor) = self
            .connect_pending_transport_with_oauth_recovery(
                pending_transport,
                initialize_context.client_service,
                initialize_context.timeout,
            )
            .await?;

        {
            let mut guard = self.state.lock().await;
            if matches!(*guard, ClientState::Closed) {
                return Err(anyhow!("MCP client is shut down"));
            }
            *guard = ClientState::Ready {
                service,
                oauth: oauth_persistor.clone(),
            };
        }

        Ok(())
    }
}

async fn create_oauth_transport_client(
    server_name: &str,
    url: &str,
    initial_tokens: StoredOAuthTokens,
    credential_store: ResolvedOAuthCredentialStore,
    default_headers: HeaderMap,
    http_client: Arc<dyn HttpClient>,
) -> Result<OAuthTransportClient> {
    let oauth_http_client = Arc::new(OAuthHttpClientAdapter::new(
        http_client.clone(),
        default_headers.clone(),
    ));
    let mut oauth_state =
        OAuthState::new_with_oauth_http_client(url.to_string(), oauth_http_client).await?;

    oauth_state
        .set_credentials(
            &initial_tokens.client_id,
            request_oauth_token_response(&initial_tokens),
        )
        .await?;

    let manager = match oauth_state {
        OAuthState::Authorized(manager) => manager,
        OAuthState::Unauthorized(manager) => manager,
        _ => {
            return Err(anyhow!("unexpected OAuth state during client setup"));
        }
    };

    let auth_client = AuthClient::new(
        StreamableHttpClientAdapter::new(http_client, default_headers, /*auth_provider*/ None),
        manager,
    );
    let auth_manager = auth_client.auth_manager.clone();

    let runtime = OAuthPersistor::new(
        server_name.to_string(),
        url.to_string(),
        auth_manager,
        credential_store,
        Some(initial_tokens),
    );

    Ok(OAuthTransportClient::new(auth_client, runtime))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use pretty_assertions::assert_eq;
    use tokio::time;

    use super::*;

    #[tokio::test]
    async fn active_time_timeout_pauses_while_elicitation_is_pending() {
        let pause_state = ElicitationPauseState::new();
        let pause = pause_state.enter();
        tokio::spawn(async move {
            time::sleep(Duration::from_millis(75)).await;
            drop(pause);
        });

        let result =
            active_time_timeout(Duration::from_millis(50), pause_state.subscribe(), async {
                time::sleep(Duration::from_millis(90)).await;
                "done"
            })
            .await;

        assert_eq!(Ok("done"), result);
    }
}
