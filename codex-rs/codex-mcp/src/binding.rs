//! Immutable MCP catalog and execution handles.

use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use codex_config::AppToolApproval;
use codex_protocol::mcp::CallToolResult;
use codex_protocol::models::PermissionProfile;
use rmcp::model::ListResourceTemplatesResult;
use rmcp::model::ListResourcesResult;
use rmcp::model::PaginatedRequestParams;
use rmcp::model::ReadResourceRequestParams;
use rmcp::model::ReadResourceResult;
use rmcp::model::Resource;
use rmcp::model::ResourceTemplate;
use serde_json::Value as JsonValue;

use crate::McpConfig;
use crate::binding_clients::McpBindingClients;
use crate::connection_manager::McpConnectionSet;
use crate::rmcp_client::ManagedClient;
use crate::server::McpServerMetadata;
use crate::tools::ToolInfo;

/// The exact tool catalog and execution handles shared by compatible sampling steps.
pub struct McpBinding {
    connections: Arc<McpConnectionSet>,
    clients: Arc<McpBindingClients>,
    config: Arc<McpConfig>,
    plugins_available: bool,
    tools: Vec<ToolInfo>,
    calls: HashMap<(String, String), PreparedMcpCall>,
}

impl McpBinding {
    /// Creates an empty binding for tests and callers without a materialized runtime.
    pub fn empty(config: Arc<McpConfig>) -> Self {
        Self::new(
            Arc::new(McpConnectionSet::empty(config.prefix_mcp_tool_names)),
            Arc::new(McpBindingClients::new(HashMap::new())),
            config,
            /*plugins_available*/ false,
            Vec::new(),
            HashMap::new(),
        )
    }

    pub(crate) fn new(
        connections: Arc<McpConnectionSet>,
        clients: Arc<McpBindingClients>,
        config: Arc<McpConfig>,
        plugins_available: bool,
        tools: Vec<ToolInfo>,
        calls: HashMap<(String, String), PreparedMcpCall>,
    ) -> Self {
        Self {
            connections,
            clients,
            config,
            plugins_available,
            tools,
            calls,
        }
    }

    pub fn config(&self) -> &Arc<McpConfig> {
        &self.config
    }

    pub fn plugins_available(&self) -> bool {
        self.plugins_available
    }

    /// Returns the frozen model-visible catalog captured for this binding.
    pub fn tools(&self) -> &[ToolInfo] {
        &self.tools
    }

    /// Returns permitted tool metadata, including app-only tools.
    pub fn tool_info(&self, server: &str, tool: &str) -> Option<&ToolInfo> {
        self.calls
            .get(&(server.to_string(), tool.to_string()))
            .map(PreparedMcpCall::tool_info)
    }

    /// Binds a model-visible call to the exact client and metadata in this binding.
    pub fn prepare_call(&self, server: &str, tool: &str) -> Option<PreparedMcpCall> {
        self.calls
            .get(&(server.to_string(), tool.to_string()))
            .filter(|call| crate::tool_is_model_visible(call.tool_info()))
            .cloned()
    }

    pub fn has_servers(&self) -> bool {
        self.connections.has_servers()
    }

    pub async fn list_resources(
        &self,
        server: &str,
        params: Option<PaginatedRequestParams>,
    ) -> Result<ListResourcesResult> {
        if self.clients.client(server).is_some() {
            self.clients.list_resources(server, params).await
        } else {
            self.connections.list_resources(server, params).await
        }
    }

    pub async fn list_all_resources(
        &self,
        include_server: impl Fn(&str) -> bool,
    ) -> HashMap<String, Vec<Resource>> {
        self.clients.list_all_resources(include_server).await
    }

    pub async fn list_resource_templates(
        &self,
        server: &str,
        params: Option<PaginatedRequestParams>,
    ) -> Result<ListResourceTemplatesResult> {
        if self.clients.client(server).is_some() {
            self.clients.list_resource_templates(server, params).await
        } else {
            self.connections
                .list_resource_templates(server, params)
                .await
        }
    }

    pub async fn list_all_resource_templates(
        &self,
        include_server: impl Fn(&str) -> bool,
    ) -> HashMap<String, Vec<ResourceTemplate>> {
        self.clients
            .list_all_resource_templates(include_server)
            .await
    }

    pub async fn read_resource(
        &self,
        server: &str,
        params: ReadResourceRequestParams,
    ) -> Result<ReadResourceResult> {
        if self.clients.client(server).is_some() {
            self.clients.read_resource(server, params).await
        } else {
            self.connections.read_resource(server, params).await
        }
    }
}

impl fmt::Debug for McpBinding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("McpBinding")
            .field("tools", &self.tools)
            .field("prepared_call_count", &self.calls.len())
            .finish_non_exhaustive()
    }
}

/// A call bound to the exact client, tool, timeout, and server metadata seen by
/// one [`McpBinding`].
#[derive(Clone)]
pub struct PreparedMcpCall {
    connections: Arc<McpConnectionSet>,
    client: Arc<ManagedClient>,
    config: Arc<McpConfig>,
    catalog_revision: u64,
    tool_info: ToolInfo,
    server_name: String,
    server_metadata: McpServerMetadata,
    plugin_id: Option<String>,
    selected_plugin_server: bool,
}

impl PreparedMcpCall {
    #[expect(
        clippy::too_many_arguments,
        reason = "the exact call authority stays together"
    )]
    pub(crate) fn new(
        connections: Arc<McpConnectionSet>,
        client: Arc<ManagedClient>,
        config: Arc<McpConfig>,
        catalog_revision: u64,
        tool_info: ToolInfo,
        server_metadata: McpServerMetadata,
        plugin_id: Option<String>,
        selected_plugin_server: bool,
    ) -> Option<Self> {
        let server_name = tool_info.server_name.clone();
        config.permission_profile_for_server(&server_name)?;
        Some(Self {
            connections,
            client,
            config,
            catalog_revision,
            tool_info,
            server_name,
            server_metadata,
            plugin_id,
            selected_plugin_server,
        })
    }

    pub fn tool_info(&self) -> &ToolInfo {
        &self.tool_info
    }

    /// Returns the configuration and approval authority captured with this client.
    pub fn config(&self) -> &McpConfig {
        &self.config
    }

    /// Returns the owner permissions validated when this immutable call was prepared.
    pub fn permission_profile(&self) -> &PermissionProfile {
        let Some(permission_profile) = self.config.permission_profile_for_server(&self.server_name)
        else {
            unreachable!("prepared MCP calls retain their immutable permission authority");
        };
        permission_profile
    }

    pub fn server_name(&self) -> &str {
        &self.server_name
    }

    /// Returns whether this call is bound to the host-owned Codex Apps server.
    pub fn is_host_owned_apps(&self) -> bool {
        self.config
            .mcp_server_catalog
            .server(&self.server_name)
            .is_some_and(|registration| {
                registration
                    .source()
                    .is_host_owned_apps(&self.server_name, registration.config())
            })
    }

    pub fn server_origin(&self) -> Option<&str> {
        self.server_metadata
            .origin
            .as_ref()
            .map(super::server::McpServerOrigin::as_str)
    }

    pub fn server_environment_id(&self) -> &str {
        &self.server_metadata.environment_id
    }

    pub fn server_pollutes_memory(&self) -> bool {
        self.server_metadata.pollutes_memory
    }

    pub fn tool_approval_mode(&self) -> AppToolApproval {
        self.server_metadata
            .tool_approval_mode(&self.tool_info.tool.name)
    }

    /// Returns the explicit output budget captured with this call's effective server config.
    pub fn output_token_limit(&self) -> Option<usize> {
        self.config
            .mcp_server_catalog
            .server(&self.server_name)?
            .config()
            .tools
            .get(self.tool_info.tool.name.as_ref())?
            .output_token_limit
            .map(std::num::NonZeroUsize::get)
    }

    pub fn plugin_id(&self) -> Option<&str> {
        self.plugin_id.as_deref()
    }

    pub fn is_selected_plugin_server(&self) -> bool {
        self.selected_plugin_server
    }

    pub async fn server_supports_sandbox_state_meta_capability(&self) -> Result<bool> {
        Ok(self.client.server_supports_sandbox_state_meta_capability)
    }

    pub async fn call(
        &self,
        arguments: Option<JsonValue>,
        meta: Option<JsonValue>,
        timeout: Option<Duration>,
    ) -> Result<CallToolResult> {
        self.call_with_preparation(timeout, || async move { Ok((arguments, meta)) })
            .await
    }

    /// Runs irreversible call preparation and execution under the authority of
    /// this call's exact catalog revision and the extensions owned by the Codex session.
    /// A caller-supplied timeout can further restrict the server's configured timeout.
    pub async fn call_with_preparation<F, Fut>(
        &self,
        requested_timeout: Option<Duration>,
        prepare: F,
    ) -> Result<CallToolResult>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<(Option<JsonValue>, Option<JsonValue>)>>,
    {
        let effective_timeout = match (self.client.tool_timeout, requested_timeout) {
            (Some(server_timeout), Some(requested_timeout)) => {
                Some(server_timeout.min(requested_timeout))
            }
            (server_timeout, requested_timeout) => server_timeout.or(requested_timeout),
        };
        let tool_name = self.tool_info.tool.name.to_string();
        self.client
            .tool_catalog
            .run_with_revision(self.catalog_revision, || async {
                let (arguments, meta) = prepare().await?;
                let timeout_deadline =
                    effective_timeout.map(|timeout| tokio::time::Instant::now() + timeout);
                let add_trusted_access_context = self.connections.add_trusted_access_context(
                    &self.tool_info,
                    &self.server_metadata,
                    arguments.as_ref(),
                    meta,
                );
                let meta = match effective_timeout.zip(timeout_deadline) {
                    Some((timeout, deadline)) => {
                        tokio::time::timeout_at(deadline, add_trusted_access_context)
                            .await
                            .map_err(|_| {
                                anyhow::anyhow!("timed out awaiting tools/call after {timeout:.0?}")
                            })?
                    }
                    None => add_trusted_access_context.await,
                };
                let remaining_timeout = match effective_timeout.zip(timeout_deadline) {
                    Some((timeout, deadline)) => {
                        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                        if remaining.is_zero() {
                            return Err(anyhow::anyhow!(
                                "timed out awaiting tools/call after {timeout:.0?}"
                            ));
                        }
                        Some(remaining)
                    }
                    None => None,
                };
                self.client
                    .client
                    .call_tool(tool_name.clone(), arguments, meta, remaining_timeout)
                    .await
                    .with_context(|| format!("tool call failed for `{}/{tool_name}`", self.server_name))
            })
            .await
            .ok_or_else(|| anyhow::anyhow!(
                "tool call rejected because the catalog changed after `{}/{tool_name}` was prepared",
                self.server_name
            ))?
            .map(call_tool_result_from_rmcp)
    }
}

pub(crate) fn call_tool_result_from_rmcp(result: rmcp::model::CallToolResult) -> CallToolResult {
    let content = result
        .content
        .into_iter()
        .map(|content| {
            serde_json::to_value(content)
                .unwrap_or_else(|_| JsonValue::String("<content>".to_string()))
        })
        .collect();
    CallToolResult {
        content,
        structured_content: result.structured_content,
        is_error: result.is_error,
        meta: result.meta.and_then(|meta| serde_json::to_value(meta).ok()),
    }
}

#[cfg(test)]
#[path = "binding_tests.rs"]
mod tests;
