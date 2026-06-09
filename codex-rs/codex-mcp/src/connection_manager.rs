//! Aggregates MCP server connections for Codex.
//!
//! [`McpConnectionManager`] owns the set of running async RMCP clients keyed by
//! MCP server name. It coordinates startup status events, keeps server origin
//! metadata, aggregates tools/resources/templates across servers, routes tool
//! calls to the right client, and exposes the public manager API used by
//! `codex-core`.

use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use crate::McpAuthStatusEntry;
use crate::codex_apps::write_cached_codex_apps_tools_if_needed;
use crate::connection_generation::ConnectionGeneration;
use crate::connection_generation::ConnectionGenerationLease;
use crate::connection_lifecycle::McpConnectionManager;
use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;
use crate::rmcp_client::DEFAULT_STARTUP_TIMEOUT;
use crate::rmcp_client::MCP_TOOLS_FETCH_UNCACHED_DURATION_METRIC;
use crate::rmcp_client::MCP_TOOLS_LIST_DURATION_METRIC;
use crate::rmcp_client::ManagedClient;
use crate::rmcp_client::StartupOutcomeError;
use crate::rmcp_client::list_tools_for_client_uncached;
use crate::runtime::emit_duration;
use crate::server::McpServerOrigin;
use crate::tools::ToolInfo;
use crate::tools::filter_tools;
use crate::tools::normalize_tools_for_model_with_prefix;
use crate::tools::tool_with_model_visible_input_schema;
use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use async_channel::Sender;
use codex_config::McpServerTransportConfig;
use codex_protocol::mcp::CallToolResult;
use codex_protocol::mcp::McpServerInfo;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::McpStartupFailure;
use codex_protocol::protocol::McpStartupUpdateEvent;
use codex_rmcp_client::ElicitationResponse;
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
use tracing::Instrument;
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

impl McpConnectionManager {
    pub fn has_servers(&self) -> bool {
        !self.generation().clients.is_empty()
    }

    /// Returns a future that atomically removes and stops all MCP clients.
    pub fn begin_shutdown(&self) -> impl std::future::Future<Output = ()> + Send + 'static {
        let manager = self.clone();
        async move { manager.shutdown().await }
    }

    pub fn server_origin(&self, server_name: &str) -> Option<String> {
        self.generation()
            .server_metadata
            .get(server_name)
            .and_then(|metadata| metadata.origin.as_ref())
            .map(McpServerOrigin::as_str)
            .map(ToOwned::to_owned)
    }

    pub fn server_pollutes_memory(&self, server_name: &str) -> bool {
        self.generation()
            .server_metadata
            .get(server_name)
            .is_none_or(|metadata| metadata.pollutes_memory)
    }

    pub fn plugin_id_for_mcp_server_name(&self, server_name: &str) -> Option<String> {
        self.generation()
            .tool_plugin_provenance
            .plugin_id_for_mcp_server_name(server_name)
            .map(ToOwned::to_owned)
    }

    pub fn is_host_owned_codex_apps_server(&self, server_name: &str) -> bool {
        self.generation().host_owned_codex_apps_enabled && server_name == CODEX_APPS_MCP_SERVER_NAME
    }

    pub fn set_approval_policy(&self, approval_policy: codex_protocol::protocol::AskForApproval) {
        if let Ok(mut policy) = self.elicitation_requests().approval_policy.lock() {
            *policy = approval_policy;
        }
    }

    pub fn set_permission_profile(&self, permission_profile: PermissionProfile) {
        if let Ok(mut profile) = self.elicitation_requests().permission_profile.lock() {
            *profile = permission_profile;
        }
    }

    pub fn elicitations_auto_deny(&self) -> bool {
        self.elicitation_requests().auto_deny()
    }

    pub fn set_elicitations_auto_deny(&self, auto_deny: bool) {
        self.elicitation_requests().set_auto_deny(auto_deny);
    }

    pub async fn resolve_elicitation(
        &self,
        server_name: String,
        id: RequestId,
        response: ElicitationResponse,
    ) -> Result<()> {
        self.elicitation_requests()
            .resolve(server_name, id, response)
            .await
    }

    pub async fn wait_for_server_ready(&self, server_name: &str, timeout: Duration) -> bool {
        let generation = self.generation();
        let Some(async_managed_client) = generation.clients.get(server_name) else {
            return false;
        };

        match tokio::time::timeout(timeout, async_managed_client.client()).await {
            Ok(Ok(_)) => true,
            Ok(Err(_)) | Err(_) => false,
        }
    }

    pub async fn required_startup_failures(
        &self,
        required_servers: &[String],
    ) -> Vec<McpStartupFailure> {
        let generation = self.generation();
        let mut failures = Vec::new();
        for server_name in required_servers {
            let Some(async_managed_client) = generation.clients.get(server_name).cloned() else {
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

    /// Returns all tools with model-visible names normalized.
    #[instrument(level = "trace", skip_all)]
    pub async fn list_all_tools(&self) -> Vec<ToolInfo> {
        let generation = self.generation();
        let mut tools = Vec::new();
        for (server_name, managed_client) in &generation.clients {
            let has_cached_tool_info_snapshot = managed_client.cached_tool_info_snapshot.is_some();
            let startup_complete = managed_client
                .startup_complete
                .load(std::sync::atomic::Ordering::Acquire);
            trace!(
                server_name = %server_name,
                has_cached_tool_info_snapshot,
                startup_complete,
                "waiting for MCP server tools while building tool list"
            );
            let Some(server_tools) = managed_client
                .listed_tools()
                .instrument(trace_span!(
                    "list_tools_for_server",
                    server_name = %server_name,
                    has_cached_tool_info_snapshot,
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
                server_tools
                    .into_iter()
                    .map(|tool| Self::with_server_metadata(&generation, tool)),
            );
        }
        normalize_tools_for_model_with_prefix(tools, generation.prefix_mcp_tool_names)
    }

    /// Returns presentation metadata without waiting for uncached clients still initializing.
    /// Cached values will be used if available and the server is still starting up.
    pub async fn list_available_server_infos(&self) -> HashMap<String, McpServerInfo> {
        let generation = self.generation();
        let mut server_infos = HashMap::new();
        for (server_name, client) in &generation.clients {
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

    /// Force-refresh codex apps tools by bypassing the in-process cache.
    ///
    /// On success, the refreshed tools replace the cache contents and the
    /// latest filtered tools are returned directly to the caller. On
    /// failure, the existing cache remains unchanged.
    pub async fn hard_refresh_codex_apps_tools_cache(&self) -> Result<Vec<ToolInfo>> {
        let generation = self.generation();
        let managed_client = generation
            .clients
            .get(CODEX_APPS_MCP_SERVER_NAME)
            .ok_or_else(|| anyhow!("unknown MCP server '{CODEX_APPS_MCP_SERVER_NAME}'"))?
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
            &managed_client.server_info,
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
                Self::with_server_metadata(&generation, tool)
            });
        Ok(normalize_tools_for_model_with_prefix(
            tools,
            generation.prefix_mcp_tool_names,
        ))
    }

    fn with_server_metadata(generation: &ConnectionGeneration, mut tool: ToolInfo) -> ToolInfo {
        let Some(metadata) = generation.server_metadata.get(&tool.server_name) else {
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

    /// Returns a single map that contains all resources. Each key is the
    /// server name and the value is a vector of resources.
    pub async fn list_all_resources(&self) -> HashMap<String, Vec<Resource>> {
        let generation = self.generation();
        let mut join_set = JoinSet::new();

        for (server_name, async_managed_client) in &generation.clients {
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

    /// Returns a single map that contains all resource templates. Each key is the
    /// server name and the value is a vector of resource templates.
    pub async fn list_all_resource_templates(&self) -> HashMap<String, Vec<ResourceTemplate>> {
        let generation = self.generation();
        let mut join_set = JoinSet::new();

        for (server_name, async_managed_client) in &generation.clients {
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
        let (_generation, client) = self.client_by_name(server).await?;
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
        let (_generation, client) = self.client_by_name(server).await?;
        Ok(client.server_supports_sandbox_state_meta_capability)
    }

    /// List resources from the specified server.
    pub async fn list_resources(
        &self,
        server: &str,
        params: Option<PaginatedRequestParams>,
    ) -> Result<ListResourcesResult> {
        let (_generation, managed) = self.client_by_name(server).await?;
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
        let (_generation, managed) = self.client_by_name(server).await?;
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
        let (_generation, managed) = self.client_by_name(server).await?;
        let client = managed.client.clone();
        let timeout = managed.tool_timeout;
        let uri = params.uri.clone();

        client
            .read_resource(params, timeout)
            .await
            .with_context(|| format!("resources/read failed for `{server}` ({uri})"))
    }

    async fn client_by_name(
        &self,
        name: &str,
    ) -> Result<(ConnectionGenerationLease, ManagedClient)> {
        let generation = self.generation();
        let client = generation
            .clients
            .get(name)
            .ok_or_else(|| anyhow!("unknown MCP server '{name}'"))?
            .client()
            .await
            .context("failed to get client")?;
        Ok((generation, client))
    }
}

pub(crate) async fn emit_update(
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
        _ => false,
    }
}

fn is_mcp_client_startup_timeout_error(error: &StartupOutcomeError) -> bool {
    match error {
        StartupOutcomeError::Failed { error } => {
            error.contains("request timed out")
                || error.contains("timed out handshaking with MCP server")
        }
        _ => false,
    }
}

#[cfg(test)]
#[path = "connection_manager_tests.rs"]
mod tests;
