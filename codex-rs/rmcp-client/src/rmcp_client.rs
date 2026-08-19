use std::collections::HashMap;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::future::Future;
use std::io;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::OnceLock;
use std::sync::PoisonError;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use codex_api::SharedAuthProvider;
use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::McpServerEnvVar;
use codex_exec_server::HttpClient;
use codex_keyring_store::DefaultKeyringStore;
use futures::FutureExt;
use futures::future::BoxFuture;
use http::HeaderMap;
use http::header::AUTHORIZATION;
use oauth2::TokenResponse;
use rmcp::model::CallToolRequestParams;
use rmcp::model::CallToolResult;
use rmcp::model::ClientNotification;
use rmcp::model::ClientRequest;
use rmcp::model::CustomNotification;
use rmcp::model::CustomRequest;
use rmcp::model::ElicitRequestParams;
use rmcp::model::ElicitResult;
use rmcp::model::ElicitationAction;
use rmcp::model::Extensions;
use rmcp::model::InitializeRequestParams;
use rmcp::model::ListResourceTemplatesResult;
use rmcp::model::ListResourcesResult;
use rmcp::model::ListToolsResult;
use rmcp::model::PaginatedRequestParams;
use rmcp::model::ProtocolVersion;
use rmcp::model::ReadResourceRequestParams;
use rmcp::model::ReadResourceResult;
use rmcp::model::RequestId;
use rmcp::model::RequestMetaObject;
use rmcp::model::RequestParamsMeta;
use rmcp::model::ServerPeerInfo;
use rmcp::model::ServerResult;
use rmcp::model::Tool;
use rmcp::service::ClientCacheConfig;
use rmcp::service::ClientServiceExt;
use rmcp::service::RequestHandle;
use rmcp::service::RoleClient;
use rmcp::service::RunningService;
use rmcp::transport::AuthorizationManager;
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::auth::AuthClient;
use rmcp::transport::auth::AuthError;
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
use crate::event_notification_transport::capture_event_notifications;
use crate::event_notification_transport::event_notification_channel;
use crate::http_client_adapter::StreamableHttpClientAdapter;
use crate::http_client_adapter::StreamableHttpClientAdapterError;
use crate::http_client_adapter::StreamableHttpRedirectMode;
use crate::in_process_transport::InProcessTransportFactory;
use crate::oauth::OAuthPersistor;
use crate::oauth::ResolvedOAuthCredentialStore;
use crate::oauth::ResolvedOAuthTokens;
use crate::oauth::StoredOAuthTokens;
use crate::oauth::install_tokens_in_manager;
use crate::oauth::resolve_oauth_tokens_from_store_policy;
use crate::oauth::validate_refresh_token_issuer;
use crate::oauth_http_client::OAuthHttpClientAdapter;
use crate::protocol_mode::McpProtocolMode;
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
        transport: Box<StdioServerTransport>,
    },
    StreamableHttp {
        transport: StreamableHttpClientTransport<StreamableHttpClientAdapter>,
    },
    StreamableHttpWithOAuth {
        transport: StreamableHttpClientTransport<AuthClient<StreamableHttpClientAdapter>>,
        oauth_persistor: OAuthPersistor,
    },
    StreamableHttpWithAccessTokenOnly {
        transport: StreamableHttpClientTransport<AuthClient<StreamableHttpClientAdapter>>,
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
        pinned_credential_store: Arc<OnceLock<ResolvedOAuthCredentialStore>>,
        http_client: Arc<dyn HttpClient>,
        auth_provider: Option<SharedAuthProvider>,
        redirect_mode: StreamableHttpRedirectMode,
        initialize_deadline: Arc<StdMutex<Option<Instant>>>,
    },
}

struct InitializeDeadlineGuard {
    deadline: Arc<StdMutex<Option<Instant>>>,
}

impl Drop for InitializeDeadlineGuard {
    fn drop(&mut self) {
        *self.deadline.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }
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
pub(crate) enum ClientOperationError {
    #[error(transparent)]
    Service(#[from] rmcp::service::ServiceError),
    #[error("timed out awaiting {label} after {duration:.0?}")]
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
    Mcp(ElicitRequestParams),
    OpenAiForm {
        meta: Option<serde_json::Value>,
        message: String,
        requested_schema: serde_json::Value,
    },
}

impl Elicitation {
    pub fn meta(&self) -> Option<&serde_json::Map<String, serde_json::Value>> {
        match self {
            Self::Mcp(request) => request.meta().map(|meta| &meta.0.0),
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

impl From<ElicitResult> for ElicitationResponse {
    fn from(value: ElicitResult) -> Self {
        Self {
            action: value.action,
            content: value.content,
            meta: value.meta.map(|meta| Value::Object(meta.0)),
        }
    }
}

impl From<ElicitationResponse> for ElicitResult {
    fn from(value: ElicitationResponse) -> Self {
        let mut result = Self::new(value.action);
        result.content = value.content;
        result.meta = value.meta.and_then(|meta| match meta {
            Value::Object(meta) => Some(rmcp::model::MetaObject::from(meta)),
            _ => None,
        });
        result
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

/// An active Plugin Runtime event request and its request-scoped notifications.
pub struct CancellableEventStreamRequest {
    pub handle: RequestHandle<RoleClient>,
    pub notifications: crate::EventNotificationReceiver,
}

/// MCP client implemented on top of the official `rmcp` SDK.
/// https://github.com/modelcontextprotocol/rust-sdk
pub struct RmcpClient {
    state: Mutex<ClientState>,
    stdio_process: Option<StdioServerProcessHandle>,
    transport_recipe: TransportRecipe,
    protocol_mode: McpProtocolMode,
    initialize_context: Mutex<Option<InitializeContext>>,
    session_recovery_lock: Semaphore,
    elicitation_pause_state: ElicitationPauseState,
}

impl RmcpClient {
    /// Returns the protocol compatibility policy captured when this client was created.
    pub fn protocol_mode(&self) -> McpProtocolMode {
        self.protocol_mode
    }

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
            protocol_mode: McpProtocolMode::Legacy,
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
        Self::new_stdio_client_with_protocol_mode(
            program,
            args,
            env,
            env_vars,
            cwd,
            launcher,
            McpProtocolMode::Legacy,
        )
        .await
    }

    /// Constructs a stdio client with an explicitly selected compatibility policy.
    #[allow(clippy::too_many_arguments)]
    pub async fn new_stdio_client_with_protocol_mode(
        program: OsString,
        args: Vec<OsString>,
        mut env: Option<HashMap<OsString, OsString>>,
        env_vars: &[McpServerEnvVar],
        cwd: Option<String>,
        launcher: Arc<dyn StdioServerLauncher>,
        protocol_mode: McpProtocolMode,
    ) -> io::Result<Self> {
        let requested_stdio_version = match protocol_mode {
            McpProtocolMode::Legacy => None,
            McpProtocolMode::V20260728 => env
                .as_mut()
                .and_then(|env| env.remove(OsStr::new("CODEX_MCP_PROTOCOL_VERSION"))),
        };
        let protocol_mode = protocol_mode.stdio_mode(requested_stdio_version.as_deref())?;
        let transport_recipe = TransportRecipe::Stdio {
            command: StdioServerCommand::new(
                program,
                args,
                env,
                env_vars.to_vec(),
                cwd,
                protocol_mode,
            ),
            launcher,
        };
        let transport = Self::create_pending_transport(&transport_recipe)
            .await
            .map_err(io::Error::other)?;
        let stdio_process = match &transport {
            PendingTransport::Stdio { transport } => Some(transport.process_handle()),
            PendingTransport::InProcess { .. }
            | PendingTransport::StreamableHttp { .. }
            | PendingTransport::StreamableHttpWithOAuth { .. }
            | PendingTransport::StreamableHttpWithAccessTokenOnly { .. } => None,
        };

        Ok(Self {
            state: Mutex::new(ClientState::Connecting {
                transport: Some(transport),
            }),
            stdio_process,
            transport_recipe,
            protocol_mode,
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
        Self::new_streamable_http_client_with_protocol_mode(
            server_name,
            url,
            bearer_token,
            http_headers,
            env_http_headers,
            store_mode,
            keyring_backend_kind,
            http_client,
            auth_provider,
            McpProtocolMode::Legacy,
        )
        .await
    }

    /// Constructs a streamable HTTP client with an explicitly selected compatibility policy.
    #[allow(clippy::too_many_arguments)]
    pub async fn new_streamable_http_client_with_protocol_mode(
        server_name: &str,
        url: &str,
        bearer_token: Option<String>,
        http_headers: Option<HashMap<String, String>>,
        env_http_headers: Option<HashMap<String, String>>,
        store_mode: OAuthCredentialsStoreMode,
        keyring_backend_kind: AuthKeyringBackendKind,
        http_client: Arc<dyn HttpClient>,
        auth_provider: Option<SharedAuthProvider>,
        protocol_mode: McpProtocolMode,
    ) -> Result<Self> {
        Self::new_streamable_http_client_with_protocol_mode_and_redirect_mode(
            server_name,
            url,
            bearer_token,
            http_headers,
            env_http_headers,
            store_mode,
            keyring_backend_kind,
            http_client,
            auth_provider,
            protocol_mode,
            StreamableHttpRedirectMode::Legacy,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn new_streamable_http_client_with_protocol_mode_and_redirect_mode(
        server_name: &str,
        url: &str,
        bearer_token: Option<String>,
        http_headers: Option<HashMap<String, String>>,
        env_http_headers: Option<HashMap<String, String>>,
        store_mode: OAuthCredentialsStoreMode,
        keyring_backend_kind: AuthKeyringBackendKind,
        http_client: Arc<dyn HttpClient>,
        auth_provider: Option<SharedAuthProvider>,
        protocol_mode: McpProtocolMode,
        redirect_mode: StreamableHttpRedirectMode,
    ) -> Result<Self> {
        let transport_recipe = TransportRecipe::StreamableHttp {
            server_name: server_name.to_string(),
            url: url.to_string(),
            bearer_token,
            http_headers,
            env_http_headers,
            store_mode,
            keyring_backend_kind,
            pinned_credential_store: Arc::new(OnceLock::new()),
            http_client,
            auth_provider,
            redirect_mode,
            initialize_deadline: Arc::new(StdMutex::new(None)),
        };
        let transport = Self::create_pending_transport(&transport_recipe).await?;
        Ok(Self {
            state: Mutex::new(ClientState::Connecting {
                transport: Some(transport),
            }),
            stdio_process: None,
            transport_recipe,
            protocol_mode,
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
    ) -> Result<ServerPeerInfo> {
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
            .connect_pending_transport_with_initialize_retries(
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

        if let Some(runtime) = oauth_persistor
            && let Err(error) = runtime.persist_if_needed().await
        {
            warn!("failed to persist OAuth tokens after initialize: {error}");
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
        self.persist_oauth_tokens().await;
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
        self.persist_oauth_tokens().await;
        Ok(ListToolsWithConnectorIdResult {
            next_cursor: result.next_cursor,
            tools,
        })
    }

    fn meta_string(meta: Option<&rmcp::model::MetaObject>, key: &str) -> Option<String> {
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
        self.persist_oauth_tokens().await;
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
        self.persist_oauth_tokens().await;
        Ok(result)
    }

    pub async fn read_resource(
        &self,
        params: ReadResourceRequestParams,
        timeout: Option<Duration>,
    ) -> Result<ReadResourceResult> {
        self.refresh_oauth_if_needed().await?;
        let requested_modern = self.protocol_mode == McpProtocolMode::V20260728;
        let result = self
            .run_service_operation("resources/read", timeout, move |service| {
                let params = params.clone();
                async move {
                    let modern_session = requested_modern
                        && service.peer().peer_info().is_some_and(|info| {
                            info.protocol_version == ProtocolVersion::V_2026_07_28
                        });
                    if modern_session {
                        service.read_resource(params).await
                    } else {
                        service.peer().read_resource(params).await
                    }
                }
                .boxed()
            })
            .await?;
        self.persist_oauth_tokens().await;
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
            Some(Value::Object(map)) => Some(RequestMetaObject::from(map)),
            Some(other) => {
                return Err(anyhow!(
                    "MCP tool request _meta must be a JSON object, got {other}"
                ));
            }
            None => None,
        };
        let mut rmcp_params = CallToolRequestParams::new(name);
        rmcp_params.arguments = arguments;
        let requested_modern = self.protocol_mode == McpProtocolMode::V20260728;
        let result = self
            .run_service_operation("tools/call", timeout, move |service| {
                let mut rmcp_params = rmcp_params.clone();
                let meta = meta.clone();
                async move {
                    let modern_session = requested_modern
                        && service.peer().peer_info().is_some_and(|info| {
                            info.protocol_version == ProtocolVersion::V_2026_07_28
                        });
                    if modern_session {
                        rmcp_params.meta = meta;
                        return service.call_tool(rmcp_params).await;
                    }
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
        self.persist_oauth_tokens().await;
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
        self.persist_oauth_tokens().await;
        Ok(())
    }

    pub async fn send_custom_request(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<ServerResult> {
        self.send_custom_request_with_timeout(method, params, /*timeout*/ None)
            .await
    }

    pub async fn send_custom_request_with_timeout(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
        timeout: Option<Duration>,
    ) -> Result<ServerResult> {
        self.refresh_oauth_if_needed().await?;
        let response = self
            .run_service_operation("requests/custom", timeout, move |service| {
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
        self.persist_oauth_tokens().await;
        Ok(response)
    }

    /// Starts a Plugin Runtime event stream without waiting for its final response.
    pub async fn send_event_stream_request(
        &self,
        params: Option<serde_json::Value>,
    ) -> Result<CancellableEventStreamRequest> {
        let service = self.service().await?;
        let (sender, notifications) = event_notification_channel();
        let mut request = CustomRequest::new("events/stream", params);
        request.extensions.insert(sender);
        let handle = service
            .peer()
            .send_cancellable_request(
                ClientRequest::CustomRequest(request),
                rmcp::service::PeerRequestOptions::no_options(),
            )
            .await?;

        Ok(CancellableEventStreamRequest {
            handle,
            notifications,
        })
    }

    async fn service(&self) -> Result<Arc<RunningService<RoleClient, ElicitationClientService>>> {
        let guard = self.state.lock().await;
        match &*guard {
            ClientState::Ready { service, .. } => Ok(Arc::clone(service)),
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

    /// Returns `None` when this client does not manage stored OAuth credentials.
    pub async fn managed_oauth_credentials(&self) -> Option<Option<StoredOAuthTokens>> {
        let persistor = self.oauth_persistor().await?;
        Some(persistor.stored_credentials().await)
    }

    /// Returns whether an initialized transport or its underlying service has stopped.
    pub async fn is_closed(&self) -> bool {
        let state = self.state.lock().await;
        match &*state {
            ClientState::Ready { service, .. } => {
                service.is_closed() || service.peer().is_transport_closed()
            }
            ClientState::Connecting { .. } => false,
            ClientState::Closed => true,
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

    /// This should be called after every tool call so that if a given tool call triggered
    /// a refresh of the OAuth tokens, they are persisted.
    async fn persist_oauth_tokens(&self) {
        if let Some(runtime) = self.oauth_persistor().await
            && let Err(error) = runtime.persist_if_needed().await
        {
            warn!("failed to persist OAuth tokens: {error}");
        }
    }

    /// OAuth uses independent lock/request bounds and completes before the operation timeout starts.
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
                Ok(PendingTransport::Stdio {
                    transport: Box::new(transport),
                })
            }
            TransportRecipe::StreamableHttp {
                server_name,
                url,
                bearer_token,
                http_headers,
                env_http_headers,
                store_mode,
                keyring_backend_kind,
                pinned_credential_store,
                http_client,
                auth_provider,
                redirect_mode,
                initialize_deadline,
            } => {
                let has_configured_headers = http_headers
                    .as_ref()
                    .is_some_and(|headers| !headers.is_empty())
                    || env_http_headers
                        .as_ref()
                        .is_some_and(|headers| !headers.is_empty());
                let default_headers =
                    build_default_headers(http_headers.clone(), env_http_headers.clone())?;
                let auth_provider =
                    if bearer_token.is_some() || default_headers.contains_key(AUTHORIZATION) {
                        None
                    } else {
                        auth_provider.clone()
                    };

                let resolved_oauth_tokens = if bearer_token.is_none()
                    && auth_provider.is_none()
                    && !default_headers.contains_key(AUTHORIZATION)
                {
                    let oauth_server_name = server_name.clone();
                    let oauth_url = url.clone();
                    let oauth_store_mode = *store_mode;
                    let oauth_keyring_backend_kind = *keyring_backend_kind;
                    let pinned_credential_store = Arc::clone(pinned_credential_store);

                    tokio::task::spawn_blocking(move || -> Result<Option<ResolvedOAuthTokens>> {
                        if let Some(store) = pinned_credential_store.get().copied() {
                            // Rebuilds reread the source selected during first construction. Only
                            // initial construction below evaluates configured store policy.
                            return store
                                .load(&DefaultKeyringStore, &oauth_server_name, &oauth_url)
                                .map(|tokens| tokens.map(|tokens| ResolvedOAuthTokens { tokens, store }));
                        }

                        match resolve_oauth_tokens_from_store_policy(
                            &DefaultKeyringStore,
                            &oauth_server_name,
                            &oauth_url,
                            oauth_store_mode,
                            oauth_keyring_backend_kind,
                        ) {
                            Ok(tokens) => {
                                if let Some(resolved) = tokens.as_ref() {
                                    // Retries and session recovery rebuild this transport. Pin the
                                    // first concrete source so Auto is not reevaluated mid-client.
                                    pinned_credential_store.set(resolved.store).map_err(|_| {
                                        anyhow!(
                                            "OAuth credential store pinned concurrently for MCP server `{oauth_server_name}`"
                                        )
                                    })?;
                                }
                                Ok(tokens)
                            }
                            Err(err) => {
                                warn!(
                                    "failed to read tokens for server `{oauth_server_name}`: {err}"
                                );
                                Ok(None)
                            }
                        }
                    })
                    .await
                    .map_err(|error| anyhow!("OAuth credential loading task failed: {error}"))??
                } else {
                    None
                };

                if let Some(ResolvedOAuthTokens {
                    tokens: initial_tokens,
                    store: credential_store,
                }) = resolved_oauth_tokens
                {
                    match create_oauth_transport_and_runtime(
                        server_name,
                        url,
                        initial_tokens.clone(),
                        credential_store,
                        default_headers.clone(),
                        Arc::clone(http_client),
                        has_configured_headers,
                        *redirect_mode,
                        Arc::clone(initialize_deadline),
                    )
                    .await
                    {
                        Ok(pending_transport) => Ok(pending_transport),
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
                                    has_configured_headers,
                                    *redirect_mode,
                                    Arc::clone(initialize_deadline),
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
                            has_configured_headers,
                            *redirect_mode,
                            Arc::clone(initialize_deadline),
                        ),
                        http_config,
                    );
                    Ok(PendingTransport::StreamableHttp { transport })
                }
            }
        }
    }

    async fn connect_pending_transport(
        &self,
        pending_transport: PendingTransport,
        client_service: ElicitationClientService,
        timeout: Option<Duration>,
    ) -> Result<(
        Arc<RunningService<RoleClient, ElicitationClientService>>,
        Option<OAuthPersistor>,
    )> {
        let _initialize_deadline = match &self.transport_recipe {
            TransportRecipe::StreamableHttp {
                initialize_deadline,
                ..
            } => {
                *initialize_deadline
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner) =
                    timeout.and_then(|duration| Instant::now().checked_add(duration));
                Some(InitializeDeadlineGuard {
                    deadline: Arc::clone(initialize_deadline),
                })
            }
            TransportRecipe::InProcess { .. } | TransportRecipe::Stdio { .. } => None,
        };
        let lifecycle = self.protocol_mode.client_lifecycle();
        let (transport, oauth_persistor) = match pending_transport {
            PendingTransport::InProcess { transport } => (
                client_service
                    .serve_with_lifecycle(transport, lifecycle)
                    .boxed(),
                None,
            ),
            PendingTransport::Stdio { transport } => (
                client_service
                    .serve_with_lifecycle(*transport, lifecycle)
                    .boxed(),
                None,
            ),
            PendingTransport::StreamableHttp { transport } => (
                client_service
                    .serve_with_lifecycle(capture_event_notifications(transport), lifecycle)
                    .boxed(),
                None,
            ),
            PendingTransport::StreamableHttpWithOAuth {
                transport,
                oauth_persistor,
            } => (
                client_service
                    .serve_with_lifecycle(transport, lifecycle)
                    .boxed(),
                Some(oauth_persistor),
            ),
            PendingTransport::StreamableHttpWithAccessTokenOnly { transport } => (
                client_service
                    .serve_with_lifecycle(transport, lifecycle)
                    .boxed(),
                None,
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
        let service = match service_result {
            Ok(service) => service,
            Err(error) => {
                if let Some(runtime) = oauth_persistor.as_ref()
                    && let Err(persist_error) = runtime.persist_if_needed().await
                {
                    warn!(
                        "failed to persist OAuth tokens after failed initialize: {persist_error}"
                    );
                }
                return Err(error);
            }
        };

        // Preserve Codex's existing snapshot and request-freshness behavior. rmcp 3
        // enables response caching and stale-on-error fallback by default.
        service
            .peer()
            .set_response_cache_config(ClientCacheConfig::disabled())
            .await;

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
        let service = self.service().await?;
        match Self::run_service_operation_with_transient_retries(
            Arc::clone(&service),
            label,
            timeout,
            self.elicitation_pause_state.clone(),
            &operation,
        )
        .await
        {
            Ok(result) => Ok(result),
            Err(error) if Self::is_session_expired_404(&error) => {
                self.reinitialize_after_session_expiry(&service).await?;
                let recovered_service = self.service().await?;
                Self::run_service_operation_with_transient_retries(
                    recovered_service,
                    label,
                    timeout,
                    self.elicitation_pause_state.clone(),
                    &operation,
                )
                .await
                .map_err(Into::into)
            }
            Err(error) => Err(error.into()),
        }
    }

    async fn run_service_operation_with_transient_retries<T, F, Fut>(
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
        let retry_deadline = timeout.map(|duration| Instant::now() + duration);
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
            .connect_pending_transport_with_initialize_retries(
                pending_transport,
                initialize_context.client_service,
                initialize_context.timeout,
            )
            .await?;
        service
            .peer()
            .peer_info()
            .ok_or_else(|| anyhow!("recovered handshake succeeded but server info was missing"))?;

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

        if let Some(runtime) = oauth_persistor
            && let Err(error) = runtime.persist_if_needed().await
        {
            warn!("failed to persist OAuth tokens after session recovery: {error}");
        }

        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
async fn create_oauth_transport_and_runtime(
    server_name: &str,
    url: &str,
    initial_tokens: StoredOAuthTokens,
    credential_store: ResolvedOAuthCredentialStore,
    default_headers: HeaderMap,
    http_client: Arc<dyn HttpClient>,
    has_configured_headers: bool,
    redirect_mode: StreamableHttpRedirectMode,
    initialize_deadline: Arc<StdMutex<Option<Instant>>>,
) -> Result<PendingTransport> {
    let oauth_http_client = Arc::new(OAuthHttpClientAdapter::new_with_redirect_mode(
        http_client.clone(),
        default_headers.clone(),
        url,
        has_configured_headers,
        redirect_mode,
    )?);
    let mut manager =
        AuthorizationManager::new_with_oauth_http_client(url.to_string(), oauth_http_client)
            .await?;
    manager.set_allow_missing_issuer(true);
    let metadata = manager
        .resolve_metadata()
        .await
        .context("failed to resolve OAuth metadata before using stored credentials")?
        .metadata;
    let use_stored_access_token_only =
        match validate_refresh_token_issuer(&metadata, &initial_tokens) {
            Ok(()) => false,
            Err(_error) if initial_tokens.access_token_is_usable_without_refresh() => true,
            Err(error) => return Err(error),
        };
    manager.set_metadata(metadata);
    let mut runtime_tokens = initial_tokens.clone();
    if use_stored_access_token_only {
        runtime_tokens.token_response.0.set_refresh_token(None);
        runtime_tokens.issuer = None;
    }
    install_tokens_in_manager(&mut manager, &runtime_tokens).await?;

    let auth_client = AuthClient::new(
        StreamableHttpClientAdapter::new(
            http_client,
            default_headers,
            /*auth_provider*/ None,
            has_configured_headers,
            redirect_mode,
            initialize_deadline,
        ),
        manager,
    );
    let auth_manager = auth_client.auth_manager.clone();

    let transport = StreamableHttpClientTransport::with_client(
        auth_client,
        StreamableHttpClientTransportConfig::with_uri(url.to_string()),
    );

    if use_stored_access_token_only {
        warn!(
            "stored OAuth refresh credentials could not be bound to their issuer for MCP server `{server_name}`; using the stored access token without refresh"
        );
        return Ok(PendingTransport::StreamableHttpWithAccessTokenOnly { transport });
    }

    let runtime = OAuthPersistor::new(
        server_name.to_string(),
        url.to_string(),
        auth_manager,
        credential_store,
        Some(initial_tokens),
    );

    Ok(PendingTransport::StreamableHttpWithOAuth {
        transport,
        oauth_persistor: runtime,
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use pretty_assertions::assert_eq;
    use tokio::time;

    use super::*;

    #[test]
    fn client_operation_timeout_rounds_duration() {
        let error = ClientOperationError::Timeout {
            label: "tools/list".to_string(),
            duration: Duration::from_nanos(29_999_999_875),
        };

        assert_eq!(error.to_string(), "timed out awaiting tools/list after 30s");
    }

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
