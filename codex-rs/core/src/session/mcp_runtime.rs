use std::fmt;
use std::sync::Arc;

use crate::config::Config;
use crate::mcp::McpConfiguredBase;
use crate::mcp::McpContributorsRevision;
use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_mcp::McpConfig;
use codex_mcp::McpConnectionManager;
use codex_mcp::McpRuntimeContext;

/// Source inputs that produced an MCP runtime snapshot.
///
/// `contributor_config` is the session config used to evaluate contributor policy. It is
/// intentionally independent from `configured_base`: callers such as skill dependency install
/// may temporarily supply a different sourceful base without changing session policy.
#[derive(Clone)]
pub(crate) struct McpRuntimeInputs {
    pub(crate) contributor_config: Arc<Config>,
    pub(crate) configured_base: McpConfiguredBase,
    pub(crate) store_mode: OAuthCredentialsStoreMode,
    pub(crate) keyring_backend_kind: AuthKeyringBackendKind,
    pub(crate) contributors_revision: McpContributorsRevision,
}

impl McpRuntimeInputs {
    pub(crate) fn new(
        contributor_config: Arc<Config>,
        configured_base: McpConfiguredBase,
        store_mode: OAuthCredentialsStoreMode,
        keyring_backend_kind: AuthKeyringBackendKind,
        contributors_revision: McpContributorsRevision,
    ) -> Self {
        Self {
            contributor_config,
            configured_base,
            store_mode,
            keyring_backend_kind,
            contributors_revision,
        }
    }

    fn matches(
        &self,
        contributor_config: &Arc<Config>,
        contributors_revision: &McpContributorsRevision,
    ) -> bool {
        Arc::ptr_eq(&self.contributor_config, contributor_config)
            && &self.contributors_revision == contributors_revision
    }
}

/// MCP config, exact environment bindings, and manager used by one model request.
pub struct McpRuntimeSnapshot {
    config: Arc<McpConfig>,
    manager: Arc<McpConnectionManager>,
    runtime_context: McpRuntimeContext,
    available_environment_ids: Vec<String>,
    inputs: McpRuntimeInputs,
}

impl McpRuntimeSnapshot {
    pub(crate) fn new(
        config: Arc<McpConfig>,
        manager: Arc<McpConnectionManager>,
        runtime_context: McpRuntimeContext,
        available_environment_ids: Vec<String>,
        inputs: McpRuntimeInputs,
    ) -> Self {
        Self {
            config,
            manager,
            runtime_context,
            available_environment_ids,
            inputs,
        }
    }

    pub fn config(&self) -> &McpConfig {
        self.config.as_ref()
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

    pub(crate) fn available_environment_ids(&self) -> &[String] {
        &self.available_environment_ids
    }

    pub(crate) fn inputs(&self) -> &McpRuntimeInputs {
        &self.inputs
    }

    pub(crate) fn matches(
        &self,
        contributor_config: &Arc<Config>,
        contributors_revision: &McpContributorsRevision,
        runtime_context: &McpRuntimeContext,
        available_environment_ids: &[String],
    ) -> bool {
        self.available_environment_ids == available_environment_ids
            && self
                .inputs
                .matches(contributor_config, contributors_revision)
            && self
                .runtime_context
                .has_same_launch_context(runtime_context)
    }

    #[cfg(test)]
    pub(crate) fn new_uninitialized_for_test(
        config: Arc<Config>,
        runtime_context: McpRuntimeContext,
    ) -> Arc<Self> {
        use codex_features::Feature;
        use codex_mcp::ResolvedMcpCatalog;
        use rmcp::model::ElicitationCapability;

        let mcp_config = McpConfig {
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
            prefix_mcp_tool_names: config.prefix_mcp_tool_names(),
            client_elicitation_capability: ElicitationCapability::default(),
            mcp_server_catalog: ResolvedMcpCatalog::default(),
        };
        let manager = McpConnectionManager::new_uninitialized(config.prefix_mcp_tool_names());
        let inputs = McpRuntimeInputs::new(
            Arc::clone(&config),
            McpConfiguredBase::from_servers(config.mcp_servers.get().clone()),
            config.mcp_oauth_credentials_store_mode,
            config.auth_keyring_backend_kind(),
            Vec::new(),
        );
        Arc::new(Self::new(
            Arc::new(mcp_config),
            Arc::new(manager),
            runtime_context,
            Vec::new(),
            inputs,
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
