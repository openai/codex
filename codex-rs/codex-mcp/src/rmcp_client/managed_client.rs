//! A chat's access to an initialized MCP client and its tool catalog.
//! A pooled connection honors this chat's lease and routes input requests back to it.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use super::MCP_TOOLS_LIST_DURATION_METRIC;
use crate::client_tool_catalog::ClientToolCatalog;
use crate::connection_pool::ConnectionSharing;
use crate::runtime::emit_duration;
use crate::tools::ToolInfo;
use anyhow::Result;
use anyhow::anyhow;
use codex_connectors::ConnectorRuntimeContext;
use codex_protocol::mcp::McpServerInfo;
use codex_rmcp_client::RmcpClient;
use codex_rmcp_client::SendElicitation;
use rmcp::model::ListResourceTemplatesResult;
use rmcp::model::ListResourcesResult;
use rmcp::model::PaginatedRequestParams;
use rmcp::model::ServerCapabilities;
use serde_json::Value;
use tokio_util::task::AbortOnDropHandle;

#[derive(Clone)]
pub(crate) struct ManagedClient {
    pub(crate) connection_sharing: ConnectionSharing,
    /// The calling chat's input handler, separate from the shared transport's startup handler.
    pub(crate) shared_elicitation: Option<Arc<SendElicitation>>,
    pub(crate) _auth_change_notifications: Option<Arc<AbortOnDropHandle<()>>>,
    pub(crate) client: Arc<RmcpClient>,
    pub(crate) server_info: McpServerInfo,
    pub(crate) tool_catalog: Arc<ClientToolCatalog>,
    pub(crate) tool_timeout: Option<Duration>,
    pub(crate) server_instructions: Option<String>,
    pub(crate) server_supports_sandbox_state_meta_capability: bool,
    pub(crate) client_supports_network_policy_meta_capability: bool,
    pub(crate) codex_apps_tools_cache_context: Option<ConnectorRuntimeContext<ToolInfo>>,
}

impl ManagedClient {
    pub(crate) async fn with_active_shared_lease<T>(
        &self,
        operation: impl Future<Output = T>,
    ) -> Result<T> {
        match &self.connection_sharing {
            ConnectionSharing::Shared(lease) => tokio::select! {
                biased;
                () = lease.cancellation().cancelled() => {
                    Err(anyhow!("MCP connection lease has been released"))
                }
                result = operation => Ok(result),
            },
            ConnectionSharing::Private
            | ConnectionSharing::Negotiated
            | ConnectionSharing::Owned(_) => Ok(operation.await),
        }
    }

    pub(crate) async fn call_tool(
        &self,
        name: String,
        arguments: Option<Value>,
        meta_for_session: impl Fn(&ServerCapabilities) -> Result<Option<Value>> + Send + Sync,
        timeout: Option<Duration>,
    ) -> Result<rmcp::model::CallToolResult> {
        self.with_active_shared_lease(async {
            if let Some(send_elicitation) = &self.shared_elicitation {
                self.client
                    .call_tool_with_elicitation(
                        name,
                        arguments,
                        meta_for_session,
                        timeout,
                        Arc::clone(send_elicitation),
                    )
                    .await
            } else {
                self.client
                    .call_tool(name, arguments, meta_for_session, timeout)
                    .await
            }
        })
        .await?
    }

    /// Lists resources within this chat's lease lifetime.
    pub(crate) async fn list_resources(
        &self,
        params: Option<PaginatedRequestParams>,
        timeout: Option<Duration>,
    ) -> Result<ListResourcesResult> {
        self.with_active_shared_lease(self.client.list_resources(params, timeout))
            .await?
    }

    /// Lists resource templates within this chat's lease lifetime.
    pub(crate) async fn list_resource_templates(
        &self,
        params: Option<PaginatedRequestParams>,
        timeout: Option<Duration>,
    ) -> Result<ListResourceTemplatesResult> {
        self.with_active_shared_lease(self.client.list_resource_templates(params, timeout))
            .await?
    }

    /// Reads a resource using this chat's input authority and lease lifetime.
    pub(crate) async fn read_resource(
        &self,
        params: rmcp::model::ReadResourceRequestParams,
        timeout: Option<Duration>,
    ) -> Result<rmcp::model::ReadResourceResult> {
        self.with_active_shared_lease(async {
            if let Some(send_elicitation) = &self.shared_elicitation {
                self.client
                    .read_resource_with_elicitation(params, timeout, Arc::clone(send_elicitation))
                    .await
            } else {
                self.client.read_resource(params, timeout).await
            }
        })
        .await?
    }

    pub(crate) async fn listed_catalog(&self) -> (Vec<ToolInfo>, Option<Arc<serde_json::Value>>) {
        let total_start = Instant::now();
        self.tool_catalog
            .read(|catalog| {
                // Discovery may use the shared cache until this client is refreshed.
                // Executable bindings always capture this client's own catalog.
                if catalog.revision == 0
                    && let Some(cache_context) = &self.codex_apps_tools_cache_context
                {
                    let tools = cache_context.current_catalog_snapshot();
                    emit_duration(
                        MCP_TOOLS_LIST_DURATION_METRIC,
                        total_start.elapsed(),
                        &[("cache", if tools.is_some() { "hit" } else { "miss" })],
                    );
                    if let Some(tools) = tools {
                        return (tools.tools().to_vec(), tools.shared_tool_list_meta());
                    }
                }
                (catalog.tools.to_vec(), catalog.meta)
            })
            .await
    }
}
