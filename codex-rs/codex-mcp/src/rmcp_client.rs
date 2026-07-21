//! RMCP client lifecycle for MCP server connections.
//!
//! This module owns startup of individual RMCP clients: building the transport,
//! initializing the server, listing raw tools, applying per-server tool filters,
//! and exposing cached Codex Apps tools while a client is still connecting.
//! Higher-level aggregation and resource/tool APIs live in
//! [`crate::connection_manager`].

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::env;
use std::ffi::OsString;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use crate::codex_apps::normalize_codex_apps_callable_name;
use crate::codex_apps::normalize_codex_apps_callable_namespace;
use crate::codex_apps::normalize_codex_apps_tool_title;
use crate::codex_apps::prepare_openai_file_params_for_model;
use crate::elicitation::ElicitationRequestManager;
use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;
use crate::mcp::ToolPluginProvenance;
use crate::openai_docs_source_attribution::maybe_with_openai_docs_source_attribution;
use crate::runtime::McpRuntimeContext;
use crate::runtime::emit_duration;
use crate::server::EffectiveMcpServer;
use crate::server::McpServerLaunch;
use crate::tool_catalog_cache::McpToolCatalogCacheContext;
use crate::tool_catalog_cache::McpToolCatalogFetchTicket;
use crate::tools::ToolFilter;
use crate::tools::ToolInfo;
use crate::tools::filter_tools;
use anyhow::Result;
use anyhow::anyhow;
use async_channel::Sender;
use codex_api::SharedAuthProvider;
use codex_async_utils::CancelErr;
use codex_async_utils::OrCancelExt;
use codex_config::McpServerConfig;
use codex_config::McpServerTransportConfig;
use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_connectors::ConnectorRuntimeContext;
use codex_connectors::ConnectorRuntimeFetchSource;
use codex_exec_server::Environment;
use codex_exec_server::HttpClient;
use codex_exec_server::ReqwestHttpClient;
use codex_protocol::mcp::McpServerInfo;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::McpStartupStatus;
use codex_protocol::protocol::McpStartupUpdateEvent;
use codex_rmcp_client::ExecutorStdioServerLauncher;
use codex_rmcp_client::LocalStdioServerLauncher;
use codex_rmcp_client::RmcpClient;
use codex_rmcp_client::StdioServerLauncher;
use codex_rmcp_client::ToolWithConnectorId;
use codex_rmcp_client::is_authentication_required_error;
use futures::future::BoxFuture;
use futures::future::FutureExt;
use futures::future::Shared;
use rmcp::model::ClientCapabilities;
use rmcp::model::ElicitationCapability;
use rmcp::model::Implementation;
use rmcp::model::InitializeRequestParams;
use rmcp::model::JsonObject;
use rmcp::model::ProtocolVersion;
use rmcp::model::Tool as RmcpTool;
use tokio::time::Instant as TokioInstant;
use tokio_util::sync::CancellationToken;
use tracing::Instrument;
use tracing::instrument;
use tracing::warn;

/// MCP server capability indicating that Codex should include [`SandboxState`]
/// in tool-call request `_meta` under this key.
pub const MCP_SANDBOX_STATE_META_CAPABILITY: &str = "codex/sandbox-state-meta";
/// Experimental MCP server capability for development and testing only; production servers should
/// not use it. Its `cacheable: false` property disables sharing tool definitions across connections.
const MCP_TOOL_CATALOG_CACHE_CAPABILITY: &str = "codex/tool-catalog-cache";
const MCP_TOOL_CATALOG_CACHEABLE_PROPERTY: &str = "cacheable";
pub const OPENAI_FORM_CAPABILITY: &str = "openai/form";

pub(crate) const MCP_TOOLS_LIST_DURATION_METRIC: &str = "codex.mcp.tools.list.duration_ms";
pub(crate) const MCP_TOOLS_FETCH_UNCACHED_DURATION_METRIC: &str =
    "codex.mcp.tools.fetch_uncached.duration_ms";
pub(crate) const CODEX_APPS_REFRESH_DURATION_METRIC: &str = "codex.apps.refresh.duration_ms";
pub(crate) const DEFAULT_STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
pub(crate) const DEFAULT_TOOL_TIMEOUT: Duration = Duration::from_secs(300);

pub(crate) const CODEX_APPS_RECONNECT_INITIAL_BACKOFF: Duration = Duration::from_secs(1);
const CODEX_APPS_RECONNECT_MAX_BACKOFF: Duration = Duration::from_secs(30);

const UNTRUSTED_CONNECTOR_META_KEYS: &[&str] = &[
    "connector_id",
    "connector_name",
    "connector_display_name",
    "connector_description",
    "connectorDescription",
];

#[derive(Clone)]
pub(crate) struct ManagedClient {
    pub(crate) client: Arc<RmcpClient>,
    pub(crate) server_info: McpServerInfo,
    pub(crate) tools: Vec<ToolInfo>,
    pub(crate) tool_filter: ToolFilter,
    pub(crate) tool_timeout: Option<Duration>,
    pub(crate) server_instructions: Option<String>,
    pub(crate) server_supports_sandbox_state_meta_capability: bool,
    pub(crate) codex_apps_tools_cache_context: Option<ConnectorRuntimeContext<ToolInfo>>,
}

impl ManagedClient {
    pub(crate) fn listed_tools(&self) -> Vec<ToolInfo> {
        let total_start = Instant::now();
        if let Some(tools) = self
            .codex_apps_tools_cache_context
            .as_ref()
            .and_then(ConnectorRuntimeContext::current_tools)
        {
            emit_duration(
                MCP_TOOLS_LIST_DURATION_METRIC,
                total_start.elapsed(),
                &[("cache", "hit")],
            );
            return filter_tools(tools, &self.tool_filter);
        }

        if self.codex_apps_tools_cache_context.is_some() {
            emit_duration(
                MCP_TOOLS_LIST_DURATION_METRIC,
                total_start.elapsed(),
                &[("cache", "miss")],
            );
        }

        filter_tools(self.tools.clone(), &self.tool_filter)
    }
}

pub(crate) type ManagedClientFuture =
    Shared<BoxFuture<'static, Result<ManagedClient, StartupOutcomeError>>>;

#[derive(Default)]
struct CodexAppsStartupReconnectState {
    current_client: Option<ManagedClient>,
    reconnect_in_flight: bool,
    consecutive_failures: u32,
    retry_not_before: Option<TokioInstant>,
}

#[derive(Clone)]
struct CodexAppsStartupStatusContext {
    submit_id: String,
    server_name: String,
    tx_event: Sender<Event>,
}

pub(crate) struct CodexAppsStartupReconnect {
    factory: Arc<dyn Fn() -> ManagedClientFuture + Send + Sync>,
    state: StdMutex<CodexAppsStartupReconnectState>,
    startup_status_context: Option<CodexAppsStartupStatusContext>,
}

impl CodexAppsStartupReconnect {
    pub(crate) fn new(factory: Arc<dyn Fn() -> ManagedClientFuture + Send + Sync>) -> Self {
        Self {
            factory,
            state: StdMutex::new(CodexAppsStartupReconnectState::default()),
            startup_status_context: None,
        }
    }

    fn with_startup_status_context(
        mut self,
        submit_id: String,
        server_name: String,
        tx_event: Option<Sender<Event>>,
    ) -> Self {
        self.startup_status_context = tx_event.map(|tx_event| CodexAppsStartupStatusContext {
            submit_id,
            server_name,
            tx_event,
        });
        self
    }

    fn current_client(&self) -> Option<ManagedClient> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .current_client
            .clone()
    }

    fn reconnect_in_background(self: &Arc<Self>) {
        {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.current_client.is_some() || state.reconnect_in_flight {
                return;
            }
            if state
                .retry_not_before
                .is_some_and(|retry_not_before| TokioInstant::now() < retry_not_before)
            {
                return;
            }
            state.reconnect_in_flight = true;
        }

        let reconnect = Arc::clone(self);
        tokio::spawn(async move {
            let result = (reconnect.factory)().await;
            let startup_status_context = reconnect.startup_status_context.clone();
            let recovered = {
                let mut state = reconnect
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                state.reconnect_in_flight = false;
                match result {
                    Ok(client) => {
                        state.current_client = Some(client);
                        state.consecutive_failures = 0;
                        state.retry_not_before = None;
                        true
                    }
                    Err(error) => {
                        state.consecutive_failures = state.consecutive_failures.saturating_add(1);
                        let retry_after = codex_apps_reconnect_backoff(state.consecutive_failures);
                        state.retry_not_before = Some(TokioInstant::now() + retry_after);
                        warn!(
                            error = %error,
                            retry_after_ms = retry_after.as_millis(),
                            "Apps MCP startup reconnect failed; continuing with cached tools"
                        );
                        false
                    }
                }
            };

            if recovered && let Some(context) = startup_status_context {
                let _ = context
                    .tx_event
                    .send(Event {
                        id: context.submit_id,
                        msg: EventMsg::McpStartupUpdate(McpStartupUpdateEvent {
                            server: context.server_name,
                            status: McpStartupStatus::Ready,
                        }),
                    })
                    .await;
            }
        });
    }
}

fn codex_apps_reconnect_backoff(consecutive_failures: u32) -> Duration {
    let exponent = consecutive_failures.saturating_sub(1).min(5);
    CODEX_APPS_RECONNECT_INITIAL_BACKOFF
        .saturating_mul(1 << exponent)
        .min(CODEX_APPS_RECONNECT_MAX_BACKOFF)
}

#[derive(Clone)]
struct ManagedClientStartup {
    server_name: String,
    server: EffectiveMcpServer,
    store_mode: OAuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
    tx_event: Option<Sender<Event>>,
    elicitation_requests: ElicitationRequestManager,
    codex_apps_tools_cache_context: Option<ConnectorRuntimeContext<ToolInfo>>,
    tool_catalog_cache_context: Option<McpToolCatalogCacheContext>,
    runtime_context: McpRuntimeContext,
    resolved_environment: std::result::Result<Option<Arc<Environment>>, String>,
    runtime_auth_provider: Option<SharedAuthProvider>,
    client_elicitation_capability: ElicitationCapability,
    supports_openai_form_elicitation: bool,
    cancel_token: CancellationToken,
    startup_complete: Arc<AtomicBool>,
}

impl ManagedClientStartup {
    fn start(&self) -> ManagedClientFuture {
        let Self {
            server_name,
            server,
            store_mode,
            keyring_backend_kind,
            tx_event,
            elicitation_requests,
            codex_apps_tools_cache_context,
            tool_catalog_cache_context,
            runtime_context,
            resolved_environment,
            runtime_auth_provider,
            client_elicitation_capability,
            supports_openai_form_elicitation,
            cancel_token,
            startup_complete,
        } = self.clone();
        let is_codex_apps_mcp_server = server_name == CODEX_APPS_MCP_SERVER_NAME;
        let tool_filter = server
            .configured_config()
            .map(ToolFilter::from_config)
            .unwrap_or_default();
        let startup_timeout = server
            .configured_config()
            .and_then(|config| config.startup_timeout_sec)
            .unwrap_or(DEFAULT_STARTUP_TIMEOUT);
        let cancel_token_for_fut = cancel_token;
        let tool_catalog_fetch_ticket = tool_catalog_cache_context
            .as_ref()
            .map(McpToolCatalogCacheContext::begin_fetch);
        async move {
            let refresh_start = is_codex_apps_mcp_server.then(Instant::now);
            let outcome = match async {
                if let Err(error) = validate_mcp_server_name(&server_name) {
                    return Err(error.into());
                }

                let client = match tokio::time::timeout(
                    startup_timeout,
                    make_rmcp_client(
                        &server_name,
                        server.clone(),
                        store_mode,
                        keyring_backend_kind,
                        runtime_context,
                        resolved_environment,
                        runtime_auth_provider,
                    ),
                )
                .await
                {
                    Ok(result) => Arc::new(result?),
                    Err(_) => {
                        return Err(StartupOutcomeError::from(anyhow!(
                            "MCP client startup timed out after {startup_timeout:?}"
                        )));
                    }
                };
                start_server_task(
                    server_name,
                    client,
                    StartServerTaskParams {
                        is_codex_apps_mcp_server,
                        startup_timeout: Some(startup_timeout),
                        tool_timeout: server
                            .configured_config()
                            .and_then(|config| config.tool_timeout_sec)
                            .unwrap_or(DEFAULT_TOOL_TIMEOUT),
                        tool_filter,
                        tx_event,
                        elicitation_requests,
                        codex_apps_tools_cache_context,
                        tool_catalog_cache_context,
                        tool_catalog_fetch_ticket,
                        client_elicitation_capability,
                        supports_openai_form_elicitation,
                    },
                )
                .await
            }
            .or_cancel(&cancel_token_for_fut)
            .await
            {
                Ok(result) => result,
                Err(CancelErr::Cancelled) => Err(StartupOutcomeError::Cancelled),
            };
            if outcome.is_ok()
                && let Some(refresh_start) = refresh_start
            {
                emit_duration(
                    CODEX_APPS_REFRESH_DURATION_METRIC,
                    refresh_start.elapsed(),
                    &[("path", "legacy"), ("trigger", "initial")],
                );
            }

            startup_complete.store(true, Ordering::Release);
            outcome
        }
        .in_current_span()
        .boxed()
        .shared()
    }
}

#[derive(Clone)]
pub(crate) struct AsyncManagedClient {
    pub(crate) client: ManagedClientFuture,
    pub(crate) is_codex_apps_mcp_server: bool,
    pub(crate) cached_server_info: Option<McpServerInfo>,
    pub(crate) codex_apps_tools_cache_context: Option<ConnectorRuntimeContext<ToolInfo>>,
    pub(crate) tool_catalog_cache_context: Option<McpToolCatalogCacheContext>,
    pub(crate) tool_filter: ToolFilter,
    pub(crate) startup_complete: Arc<AtomicBool>,
    pub(crate) startup_reconnect: Option<Arc<CodexAppsStartupReconnect>>,
    pub(crate) tool_plugin_provenance: Arc<ToolPluginProvenance>,
    pub(crate) cancel_token: CancellationToken,
}

impl AsyncManagedClient {
    // Keep this constructor flat so the startup inputs remain readable at the
    // single call site instead of introducing a one-off params wrapper.
    #[instrument(level = "trace", skip_all, fields(server_name = %server_name))]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        server_name: String,
        startup_submit_id: String,
        server: EffectiveMcpServer,
        store_mode: OAuthCredentialsStoreMode,
        keyring_backend_kind: AuthKeyringBackendKind,
        cancel_token: CancellationToken,
        tx_event: Option<Sender<Event>>,
        elicitation_requests: ElicitationRequestManager,
        codex_apps_tools_cache_context: Option<ConnectorRuntimeContext<ToolInfo>>,
        tool_catalog_cache_context: Option<McpToolCatalogCacheContext>,
        tool_plugin_provenance: Arc<ToolPluginProvenance>,
        runtime_context: McpRuntimeContext,
        resolved_environment: std::result::Result<Option<Arc<Environment>>, String>,
        runtime_auth_provider: Option<SharedAuthProvider>,
        client_elicitation_capability: ElicitationCapability,
        supports_openai_form_elicitation: bool,
    ) -> Self {
        let is_codex_apps_mcp_server = server_name == CODEX_APPS_MCP_SERVER_NAME;
        let reconnect_server_name = server_name.clone();
        let reconnect_tx_event = tx_event.clone();
        let tool_filter = server
            .configured_config()
            .map(ToolFilter::from_config)
            .unwrap_or_default();
        let cached_server_info = if is_codex_apps_mcp_server {
            codex_apps_tools_cache_context
                .as_ref()
                .and_then(ConnectorRuntimeContext::cached_server_info)
        } else {
            None
        };
        let startup_complete = Arc::new(AtomicBool::new(false));
        let startup = Arc::new(ManagedClientStartup {
            server_name,
            server,
            store_mode,
            keyring_backend_kind,
            tx_event,
            elicitation_requests,
            codex_apps_tools_cache_context: codex_apps_tools_cache_context.clone(),
            tool_catalog_cache_context: tool_catalog_cache_context.clone(),
            runtime_context,
            resolved_environment,
            runtime_auth_provider,
            client_elicitation_capability,
            supports_openai_form_elicitation,
            cancel_token: cancel_token.clone(),
            startup_complete: Arc::clone(&startup_complete),
        });
        let client = startup.start();
        let startup_reconnect = is_codex_apps_mcp_server.then(|| {
            let startup = Arc::clone(&startup);
            Arc::new(
                CodexAppsStartupReconnect::new(Arc::new(move || startup.start()))
                    .with_startup_status_context(
                        startup_submit_id,
                        reconnect_server_name,
                        reconnect_tx_event,
                    ),
            )
        });
        if codex_apps_tools_cache_context
            .as_ref()
            .is_some_and(ConnectorRuntimeContext::has_current_tools)
            || tool_catalog_cache_context
                .as_ref()
                .is_some_and(McpToolCatalogCacheContext::has_tools)
        {
            let startup_task = client.clone();
            tokio::spawn(async move {
                let _ = startup_task.await;
            });
        }

        Self {
            client,
            is_codex_apps_mcp_server,
            cached_server_info,
            codex_apps_tools_cache_context,
            tool_catalog_cache_context,
            tool_filter,
            startup_complete,
            startup_reconnect,
            tool_plugin_provenance,
            cancel_token,
        }
    }

    pub(crate) async fn client(&self) -> Result<ManagedClient, StartupOutcomeError> {
        if let Some(client) = self
            .startup_reconnect
            .as_ref()
            .and_then(|reconnect| reconnect.current_client())
        {
            return Ok(client);
        }
        self.client.clone().await
    }

    /// Captures the ready client revision that is current now.
    ///
    /// A recovered Codex Apps connection replaces the failed startup future for
    /// future steps, but cannot reroute a call that was already prepared.
    fn ready_client_snapshot(&self) -> Option<ManagedClientFuture> {
        if let Some(client) = self
            .startup_reconnect
            .as_ref()
            .and_then(|reconnect| reconnect.current_client())
        {
            return Some(futures::future::ready(Ok(client)).boxed().shared());
        }
        match self.client.peek() {
            Some(Ok(_)) => Some(self.client.clone()),
            Some(Err(_)) | None => None,
        }
    }

    /// Captures one ready client and derives its model-visible tools.
    ///
    /// A fresh client waits for initial startup even when metadata is cached.
    /// After a failed startup, cached tools remain available to metadata-only
    /// callers while recovery runs, but a model step still requires one exact
    /// ready client.
    pub(crate) async fn capture_ready_client_and_tools(
        &self,
        catalog_override: Option<Vec<ToolInfo>>,
    ) -> Option<(Arc<ManagedClient>, Vec<ToolInfo>)> {
        if !self.startup_complete.load(Ordering::Acquire) {
            let _ = self.client().await;
        }
        self.reconnect_failed_startup().await;
        let client = if self.has_cached_tools() {
            self.ready_client_snapshot()?
        } else {
            self.client().await.ok()?;
            self.ready_client_snapshot()?
        };
        let managed_client = Arc::new(client.await.ok()?);
        let tools = catalog_override.unwrap_or_else(|| managed_client.tools.clone());
        let tools = filter_tools(tools, &managed_client.tool_filter);
        Some((Arc::clone(&managed_client), self.prepare_tools(tools)))
    }

    pub(crate) async fn reconnect_failed_startup(&self) {
        let Some(startup_reconnect) = self.startup_reconnect.as_ref() else {
            return;
        };
        if !self.startup_complete.load(Ordering::Acquire) {
            return;
        }
        if matches!(self.client().await, Err(StartupOutcomeError::Failed { .. })) {
            startup_reconnect.reconnect_in_background();
        }
    }

    pub(crate) async fn shutdown(&self) {
        self.cancel_token.cancel();
        match self.client().await {
            Ok(client) => client.client.shutdown().await,
            Err(StartupOutcomeError::Cancelled) => {}
            Err(error) => {
                warn!("failed to initialize MCP client during shutdown: {error:#}");
            }
        }
    }

    pub(crate) fn has_cached_tools(&self) -> bool {
        self.codex_apps_tools_cache_context
            .as_ref()
            .is_some_and(ConnectorRuntimeContext::has_current_tools)
            || self
                .tool_catalog_cache_context
                .as_ref()
                .is_some_and(McpToolCatalogCacheContext::has_tools)
    }

    fn cached_tools(&self) -> Option<Vec<ToolInfo>> {
        self.codex_apps_tools_cache_context
            .as_ref()
            .and_then(ConnectorRuntimeContext::current_tools)
            .or_else(|| {
                self.tool_catalog_cache_context
                    .as_ref()
                    .and_then(McpToolCatalogCacheContext::current_tools)
            })
            .map(|tools| filter_tools(tools, &self.tool_filter))
    }

    pub(crate) fn prepare_tools(&self, tools: Vec<ToolInfo>) -> Vec<ToolInfo> {
        if self.is_codex_apps_mcp_server {
            prepare_codex_apps_tools_for_model(tools, &self.tool_plugin_provenance)
        } else {
            prepare_regular_mcp_tools_for_model(tools, &self.tool_plugin_provenance)
        }
    }

    pub(crate) async fn listed_tools(&self) -> Option<Vec<ToolInfo>> {
        // Plugin provenance is resolved per-session rather than stored in shared cache payloads.
        let tools = if !self.startup_complete.load(Ordering::Acquire)
            && let Some(startup_tools) = self.cached_tools()
        {
            Some(startup_tools)
        } else {
            match self.client().await {
                Ok(client) => Some(client.listed_tools()),
                Err(_) if self.is_codex_apps_mcp_server => self.cached_tools(),
                Err(_) => None,
            }
        }?;
        Some(self.prepare_tools(tools))
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub(crate) enum StartupOutcomeError {
    #[error("MCP startup cancelled")]
    Cancelled,
    // We can't store the original error here because anyhow::Error doesn't implement
    // `Clone`.
    #[error("MCP startup failed: {error}")]
    Failed {
        error: String,
        is_authentication_required: bool,
    },
}

impl StartupOutcomeError {
    pub(crate) fn is_authentication_required(&self) -> bool {
        match self {
            Self::Cancelled => false,
            Self::Failed {
                is_authentication_required,
                ..
            } => *is_authentication_required,
        }
    }
}

impl From<anyhow::Error> for StartupOutcomeError {
    fn from(error: anyhow::Error) -> Self {
        let is_authentication_required = is_authentication_required_error(&error);
        Self::Failed {
            error: error.to_string(),
            is_authentication_required,
        }
    }
}

#[instrument(level = "trace", skip_all, fields(server_name = %server_name))]
pub(crate) async fn list_tools_for_client_uncached(
    server_name: &str,
    is_codex_apps_mcp_server: bool,
    codex_apps_refresh_trigger: &'static str,
    client: &Arc<RmcpClient>,
    timeout: Option<Duration>,
    server_instructions: Option<&str>,
) -> Result<Vec<ToolInfo>> {
    let fetch_start = Instant::now();
    let resp = client
        .list_tools_with_connector_ids(/*params*/ None, timeout)
        .await?;
    let tools = resp
        .tools
        .into_iter()
        .map(|tool| {
            tool_info_from_listed_tool(
                server_name,
                is_codex_apps_mcp_server,
                server_instructions,
                tool,
            )
        })
        .collect();
    if is_codex_apps_mcp_server {
        emit_duration(
            MCP_TOOLS_FETCH_UNCACHED_DURATION_METRIC,
            fetch_start.elapsed(),
            &[("trigger", codex_apps_refresh_trigger)],
        );
    } else {
        emit_duration(
            MCP_TOOLS_FETCH_UNCACHED_DURATION_METRIC,
            fetch_start.elapsed(),
            &[],
        );
    }
    Ok(tools)
}

/// Presents declared Codex Apps file parameters to the model as local-path inputs and adds plugin
/// names to each tool. Plugin membership is resolved by connector ID, falling back to the MCP
/// server when absent.
fn prepare_codex_apps_tools_for_model(
    mut tools: Vec<ToolInfo>,
    tool_plugin_provenance: &ToolPluginProvenance,
) -> Vec<ToolInfo> {
    for tool in &mut tools {
        prepare_openai_file_params_for_model(tool);
        let plugin_names = match tool.connector_id.as_deref() {
            Some(connector_id) => {
                tool_plugin_provenance.plugin_display_names_for_connector_id(connector_id)
            }
            None => tool_plugin_provenance
                .plugin_display_names_for_mcp_server_name(tool.server_name.as_str()),
        };
        add_plugin_provenance_to_tool(tool, plugin_names);
    }
    tools
}

/// Stores plugin names on the tool and appends a model-visible plugin membership note.
fn add_plugin_provenance_to_tool(tool: &mut ToolInfo, plugin_names: &[String]) {
    tool.plugin_display_names = plugin_names.to_vec();
    if plugin_names.is_empty() {
        return;
    }

    let plugin_source_note = if plugin_names.len() == 1 {
        format!("This tool is part of plugin `{}`.", plugin_names[0])
    } else {
        format!(
            "This tool is part of plugins {}.",
            plugin_names
                .iter()
                .map(|plugin_name| format!("`{plugin_name}`"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let description = tool
        .tool
        .description
        .as_deref()
        .map(str::trim)
        .unwrap_or("");
    let annotated_description = if description.is_empty() {
        plugin_source_note
    } else if matches!(description.chars().last(), Some('.' | '!' | '?')) {
        format!("{description} {plugin_source_note}")
    } else {
        format!("{description}. {plugin_source_note}")
    };
    tool.tool.description = Some(Cow::Owned(annotated_description));
}

/// Adds server-scoped plugin names to regular MCP tools without changing their input schemas.
fn prepare_regular_mcp_tools_for_model(
    mut tools: Vec<ToolInfo>,
    tool_plugin_provenance: &ToolPluginProvenance,
) -> Vec<ToolInfo> {
    for tool in &mut tools {
        let plugin_names = tool_plugin_provenance
            .plugin_display_names_for_mcp_server_name(tool.server_name.as_str());
        add_plugin_provenance_to_tool(tool, plugin_names);
    }
    tools
}

fn tool_info_from_listed_tool(
    server_name: &str,
    is_codex_apps_mcp_server: bool,
    server_instructions: Option<&str>,
    tool: ToolWithConnectorId,
) -> ToolInfo {
    if is_codex_apps_mcp_server {
        codex_apps_tool_info_from_listed_tool(server_name, server_instructions, tool)
    } else {
        regular_mcp_tool_info_from_listed_tool(server_name, server_instructions, tool)
    }
}

/// Converts a Codex Apps tool by preserving connector fields, removing connector prefixes from
/// model-visible names and titles, and using the connector description for its tool namespace.
fn codex_apps_tool_info_from_listed_tool(
    server_name: &str,
    server_instructions: Option<&str>,
    tool: ToolWithConnectorId,
) -> ToolInfo {
    let mut tool_def = tool.tool;
    let connector_id = tool.connector_id;
    let connector_name = tool.connector_name;
    let connector_description = tool.connector_description;
    let callable_name = normalize_codex_apps_callable_name(
        &tool_def.name,
        connector_id.as_deref(),
        connector_name.as_deref(),
    );
    let callable_namespace =
        normalize_codex_apps_callable_namespace(server_name, connector_name.as_deref());
    if let Some(title) = tool_def.title.as_deref() {
        let normalized_title = normalize_codex_apps_tool_title(connector_name.as_deref(), title);
        if tool_def.title.as_deref() != Some(normalized_title.as_str()) {
            tool_def.title = Some(normalized_title);
        }
    }
    let has_connector_metadata =
        connector_id.is_some() || connector_name.is_some() || connector_description.is_some();
    let namespace_description = if has_connector_metadata {
        connector_description
    } else {
        server_instructions.map(str::to_string)
    };
    ToolInfo {
        server_name: server_name.to_owned(),
        supports_parallel_tool_calls: false,
        server_origin: None,
        callable_name,
        callable_namespace,
        namespace_description,
        tool: tool_def,
        openai_file_input_optional_fields: HashMap::new(),
        connector_id,
        connector_name,
        plugin_display_names: Vec::new(),
    }
}

/// Converts a regular MCP tool by removing reserved connector metadata, keeping its raw tool name,
/// and using the MCP server name and instructions for the model-visible namespace.
fn regular_mcp_tool_info_from_listed_tool(
    server_name: &str,
    server_instructions: Option<&str>,
    tool: ToolWithConnectorId,
) -> ToolInfo {
    let mut tool_def = tool.tool;
    strip_untrusted_connector_meta(&mut tool_def);
    ToolInfo {
        server_name: server_name.to_owned(),
        supports_parallel_tool_calls: false,
        server_origin: None,
        callable_name: tool_def.name.to_string(),
        callable_namespace: server_name.to_string(),
        namespace_description: server_instructions.map(str::to_string),
        tool: tool_def,
        openai_file_input_optional_fields: HashMap::new(),
        connector_id: None,
        connector_name: None,
        plugin_display_names: Vec::new(),
    }
}

fn strip_untrusted_connector_meta(tool: &mut RmcpTool) {
    if let Some(meta) = tool.meta.as_mut() {
        meta.retain(|key, _| !is_untrusted_connector_meta_key(key));
    }
}

fn is_untrusted_connector_meta_key(key: &str) -> bool {
    UNTRUSTED_CONNECTOR_META_KEYS.contains(&key)
}

fn resolve_bearer_token(
    server_name: &str,
    bearer_token_env_var: Option<&str>,
) -> Result<Option<String>> {
    let Some(env_var) = bearer_token_env_var else {
        return Ok(None);
    };

    match env::var(env_var) {
        Ok(value) => {
            if value.is_empty() {
                Err(anyhow!(
                    "Environment variable {env_var} for MCP server '{server_name}' is empty"
                ))
            } else {
                Ok(Some(value))
            }
        }
        Err(env::VarError::NotPresent) => Err(anyhow!(
            "Environment variable {env_var} for MCP server '{server_name}' is not set"
        )),
        Err(env::VarError::NotUnicode(_)) => Err(anyhow!(
            "Environment variable {env_var} for MCP server '{server_name}' contains invalid Unicode"
        )),
    }
}

fn validate_mcp_server_name(server_name: &str) -> Result<()> {
    let re = regex_lite::Regex::new(r"^[a-zA-Z0-9_-]+$")?;
    if !re.is_match(server_name) {
        return Err(anyhow!(
            "Invalid MCP server name '{server_name}': must match pattern {pattern}",
            pattern = re.as_str()
        ));
    }
    Ok(())
}

#[instrument(level = "trace", skip_all, fields(server_name = %server_name))]
async fn start_server_task(
    server_name: String,
    client: Arc<RmcpClient>,
    params: StartServerTaskParams,
) -> Result<ManagedClient, StartupOutcomeError> {
    let StartServerTaskParams {
        is_codex_apps_mcp_server,
        startup_timeout,
        tool_timeout,
        tool_filter,
        tx_event,
        elicitation_requests,
        codex_apps_tools_cache_context,
        tool_catalog_cache_context,
        tool_catalog_fetch_ticket,
        client_elicitation_capability,
        supports_openai_form_elicitation,
    } = params;
    let params = mcp_initialize_request_params(
        client_elicitation_capability,
        supports_openai_form_elicitation,
    );
    let send_elicitation = elicitation_requests.make_sender(server_name.clone(), tx_event);

    let initialize_result = client
        .initialize(params, startup_timeout, send_elicitation)
        .await
        .map_err(StartupOutcomeError::from)?;

    let server_disables_tool_catalog_cache = initialize_result
        .capabilities
        .experimental
        .as_ref()
        .and_then(|experimental| experimental.get(MCP_TOOL_CATALOG_CACHE_CAPABILITY))
        .and_then(|capability| capability.get(MCP_TOOL_CATALOG_CACHEABLE_PROPERTY))
        .and_then(serde_json::Value::as_bool)
        == Some(false);
    if server_disables_tool_catalog_cache
        && let Some(cache_context) = tool_catalog_cache_context.as_ref()
    {
        cache_context.disable();
    }
    let server_supports_sandbox_state_meta_capability = initialize_result
        .capabilities
        .experimental
        .as_ref()
        .and_then(|exp| exp.get(MCP_SANDBOX_STATE_META_CAPABILITY))
        .is_some();
    let list_start = Instant::now();
    let fetch_ticket = codex_apps_tools_cache_context
        .as_ref()
        .map(|cache_context| cache_context.begin_fetch(ConnectorRuntimeFetchSource::Startup));
    let client_tools = list_tools_for_client_uncached(
        &server_name,
        is_codex_apps_mcp_server,
        /*codex_apps_refresh_trigger*/ "initial",
        &client,
        startup_timeout,
        initialize_result.instructions.as_deref(),
    )
    .await
    .map_err(StartupOutcomeError::from)?;
    let server_info = mcp_server_info_from_implementation(initialize_result.server_info);
    let shared_tools = match (codex_apps_tools_cache_context.as_ref(), fetch_ticket) {
        (Some(cache_context), Some(fetch_ticket)) => cache_context.publish_if_newest_accepted(
            fetch_ticket,
            &server_info,
            client_tools.clone(),
        ),
        (None, None) => client_tools.clone(),
        _ => unreachable!("Codex Apps fetch ticket requires cache context"),
    };
    let has_shared_tool_catalog = is_codex_apps_mcp_server || tool_catalog_cache_context.is_some();
    if let (Some(cache_context), Some(fetch_ticket)) = (
        tool_catalog_cache_context.as_ref(),
        tool_catalog_fetch_ticket,
    ) {
        cache_context.publish_if_newest(fetch_ticket, &shared_tools);
    }
    if has_shared_tool_catalog {
        emit_duration(
            MCP_TOOLS_LIST_DURATION_METRIC,
            list_start.elapsed(),
            &[("cache", "miss")],
        );
    }
    let managed = ManagedClient {
        client: Arc::clone(&client),
        server_info,
        tools: client_tools,
        tool_timeout: Some(tool_timeout),
        tool_filter,
        server_instructions: initialize_result.instructions,
        server_supports_sandbox_state_meta_capability,
        codex_apps_tools_cache_context,
    };

    Ok(managed)
}

fn mcp_initialize_request_params(
    client_elicitation_capability: ElicitationCapability,
    supports_openai_form_elicitation: bool,
) -> InitializeRequestParams {
    let mut capabilities = ClientCapabilities::default();
    capabilities.elicitation = Some(client_elicitation_capability);
    if supports_openai_form_elicitation {
        capabilities.extensions = Some(BTreeMap::from([(
            OPENAI_FORM_CAPABILITY.to_string(),
            JsonObject::new(),
        )]));
    }
    InitializeRequestParams::new(
        capabilities,
        Implementation::new("codex-mcp-client", env!("CARGO_PKG_VERSION")).with_title("Codex"),
    )
    .with_protocol_version(ProtocolVersion::V_2025_06_18)
}

fn mcp_server_info_from_implementation(server_info: Implementation) -> McpServerInfo {
    McpServerInfo {
        name: server_info.name,
        title: server_info.title,
        version: server_info.version,
        description: server_info.description,
        icons: server_info.icons.map(|icons| {
            icons
                .into_iter()
                .filter_map(|icon| serde_json::to_value(icon).ok())
                .collect()
        }),
        website_url: server_info.website_url,
    }
}

struct StartServerTaskParams {
    is_codex_apps_mcp_server: bool,
    startup_timeout: Option<Duration>, // TODO: cancel_token should handle this.
    tool_timeout: Duration,
    tool_filter: ToolFilter,
    tx_event: Option<Sender<Event>>,
    elicitation_requests: ElicitationRequestManager,
    codex_apps_tools_cache_context: Option<ConnectorRuntimeContext<ToolInfo>>,
    tool_catalog_cache_context: Option<McpToolCatalogCacheContext>,
    tool_catalog_fetch_ticket: Option<McpToolCatalogFetchTicket>,
    client_elicitation_capability: ElicitationCapability,
    supports_openai_form_elicitation: bool,
}

#[instrument(level = "trace", skip_all, fields(server_name = %server_name))]
async fn make_rmcp_client(
    server_name: &str,
    server: EffectiveMcpServer,
    store_mode: OAuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
    runtime_context: McpRuntimeContext,
    resolved_environment: std::result::Result<Option<Arc<Environment>>, String>,
    runtime_auth_provider: Option<SharedAuthProvider>,
) -> Result<RmcpClient, StartupOutcomeError> {
    let config = match server.launch() {
        McpServerLaunch::Configured(config) => config.as_ref().clone(),
    };
    let resolved_environment =
        resolved_environment.map_err(|err| StartupOutcomeError::from(anyhow!(err)))?;
    let is_local_environment = config.is_local_environment();
    let McpServerConfig { transport, .. } = config;

    match transport {
        McpServerTransportConfig::Stdio {
            command,
            args,
            env,
            env_vars,
            cwd,
        } => {
            let command_os: OsString = command.into();
            let args_os: Vec<OsString> = args.into_iter().map(Into::into).collect();
            let env_os = env.map(|env| {
                env.into_iter()
                    .map(|(key, value)| (key.into(), value.into()))
                    .collect::<HashMap<_, _>>()
            });
            let launcher = if is_local_environment {
                // TODO(starr): Unify local stdio MCP launch with
                // `ExecutorStdioServerLauncher` once the executor-backed path
                // preserves `LocalStdioServerLauncher` semantics.
                Arc::new(LocalStdioServerLauncher::new(
                    runtime_context.local_stdio_fallback_cwd(),
                )) as Arc<dyn StdioServerLauncher>
            } else {
                let Some(environment) = resolved_environment.as_ref() else {
                    unreachable!(
                        "non-local stdio MCP servers resolve an environment before launch"
                    );
                };
                Arc::new(ExecutorStdioServerLauncher::new(
                    environment.get_exec_backend(),
                )) as Arc<dyn StdioServerLauncher>
            };

            let cwd = cwd.map(codex_utils_path_uri::LegacyAppPathString::into_string);
            RmcpClient::new_stdio_client(command_os, args_os, env_os, &env_vars, cwd, launcher)
                .await
                .map_err(|err| StartupOutcomeError::from(anyhow!(err)))
        }
        McpServerTransportConfig::StreamableHttp {
            url,
            http_headers,
            env_http_headers,
            bearer_token_env_var,
        } => {
            let http_client = resolved_environment.as_ref().map_or_else(
                || Arc::new(ReqwestHttpClient) as Arc<dyn HttpClient>,
                |environment| environment.get_http_client(),
            );
            let http_client = maybe_with_openai_docs_source_attribution(&url, http_client);
            let resolved_bearer_token =
                match resolve_bearer_token(server_name, bearer_token_env_var.as_deref()) {
                    Ok(token) => token,
                    Err(error) => return Err(error.into()),
                };
            RmcpClient::new_streamable_http_client(
                server_name,
                &url,
                resolved_bearer_token,
                http_headers,
                env_http_headers,
                store_mode,
                keyring_backend_kind,
                http_client,
                runtime_auth_provider,
            )
            .await
            .map_err(StartupOutcomeError::from)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use rmcp::model::JsonObject;
    use rmcp::model::Meta;
    use rmcp::transport::auth::AuthError;

    #[test]
    fn startup_outcome_error_identifies_authentication_required() {
        let error = anyhow::Error::new(AuthError::AuthorizationRequired)
            .context("failed to initialize MCP server");

        let error = StartupOutcomeError::from(error);

        assert!(error.is_authentication_required());
    }

    #[test]
    fn mcp_initialize_advertises_openai_form_only_when_supported() {
        let unsupported = mcp_initialize_request_params(
            ElicitationCapability::default(),
            /*supports_openai_form_elicitation*/ false,
        );
        assert_eq!(unsupported.capabilities.extensions, None);

        let supported = mcp_initialize_request_params(
            ElicitationCapability::default(),
            /*supports_openai_form_elicitation*/ true,
        );
        assert_eq!(
            supported.capabilities.extensions,
            Some(BTreeMap::from([(
                OPENAI_FORM_CAPABILITY.to_string(),
                JsonObject::new(),
            )]))
        );
    }

    fn tool_with_connector_meta() -> RmcpTool {
        RmcpTool::new(
            "capture_file_upload",
            "test tool",
            Arc::new(JsonObject::default()),
        )
        .with_meta(Meta(
            serde_json::json!({
                "connector_id": "connector_gmail",
                "connector_name": "Gmail",
                "connector_display_name": "Gmail",
                "connector_description": "Mail connector",
                "connectorDescription": "Mail connector",
                "connectorFutureField": "future connector metadata",
                "CONNECTOR_UPPERCASE": "uppercase connector metadata",
                "openai/fileParams": ["file"],
                "custom": "kept"
            })
            .as_object()
            .expect("object")
            .clone(),
        ))
    }

    #[test]
    fn custom_mcp_connector_metadata_is_stripped() {
        let mut tool = tool_with_connector_meta();

        strip_untrusted_connector_meta(&mut tool);

        let meta = tool.meta.as_ref().expect("meta");
        for key in [
            "connector_id",
            "connector_name",
            "connector_display_name",
            "connector_description",
            "connectorDescription",
        ] {
            assert!(!meta.0.contains_key(key), "{key} should be stripped");
        }
        assert!(meta.0.contains_key("connectorFutureField"));
        assert!(meta.0.contains_key("CONNECTOR_UPPERCASE"));
        assert!(meta.0.contains_key("openai/fileParams"));
        assert_eq!(
            meta.0.get("custom").and_then(|value| value.as_str()),
            Some("kept")
        );
    }

    #[test]
    fn codex_apps_connector_metadata_is_preserved() {
        let tool = tool_with_connector_meta();
        let expected_tool = tool.clone();

        let tool_info = tool_info_from_listed_tool(
            CODEX_APPS_MCP_SERVER_NAME,
            /*is_codex_apps_mcp_server*/ true,
            /*server_instructions*/ None,
            ToolWithConnectorId {
                tool,
                connector_id: Some("connector_gmail".to_string()),
                connector_name: Some("Gmail".to_string()),
                connector_description: Some("Mail connector".to_string()),
            },
        );

        let expected = ToolInfo {
            server_name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
            supports_parallel_tool_calls: false,
            server_origin: None,
            callable_name: "capture_file_upload".to_string(),
            callable_namespace: "codex_apps__gmail".to_string(),
            namespace_description: Some("Mail connector".to_string()),
            tool: expected_tool,
            openai_file_input_optional_fields: HashMap::new(),
            connector_id: Some("connector_gmail".to_string()),
            connector_name: Some("Gmail".to_string()),
            plugin_display_names: Vec::new(),
        };
        assert_eq!(
            serde_json::to_value(tool_info).expect("serialize actual tool info"),
            serde_json::to_value(expected).expect("serialize expected tool info")
        );
    }
}
