//! Aggregates MCP server connections for Codex.
//!
//! [`McpConnectionManager`] owns the set of running async RMCP clients keyed by
//! MCP server name. It coordinates startup status events, keeps server origin
//! metadata, aggregates tools/resources/templates across servers, routes tool
//! calls to the right client, and exposes the public manager API used by
//! `codex-core`.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::McpAuthStatusEntry;
use crate::elicitation::ElicitationRequestManager;
use crate::elicitation::ElicitationReviewerHandle;
use crate::elicitation::McpElicitationState;
use crate::mcp::ToolPluginProvenance;
use crate::rmcp_client::AsyncManagedClient;
use crate::rmcp_client::DEFAULT_STARTUP_TIMEOUT;
use crate::rmcp_client::ManagedClient;
use crate::rmcp_client::StartupOutcomeError;
use crate::rmcp_client::prepare_regular_mcp_tools_for_model;
use crate::runtime::McpRuntimeContext;
use crate::server::EffectiveMcpServer;
use crate::server::McpElicitationRuntimeMetadata;
use crate::server::McpSandboxStateSource;
use crate::server::McpServerMetadata;
use crate::server::McpToolRuntimeMetadata;
use crate::tools::ToolInfo;
use crate::tools::normalize_tools_for_model_with_prefix;
use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use async_channel::Sender;
use codex_api::SharedAuthProvider;
use codex_config::Constrained;
use codex_config::McpServerAuth;
use codex_config::McpServerTransportConfig;
use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
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
    server_registrations: HashMap<String, EffectiveMcpServer>,
    server_metadata: HashMap<String, McpServerMetadata>,
    required_servers: Vec<String>,
    tool_plugin_provenance: Arc<ToolPluginProvenance>,
    prefix_mcp_tool_names: bool,
    elicitation_requests: HashMap<String, ElicitationRequestManager>,
    elicitation_state: McpElicitationState,
    elicitation_reviewer: Option<ElicitationReviewerHandle>,
    reuse_context: Option<McpClientReuseContext>,
}

/// How a new MCP connection set relates to the currently installed set.
pub enum McpConnectionRefresh<'a> {
    /// Starts every configured client with fresh elicitation state.
    Restart,
    /// Starts every configured client while retaining session-level elicitation state.
    RestartPreservingState(&'a McpConnectionManager),
    /// Retains compatible live clients and restarts only changed or terminal clients.
    ReuseUnchanged(&'a McpConnectionManager),
}

/// One consistent authentication observation used to reconcile MCP clients.
pub struct McpAuthSnapshot<'a> {
    auth: Option<&'a CodexAuth>,
    revision: u64,
}

/// Inputs shared by initial MCP startup and later connection reconciliation.
pub struct McpConnectionManagerInput<'a> {
    pub store_mode: OAuthCredentialsStoreMode,
    pub keyring_backend_kind: AuthKeyringBackendKind,
    pub auth_entries: HashMap<String, McpAuthStatusEntry>,
    pub approval_policy: &'a Constrained<AskForApproval>,
    pub submit_id: String,
    pub tx_event: Sender<Event>,
    pub startup_cancellation_token: CancellationToken,
    pub initial_permission_profile: PermissionProfile,
    pub runtime_context: McpRuntimeContext,
    pub prefix_mcp_tool_names: bool,
    pub client_elicitation_capability: ElicitationCapability,
    pub supports_openai_form_elicitation: bool,
    pub tool_plugin_provenance: ToolPluginProvenance,
    pub auth_snapshot: McpAuthSnapshot<'a>,
    pub elicitation_reviewer: Option<ElicitationReviewerHandle>,
}

impl<'a> McpAuthSnapshot<'a> {
    pub fn new(auth: Option<&'a CodexAuth>, revision: u64) -> Self {
        Self { auth, revision }
    }
}

#[derive(Clone)]
struct McpClientReuseContext {
    store_mode: OAuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
    auth_revision: u64,
    tx_event: Sender<Event>,
    runtime_context: McpRuntimeContext,
    client_elicitation_capability: ElicitationCapability,
    supports_openai_form_elicitation: bool,
}

impl McpClientReuseContext {
    fn is_compatible_with(&self, other: &Self) -> bool {
        self.store_mode == other.store_mode
            && self.keyring_backend_kind == other.keyring_backend_kind
            && self.tx_event.same_channel(&other.tx_event)
            && self.client_elicitation_capability == other.client_elicitation_capability
            && self.supports_openai_form_elicitation == other.supports_openai_form_elicitation
    }

    fn auth_is_compatible_for(&self, other: &Self, server: &EffectiveMcpServer) -> bool {
        !matches!(server.config().auth, McpServerAuth::ChatGpt)
            || self.auth_revision == other.auth_revision
    }
}

impl McpConnectionManager {
    pub async fn new(
        mcp_servers: &HashMap<String, EffectiveMcpServer>,
        input: McpConnectionManagerInput<'_>,
    ) -> Self {
        Self::new_with_refresh(mcp_servers, input, McpConnectionRefresh::Restart).await
    }

    pub async fn new_with_refresh(
        mcp_servers: &HashMap<String, EffectiveMcpServer>,
        input: McpConnectionManagerInput<'_>,
        refresh: McpConnectionRefresh<'_>,
    ) -> Self {
        let McpConnectionManagerInput {
            store_mode,
            keyring_backend_kind,
            auth_entries,
            approval_policy,
            submit_id,
            tx_event,
            startup_cancellation_token,
            initial_permission_profile,
            runtime_context,
            prefix_mcp_tool_names,
            client_elicitation_capability,
            supports_openai_form_elicitation,
            tool_plugin_provenance,
            auth_snapshot,
            elicitation_reviewer,
        } = input;
        let (reusable_previous, elicitation_state) = match refresh {
            McpConnectionRefresh::Restart => (None, McpElicitationState::default()),
            McpConnectionRefresh::RestartPreservingState(previous) => {
                (None, previous.elicitation_state.clone())
            }
            McpConnectionRefresh::ReuseUnchanged(previous) => {
                (Some(previous), previous.elicitation_state.clone())
            }
        };
        let reuse_context = McpClientReuseContext {
            store_mode,
            keyring_backend_kind,
            auth_revision: auth_snapshot.revision,
            tx_event: tx_event.clone(),
            runtime_context: runtime_context.clone(),
            client_elicitation_capability: client_elicitation_capability.clone(),
            supports_openai_form_elicitation,
        };
        let reusable_previous = reusable_previous
            .filter(|previous| {
                previous
                    .reuse_context
                    .as_ref()
                    .is_some_and(|previous_context| {
                        reuse_context.is_compatible_with(previous_context)
                    })
            })
            .filter(|previous| {
                same_elicitation_reviewer(
                    previous.elicitation_reviewer.as_ref(),
                    elicitation_reviewer.as_ref(),
                )
            });
        let mut required_servers = mcp_servers
            .iter()
            .filter(|(_, server)| server.enabled() && server.required())
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        required_servers.sort();
        let mut clients = HashMap::new();
        let mut server_metadata = HashMap::new();
        let mut elicitation_requests = HashMap::new();
        let mut join_set = JoinSet::new();
        let tool_plugin_provenance = Arc::new(tool_plugin_provenance);
        let startup_submit_id = submit_id.clone();
        let chatgpt_auth_provider = auth_snapshot
            .auth
            .filter(|auth| auth.uses_codex_backend())
            .map(codex_model_provider::auth_provider_from_auth);
        let server_registrations = mcp_servers
            .iter()
            .filter(|(_, server)| server.enabled())
            .map(|(name, server)| (name.clone(), server.clone()))
            .collect::<HashMap<_, _>>();
        for (server_name, server) in server_registrations.clone() {
            server_metadata.insert(server_name.clone(), McpServerMetadata::from(&server));
            let _ = emit_update(
                startup_submit_id.as_str(),
                &tx_event,
                McpStartupUpdateEvent {
                    server: server_name.clone(),
                    status: McpStartupStatus::Starting,
                },
            )
            .await;
            let reused = reusable_previous.and_then(|previous| {
                let previous_context = previous.reuse_context.as_ref()?;
                let previous_server = previous.server_registrations.get(&server_name)?;
                if !previous_server.has_same_launch_config(&server)
                    || McpElicitationRuntimeMetadata::from(previous_server.runtime_metadata())
                        != McpElicitationRuntimeMetadata::from(server.runtime_metadata())
                    || !reuse_context
                        .runtime_context
                        .has_same_launch_environment_for(
                            &previous_context.runtime_context,
                            server.config(),
                        )
                    || !reuse_context.auth_is_compatible_for(previous_context, &server)
                {
                    return None;
                }
                let client = previous.clients.get(&server_name)?;
                if !client.can_reuse() {
                    return None;
                }
                Some((
                    client.clone(),
                    previous.elicitation_requests.get(&server_name)?.clone(),
                ))
            });
            let (async_managed_client, server_elicitation_requests, round_cancel_token) =
                match reused {
                    Some((client, requests)) => (client, requests, None),
                    None => {
                        let cancel_token = startup_cancellation_token.child_token();
                        let requests = ElicitationRequestManager::new_with_state(
                            approval_policy.value(),
                            initial_permission_profile.clone(),
                            elicitation_reviewer.clone(),
                            McpElicitationRuntimeMetadata::from(server.runtime_metadata()),
                            elicitation_state.clone(),
                        );
                        let runtime_auth_provider = chatgpt_auth_provider_for_server(
                            &server,
                            chatgpt_auth_provider.clone(),
                        );
                        let client = AsyncManagedClient::new(
                            server_name.clone(),
                            server,
                            store_mode,
                            keyring_backend_kind,
                            cancel_token.clone(),
                            tx_event.clone(),
                            requests.clone(),
                            runtime_context.clone(),
                            runtime_auth_provider,
                            client_elicitation_capability.clone(),
                            supports_openai_form_elicitation,
                        );
                        (client, requests, Some(cancel_token))
                    }
                };
            if let Ok(mut current) = server_elicitation_requests.approval_policy.lock() {
                *current = approval_policy.value();
            }
            if let Ok(mut current) = server_elicitation_requests.permission_profile.lock() {
                *current = initial_permission_profile.clone();
            }
            elicitation_requests.insert(server_name.clone(), server_elicitation_requests);
            clients.insert(server_name.clone(), async_managed_client.clone());
            let startup = async_managed_client.client.clone();
            let tx_event = tx_event.clone();
            let submit_id = startup_submit_id.clone();
            let auth_entry = auth_entries.get(&server_name).cloned();
            join_set.spawn(async move {
                let mut outcome = startup.await;
                if round_cancel_token.is_some_and(|token| token.is_cancelled()) {
                    outcome = Err(StartupOutcomeError::Cancelled);
                }
                let status = match &outcome {
                    Ok(_) => McpStartupStatus::Ready,
                    Err(StartupOutcomeError::Cancelled) => McpStartupStatus::Cancelled,
                    Err(error) => {
                        let reason = mcp_startup_failure_reason(auth_entry.as_ref(), error);
                        let error_str = mcp_init_error_display(
                            server_name.as_str(),
                            auth_entry.as_ref(),
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

                (server_name, outcome)
            });
        }
        let manager = Self {
            clients,
            server_registrations,
            server_metadata,
            required_servers,
            tool_plugin_provenance,
            prefix_mcp_tool_names,
            elicitation_requests,
            elicitation_state,
            elicitation_reviewer,
            reuse_context: Some(reuse_context),
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

    pub fn new_uninitialized(prefix_mcp_tool_names: bool) -> Self {
        Self {
            clients: HashMap::new(),
            server_registrations: HashMap::new(),
            server_metadata: HashMap::new(),
            required_servers: Vec::new(),
            tool_plugin_provenance: Arc::new(ToolPluginProvenance::default()),
            prefix_mcp_tool_names,
            elicitation_requests: HashMap::new(),
            elicitation_state: McpElicitationState::default(),
            elicitation_reviewer: None,
            reuse_context: None,
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
        let clients = self.clients.values().cloned().collect::<Vec<_>>();
        for client in &clients {
            client.cancel_startup();
        }
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

    /// Stops clients that are not shared with the successor manager.
    pub async fn shutdown_superseded_by(&self, successor: &Self) {
        let clients = self
            .clients
            .iter()
            .filter(|&(name, client)| {
                !successor
                    .clients
                    .get(name)
                    .is_some_and(|next| client.same_instance(next))
            })
            .map(|(_, client)| client.clone())
            .collect::<Vec<_>>();
        for client in &clients {
            client.cancel_startup();
        }
        let shutdown_task = tokio::spawn(async move {
            for client in clients {
                client.shutdown().await;
            }
        });
        if let Err(error) = shutdown_task.await {
            warn!("superseded MCP client shutdown task failed: {error}");
        }
    }

    pub fn cancel_startup(&self) {
        for client in self.clients.values() {
            client.cancel_startup();
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

    pub fn server_sandbox_state_source(&self, server_name: &str) -> McpSandboxStateSource {
        self.server_metadata
            .get(server_name)
            .map(|metadata| metadata.sandbox_state_source)
            .unwrap_or_default()
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
    ) -> codex_config::McpToolApproval {
        self.server_metadata
            .get(server_name)
            .map(|metadata| metadata.tool_approval_mode(tool_name))
            .unwrap_or_default()
    }

    pub fn tool_runtime_metadata(
        &self,
        server_name: &str,
        tool_name: &str,
    ) -> Option<&crate::server::McpToolRuntimeMetadata> {
        self.server_metadata
            .get(server_name)?
            .tool_runtime_metadata
            .get(tool_name)
    }

    pub fn server_trusts_approval_context(&self, server_name: &str) -> bool {
        self.server_metadata
            .get(server_name)
            .is_some_and(|metadata| metadata.trusts_approval_context)
    }

    pub fn approvals_reviewer(
        &self,
        server_name: &str,
    ) -> Option<codex_config::types::ApprovalsReviewer> {
        self.server_metadata
            .get(server_name)
            .and_then(|metadata| metadata.approvals_reviewer)
    }

    pub fn set_approval_policy(&self, approval_policy: &Constrained<AskForApproval>) {
        for requests in self.elicitation_requests.values() {
            if let Ok(mut policy) = requests.approval_policy.lock() {
                *policy = approval_policy.value();
            }
        }
    }

    pub fn set_permission_profile(&self, permission_profile: PermissionProfile) {
        for requests in self.elicitation_requests.values() {
            if let Ok(mut profile) = requests.permission_profile.lock() {
                *profile = permission_profile.clone();
            }
        }
    }

    pub fn elicitations_auto_deny(&self) -> bool {
        self.elicitation_state.auto_deny()
    }

    pub fn set_elicitations_auto_deny(&self, auto_deny: bool) {
        self.elicitation_state.set_auto_deny(auto_deny);
    }

    pub fn elicitation_reviewer(&self) -> Option<ElicitationReviewerHandle> {
        self.elicitation_reviewer.clone()
    }

    pub async fn resolve_elicitation(
        &self,
        server_name: String,
        id: RequestId,
        response: ElicitationResponse,
    ) -> Result<()> {
        self.elicitation_state.resolve(server_name, id, response)
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
        for (server_name, managed_client) in &self.clients {
            let startup_complete = managed_client.startup_is_complete();
            trace!(
                server_name = %server_name,
                startup_complete,
                "waiting for MCP server tools while building tool list"
            );
            let Some(server_tools) = managed_client
                .listed_tools()
                .instrument(trace_span!(
                    "list_tools_for_server",
                    server_name = %server_name,
                    startup_complete
                ))
                .await
            else {
                continue;
            };
            trace!(
                server_name = %server_name,
                tool_count = server_tools.len(),
                "listed MCP server tools while building tool list"
            );
            tools.extend(
                prepare_regular_mcp_tools_for_model(server_tools, &self.tool_plugin_provenance)
                    .into_iter()
                    .map(|tool| self.with_server_metadata(tool)),
            );
        }
        normalize_tools_for_model_with_prefix(tools, self.prefix_mcp_tool_names)
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

    pub async fn server_supports_trusted_tool_input(&self, server: &str) -> Result<bool> {
        if !self
            .server_metadata
            .get(server)
            .is_some_and(|metadata| metadata.trusts_tool_input)
        {
            return Ok(false);
        }
        Ok(self
            .client_by_name(server)
            .await?
            .server_supports_tool_input_meta_capability)
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

    /// Returns presentation metadata without waiting for clients still initializing.
    pub(crate) async fn list_available_server_infos(&self) -> HashMap<String, McpServerInfo> {
        let mut server_infos = HashMap::new();
        for (server_name, client) in &self.clients {
            if !client.startup_is_complete() {
                continue;
            }
            if let Ok(managed_client) = client.client().await {
                server_infos.insert(server_name.clone(), managed_client.server_info);
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
        tool.search_aliases = metadata
            .tool_runtime_metadata
            .get(tool.tool.name.as_ref())
            .map(McpToolRuntimeMetadata::search_aliases)
            .unwrap_or_default()
            .to_vec();
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
}

impl Drop for McpConnectionManager {
    fn drop(&mut self) {
        self.clients.clear();
    }
}

fn same_elicitation_reviewer(
    left: Option<&ElicitationReviewerHandle>,
    right: Option<&ElicitationReviewerHandle>,
) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => Arc::ptr_eq(left, right),
        (None, None) => true,
        _ => false,
    }
}

/// Makes ChatGPT authentication available to servers that explicitly opt in.
/// The HTTP transport applies it only when no configured authorization resolves.
fn chatgpt_auth_provider_for_server(
    server: &EffectiveMcpServer,
    chatgpt_auth_provider: Option<SharedAuthProvider>,
) -> Option<SharedAuthProvider> {
    if !matches!(&server.config().auth, McpServerAuth::ChatGpt) {
        return None;
    }
    chatgpt_auth_provider
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
    entry: Option<&McpAuthStatusEntry>,
    error: &StartupOutcomeError,
) -> Option<McpStartupFailureReason> {
    if !error.is_authentication_required() {
        return None;
    }

    match entry.map(|entry| entry.auth_state) {
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
    entry: Option<&McpAuthStatusEntry>,
    err: &StartupOutcomeError,
) -> String {
    if let Some(McpServerTransportConfig::StreamableHttp {
        url,
        bearer_token_env_var,
        http_headers,
        ..
    }) = entry.and_then(|entry| entry.config.as_ref().map(|config| &config.transport))
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
        let startup_timeout_secs = match entry {
            Some(entry) => match entry
                .config
                .as_ref()
                .and_then(|config| config.startup_timeout_sec)
            {
                Some(timeout) => timeout,
                None => DEFAULT_STARTUP_TIMEOUT,
            },
            None => DEFAULT_STARTUP_TIMEOUT,
        }
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
        }
        _ => false,
    }
}

#[cfg(test)]
#[path = "connection_manager_tests.rs"]
mod tests;
