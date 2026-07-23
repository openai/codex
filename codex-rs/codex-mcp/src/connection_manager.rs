//! Aggregates MCP server connections for Codex.
//!
//! [`McpConnectionSet`] is the private connection set behind
//! [`crate::McpRuntime`] and [`crate::McpBinding`]. It coordinates startup status
//! events, keeps server metadata, and aggregates tools and resources across
//! running RMCP clients.

#[path = "connection_manager/required.rs"]
mod required;
#[path = "connection_manager/resources.rs"]
mod resources;
#[path = "connection_manager/startup.rs"]
mod startup;
#[path = "connection_manager/tool_catalog.rs"]
mod tool_catalog;

use startup::chatgpt_auth_provider_for_server;
use startup::emit_update;
use startup::mcp_init_error_display;
use startup::mcp_startup_failure_reason;
use startup::should_share_codex_apps_tools_cache;
pub use tool_catalog::tool_is_model_visible;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use crate::elicitation::ElicitationRequestManager;
use crate::elicitation::ElicitationRequestRouter;
use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;
use crate::mcp::ToolPluginProvenance;
use crate::rmcp_client::AsyncManagedClient;
use crate::rmcp_client::DEFAULT_TOOL_TIMEOUT;
use crate::rmcp_client::ManagedClient;
use crate::rmcp_client::StartupOutcomeError;
use crate::rmcp_client::prepare_codex_apps_tools_for_model;
use crate::rmcp_client::prepare_regular_mcp_tools_for_model;
use crate::runtime::McpPublicationGate;
use crate::runtime::McpRuntimeInput;
use crate::server::McpServerConnectionIdentity;
use crate::server::McpServerMetadata;
use crate::tools::ToolFilter;
use crate::tools::ToolInfo;
use crate::tools::filter_tools;
use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use codex_config::McpServerTransportConfig;
use codex_protocol::mcp::CallToolResult;
use codex_protocol::mcp::McpServerInfo;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::McpStartupCompleteEvent;
use codex_protocol::protocol::McpStartupFailure;
use codex_protocol::protocol::McpStartupStatus;
use codex_protocol::protocol::McpStartupUpdateEvent;
use codex_rmcp_client::determine_streamable_http_auth_status_from_credentials;
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use tokio::task::JoinSet;
use tracing::warn;

pub(crate) struct McpServerConnection {
    identity: Option<McpServerConnectionIdentity>,
    client: AsyncManagedClient,
}

impl McpServerConnection {
    async fn reusable_client(
        &self,
        desired: &McpServerConnectionIdentity,
    ) -> Option<ManagedClient> {
        let current = self.identity.as_ref()?;
        if !current.has_same_connection_config(desired) {
            return None;
        }
        if !self.client.startup_complete.load(Ordering::Acquire) {
            return None;
        }
        let client = self.client.client().await.ok()?;
        if client.client.is_closed().await {
            return None;
        }
        let Ok(desired_credentials) = desired.oauth_credentials() else {
            return Some(client);
        };
        let reusable = match client.client.managed_oauth_credentials().await {
            Some(live_credentials) => &live_credentials == desired_credentials,
            None => current
                .oauth_credentials()
                .is_ok_and(|startup_credentials| startup_credentials == desired_credentials),
        };
        if reusable { Some(client) } else { None }
    }

    pub(crate) async fn client(&self) -> Result<ManagedClient, StartupOutcomeError> {
        self.client.client().await
    }

    async fn shutdown(&self) {
        self.client.shutdown().await;
    }

    fn cancel_startup(&self) {
        if !self.client.startup_complete.load(Ordering::Acquire) {
            self.client.cancel_token.cancel();
        }
    }
}

impl Drop for McpServerConnection {
    fn drop(&mut self) {
        self.client.cancel_token.cancel();
    }
}

#[derive(Clone)]
struct McpServerView {
    connection: Arc<McpServerConnection>,
    metadata: McpServerMetadata,
    tool_filter: ToolFilter,
    tool_timeout: Option<Duration>,
}

impl McpServerView {
    async fn listed_tools(
        &self,
        tool_plugin_provenance: &ToolPluginProvenance,
    ) -> Option<Vec<ToolInfo>> {
        let tools = self.connection.client.listed_tools().await?;
        let tools = filter_tools(tools, &self.tool_filter);
        Some(if self.connection.client.is_codex_apps_mcp_server {
            prepare_codex_apps_tools_for_model(tools, tool_plugin_provenance)
        } else {
            prepare_regular_mcp_tools_for_model(tools, tool_plugin_provenance)
        })
    }
}

/// A published view over a set of running MCP server connections.
pub(crate) struct McpConnectionSet {
    servers: HashMap<String, McpServerView>,
    required_servers: Vec<String>,
    tool_catalog_revision: Arc<RwLock<u64>>,
    codex_apps_tools_override: RwLock<Option<Vec<ToolInfo>>>,
    codex_apps_refresh_lock: Mutex<()>,
    tool_plugin_provenance: Arc<ToolPluginProvenance>,
    prefix_mcp_tool_names: bool,
    elicitation_requests: ElicitationRequestManager,
}

impl McpConnectionSet {
    /// Creates an MCP connection manager. Threadless callers can pass no `tx_event`; startup
    /// notifications are then skipped and interactive elicitations are declined.
    pub async fn new(
        previous: Option<&Self>,
        publication_gate: McpPublicationGate,
        input: McpRuntimeInput,
        elicitation_router: ElicitationRequestRouter,
    ) -> Self {
        let McpRuntimeInput {
            config,
            plugins_available: _,
            ready_selected_capability_roots: _,
            mcp_servers,
            submit_id,
            tx_event,
            startup_cancellation_token,
            runtime_context,
            codex_apps_tools_cache,
            tool_catalog_cache,
            codex_apps_tools_cache_key,
            supports_openai_form_elicitation,
            auth,
            codex_apps_auth_manager,
            elicitation_reviewer,
            elicitation_lifecycle,
        } = input;
        let store_mode = config.mcp_oauth_credentials_store_mode;
        let keyring_backend_kind = config.auth_keyring_backend_kind;
        let approval_policy = &config.approval_policy;
        let initial_permission_profile = config.permission_profile.clone();
        let codex_home = config.codex_home.clone();
        let prefix_mcp_tool_names = config.prefix_mcp_tool_names;
        let client_elicitation_capability = config.client_elicitation_capability.clone();
        let tool_plugin_provenance = crate::mcp::tool_plugin_provenance(&config);
        let auth = auth.as_ref();
        let mut servers = HashMap::new();
        let mut required_servers = mcp_servers
            .iter()
            .filter(|(_, server)| server.enabled() && server.required())
            .map(|(server_name, _)| server_name.clone())
            .collect::<Vec<_>>();
        required_servers.sort();
        let mut reused_ready = Vec::new();
        let mut join_set = JoinSet::new();
        let reusable_previous = previous.filter(|previous| {
            !previous.servers.is_empty()
                && previous.elicitation_requests.update(
                    approval_policy.value(),
                    initial_permission_profile.clone(),
                    elicitation_reviewer.clone(),
                    elicitation_lifecycle.clone(),
                )
        });
        let elicitation_requests = if let Some(previous) = reusable_previous {
            previous.elicitation_requests.clone()
        } else {
            ElicitationRequestManager::new(
                approval_policy.value(),
                initial_permission_profile,
                elicitation_reviewer,
                elicitation_lifecycle,
                elicitation_router,
            )
        };
        let tool_plugin_provenance = Arc::new(tool_plugin_provenance);
        let startup_submit_id = submit_id;
        let static_chatgpt_auth_provider = auth
            .filter(|auth| auth.uses_codex_backend())
            .map(codex_model_provider::auth_provider_from_auth);
        let codex_apps_auth_provider = codex_apps_auth_manager.and_then(|auth_manager| {
            auth.filter(|auth| auth.uses_codex_backend()).map(|auth| {
                codex_model_provider::auth_provider_from_auth_manager(auth_manager, auth)
            })
        });
        for (server_name, server) in mcp_servers
            .into_iter()
            .filter(|(_, server)| server.enabled())
        {
            let metadata = McpServerMetadata::from(&server);
            let configured_config = server.config().clone();
            let configured_tool_filter = ToolFilter::from_config(&configured_config);
            let configured_tool_timeout = Some(
                configured_config
                    .tool_timeout_sec
                    .unwrap_or(DEFAULT_TOOL_TIMEOUT),
            );
            let resolved_environment =
                runtime_context.resolve_server_environment(&server_name, &configured_config);
            // For built-in Codex Apps, `CODEX_CONNECTORS_TOKEN` is a debug
            // override: it supplies runtime auth but bypasses the shared tools
            // cache.
            let uses_env_bearer_token = match &configured_config.transport {
                McpServerTransportConfig::StreamableHttp {
                    bearer_token_env_var,
                    ..
                } => bearer_token_env_var.is_some(),
                McpServerTransportConfig::Stdio { .. } => false,
            };
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
            let connection_identity = McpServerConnectionIdentity::new(
                &server_name,
                &server,
                store_mode,
                keyring_backend_kind,
                &resolved_environment,
                &runtime_context,
                runtime_auth_provider.as_ref(),
                auth,
                shares_codex_apps_tools_cache
                    .then(|| (codex_home.clone(), codex_apps_tools_cache_key.clone())),
                client_elicitation_capability.clone(),
                supports_openai_form_elicitation,
            );
            if let Some(previous_view) =
                reusable_previous.and_then(|previous| previous.servers.get(&server_name))
            {
                let connection = Arc::clone(&previous_view.connection);
                if connection
                    .reusable_client(&connection_identity)
                    .await
                    .is_some()
                {
                    servers.insert(
                        server_name.clone(),
                        McpServerView {
                            connection,
                            metadata,
                            tool_filter: configured_tool_filter,
                            tool_timeout: configured_tool_timeout,
                        },
                    );
                    reused_ready.push(server_name);
                    continue;
                }
            }
            let cancel_token = startup_cancellation_token.child_token();
            let tool_catalog_cache_context = if server_name == CODEX_APPS_MCP_SERVER_NAME {
                None
            } else if let Ok(environment) = resolved_environment.as_ref() {
                tool_catalog_cache.context(
                    &server_name,
                    &configured_config,
                    &runtime_context,
                    environment.as_ref(),
                    &client_elicitation_capability,
                    supports_openai_form_elicitation,
                )
            } else {
                None
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
                tool_catalog_cache_context,
                runtime_context.clone(),
                resolved_environment,
                runtime_auth_provider,
                client_elicitation_capability.clone(),
                supports_openai_form_elicitation,
            );
            servers.insert(
                server_name.clone(),
                McpServerView {
                    connection: Arc::new(McpServerConnection {
                        identity: Some(connection_identity),
                        client: async_managed_client.clone(),
                    }),
                    metadata,
                    tool_filter: configured_tool_filter,
                    tool_timeout: configured_tool_timeout,
                },
            );
            let tx_event = tx_event.clone();
            let submit_id = startup_submit_id.clone();
            let publication_gate = publication_gate.clone();
            join_set.spawn(async move {
                if !publication_gate.wait().await {
                    return (server_name, Err(StartupOutcomeError::Cancelled));
                }
                if let Some(tx_event) = tx_event.as_ref() {
                    let _ = emit_update(
                        submit_id.as_str(),
                        tx_event,
                        McpStartupUpdateEvent {
                            server: server_name.clone(),
                            status: McpStartupStatus::Starting,
                        },
                    )
                    .await;
                }
                let mut outcome = async_managed_client.client().await;
                if cancel_token.is_cancelled() {
                    outcome = Err(StartupOutcomeError::Cancelled);
                }
                if let Some(tx_event) = tx_event.as_ref() {
                    let auth_state = match &outcome {
                        Err(error) if error.is_authentication_required() && !has_runtime_auth => {
                            match &configured_config.transport {
                                McpServerTransportConfig::StreamableHttp {
                                    url,
                                    bearer_token_env_var,
                                    http_headers,
                                    env_http_headers,
                                } => {
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
                                }
                                McpServerTransportConfig::Stdio { .. } => None,
                            }
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
                                Some(&configured_config),
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
                        tx_event,
                        McpStartupUpdateEvent {
                            server: server_name.clone(),
                            status,
                        },
                    )
                    .await;
                }
                if cancel_token.is_cancelled() {
                    outcome = Err(StartupOutcomeError::Cancelled);
                }

                if matches!(&outcome, Err(StartupOutcomeError::Failed { .. })) {
                    async_managed_client.reconnect_failed_startup().await;
                }

                (server_name, outcome)
            });
        }
        let manager = Self {
            servers,
            required_servers,
            tool_catalog_revision: Arc::new(RwLock::new(0)),
            codex_apps_tools_override: RwLock::new(None),
            codex_apps_refresh_lock: Mutex::new(()),
            tool_plugin_provenance,
            prefix_mcp_tool_names,
            elicitation_requests: elicitation_requests.clone(),
        };
        let summary_publication_gate = publication_gate;
        tokio::spawn(async move {
            let outcomes = join_set.join_all().await;
            if let Some(tx_event) = tx_event {
                if !summary_publication_gate.wait().await {
                    return;
                }
                let mut summary = McpStartupCompleteEvent {
                    ready: reused_ready,
                    ..Default::default()
                };
                for server_name in &summary.ready {
                    let _ = emit_update(
                        startup_submit_id.as_str(),
                        &tx_event,
                        McpStartupUpdateEvent {
                            server: server_name.clone(),
                            status: McpStartupStatus::Ready,
                        },
                    )
                    .await;
                }
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
            }
        });
        manager
    }

    pub fn empty(prefix_mcp_tool_names: bool) -> Self {
        Self {
            servers: HashMap::new(),
            required_servers: Vec::new(),
            tool_catalog_revision: Arc::new(RwLock::new(0)),
            codex_apps_tools_override: RwLock::new(None),
            codex_apps_refresh_lock: Mutex::new(()),
            tool_plugin_provenance: Arc::new(ToolPluginProvenance::default()),
            prefix_mcp_tool_names,
            elicitation_requests: ElicitationRequestManager::new(
                AskForApproval::Never,
                PermissionProfile::default(),
                /*reviewer*/ None,
                /*lifecycle*/ None,
                ElicitationRequestRouter::default(),
            ),
        }
    }

    pub fn has_servers(&self) -> bool {
        !self.servers.is_empty()
    }

    pub(crate) fn contains_server(&self, server_name: &str) -> bool {
        self.servers.contains_key(server_name)
    }

    /// Stop all MCP clients owned by this manager and terminate stdio server processes.
    pub async fn shutdown(&self) {
        let connections = self
            .servers
            .values()
            .map(|view| Arc::clone(&view.connection))
            .collect::<Vec<_>>();
        // Keep cleanup alive if an interrupt cancels the refresh that requested it.
        let shutdown_task = tokio::spawn(async move {
            for connection in connections {
                connection.shutdown().await;
            }
        });
        if let Err(error) = shutdown_task.await {
            warn!("MCP client shutdown task failed: {error}");
        }
    }

    pub(crate) fn cancel_startup(&self) {
        for view in self.servers.values() {
            view.connection.cancel_startup();
        }
    }

    pub fn plugin_id_for_mcp_server_name(&self, server_name: &str) -> Option<&str> {
        self.tool_plugin_provenance
            .plugin_id_for_mcp_server_name(server_name)
    }

    pub fn is_selected_plugin_mcp_server(&self, server_name: &str) -> bool {
        self.tool_plugin_provenance
            .is_selected_plugin_mcp_server(server_name)
    }

    pub async fn wait_for_server_ready(&self, server_name: &str, timeout: Duration) -> bool {
        let Some(view) = self.servers.get(server_name) else {
            return false;
        };

        match tokio::time::timeout(timeout, view.connection.client()).await {
            Ok(Ok(_)) => true,
            Ok(Err(_)) | Err(_) => false,
        }
    }

    /// Invoke the tool indicated by the (server, tool) pair.
    pub async fn call_tool(
        &self,
        server: &str,
        tool: &str,
        arguments: Option<serde_json::Value>,
        meta: Option<serde_json::Value>,
    ) -> Result<CallToolResult> {
        let view = self
            .servers
            .get(server)
            .ok_or_else(|| anyhow!("unknown MCP server '{server}'"))?;
        if !view.tool_filter.allows(tool) {
            return Err(anyhow!(
                "tool '{tool}' is disabled for MCP server '{server}'"
            ));
        }
        let client = view
            .connection
            .client()
            .await
            .context("failed to get client")?;

        let result: rmcp::model::CallToolResult = client
            .client
            .call_tool(tool.to_string(), arguments, meta, view.tool_timeout)
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

    /// Returns presentation metadata from the current connection.
    /// Codex Apps metadata may come from its existing cache; regular MCP server information is
    /// connection-specific, so pending regular clients are awaited.
    pub(crate) async fn list_available_server_infos(&self) -> HashMap<String, McpServerInfo> {
        let mut server_infos = HashMap::new();
        for (server_name, view) in &self.servers {
            let client = &view.connection.client;
            if !client.startup_complete.load(Ordering::Acquire)
                && let Some(server_info) = client.cached_server_info.clone()
            {
                server_infos.insert(server_name.clone(), server_info);
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
}

#[cfg(test)]
#[path = "connection_manager_tests.rs"]
mod tests;
