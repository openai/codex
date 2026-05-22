//! Client and process ownership for MCP servers.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

use crate::McpAuthStatusEntry;
use crate::codex_apps::CodexAppsToolsCacheContext;
use crate::codex_apps::CodexAppsToolsCacheKey;
use crate::codex_apps::write_cached_codex_apps_tools_if_needed;
use crate::elicitation::ElicitationRequestManager;
use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;
use crate::mcp::ToolPluginProvenance;
use crate::rmcp_client::AsyncManagedClient;
use crate::rmcp_client::DEFAULT_STARTUP_TIMEOUT;
use crate::rmcp_client::MCP_TOOLS_FETCH_UNCACHED_DURATION_METRIC;
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
use crate::tools::normalize_tools_for_model;
use crate::tools::tool_with_model_visible_input_schema;
use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use async_channel::Sender;
use codex_config::McpServerTransportConfig;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_login::CodexAuth;
use codex_protocol::mcp::CallToolResult;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::McpStartupCompleteEvent;
use codex_protocol::protocol::McpStartupFailure;
use codex_protocol::protocol::McpStartupStatus;
use codex_protocol::protocol::McpStartupUpdateEvent;
use rmcp::model::ElicitationCapability;
use rmcp::model::ListResourceTemplatesResult;
use rmcp::model::ListResourcesResult;
use rmcp::model::PaginatedRequestParams;
use rmcp::model::ReadResourceRequestParams;
use rmcp::model::ReadResourceResult;
use rmcp::model::Resource;
use rmcp::model::ResourceTemplate;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::instrument;
use tracing::warn;

/// Owns running MCP clients and their backing transports.
pub(crate) struct McpClientPool {
    clients: Mutex<HashMap<String, AsyncManagedClient>>,
    server_metadata: HashMap<String, McpServerMetadata>,
    tool_plugin_provenance: Arc<ToolPluginProvenance>,
    host_owned_codex_apps_enabled: bool,
    startup_cancellation_token: CancellationToken,
}

impl McpClientPool {
    pub(crate) fn new_uninitialized() -> Self {
        Self {
            clients: Mutex::new(HashMap::new()),
            server_metadata: HashMap::new(),
            tool_plugin_provenance: Arc::new(ToolPluginProvenance::default()),
            host_owned_codex_apps_enabled: false,
            startup_cancellation_token: CancellationToken::new(),
        }
    }

    #[allow(clippy::new_ret_no_self, clippy::too_many_arguments)]
    pub(crate) async fn new(
        mcp_servers: &HashMap<String, EffectiveMcpServer>,
        store_mode: OAuthCredentialsStoreMode,
        auth_entries: HashMap<String, McpAuthStatusEntry>,
        submit_id: String,
        tx_event: Sender<Event>,
        runtime_context: McpRuntimeContext,
        codex_home: PathBuf,
        codex_apps_tools_cache_key: CodexAppsToolsCacheKey,
        host_owned_codex_apps_enabled: bool,
        client_elicitation_capability: ElicitationCapability,
        tool_plugin_provenance: ToolPluginProvenance,
        auth: Option<&CodexAuth>,
        elicitation_requests: ElicitationRequestManager,
    ) -> (Self, CancellationToken) {
        let cancel_token = CancellationToken::new();
        let mut clients = HashMap::new();
        let mut server_metadata = HashMap::new();
        let mut join_set = JoinSet::new();
        let tool_plugin_provenance = Arc::new(tool_plugin_provenance);
        let startup_submit_id = submit_id.clone();
        let codex_apps_auth_provider = auth
            .filter(|auth| auth.uses_codex_backend())
            .map(codex_model_provider::auth_provider_from_auth);
        let mcp_servers = mcp_servers.clone();
        for (server_name, server) in mcp_servers
            .into_iter()
            .filter(|(_, server)| server.enabled())
        {
            server_metadata.insert(server_name.clone(), McpServerMetadata::from(&server));
            let cancel_token = cancel_token.child_token();
            let _ = emit_update(
                startup_submit_id.as_str(),
                &tx_event,
                McpStartupUpdateEvent {
                    server: server_name.clone(),
                    status: McpStartupStatus::Starting,
                },
            )
            .await;
            let codex_apps_tools_cache_context = if server_name == CODEX_APPS_MCP_SERVER_NAME {
                Some(CodexAppsToolsCacheContext {
                    codex_home: codex_home.clone(),
                    user_key: codex_apps_tools_cache_key.clone(),
                })
            } else {
                None
            };
            let uses_env_bearer_token =
                server
                    .configured_config()
                    .is_some_and(|config| match &config.transport {
                        McpServerTransportConfig::StreamableHttp {
                            bearer_token_env_var,
                            ..
                        } => bearer_token_env_var.is_some(),
                        McpServerTransportConfig::Stdio { .. } => false,
                    });
            let runtime_auth_provider =
                if server_name == CODEX_APPS_MCP_SERVER_NAME && !uses_env_bearer_token {
                    codex_apps_auth_provider.clone()
                } else {
                    None
                };
            let async_managed_client = AsyncManagedClient::new(
                server_name.clone(),
                server,
                store_mode,
                cancel_token.clone(),
                tx_event.clone(),
                elicitation_requests.clone(),
                codex_apps_tools_cache_context,
                Arc::clone(&tool_plugin_provenance),
                runtime_context.clone(),
                runtime_auth_provider,
                client_elicitation_capability.clone(),
            );
            clients.insert(server_name.clone(), async_managed_client.clone());
            let tx_event = tx_event.clone();
            let submit_id = startup_submit_id.clone();
            let auth_entry = auth_entries.get(&server_name).cloned();
            join_set.spawn(async move {
                let mut outcome = async_managed_client.client().await;
                if cancel_token.is_cancelled() {
                    outcome = Err(StartupOutcomeError::Cancelled);
                }
                let status = match &outcome {
                    Ok(_) => McpStartupStatus::Ready,
                    Err(StartupOutcomeError::Cancelled) => McpStartupStatus::Cancelled,
                    Err(error) => McpStartupStatus::Failed {
                        error: mcp_init_error_display(
                            server_name.as_str(),
                            auth_entry.as_ref(),
                            error,
                        ),
                    },
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
        let pool = Self {
            clients: Mutex::new(clients),
            server_metadata,
            tool_plugin_provenance,
            host_owned_codex_apps_enabled,
            startup_cancellation_token: cancel_token.clone(),
        };
        tokio::spawn(async move {
            let outcomes = join_set.join_all().await;
            let mut summary = McpStartupCompleteEvent::default();
            for (server_name, outcome) in outcomes {
                match outcome {
                    Ok(_) => summary.ready.push(server_name),
                    Err(StartupOutcomeError::Cancelled) => summary.cancelled.push(server_name),
                    Err(StartupOutcomeError::Failed { error }) => {
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
        (pool, cancel_token)
    }

    pub(crate) fn has_servers(&self) -> bool {
        self.clients.lock().is_ok_and(|clients| !clients.is_empty())
    }

    #[cfg(test)]
    pub(crate) fn has_server(&self, server_name: &str) -> bool {
        self.clients
            .lock()
            .is_ok_and(|clients| clients.contains_key(server_name))
    }

    /// Drain all MCP clients and return a future that stops transports and
    /// terminates stdio server processes.
    pub(crate) fn begin_shutdown(&self) -> impl std::future::Future<Output = ()> + Send + 'static {
        self.startup_cancellation_token.cancel();
        let clients = self
            .clients
            .lock()
            .map(|mut clients| std::mem::take(&mut *clients))
            .unwrap_or_default();
        async move {
            for client in clients.into_values() {
                client.shutdown().await;
            }
        }
    }

    pub(crate) async fn shutdown(&self) {
        self.begin_shutdown().await;
    }

    pub(crate) fn server_origin(&self, server_name: &str) -> Option<&str> {
        self.server_metadata
            .get(server_name)
            .and_then(|metadata| metadata.origin.as_ref())
            .map(super::server::McpServerOrigin::as_str)
    }

    pub(crate) fn server_pollutes_memory(&self, server_name: &str) -> bool {
        self.server_metadata
            .get(server_name)
            .is_none_or(|metadata| metadata.pollutes_memory)
    }

    pub(crate) fn plugin_id_for_mcp_server_name(&self, server_name: &str) -> Option<&str> {
        self.tool_plugin_provenance
            .plugin_id_for_mcp_server_name(server_name)
    }

    pub(crate) fn is_host_owned_codex_apps_server(&self, server_name: &str) -> bool {
        self.host_owned_codex_apps_enabled && server_name == CODEX_APPS_MCP_SERVER_NAME
    }

    pub(crate) async fn wait_for_server_ready(&self, server_name: &str, timeout: Duration) -> bool {
        let async_managed_client = self
            .clients
            .lock()
            .ok()
            .and_then(|clients| clients.get(server_name).cloned());
        let Some(async_managed_client) = async_managed_client else {
            return false;
        };

        match tokio::time::timeout(timeout, async_managed_client.client()).await {
            Ok(Ok(_)) => true,
            Ok(Err(_)) | Err(_) => false,
        }
    }

    pub(crate) async fn required_startup_failures(
        &self,
        required_servers: &[String],
    ) -> Vec<McpStartupFailure> {
        let mut failures = Vec::new();
        for server_name in required_servers {
            let async_managed_client = self
                .clients
                .lock()
                .ok()
                .and_then(|clients| clients.get(server_name).cloned());
            let Some(async_managed_client) = async_managed_client else {
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

    #[instrument(level = "trace", skip_all)]
    pub(crate) async fn list_all_tools(&self) -> Vec<ToolInfo> {
        let mut tools = Vec::new();
        let clients = self
            .clients
            .lock()
            .map(|clients| clients.values().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        for managed_client in clients {
            let Some(server_tools) = managed_client.listed_tools().await else {
                continue;
            };
            tools.extend(
                server_tools
                    .into_iter()
                    .map(|tool| self.with_server_metadata(tool)),
            );
        }
        normalize_tools_for_model(tools)
    }

    pub(crate) async fn hard_refresh_codex_apps_tools_cache(&self) -> Result<Vec<ToolInfo>> {
        let async_managed_client = self
            .clients
            .lock()
            .ok()
            .and_then(|clients| clients.get(CODEX_APPS_MCP_SERVER_NAME).cloned())
            .ok_or_else(|| anyhow!("unknown MCP server '{CODEX_APPS_MCP_SERVER_NAME}'"))?;
        let managed_client = async_managed_client
            .client()
            .await
            .context("failed to get client")?;

        let list_start = Instant::now();
        let fetch_start = Instant::now();
        let tools = list_tools_for_client_uncached(
            CODEX_APPS_MCP_SERVER_NAME,
            &managed_client.client,
            managed_client.tool_timeout,
            managed_client.server_instructions.as_deref(),
        )
        .await
        .with_context(|| {
            format!("failed to refresh tools for MCP server '{CODEX_APPS_MCP_SERVER_NAME}'")
        })?;
        emit_duration(
            MCP_TOOLS_FETCH_UNCACHED_DURATION_METRIC,
            fetch_start.elapsed(),
            &[],
        );

        write_cached_codex_apps_tools_if_needed(
            CODEX_APPS_MCP_SERVER_NAME,
            managed_client.codex_apps_tools_cache_context.as_ref(),
            &tools,
        );
        emit_duration(
            MCP_TOOLS_LIST_DURATION_METRIC,
            list_start.elapsed(),
            &[("cache", "miss")],
        );
        let tools = filter_tools(tools, &managed_client.tool_filter)
            .into_iter()
            .map(|mut tool| {
                tool.tool = tool_with_model_visible_input_schema(&tool.tool);
                self.with_server_metadata(tool)
            });
        Ok(normalize_tools_for_model(tools))
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

    pub(crate) async fn list_all_resources(&self) -> HashMap<String, Vec<Resource>> {
        let mut join_set = JoinSet::new();

        let clients = self
            .clients
            .lock()
            .map(|clients| {
                clients
                    .iter()
                    .map(|(server_name, client)| (server_name.clone(), client.clone()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        for (server_name, async_managed_client) in clients {
            let Ok(managed_client) = async_managed_client.client().await else {
                continue;
            };
            let timeout = managed_client.tool_timeout;
            let client = managed_client.client.clone();

            join_set.spawn(async move {
                let mut collected: Vec<Resource> = Vec::new();
                let mut cursor: Option<String> = None;

                loop {
                    let params = cursor.as_ref().map(|next| PaginatedRequestParams {
                        meta: None,
                        cursor: Some(next.clone()),
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

    pub(crate) async fn list_all_resource_templates(
        &self,
    ) -> HashMap<String, Vec<ResourceTemplate>> {
        let mut join_set = JoinSet::new();

        let clients = self
            .clients
            .lock()
            .map(|clients| {
                clients
                    .iter()
                    .map(|(server_name, client)| (server_name.clone(), client.clone()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        for (server_name, async_managed_client) in clients {
            let Ok(managed_client) = async_managed_client.client().await else {
                continue;
            };
            let client = managed_client.client.clone();
            let timeout = managed_client.tool_timeout;

            join_set.spawn(async move {
                let mut collected: Vec<ResourceTemplate> = Vec::new();
                let mut cursor: Option<String> = None;

                loop {
                    let params = cursor.as_ref().map(|next| PaginatedRequestParams {
                        meta: None,
                        cursor: Some(next.clone()),
                    });
                    let response = match client.list_resource_templates(params, timeout).await {
                        Ok(result) => result,
                        Err(err) => return (server_name, Err(err)),
                    };

                    collected.extend(response.resource_templates);

                    match response.next_cursor {
                        Some(next) => {
                            if cursor.as_ref() == Some(&next) {
                                return (
                                    server_name,
                                    Err(anyhow!(
                                        "resources/templates/list returned duplicate cursor"
                                    )),
                                );
                            }
                            cursor = Some(next);
                        }
                        None => return (server_name, Ok(collected)),
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

    pub(crate) async fn call_tool(
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

    pub(crate) async fn server_supports_sandbox_state_meta_capability(
        &self,
        server: &str,
    ) -> Result<bool> {
        Ok(self
            .client_by_name(server)
            .await?
            .server_supports_sandbox_state_meta_capability)
    }

    pub(crate) async fn list_resources(
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

    pub(crate) async fn list_resource_templates(
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

    pub(crate) async fn read_resource(
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

    async fn client_by_name(&self, name: &str) -> Result<ManagedClient> {
        let async_managed_client = self
            .clients
            .lock()
            .ok()
            .and_then(|clients| clients.get(name).cloned())
            .ok_or_else(|| anyhow!("unknown MCP server '{name}'"))?;
        async_managed_client
            .client()
            .await
            .context("failed to get client")
    }
}

impl Drop for McpClientPool {
    fn drop(&mut self) {
        self.startup_cancellation_token.cancel();
        if let Ok(mut clients) = self.clients.lock() {
            clients.clear();
        }
    }
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

pub(crate) fn mcp_init_error_display(
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
        StartupOutcomeError::Failed { error } => error,
    }
}

fn is_mcp_client_auth_required_error(error: &StartupOutcomeError) -> bool {
    match error {
        StartupOutcomeError::Failed { error } => error.contains("Auth required"),
        StartupOutcomeError::Cancelled => false,
    }
}

fn is_mcp_client_startup_timeout_error(error: &StartupOutcomeError) -> bool {
    match error {
        StartupOutcomeError::Failed { error } => {
            error.contains("request timed out")
                || error.contains("timed out handshaking with MCP server")
        }
        StartupOutcomeError::Cancelled => false,
    }
}

#[cfg(test)]
#[path = "client_pool_tests.rs"]
mod tests;
