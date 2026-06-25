use std::collections::HashMap;
use std::sync::Arc;

use super::INITIAL_SUBMIT_ID;
use super::McpRuntimeSnapshot;
use super::Session;
use super::TurnContext;
use crate::config::Config;
use crate::environment_selection::TurnEnvironmentSnapshot;
use codex_config::McpServerConfig;
use codex_core_plugins::ExecutorPluginRuntime;
use codex_exec_server::ResolvedSelectedCapabilityRoot;
use codex_mcp::ElicitationReviewerHandle;
use codex_mcp::McpConfig;
use codex_mcp::McpConnectionManager;
use codex_mcp::McpRuntimeContext;
use codex_mcp::codex_apps_tools_cache_key;
use codex_mcp::compute_auth_statuses;
use codex_mcp::effective_mcp_servers_from_configured;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use tokio_util::sync::CancellationToken;

pub(super) enum McpRuntimeScope<'a> {
    Turn(&'a TurnContext),
    Thread(&'a Config),
}

struct ProjectedMcpConfig {
    bindings: Vec<(usize, ResolvedSelectedCapabilityRoot)>,
    plugins: Vec<(usize, ExecutorPluginRuntime)>,
    config: McpConfig,
    runtime_context: McpRuntimeContext,
}

impl Session {
    pub(crate) async fn mcp_runtime_for_step(
        self: &Arc<Self>,
        turn_context: &TurnContext,
        environments: &TurnEnvironmentSnapshot,
        selected_roots: &[SelectedCapabilityRoot],
        resolved_roots: &[ResolvedSelectedCapabilityRoot],
    ) -> Arc<McpRuntimeSnapshot> {
        let mut cache = self.services.selected_mcp_runtime.lock().await;
        if selected_roots.is_empty() {
            let base = cache.base_runtime();
            self.services
                .publish_existing_mcp_runtime(Arc::clone(&base));
            return base;
        }
        let bindings = selected_bindings(selected_roots, resolved_roots);
        let cached_runtime = cache.runtime_for_bindings(&bindings);
        let mut plugins = cache.plugins_for_bindings(&bindings).unwrap_or_default();
        let mut discovered_plugin = false;
        if cached_runtime.is_some() {
            let unresolved = bindings
                .iter()
                .filter(|(order, _)| {
                    !plugins
                        .iter()
                        .any(|(plugin_order, _)| plugin_order == order)
                })
                .cloned()
                .collect::<Vec<_>>();
            let discovered = project_executor_plugins(&unresolved).await;
            discovered_plugin = !discovered.is_empty();
            plugins.extend(discovered);
            plugins.sort_unstable_by_key(|(order, _)| *order);
        } else {
            plugins = project_executor_plugins(&bindings).await;
        }

        let base = cache.base_runtime();
        let mcp_config = Arc::new(
            self.services
                .mcp_manager
                .runtime_config_for_executor_plugins(base.config(), &turn_context.config, &plugins),
        );
        let configured_servers = codex_mcp::configured_mcp_servers(mcp_config.as_ref());
        let pinned_roots = bindings
            .iter()
            .map(|(_, root)| root.clone())
            .collect::<Vec<_>>();
        let runtime_context =
            self.mcp_runtime_context(&turn_context.config, environments, &pinned_roots);
        if !discovered_plugin
            && let Some(runtime) = cached_runtime
            && runtime.matches_projection(mcp_config.as_ref(), &runtime_context)
        {
            runtime
                .manager()
                .set_approval_policy(&turn_context.approval_policy);
            runtime
                .manager()
                .set_permission_profile(turn_context.permission_profile());
            self.services
                .publish_existing_mcp_runtime(Arc::clone(&runtime));
            return runtime;
        }
        let runtime = if plugins.is_empty() {
            base
        } else {
            self.build_mcp_runtime(
                McpRuntimeScope::Turn(turn_context),
                mcp_config,
                configured_servers,
                runtime_context,
                Some(self.mcp_elicitation_reviewer()),
            )
            .await
        };
        cache.replace_selected_runtime(bindings, plugins, Arc::clone(&runtime));
        self.services
            .publish_existing_mcp_runtime(Arc::clone(&runtime));
        runtime
    }

    pub(crate) async fn project_mcp_config(
        &self,
        config: &Config,
    ) -> (McpConfig, McpRuntimeContext) {
        let projection = self.project_mcp_config_inner(config).await;
        (projection.config, projection.runtime_context)
    }

    async fn project_mcp_config_inner(&self, config: &Config) -> ProjectedMcpConfig {
        let environments = self.services.turn_environments.snapshot().await;
        let selected_roots = &self.services.selected_capability_roots;
        let resolved_roots = self
            .services
            .turn_environments
            .environment_manager()
            .resolve_selected_capability_roots(
                selected_roots,
                &environments.captured_environments(),
            )
            .await;
        let bindings = selected_bindings(selected_roots, &resolved_roots);
        let plugins = project_executor_plugins(&bindings).await;
        let base_config = self.services.mcp_manager.runtime_config(config).await;
        let mcp_config = self
            .services
            .mcp_manager
            .runtime_config_for_executor_plugins(&base_config, config, &plugins);
        let runtime_context = self.mcp_runtime_context(config, &environments, &resolved_roots);
        ProjectedMcpConfig {
            bindings,
            plugins,
            config: mcp_config,
            runtime_context,
        }
    }

    pub(crate) async fn project_mcp_runtime(
        self: &Arc<Self>,
        config: &Config,
    ) -> Arc<McpRuntimeSnapshot> {
        let mut cache = self.services.selected_mcp_runtime.lock().await;
        let projection = self.project_mcp_config_inner(config).await;
        let current = cache
            .runtime_for_bindings(&projection.bindings)
            .unwrap_or_else(|| self.services.latest_mcp_runtime());
        if current.matches_projection(&projection.config, &projection.runtime_context) {
            current
                .manager()
                .set_approval_policy(&config.permissions.approval_policy);
            current
                .manager()
                .set_permission_profile(config.permissions.permission_profile().clone());
            return current;
        }
        let mcp_config = Arc::new(projection.config);
        let configured_servers = codex_mcp::configured_mcp_servers(mcp_config.as_ref());
        let runtime = self
            .build_mcp_runtime(
                McpRuntimeScope::Thread(config),
                mcp_config,
                configured_servers,
                projection.runtime_context,
                Some(self.mcp_elicitation_reviewer()),
            )
            .await;
        if projection.bindings.is_empty() {
            cache.replace_base_and_invalidate_selected(Arc::clone(&runtime));
        } else {
            cache.replace_selected_runtime(
                projection.bindings,
                projection.plugins,
                Arc::clone(&runtime),
            );
        }
        self.services
            .publish_existing_mcp_runtime(Arc::clone(&runtime));
        runtime
    }

    pub(super) async fn build_mcp_runtime(
        &self,
        scope: McpRuntimeScope<'_>,
        mcp_config: Arc<McpConfig>,
        configured_servers: HashMap<String, McpServerConfig>,
        mcp_runtime_context: McpRuntimeContext,
        elicitation_reviewer: Option<ElicitationReviewerHandle>,
    ) -> Arc<McpRuntimeSnapshot> {
        let auth = self.services.auth_manager.auth().await;
        let tool_plugin_provenance = codex_mcp::tool_plugin_provenance(&mcp_config);
        let mcp_servers = effective_mcp_servers_from_configured(
            configured_servers,
            mcp_config.as_ref(),
            auth.as_ref(),
        );
        let auth_statuses = compute_auth_statuses(
            mcp_servers.iter(),
            mcp_config.mcp_oauth_credentials_store_mode,
            mcp_config.auth_keyring_backend_kind,
            auth.as_ref(),
            &mcp_runtime_context,
        )
        .await;
        let (approval_policy, submit_id, permission_profile, publish_startup_token) = match scope {
            McpRuntimeScope::Turn(turn_context) => (
                &turn_context.approval_policy,
                turn_context.sub_id.clone(),
                turn_context.permission_profile(),
                true,
            ),
            McpRuntimeScope::Thread(config) => (
                &config.permissions.approval_policy,
                INITIAL_SUBMIT_ID.to_string(),
                config.permissions.permission_profile().clone(),
                false,
            ),
        };
        let mcp_startup_cancellation_token = CancellationToken::new();
        if publish_startup_token {
            let mut guard = self.services.mcp_startup_cancellation_token.lock().await;
            // The previous runtime owns the old token and may still be serving an in-flight step.
            // Its manager cancels that token when the last runtime handle is dropped.
            *guard = mcp_startup_cancellation_token.clone();
        }
        let manager = McpConnectionManager::new(
            &mcp_servers,
            mcp_config.mcp_oauth_credentials_store_mode,
            mcp_config.auth_keyring_backend_kind,
            auth_statuses,
            approval_policy,
            submit_id,
            self.get_tx_event(),
            mcp_startup_cancellation_token,
            permission_profile,
            mcp_runtime_context.clone(),
            mcp_config.codex_home.clone(),
            codex_apps_tools_cache_key(auth.as_ref()),
            mcp_config.prefix_mcp_tool_names,
            mcp_config.client_elicitation_capability.clone(),
            self.services
                .supports_openai_form_elicitation
                .load(std::sync::atomic::Ordering::Relaxed),
            tool_plugin_provenance,
            auth.as_ref(),
            elicitation_reviewer,
        )
        .await;
        let current_manager = self.services.latest_mcp_runtime();
        manager.set_elicitations_auto_deny(current_manager.manager().elicitations_auto_deny());
        let runtime = Arc::new(McpRuntimeSnapshot::new(
            mcp_config,
            Arc::new(manager),
            mcp_runtime_context,
        ));
        self.services
            .track_mcp_elicitation_manager(&runtime.manager_arc());
        runtime
    }

    pub(super) fn mcp_runtime_context(
        &self,
        config: &Config,
        environments: &TurnEnvironmentSnapshot,
        pinned_roots: &[ResolvedSelectedCapabilityRoot],
    ) -> McpRuntimeContext {
        // TODO(anp): Migrate MCP runtime cwd plumbing to PathUri so foreign environment cwd
        // values can be used without falling back to the legacy host cwd.
        let cwd = environments
            .primary()
            .and_then(|turn_environment| turn_environment.cwd().to_abs_path().ok())
            .map(|cwd| cwd.to_path_buf())
            .unwrap_or_else(|| config.cwd.to_path_buf());
        McpRuntimeContext::new(self.services.turn_environments.environment_manager(), cwd)
            .with_pinned_environments(pinned_roots.iter().map(|root| {
                let CapabilityRootLocation::Environment { environment_id, .. } =
                    &root.selected_root().location;
                (environment_id.clone(), Arc::clone(root.environment()))
            }))
    }
}

fn selected_bindings(
    selected_roots: &[SelectedCapabilityRoot],
    resolved_roots: &[ResolvedSelectedCapabilityRoot],
) -> Vec<(usize, ResolvedSelectedCapabilityRoot)> {
    resolved_roots
        .iter()
        .filter_map(|root| {
            selected_roots
                .iter()
                .position(|selected| selected == root.selected_root())
                .map(|selection_order| (selection_order, root.clone()))
        })
        .collect()
}

async fn project_executor_plugins(
    bindings: &[(usize, ResolvedSelectedCapabilityRoot)],
) -> Vec<(usize, ExecutorPluginRuntime)> {
    let mut plugins = Vec::new();
    for (selection_order, root) in bindings {
        match ExecutorPluginRuntime::project(root).await {
            Ok(Some(runtime)) => plugins.push((*selection_order, runtime)),
            Ok(None) => {}
            Err(err) => {
                tracing::warn!(
                    selected_root = root.selected_root().id,
                    error = %err,
                    "failed to project selected executor plugin runtime"
                );
            }
        }
    }
    plugins
}
