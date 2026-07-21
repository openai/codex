//! Immutable MCP state bound to one model sampling request.

use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use codex_config::AppToolApproval;
use codex_protocol::mcp::CallToolResult;
use rmcp::model::ListResourceTemplatesResult;
use rmcp::model::ListResourcesResult;
use rmcp::model::PaginatedRequestParams;
use rmcp::model::ReadResourceRequestParams;
use rmcp::model::ReadResourceResult;
use rmcp::model::Resource;
use rmcp::model::ResourceTemplate;
use serde_json::Value as JsonValue;
use tokio::sync::RwLock;

use crate::McpConfig;
use crate::binding_clients::McpBindingClients;
use crate::connection_manager::McpConnectionManager;
use crate::resource_client::McpResourceClient;
use crate::rmcp_client::ManagedClient;
use crate::server::McpServerMetadata;
use crate::tools::ToolInfo;

/// The exact tool catalog and execution handles for one model sampling request.
pub struct McpBinding {
    connections: Arc<McpConnectionManager>,
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
            Arc::new(McpConnectionManager::empty(config.prefix_mcp_tool_names)),
            Arc::new(McpBindingClients::new(HashMap::new())),
            config,
            /*plugins_available*/ false,
            Vec::new(),
            HashMap::new(),
        )
    }

    pub(crate) fn new(
        connections: Arc<McpConnectionManager>,
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

    /// Returns the frozen catalog advertised for this sampling request.
    pub fn tools(&self) -> &[ToolInfo] {
        &self.tools
    }

    /// Binds a call to the exact client and metadata advertised by this binding.
    pub fn prepare_call(&self, server: &str, tool: &str) -> Option<PreparedMcpCall> {
        self.calls
            .get(&(server.to_string(), tool.to_string()))
            .cloned()
    }

    pub fn has_servers(&self) -> bool {
        self.connections.has_servers()
    }

    /// Returns resource access bound to this binding's exact connection set.
    pub fn resource_client(&self) -> McpResourceClient {
        McpResourceClient::for_binding(Arc::clone(&self.clients))
    }

    pub async fn list_resources(
        &self,
        server: &str,
        params: Option<PaginatedRequestParams>,
    ) -> Result<ListResourcesResult> {
        self.clients.list_resources(server, params).await
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
        self.clients.list_resource_templates(server, params).await
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
        self.clients.read_resource(server, params).await
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
    _connections: Arc<McpConnectionManager>,
    client: Arc<ManagedClient>,
    catalog_revision: u64,
    catalog_revision_source: Arc<RwLock<u64>>,
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
        connections: Arc<McpConnectionManager>,
        client: Arc<ManagedClient>,
        catalog_revision: u64,
        catalog_revision_source: Arc<RwLock<u64>>,
        tool_info: ToolInfo,
        server_metadata: McpServerMetadata,
        plugin_id: Option<String>,
        selected_plugin_server: bool,
    ) -> Self {
        let server_name = tool_info.server_name.clone();
        Self {
            _connections: connections,
            client,
            catalog_revision,
            catalog_revision_source,
            tool_info,
            server_name,
            server_metadata,
            plugin_id,
            selected_plugin_server,
        }
    }

    pub fn tool_info(&self) -> &ToolInfo {
        &self.tool_info
    }

    pub fn server_name(&self) -> &str {
        &self.server_name
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
    ) -> Result<CallToolResult> {
        self.call_with_preparation(|| async move { Ok((arguments, meta)) })
            .await
    }

    /// Runs irreversible call preparation and execution under the authority of
    /// this call's exact catalog revision.
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "catalog replacement must remain serialized with call preparation and execution"
    )]
    pub async fn call_with_preparation<F, Fut>(&self, prepare: F) -> Result<CallToolResult>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<(Option<JsonValue>, Option<JsonValue>)>>,
    {
        let tool_name = self.tool_info.tool.name.to_string();
        let current_revision = self.catalog_revision_source.read().await;
        if *current_revision != self.catalog_revision {
            return Err(anyhow::anyhow!(
                "tool call rejected because the catalog changed after `{}/{tool_name}` was prepared",
                self.server_name
            ));
        }
        let (arguments, meta) = prepare().await?;
        let result = self
            .client
            .client
            .call_tool(tool_name.clone(), arguments, meta, self.client.tool_timeout)
            .await
            .with_context(|| format!("tool call failed for `{}/{tool_name}`", self.server_name))?;
        drop(current_revision);
        Ok(call_tool_result_from_rmcp(result))
    }
}

fn call_tool_result_from_rmcp(result: rmcp::model::CallToolResult) -> CallToolResult {
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
