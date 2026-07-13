//! Aggregates MCP server connections for Codex.
//!
//! [`McpConnectionManager`] owns the set of running async RMCP clients keyed by
//! MCP server name. It coordinates startup status events, keeps server origin
//! metadata, aggregates tools/resources/templates across servers, routes tool
//! calls to the right client, and exposes the public manager API used by
//! `codex-core`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use crate::codex_apps::prepare_openai_file_params_for_model;
use crate::elicitation::ElicitationRequestManager;
use crate::elicitation::ElicitationRequestRouter;
use crate::elicitation::ElicitationReviewerHandle;
use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;
use crate::mcp::ToolPluginProvenance;
use crate::rmcp_client::AsyncManagedClient;
use crate::rmcp_client::CODEX_APPS_REFRESH_DURATION_METRIC;
use crate::rmcp_client::DEFAULT_STARTUP_TIMEOUT;
use crate::rmcp_client::MCP_TOOLS_LIST_DURATION_METRIC;
use crate::rmcp_client::ManagedClient;
use crate::rmcp_client::StartupOutcomeError;
use crate::rmcp_client::list_tools_for_client_uncached;
use crate::runtime::McpRuntimeContext;
use crate::runtime::emit_duration;
use crate::server::EffectiveMcpServer;
use crate::server::McpServerMetadata;
use crate::tools::ToolInfo;
use crate::tools::filter_tools;
use crate::tools::normalize_tools_for_model_with_prefix;
use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use async_channel::Sender;
use codex_api::SharedAuthProvider;
use codex_config::Constrained;
use codex_config::McpServerAuth;
use codex_config::McpServerConfig;
use codex_config::McpServerTransportConfig;
use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_connectors::ConnectorRuntimeContextKey;
use codex_connectors::ConnectorRuntimeFetchSource;
use codex_connectors::ConnectorRuntimeManager;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_protocol::mcp::CallToolResult;
use codex_protocol::mcp::McpServerInfo;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::McpStartupCompleteEvent;
use codex_protocol::protocol::McpStartupFailure;
use codex_protocol::protocol::McpStartupFailureReason;
use codex_protocol::protocol::McpStartupStatus;
use codex_protocol::protocol::McpStartupUpdateEvent;
use codex_rmcp_client::ElicitationResponse;
use codex_rmcp_client::McpAuthState;
use codex_rmcp_client::McpLoginRequirement;
use codex_rmcp_client::determine_streamable_http_auth_status_from_credentials;
use rmcp::model::ElicitationCapability;
use rmcp::model::ListResourceTemplatesResult;
use rmcp::model::ListResourcesResult;
use rmcp::model::PaginatedRequestParams;
use rmcp::model::ReadResourceRequestParams;
use rmcp::model::ReadResourceResult;
use rmcp::model::RequestId;
use rmcp::model::Resource;
use rmcp::model::ResourceTemplate;
use serde_json::Value as JsonValue;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::Instrument;
use tracing::info_span;
use tracing::instrument;
use tracing::trace;
use tracing::trace_span;
use tracing::warn;

const MCP_UI_META_KEY: &str = "ui";
const MCP_UI_VISIBILITY_META_KEY: &str = "visibility";
const MCP_UI_MODEL_VISIBILITY: &str = "model";

/// Returns whether a tool may be included in model-facing tool declarations.
///
/// Tools without visibility metadata remain visible.
/// Tools with visibility metadata are hidden unless they explicitly include `model`.
///
/// <https://github.com/modelcontextprotocol/ext-apps/blob/main/specification/2026-01-26/apps.mdx#resource-discovery>
pub fn tool_is_model_visible(tool: &ToolInfo) -> bool {
    let Some(visibility) = tool
        .tool
        .meta
        .as_deref()
        .and_then(|meta| meta.get(MCP_UI_META_KEY))
        .and_then(JsonValue::as_object)
        .and_then(|ui| ui.get(MCP_UI_VISIBILITY_META_KEY))
        .and_then(JsonValue::as_array)
    else {
        return true;
    };

    visibility
        .iter()
        .any(|target| target.as_str() == Some(MCP_UI_MODEL_VISIBILITY))
}

/// A thin wrapper around a set of running [`RmcpClient`] instances.
pub struct McpConnectionManager {
    clients: HashMap<String, AsyncManagedClient>,
    server_metadata: HashMap<String, McpServerMetadata>,
    required_servers: Vec<String>,
    tool_plugin_provenance: Arc<ToolPluginProvenance>,
    prefix_mcp_tool_names: bool,
    elicitation_requests: ElicitationRequestManager,
    startup_cancellation_token: CancellationToken,
}

impl McpConnectionManager {
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        mcp_servers: &HashMap<String, EffectiveMcpServer>,
        store_mode: OAuthCredentialsStoreMode,
        keyring_backend_kind: AuthKeyringBackendKind,
        approval_policy: &Constrained<AskForApproval>,
        submit_id: String,
        tx_event: Sender<Event>,
        startup_cancellation_token: CancellationToken,
        initial_permission_profile: PermissionProfile,
        runtime_context: McpRuntimeContext,
        codex_home: PathBuf,
        codex_apps_tools_cache: ConnectorRuntimeManager<ToolInfo>,
        codex_apps_tools_cache_key: ConnectorRuntimeContextKey,
        prefix_mcp_tool_names: bool,
        client_elicitation_capability: ElicitationCapability,
        supports_openai_form_elicitation: bool,
        tool_plugin_provenance: ToolPluginProvenance,
        auth: Option<&CodexAuth>,
        codex_apps_auth_manager: Option<Arc<AuthManager>>,
        elicitation_reviewer: Option<ElicitationReviewerHandle>,
        elicitation_lifecycle: Option<crate::ElicitationLifecycle>,
        elicitation_router: ElicitationRequestRouter,
    ) -> Self {
        let mut required_servers = mcp_servers
            .iter()
            .filter(|(_, server)| server.enabled() && server.required())
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        required_servers.sort();
        let mut clients = HashMap::new();
        let mut server_metadata = HashMap::new();
        let mut join_set = JoinSet::new();
        let elicitation_requests = ElicitationRequestManager::new(
            approval_policy.value(),
            initial_permission_profile,
            elicitation_reviewer,
            elicitation_lifecycle,
            elicitation_router,
        );
        let tool_plugin_provenance = Arc::new(tool_plugin_provenance);
        let startup_submit_id = submit_id.clone();
        let static_chatgpt_auth_provider = auth
            .filter(|auth| auth.uses_codex_backend())
            .map(codex_model_provider::auth_provider_from_auth);
        let codex_apps_auth_provider = codex_apps_auth_manager.and_then(|auth_manager| {
            auth.filter(|auth| auth.uses_codex_backend()).map(|auth| {
                codex_model_provider::auth_provider_from_auth_manager(auth_manager, auth)
            })
        });
        let mcp_servers = mcp_servers.clone();
        for (server_name, server) in mcp_servers
            .into_iter()
            .filter(|(_, server)| server.enabled())
        {
            server_metadata.insert(server_name.clone(), McpServerMetadata::from(&server));
            let cancel_token = startup_cancellation_token.child_token();
            let _ = emit_update(
                startup_submit_id.as_str(),
                &tx_event,
                McpStartupUpdateEvent {
                    server: server_name.clone(),
                    status: McpStartupStatus::Starting,
                },
            )
            .await;
            let configured_config = server.configured_config().cloned();
            // For built-in Codex Apps, `CODEX_CONNECTORS_TOKEN` is a debug
            // override: it supplies runtime auth but bypasses the shared tools
            // cache.
            let uses_env_bearer_token =
                configured_config
                    .as_ref()
                    .is_some_and(|config| match &config.transport {
                        McpServerTransportConfig::StreamableHttp {
                            bearer_token_env_var,
                            ..
                        } => bearer_token_env_var.is_some(),
                        McpServerTransportConfig::Stdio { .. } => false,
                    });
            let shares_codex_apps_tools_cache =
                should_share_codex_apps_tools_cache(&server_name, uses_env_bearer_token);
            let codex_apps_tools_cache_context = shares_codex_apps_tools_cache.then(|| {
                codex_apps_tools_cache
                    .context(codex_home.clone(), codex_apps_tools_cache_key.clone())
            });
            // The reserved Codex Apps registration follows the shared
            // AuthManager across refreshes. In the hosted-plugin path, this
            // is the ChatGPT /ps/mcp connection. User-configured MCP
            // registrations keep their existing configured auth path.
            let chatgpt_auth_provider = if server_name == CODEX_APPS_MCP_SERVER_NAME {
                codex_apps_auth_provider
                    .clone()
                    .or_else(|| static_chatgpt_auth_provider.clone())
            } else {
                static_chatgpt_auth_provider.clone()
            };
            // If Codex Apps has an env bearer token, that is its auth path. Do
            // not also attach the ambient CodexAuth provider.
            let runtime_auth_provider =
                if server_name == CODEX_APPS_MCP_SERVER_NAME && uses_env_bearer_token {
                    None
                } else {
                    chatgpt_auth_provider_for_server(&server, chatgpt_auth_provider)
                };
            let has_runtime_auth = runtime_auth_provider.is_some();
            let async_managed_client = AsyncManagedClient::new(
                server_name.clone(),
                startup_submit_id.clone(),
                server,
                store_mode,
                keyring_backend_kind,
                cancel_token.clone(),
                tx_event.clone(),
                elicitation_requests.clone(),
                codex_apps_tools_cache_context,
                Arc::clone(&tool_plugin_provenance),
                runtime_context.clone(),
                runtime_auth_provider,
                client_elicitation_capability.clone(),
                supports_openai_form_elicitation,
            );
            clients.insert(server_name.clone(), async_managed_client.clone());
            let tx_event = tx_event.clone();
            let submit_id = startup_submit_id.clone();
            join_set.spawn(async move {
                let mut outcome = async_managed_client.client().await;
                if cancel_token.is_cancelled() {
                    outcome = Err(StartupOutcomeError::Cancelled);
                }
                let auth_state = match &outcome {
                    Err(error) if error.is_authentication_required() && !has_runtime_auth => {
                        configured_config.as_ref().and_then(|config| {
                            let McpServerTransportConfig::StreamableHttp {
                                url,
                                bearer_token_env_var,
                                http_headers,
                                env_http_headers,
                            } = &config.transport
                            else {
                                return None;
                            };
                            match determine_streamable_http_auth_status_from_credentials(
                                &server_name,
                                url,
                                bearer_token_env_var.as_deref(),
                                http_headers.clone(),
                                env_http_headers.clone(),
                                store_mode,
                                keyring_backend_kind,
                            ) {
                                Ok(auth_state) => auth_state,
                                Err(error) => {
                                    warn!(
                                        "failed to read stored auth status for MCP server `{server_name}`: {error:?}"
                                    );
                                    None
                                }
                            }
                        })
                    }
                    Ok(_) | Err(_) => None,
                };
                if cancel_token.is_cancelled() {
                    outcome = Err(StartupOutcomeError::Cancelled);
                }
                let status = match &outcome {
                    Ok(_) => McpStartupStatus::Ready,
                    Err(StartupOutcomeError::Cancelled) => McpStartupStatus::Cancelled,
                    Err(error) => {
                        let reason = mcp_startup_failure_reason(auth_state, error);
                        let error_str = mcp_init_error_display(
                            server_name.as_str(),
                            configured_config.as_ref(),
                            error,
                        );
                        McpStartupStatus::Failed {
                            error: error_str,
                            reason,
                        }
                    }
                };

                let _ = emit_update(
                    submit_id.as_str(),
                    &tx_event,
                    McpStartupUpdateEvent {
                        server: server_name.clone(),
                        status,
                    },
                )
                .await;

                if matches!(&outcome, Err(StartupOutcomeError::Failed { .. })) {
                    async_managed_client.reconnect_failed_startup().await;
                }

                (server_name, outcome)
            });
        }
        let manager = Self {
            clients,
            server_metadata,
            required_servers,
            tool_plugin_provenance,
            prefix_mcp_tool_names,
            elicitation_requests: elicitation_requests.clone(),
            startup_cancellation_token: startup_cancellation_token.clone(),
        };
        tokio::spawn(async move {
            let outcomes = join_set.join_all().await;
            let mut summary = McpStartupCompleteEvent::default();
            for (server_name, outcome) in outcomes {
                match outcome {
                    Ok(_) => summary.ready.push(server_name),
                    Err(StartupOutcomeError::Cancelled) => summary.cancelled.push(server_name),
                    Err(StartupOutcomeError::Failed { error, .. }) => {
                        summary.failed.push(McpStartupFailure {
                            server: server_name,
                            error,
                        })
                    }
                }
            }
            let _ = tx_event
                .send(Event {
                    id: startup_submit_id,
                    msg: EventMsg::McpStartupComplete(summary),
                })
                .await;
        });
        manager
    }

    /// Waits for every required server and reports their startup failures together.
    ///
    /// Callers must make the manager reachable to request handlers before awaiting this method,
    /// because server initialization may require client elicitation.
    pub async fn validate_required_servers(&self) -> Result<()> {
        let failures = async {
            let mut failures = Vec::new();
            for server_name in &self.required_servers {
                let Some(async_managed_client) = self.clients.get(server_name).cloned() else {
                    failures.push(McpStartupFailure {
                        server: server_name.clone(),
                        error: format!("required MCP server `{server_name}` was not initialized"),
                    });
                    continue;
                };

                match async_managed_client.client().await {
                    Ok(_) => {}
                    Err(error) => failures.push(McpStartupFailure {
                        server: server_name.clone(),
                        error: startup_outcome_error_message(error),
                    }),
                }
            }
            failures
        }
        .instrument(info_span!(
            "session_init.required_mcp_wait",
            otel.name = "session_init.required_mcp_wait",
            session_init.required_mcp_server_count = self.required_servers.len(),
        ))
        .await;
        if failures.is_empty() {
            return Ok(());
        }

        let details = failures
            .iter()
            .map(|failure| format!("{}: {}", failure.server, failure.error))
            .collect::<Vec<_>>()
            .join("; ");
        Err(anyhow!(
            "required MCP servers failed to initialize: {details}"
        ))
    }

    pub fn new_uninitialized_with_permission_profile(
        approval_policy: &Constrained<AskForApproval>,
        permission_profile: &PermissionProfile,
        prefix_mcp_tool_names: bool,
    ) -> Self {
        Self {
            clients: HashMap::new(),
            server_metadata: HashMap::new(),
            required_servers: Vec::new(),
            tool_plugin_provenance: Arc::new(ToolPluginProvenance::default()),
            prefix_mcp_tool_names,
            elicitation_requests: ElicitationRequestManager::new(
                approval_policy.value(),
                permission_profile.clone(),
                /*reviewer*/ None,
                /*lifecycle*/ None,
                ElicitationRequestRouter::default(),
            ),
            startup_cancellation_token: CancellationToken::new(),
        }
    }

    pub fn has_servers(&self) -> bool {
        !self.clients.is_empty()
    }

    pub(crate) fn contains_server(&self, server_name: &str) -> bool {
        self.clients.contains_key(server_name)
    }

    /// Stop all MCP clients owned by this manager and terminate stdio server processes.
    pub async fn shutdown(&self) {
        self.startup_cancellation_token.cancel();
        let clients = self.clients.values().cloned().collect::<Vec<_>>();
        // Keep cleanup alive if an interrupt cancels the refresh that requested it.
        let shutdown_task = tokio::spawn(async move {
            for client in clients {
                client.shutdown().await;
            }
        });
        if let Err(error) = shutdown_task.await {
            warn!("MCP client shutdown task failed: {error}");
        }
    }

    pub fn server_origin(&self, server_name: &str) -> Option<&str> {
        self.server_metadata
            .get(server_name)
            .and_then(|metadata| metadata.origin.as_ref())
            .map(super::server::McpServerOrigin::as_str)
    }

    pub fn server_environment_id(&self, server_name: &str) -> Option<&str> {
        self.server_metadata
            .get(server_name)
            .map(|metadata| metadata.environment_id.as_str())
    }

    pub fn server_pollutes_memory(&self, server_name: &str) -> bool {
        self.server_metadata
            .get(server_name)
            .is_none_or(|metadata| metadata.pollutes_memory)
    }

    pub fn plugin_id_for_mcp_server_name(&self, server_name: &str) -> Option<&str> {
        self.tool_plugin_provenance
            .plugin_id_for_mcp_server_name(server_name)
    }

    pub fn is_selected_plugin_mcp_server(&self, server_name: &str) -> bool {
        self.tool_plugin_provenance
            .is_selected_plugin_mcp_server(server_name)
    }

    pub fn tool_approval_mode(
        &self,
        server_name: &str,
        tool_name: &str,
    ) -> codex_config::AppToolApproval {
        self.server_metadata
            .get(server_name)
            .map(|metadata| metadata.tool_approval_mode(tool_name))
            .unwrap_or_default()
    }

    pub fn is_host_owned_codex_apps_server(&self, server_name: &str) -> bool {
        server_name == CODEX_APPS_MCP_SERVER_NAME && self.server_metadata.contains_key(server_name)
    }

    pub fn set_approval_policy(&self, approval_policy: &Constrained<AskForApproval>) {
        if let Ok(mut policy) = self.elicitation_requests.approval_policy.lock() {
            *policy = approval_policy.value();
        }
    }

    pub fn set_permission_profile(&self, permission_profile: PermissionProfile) {
        if let Ok(mut profile) = self.elicitation_requests.permission_profile.lock() {
            *profile = permission_profile;
        }
    }

    pub fn elicitations_auto_deny(&self) -> bool {
        self.elicitation_requests.auto_deny()
    }

    pub fn set_elicitations_auto_deny(&self, auto_deny: bool) {
        self.elicitation_requests.set_auto_deny(auto_deny);
    }

    pub fn elicitation_router(&self) -> ElicitationRequestRouter {
        self.elicitation_requests.router()
    }

    pub async fn resolve_elicitation(
        &self,
        server_name: String,
        id: RequestId,
        response: ElicitationResponse,
    ) -> Result<()> {
        self.elicitation_requests
            .resolve(server_name, id, response)
            .await
    }

    pub async fn wait_for_server_ready(&self, server_name: &str, timeout: Duration) -> bool {
        let Some(async_managed_client) = self.clients.get(server_name) else {
            return false;
        };

        match tokio::time::timeout(timeout, async_managed_client.client()).await {
            Ok(Ok(_)) => true,
            Ok(Err(_)) | Err(_) => false,
        }
    }

    /// Returns all tools with model-visible names normalized.
    #[instrument(level = "trace", skip_all, fields(mcp_server_count = self.clients.len()))]
    pub async fn list_all_tools(&self) -> Vec<ToolInfo> {
        let mut tools = Vec::new();
        let mut available_server_count = 0;
        let mut unavailable_server_count = 0;
        for (server_name, managed_client) in &self.clients {
            managed_client.reconnect_failed_startup().await;
            let has_cached_tools = managed_client.has_cached_tools();
            let startup_complete = managed_client
                .startup_complete
                .load(std::sync::atomic::Ordering::Acquire);
            let Some(server_tools) = managed_client
                .listed_tools()
                .instrument(trace_span!(
                    "list_tools_for_server",
                    server_name = %server_name,
                    has_cached_tools,
                    startup_complete
                ))
                .await
            else {
                unavailable_server_count += 1;
                trace!(
                    server_name = %server_name,
                    has_cached_tools,
                    startup_complete,
                    "MCP server tools unavailable while building tool list"
                );
                continue;
            };
            available_server_count += 1;
            tools.extend(
                server_tools
                    .into_iter()
                    .map(|tool| self.with_server_metadata(tool)),
            );
        }
        let tools = normalize_tools_for_model_with_prefix(tools, self.prefix_mcp_tool_names);
        trace!(
            available_server_count,
            unavailable_server_count,
            tool_count = tools.len(),
            "built MCP tool list"
        );
        tools
    }

    /// Force-refresh codex apps tools by bypassing the in-process cache.
    ///
    /// On success, the refreshed tools replace shared cache contents when the
    /// cache is enabled and the latest filtered tools are returned directly to
    /// the caller. On failure, existing shared cache contents remain unchanged.
    pub async fn hard_refresh_codex_apps_tools_cache(&self) -> Result<Vec<ToolInfo>> {
        let refresh_start = Instant::now();
        let managed_client = self
            .clients
            .get(CODEX_APPS_MCP_SERVER_NAME)
            .ok_or_else(|| anyhow!("unknown MCP server '{CODEX_APPS_MCP_SERVER_NAME}'"))?
            .client()
            .await
            .context("failed to get client")?;

        let list_start = Instant::now();
        let fetch_ticket =
            managed_client
                .codex_apps_tools_cache_context
                .as_ref()
                .map(|cache_context| {
                    cache_context.begin_fetch(ConnectorRuntimeFetchSource::HardRefresh)
                });
        let tools = list_tools_for_client_uncached(
            CODEX_APPS_MCP_SERVER_NAME,
            /*is_codex_apps_mcp_server*/ true,
            /*codex_apps_refresh_trigger*/ "explicit",
            &managed_client.client,
            managed_client.tool_timeout,
            managed_client.server_instructions.as_deref(),
        )
        .await
        .with_context(|| {
            format!("failed to refresh tools for MCP server '{CODEX_APPS_MCP_SERVER_NAME}'")
        })?;

        let tools =
            match (
                managed_client.codex_apps_tools_cache_context.as_ref(),
                fetch_ticket,
            ) {
                (Some(cache_context), Some(fetch_ticket)) => cache_context
                    .publish_if_newest_accepted(fetch_ticket, &managed_client.server_info, tools),
                (None, None) => tools,
                _ => unreachable!("Codex Apps fetch ticket requires cache context"),
            };
        emit_duration(
            MCP_TOOLS_LIST_DURATION_METRIC,
            list_start.elapsed(),
            &[("cache", "miss")],
        );
        let tools = filter_tools(tools, &managed_client.tool_filter)
            .into_iter()
            .map(|mut tool| {
                prepare_openai_file_params_for_model(&mut tool);
                self.with_server_metadata(tool)
            });
        let tools = normalize_tools_for_model_with_prefix(tools, self.prefix_mcp_tool_names);
        emit_duration(
            CODEX_APPS_REFRESH_DURATION_METRIC,
            refresh_start.elapsed(),
            &[("path", "legacy"), ("trigger", "explicit")],
        );
        Ok(tools)
    }

    /// Returns resources from servers selected by `include_server`. Each key
    /// is the server name and the value is a vector of resources.
    pub async fn list_all_resources(
        &self,
        include_server: impl Fn(&str) -> bool,
    ) -> HashMap<String, Vec<Resource>> {
        let mut join_set = JoinSet::new();

        let clients_snapshot = &self.clients;

        for (server_name, async_managed_client) in clients_snapshot
            .iter()
            .filter(|(server_name, _)| include_server(server_name))
        {
            let server_name = server_name.clone();
            let Ok(managed_client) = async_managed_client.client().await else {
                continue;
            };
            let timeout = managed_client.tool_timeout;
            let client = managed_client.client.clone();

            join_set.spawn(async move {
                let mut collected: Vec<Resource> = Vec::new();
                let mut cursor: Option<String> = None;

                loop {
                    let params = cursor.as_ref().map(|next| {
                        PaginatedRequestParams::default().with_cursor(Some(next.clone()))
                    });
                    let response = match client.list_resources(params, timeout).await {
                        Ok(result) => result,
                        Err(err) => return (server_name, Err(err)),
                    };

                    collected.extend(response.resources);

                    match response.next_cursor {
                        Some(next) => {
                            if cursor.as_ref() == Some(&next) {
                                return (
                                    server_name,
                                    Err(anyhow!("resources/list returned duplicate cursor")),
                                );
                            }
                            cursor = Some(next);
                        }
                        None => return (server_name, Ok(collected)),
                    }
                }
            });
        }

        let mut aggregated: HashMap<String, Vec<Resource>> = HashMap::new();

        while let Some(join_res) = join_set.join_next().await {
            match join_res {
                Ok((server_name, Ok(resources))) => {
                    aggregated.insert(server_name, resources);
                }
                Ok((server_name, Err(err))) => {
                    warn!("Failed to list resources for MCP server '{server_name}': {err:#}");
                }
                Err(err) => {
                    warn!("Task panic when listing resources for MCP server: {err:#}");
                }
            }
        }

        aggregated
    }

    /// Returns resource templates from servers selected by `include_server`.
    /// Each key is the server name and the value is a vector of templates.
    pub async fn list_all_resource_templates(
        &self,
        include_server: impl Fn(&str) -> bool,
    ) -> HashMap<String, Vec<ResourceTemplate>> {
        let mut join_set = JoinSet::new();

        let clients_snapshot = &self.clients;

        for (server_name, async_managed_client) in clients_snapshot
            .iter()
            .filter(|(server_name, _)| include_server(server_name))
        {
            let server_name_cloned = server_name.clone();
            let Ok(managed_client) = async_managed_client.client().await else {
                continue;
            };
            let client = managed_client.client.clone();
            let timeout = managed_client.tool_timeout;

            join_set.spawn(async move {
                let mut collected: Vec<ResourceTemplate> = Vec::new();
                let mut cursor: Option<String> = None;

                loop {
                    let params = cursor.as_ref().map(|next| {
                        PaginatedRequestParams::default().with_cursor(Some(next.clone()))
                    });
                    let response = match client.list_resource_templates(params, timeout).await {
                        Ok(result) => result,
                        Err(err) => return (server_name_cloned, Err(err)),
                    };

                    collected.extend(response.resource_templates);

                    match response.next_cursor {
                        Some(next) => {
                            if cursor.as_ref() == Some(&next) {
                                return (
                                    server_name_cloned,
                                    Err(anyhow!(
                                        "resources/templates/list returned duplicate cursor"
                                    )),
                                );
                            }
                            cursor = Some(next);
                        }
                        None => return (server_name_cloned, Ok(collected)),
                    }
                }
            });
        }

        let mut aggregated: HashMap<String, Vec<ResourceTemplate>> = HashMap::new();

        while let Some(join_res) = join_set.join_next().await {
            match join_res {
                Ok((server_name, Ok(templates))) => {
                    aggregated.insert(server_name, templates);
                }
                Ok((server_name, Err(err))) => {
                    warn!(
                        "Failed to list resource templates for MCP server '{server_name}': {err:#}"
                    );
                }
                Err(err) => {
                    warn!("Task panic when listing resource templates for MCP server: {err:#}");
                }
            }
        }

        aggregated
    }

    /// Invoke the tool indicated by the (server, tool) pair.
    pub async fn call_tool(
        &self,
        server: &str,
        tool: &str,
        arguments: Option<serde_json::Value>,
        meta: Option<serde_json::Value>,
    ) -> Result<CallToolResult> {
        let client = self.client_by_name(server).await?;
        if !client.tool_filter.allows(tool) {
            return Err(anyhow!(
                "tool '{tool}' is disabled for MCP server '{server}'"
            ));
        }

        let result: rmcp::model::CallToolResult = client
            .client
            .call_tool(tool.to_string(), arguments, meta, client.tool_timeout)
            .await
            .with_context(|| format!("tool call failed for `{server}/{tool}`"))?;

        let content = result
            .content
            .into_iter()
            .map(|content| {
                serde_json::to_value(content)
                    .unwrap_or_else(|_| serde_json::Value::String("<content>".to_string()))
            })
            .collect();

        Ok(CallToolResult {
            content,
            structured_content: result.structured_content,
            is_error: result.is_error,
            meta: result.meta.and_then(|meta| serde_json::to_value(meta).ok()),
        })
    }

    pub async fn server_supports_sandbox_state_meta_capability(
        &self,
        server: &str,
    ) -> Result<bool> {
        Ok(self
            .client_by_name(server)
            .await?
            .server_supports_sandbox_state_meta_capability)
    }

    /// List resources from the specified server.
    pub async fn list_resources(
        &self,
        server: &str,
        params: Option<PaginatedRequestParams>,
    ) -> Result<ListResourcesResult> {
        let managed = self.client_by_name(server).await?;
        let timeout = managed.tool_timeout;

        managed
            .client
            .list_resources(params, timeout)
            .await
            .with_context(|| format!("resources/list failed for `{server}`"))
    }

    /// List resource templates from the specified server.
    pub async fn list_resource_templates(
        &self,
        server: &str,
        params: Option<PaginatedRequestParams>,
    ) -> Result<ListResourceTemplatesResult> {
        let managed = self.client_by_name(server).await?;
        let client = managed.client.clone();
        let timeout = managed.tool_timeout;

        client
            .list_resource_templates(params, timeout)
            .await
            .with_context(|| format!("resources/templates/list failed for `{server}`"))
    }

    /// Read a resource from the specified server.
    pub async fn read_resource(
        &self,
        server: &str,
        params: ReadResourceRequestParams,
    ) -> Result<ReadResourceResult> {
        let managed = self.client_by_name(server).await?;
        let client = managed.client.clone();
        let timeout = managed.tool_timeout;
        let uri = params.uri.clone();

        client
            .read_resource(params, timeout)
            .await
            .with_context(|| format!("resources/read failed for `{server}` ({uri})"))
    }

    /// Returns presentation metadata without waiting for uncached clients still initializing.
    /// Cached values will be used if available and the server is still starting up.
    pub(crate) async fn list_available_server_infos(&self) -> HashMap<String, McpServerInfo> {
        let mut server_infos = HashMap::new();
        for (server_name, client) in &self.clients {
            if !client.startup_complete.load(Ordering::Acquire) {
                if let Some(server_info) = client.cached_server_info.clone() {
                    server_infos.insert(server_name.clone(), server_info);
                }
                continue;
            }
            match client.client().await {
                Ok(managed_client) => {
                    server_infos.insert(server_name.clone(), managed_client.server_info);
                }
                Err(_) => {
                    if let Some(server_info) = client.cached_server_info.clone() {
                        server_infos.insert(server_name.clone(), server_info);
                    }
                }
            }
        }
        server_infos
    }

    fn with_server_metadata(&self, mut tool: ToolInfo) -> ToolInfo {
        let Some(metadata) = self.server_metadata.get(&tool.server_name) else {
            tool.supports_parallel_tool_calls = false;
            tool.server_origin = None;
            return tool;
        };

        tool.supports_parallel_tool_calls = metadata.supports_parallel_tool_calls;
        tool.server_origin = metadata
            .origin
            .as_ref()
            .map(|origin| origin.as_str().to_string());
        tool
    }

    async fn client_by_name(&self, name: &str) -> Result<ManagedClient> {
        self.clients
            .get(name)
            .ok_or_else(|| anyhow!("unknown MCP server '{name}'"))?
            .client()
            .await
            .context("failed to get client")
    }

    #[cfg(test)]
    fn new_uninitialized(
        approval_policy: &Constrained<AskForApproval>,
        permission_profile: &Constrained<PermissionProfile>,
        prefix_mcp_tool_names: bool,
    ) -> Self {
        Self::new_uninitialized_with_permission_profile(
            approval_policy,
            permission_profile.get(),
            prefix_mcp_tool_names,
        )
    }
}

impl Drop for McpConnectionManager {
    fn drop(&mut self) {
        self.startup_cancellation_token.cancel();
        self.clients.clear();
    }
}

/// Makes ChatGPT authentication available to servers that explicitly opt in.
/// The HTTP transport applies it only when no configured authorization resolves.
fn chatgpt_auth_provider_for_server(
    server: &EffectiveMcpServer,
    chatgpt_auth_provider: Option<SharedAuthProvider>,
) -> Option<SharedAuthProvider> {
    if !server
        .configured_config()
        .is_some_and(|config| matches!(&config.auth, McpServerAuth::ChatGpt))
    {
        return None;
    }
    chatgpt_auth_provider
}

fn should_share_codex_apps_tools_cache(server_name: &str, uses_env_bearer_token: bool) -> bool {
    server_name == CODEX_APPS_MCP_SERVER_NAME && !uses_env_bearer_token
}

async fn emit_update(
    submit_id: &str,
    tx_event: &Sender<Event>,
    update: McpStartupUpdateEvent,
) -> Result<(), async_channel::SendError<Event>> {
    tx_event
        .send(Event {
            id: submit_id.to_string(),
            msg: EventMsg::McpStartupUpdate(update),
        })
        .await
}

fn mcp_startup_failure_reason(
    auth_state: Option<McpAuthState>,
    error: &StartupOutcomeError,
) -> Option<McpStartupFailureReason> {
    if !error.is_authentication_required() {
        return None;
    }

    match auth_state {
        Some(McpAuthState::LoggedOut(McpLoginRequirement::Reauthentication)) => {
            Some(McpStartupFailureReason::ReauthenticationRequired)
        }
        Some(
            McpAuthState::Unsupported
            | McpAuthState::LoggedOut(McpLoginRequirement::Login)
            | McpAuthState::BearerToken
            | McpAuthState::OAuth,
        )
        | None => None,
    }
}

fn mcp_init_error_display(
    server_name: &str,
    config: Option<&McpServerConfig>,
    err: &StartupOutcomeError,
) -> String {
    if let Some(McpServerTransportConfig::StreamableHttp {
        url,
        bearer_token_env_var,
        http_headers,
        ..
    }) = config.map(|config| &config.transport)
        && url == "https://api.githubcopilot.com/mcp/"
        && bearer_token_env_var.is_none()
        && http_headers.as_ref().map(HashMap::is_empty).unwrap_or(true)
    {
        format!(
            "GitHub MCP does not support OAuth. Log in by adding a personal access token (https://github.com/settings/personal-access-tokens) to your environment and config.toml:\n[mcp_servers.{server_name}]\nbearer_token_env_var = CODEX_GITHUB_PERSONAL_ACCESS_TOKEN"
        )
    } else if is_mcp_client_auth_required_error(err) {
        format!(
            "The {server_name} MCP server is not logged in. Run `codex mcp login {server_name}`."
        )
    } else if is_mcp_client_startup_timeout_error(err) {
        let startup_timeout_secs = config
            .and_then(|config| config.startup_timeout_sec)
            .unwrap_or(DEFAULT_STARTUP_TIMEOUT)
            .as_secs();
        format!(
            "MCP client for `{server_name}` timed out after {startup_timeout_secs} seconds. Add or adjust `startup_timeout_sec` in your config.toml:\n[mcp_servers.{server_name}]\nstartup_timeout_sec = XX"
        )
    } else {
        format!("MCP client for `{server_name}` failed to start: {err:#}")
    }
}

fn startup_outcome_error_message(error: StartupOutcomeError) -> String {
    match error {
        StartupOutcomeError::Cancelled => "MCP startup cancelled".to_string(),
        StartupOutcomeError::Failed { error, .. } => error,
    }
}

fn is_mcp_client_auth_required_error(error: &StartupOutcomeError) -> bool {
    match error {
        StartupOutcomeError::Failed { error, .. } => error.contains("Auth required"),
        _ => false,
    }
}

fn is_mcp_client_startup_timeout_error(error: &StartupOutcomeError) -> bool {
    match error {
        StartupOutcomeError::Failed { error, .. } => {
            error.contains("request timed out")
                || error.contains("timed out handshaking with MCP server")
                || error.contains("MCP client startup timed out")
        }
        _ => false,
    }
}

#[cfg(test)]
#[path = "connection_manager_tests.rs"]
mod tests;
