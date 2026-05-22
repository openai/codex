//! Compatibility facade for session-scoped MCP server connections.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::McpAuthStatusEntry;
use crate::client_pool::McpClientPool;
use crate::codex_apps::CodexAppsToolsCacheKey;
use crate::elicitation::ElicitationRequestManager;
use crate::elicitation::ElicitationReviewerHandle;
use crate::mcp::ToolPluginProvenance;
use crate::runtime::McpRuntimeContext;
use crate::server::EffectiveMcpServer;
use crate::session_view::McpSessionView;
use crate::tools::ToolInfo;
use anyhow::Result;
use async_channel::Sender;
use codex_config::Constrained;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_login::CodexAuth;
use codex_protocol::mcp::CallToolResult;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::McpStartupFailure;
use codex_rmcp_client::ElicitationResponse;
use rmcp::model::ElicitationCapability;
use rmcp::model::ListResourceTemplatesResult;
use rmcp::model::ListResourcesResult;
use rmcp::model::PaginatedRequestParams;
use rmcp::model::ReadResourceRequestParams;
use rmcp::model::ReadResourceResult;
use rmcp::model::RequestId;
use rmcp::model::Resource;
use rmcp::model::ResourceTemplate;
use tokio_util::sync::CancellationToken;

/// The stable core-facing facade for MCP server connections.
pub struct McpConnectionManager {
    client_pool: Arc<McpClientPool>,
    session_view: McpSessionView,
}

impl McpConnectionManager {
    pub fn new_uninitialized(
        approval_policy: &Constrained<AskForApproval>,
        permission_profile: &Constrained<PermissionProfile>,
    ) -> Self {
        Self::new_uninitialized_with_permission_profile(approval_policy, permission_profile.get())
    }

    pub fn new_uninitialized_with_permission_profile(
        approval_policy: &Constrained<AskForApproval>,
        permission_profile: &PermissionProfile,
    ) -> Self {
        let client_pool = Arc::new(McpClientPool::new_uninitialized());
        Self {
            session_view: McpSessionView::new_uninitialized(
                Arc::clone(&client_pool),
                approval_policy,
                permission_profile,
            ),
            client_pool,
        }
    }

    #[allow(clippy::new_ret_no_self, clippy::too_many_arguments)]
    pub async fn new(
        mcp_servers: &HashMap<String, EffectiveMcpServer>,
        store_mode: OAuthCredentialsStoreMode,
        auth_entries: HashMap<String, McpAuthStatusEntry>,
        approval_policy: &Constrained<AskForApproval>,
        submit_id: String,
        tx_event: Sender<Event>,
        initial_permission_profile: PermissionProfile,
        runtime_context: McpRuntimeContext,
        codex_home: PathBuf,
        codex_apps_tools_cache_key: CodexAppsToolsCacheKey,
        host_owned_codex_apps_enabled: bool,
        client_elicitation_capability: ElicitationCapability,
        tool_plugin_provenance: ToolPluginProvenance,
        auth: Option<&CodexAuth>,
        elicitation_reviewer: Option<ElicitationReviewerHandle>,
    ) -> (Self, CancellationToken) {
        let elicitation_requests = ElicitationRequestManager::new(
            approval_policy.value(),
            initial_permission_profile,
            elicitation_reviewer,
        );
        let (pool, cancel_token) = McpClientPool::new(
            mcp_servers,
            store_mode,
            auth_entries,
            submit_id,
            tx_event,
            runtime_context,
            codex_home,
            codex_apps_tools_cache_key,
            host_owned_codex_apps_enabled,
            client_elicitation_capability,
            tool_plugin_provenance,
            auth,
            elicitation_requests.clone(),
        )
        .await;
        let client_pool = Arc::new(pool);
        (
            Self {
                session_view: McpSessionView::from_parts(
                    Arc::clone(&client_pool),
                    elicitation_requests,
                ),
                client_pool,
            },
            cancel_token,
        )
    }

    pub fn has_servers(&self) -> bool {
        self.client_pool.has_servers()
    }

    pub fn begin_shutdown(&mut self) -> impl std::future::Future<Output = ()> + Send + 'static {
        let client_pool = std::mem::replace(
            &mut self.client_pool,
            Arc::new(McpClientPool::new_uninitialized()),
        );
        async move {
            client_pool.begin_shutdown().await;
        }
    }

    pub async fn shutdown(&mut self) {
        let client_pool = std::mem::replace(
            &mut self.client_pool,
            Arc::new(McpClientPool::new_uninitialized()),
        );
        client_pool.shutdown().await;
    }

    pub fn server_origin(&self, server_name: &str) -> Option<&str> {
        self.client_pool.server_origin(server_name)
    }

    pub fn server_pollutes_memory(&self, server_name: &str) -> bool {
        self.client_pool.server_pollutes_memory(server_name)
    }

    pub fn plugin_id_for_mcp_server_name(&self, server_name: &str) -> Option<&str> {
        self.client_pool.plugin_id_for_mcp_server_name(server_name)
    }

    pub fn is_host_owned_codex_apps_server(&self, server_name: &str) -> bool {
        self.client_pool
            .is_host_owned_codex_apps_server(server_name)
    }

    pub fn set_approval_policy(&self, approval_policy: &Constrained<AskForApproval>) {
        self.session_view.set_approval_policy(approval_policy);
    }

    pub fn set_permission_profile(&self, permission_profile: PermissionProfile) {
        self.session_view.set_permission_profile(permission_profile);
    }

    pub fn elicitations_auto_deny(&self) -> bool {
        self.session_view.elicitations_auto_deny()
    }

    pub fn set_elicitations_auto_deny(&self, auto_deny: bool) {
        self.session_view.set_elicitations_auto_deny(auto_deny);
    }

    pub async fn resolve_elicitation(
        &self,
        server_name: String,
        id: RequestId,
        response: ElicitationResponse,
    ) -> Result<()> {
        self.session_view
            .resolve_elicitation(server_name, id, response)
            .await
    }

    pub async fn wait_for_server_ready(&self, server_name: &str, timeout: Duration) -> bool {
        self.client_pool
            .wait_for_server_ready(server_name, timeout)
            .await
    }

    pub async fn required_startup_failures(
        &self,
        required_servers: &[String],
    ) -> Vec<McpStartupFailure> {
        self.client_pool
            .required_startup_failures(required_servers)
            .await
    }

    pub async fn list_all_tools(&self) -> Vec<ToolInfo> {
        self.client_pool.list_all_tools().await
    }

    pub async fn hard_refresh_codex_apps_tools_cache(&self) -> Result<Vec<ToolInfo>> {
        self.client_pool.hard_refresh_codex_apps_tools_cache().await
    }

    pub async fn list_all_resources(&self) -> HashMap<String, Vec<Resource>> {
        self.client_pool.list_all_resources().await
    }

    pub async fn list_all_resource_templates(&self) -> HashMap<String, Vec<ResourceTemplate>> {
        self.client_pool.list_all_resource_templates().await
    }

    pub async fn call_tool(
        &self,
        server: &str,
        tool: &str,
        arguments: Option<serde_json::Value>,
        meta: Option<serde_json::Value>,
    ) -> Result<CallToolResult> {
        self.client_pool
            .call_tool(server, tool, arguments, meta)
            .await
    }

    pub async fn server_supports_sandbox_state_meta_capability(
        &self,
        server: &str,
    ) -> Result<bool> {
        self.client_pool
            .server_supports_sandbox_state_meta_capability(server)
            .await
    }

    pub async fn list_resources(
        &self,
        server: &str,
        params: Option<PaginatedRequestParams>,
    ) -> Result<ListResourcesResult> {
        self.client_pool.list_resources(server, params).await
    }

    pub async fn list_resource_templates(
        &self,
        server: &str,
        params: Option<PaginatedRequestParams>,
    ) -> Result<ListResourceTemplatesResult> {
        self.client_pool
            .list_resource_templates(server, params)
            .await
    }

    pub async fn read_resource(
        &self,
        server: &str,
        params: ReadResourceRequestParams,
    ) -> Result<ReadResourceResult> {
        self.client_pool.read_resource(server, params).await
    }
}

#[cfg(test)]
#[path = "connection_manager_tests.rs"]
mod tests;
