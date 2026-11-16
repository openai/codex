//! Read selected-server metadata from one published thread runtime.

use super::PublishedMcpRuntime;
use crate::EffectiveMcpServer;
use codex_config::McpServerAuth;
use codex_login::CodexAuth;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

use super::McpRuntime;
use super::McpRuntimeContext;
use crate::McpConfig;
use crate::McpServerKey;
use crate::McpServerStatusSnapshot;
use crate::McpSnapshotDetail;
use crate::ResolvedMcpServer;
use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;
use crate::mcp::collect_mcp_server_status_snapshot_from_manager;
use crate::mcp::compute_auth_statuses;
use crate::mcp::effective_mcp_server;

impl McpRuntime {
    /// Reuses the selected connection and catalog without starting unrelated servers.
    /// Configuration and metadata belong to the same captured runtime publication.
    pub async fn server_status_snapshot(
        &self,
        server: &str,
        environment_id: Option<&str>,
        detail: McpSnapshotDetail,
        expected_registration: Option<&ResolvedMcpServer>,
        runtime_context: &McpRuntimeContext,
    ) -> anyhow::Result<(Arc<McpConfig>, McpServerStatusSnapshot)> {
        let current = self.current.load_full();
        let config = Arc::clone(
            current
                .config
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("MCP runtime is not configured"))?,
        );
        let key = config
            .mcp_server_catalog
            .resolve_server_key(server, environment_id);
        let server = config
            .mcp_server_catalog
            .server(&key)
            .filter(|registration| {
                expected_registration.is_none_or(|expected| expected == *registration)
            })
            .and_then(|registration| {
                effective_mcp_server(&config, current.auth.as_ref(), registration)
            });
        let Some(server) = server else {
            return Ok((config, McpServerStatusSnapshot::default()));
        };
        if config.xaa_enabled && matches!(server.config().auth, McpServerAuth::EmaAuth) {
            // Finish selected EMA startup and rejected-grant cleanup before reading auth.
            current
                .connections
                .list_available_server_infos(|name| name == &key)
                .await;
        }
        let servers = HashMap::from([(key, server)]);
        let snapshot = current
            .status_snapshot(&config, servers, detail, runtime_context)
            .await;
        Ok((config, snapshot))
    }

    /// Reads tools for unchanged, connected registrations on the requested page.
    pub async fn matching_status_snapshot(
        &self,
        expected: &McpConfig,
        auth: Option<&CodexAuth>,
        keys: &HashSet<McpServerKey>,
        runtime_context: &McpRuntimeContext,
    ) -> McpServerStatusSnapshot {
        let current = self.current.load_full();
        let Some(config) = current.config.as_ref() else {
            return McpServerStatusSnapshot::default();
        };
        if !current.auth_matches(auth)
            || !current.environments_match(
                &runtime_context.environment_selections,
                &runtime_context.ready_environments,
            )
            || expected.client != config.client
            || expected.mcp_server_catalog.exposure() != config.mcp_server_catalog.exposure()
            || expected.host_cwd != config.host_cwd
            || expected.environment_use_mxc != config.environment_use_mxc
            || expected.environment_cwds != config.environment_cwds
            || expected.config_layer_stack.requirements_toml()
                != config.config_layer_stack.requirements_toml()
        {
            return McpServerStatusSnapshot::default();
        }
        let mut connected = HashMap::new();
        for key in keys {
            let Some(registration) = config.mcp_server_catalog.server(key) else {
                continue;
            };
            // Apps has its own auth, protocol, and connector filtering.
            if key.name() == CODEX_APPS_MCP_SERVER_NAME
                || expected.mcp_server_catalog.server(key) != Some(registration)
                || config.permission_profile_for_server(key)
                    != expected.permission_profile_for_server(key)
            {
                continue;
            }
            if current.connections.has_reusable_status_catalog(key).await
                && let Some(server) = effective_mcp_server(config, auth, registration)
            {
                connected.insert(key.clone(), server);
            }
        }
        current
            .status_snapshot(
                config,
                connected,
                McpSnapshotDetail::ToolsAndAuthOnly,
                runtime_context,
            )
            .await
    }
}

impl PublishedMcpRuntime {
    async fn status_snapshot(
        &self,
        config: &McpConfig,
        servers: HashMap<McpServerKey, EffectiveMcpServer>,
        detail: McpSnapshotDetail,
        runtime_context: &McpRuntimeContext,
    ) -> McpServerStatusSnapshot {
        if servers.is_empty() {
            return McpServerStatusSnapshot::default();
        }
        let runtime_context = runtime_context.clone().with_selected_environments(
            Arc::clone(&self.environment_selections),
            self.ready_environments.clone(),
        );
        let auth_statuses = compute_auth_statuses(
            servers.iter(),
            config.mcp_oauth_credentials_store_mode,
            config.auth_keyring_backend_kind,
            self.auth.as_ref(),
            self.auth_manager.as_ref(),
            &config.ema_state,
            &runtime_context,
        )
        .await;
        collect_mcp_server_status_snapshot_from_manager(
            &self.connections,
            auth_statuses,
            servers.into_keys().collect(),
            detail,
        )
        .await
    }
}
