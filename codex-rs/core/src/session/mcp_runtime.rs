use std::fmt;
use std::sync::Arc;

use codex_mcp::McpConfig;
use codex_mcp::McpConnectionManager;
use codex_mcp::McpRuntimeContext;
use codex_protocol::capabilities::SelectedCapabilityRoot;

/// MCP config, plugin availability, exact environment bindings, and manager for one request.
pub struct McpRuntimeSnapshot {
    config: Arc<McpConfig>,
    plugins_available: bool,
    manager: Arc<McpConnectionManager>,
    runtime_context: McpRuntimeContext,
    ready_selected_capability_roots: Vec<SelectedCapabilityRoot>,
}

impl McpRuntimeSnapshot {
    pub(crate) fn new(
        config: Arc<McpConfig>,
        plugins_available: bool,
        manager: Arc<McpConnectionManager>,
        runtime_context: McpRuntimeContext,
        ready_selected_capability_roots: Vec<SelectedCapabilityRoot>,
    ) -> Self {
        Self {
            config,
            plugins_available,
            manager,
            runtime_context,
            ready_selected_capability_roots,
        }
    }

    pub fn config(&self) -> &McpConfig {
        self.config.as_ref()
    }

    pub(crate) fn plugins_available(&self) -> bool {
        self.plugins_available
    }

    pub fn manager(&self) -> &McpConnectionManager {
        self.manager.as_ref()
    }

    pub(crate) fn manager_arc(&self) -> Arc<McpConnectionManager> {
        Arc::clone(&self.manager)
    }

    pub fn runtime_context(&self) -> &McpRuntimeContext {
        &self.runtime_context
    }

    pub(crate) fn ready_selected_capability_roots(&self) -> &[SelectedCapabilityRoot] {
        &self.ready_selected_capability_roots
    }

    #[cfg(test)]
    pub(crate) fn new_uninitialized_for_test(config: &crate::config::Config) -> Arc<Self> {
        use codex_exec_server::EnvironmentManager;
        use codex_features::Feature;
        use codex_mcp::ResolvedMcpCatalog;
        use rmcp::model::ElicitationCapability;

        let mcp_config = McpConfig {
            chatgpt_base_url: config.chatgpt_base_url.clone(),
            apps_mcp_product_sku: config.apps_mcp_product_sku.clone(),
            codex_home: config.codex_home.to_path_buf(),
            mcp_oauth_credentials_store_mode: config.mcp_oauth_credentials_store_mode,
            auth_keyring_backend_kind: config.auth_keyring_backend_kind(),
            mcp_oauth_callback_port: config.mcp_oauth_callback_port,
            mcp_oauth_callback_url: config.mcp_oauth_callback_url.clone(),
            skill_mcp_dependency_install_enabled: config
                .features
                .enabled(Feature::SkillMcpDependencyInstall),
            approval_policy: config.permissions.approval_policy.clone(),
            codex_linux_sandbox_exe: config.codex_linux_sandbox_exe.clone(),
            use_legacy_landlock: config.features.use_legacy_landlock(),
            apps_enabled: config.features.enabled(Feature::Apps),
            prefix_mcp_tool_names: config.prefix_mcp_tool_names(),
            client_elicitation_capability: ElicitationCapability::default(),
            mcp_server_catalog: ResolvedMcpCatalog::default(),
            connector_snapshot: codex_connectors::ConnectorSnapshot::default(),
        };
        let manager = McpConnectionManager::new_uninitialized_with_permission_profile(
            &config.permissions.approval_policy,
            config.permissions.permission_profile(),
            config.prefix_mcp_tool_names(),
        );
        let runtime_context = McpRuntimeContext::new(
            Arc::new(EnvironmentManager::default_for_tests()),
            config.cwd.to_path_buf(),
        );
        Arc::new(Self::new(
            Arc::new(mcp_config),
            /*plugins_available*/ false,
            Arc::new(manager),
            runtime_context,
            Vec::new(),
        ))
    }
}

impl fmt::Debug for McpRuntimeSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("McpRuntimeSnapshot")
            .finish_non_exhaustive()
    }
}
