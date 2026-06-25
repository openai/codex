use std::collections::HashMap;
use std::sync::Arc;

use crate::config::Config;
use codex_config::McpServerConfig;
use codex_core_plugins::PluginsManager;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionDataInit;
use codex_extension_api::ExtensionRegistry;
use codex_extension_api::McpServerContribution;
use codex_extension_api::McpServerContributionContext;
use codex_mcp::EffectiveMcpServer;
use codex_mcp::McpConfig;
use codex_mcp::McpPluginAttribution;
use codex_mcp::McpServerRegistration;
use codex_mcp::ResolvedMcpCatalog;
use codex_mcp::configured_mcp_servers;
use codex_mcp::effective_mcp_servers;

enum OrderedMcpOverlay {
    Set {
        contributor_id: &'static str,
        contribution_order: usize,
        name: String,
        config: Box<McpServerConfig>,
    },
    SetEffective {
        contributor_id: &'static str,
        contribution_order: usize,
        name: String,
        server: Box<EffectiveMcpServer>,
    },
    Remove {
        contributor_id: &'static str,
        contribution_order: usize,
        name: String,
    },
}

pub(crate) type McpContributorsRevision = Vec<(&'static str, u64)>;

/// Contributor-free MCP registrations used as the stable input to runtime overlays.
///
/// Keeping the resolved catalog preserves source precedence and plugin attribution. A strict
/// refresh received as a source-less server map intentionally creates config-owned registrations.
#[derive(Clone)]
pub(crate) struct McpConfiguredBase {
    catalog: ResolvedMcpCatalog,
}

impl McpConfiguredBase {
    pub(crate) fn from_servers(servers: HashMap<String, McpServerConfig>) -> Self {
        let mut catalog = codex_mcp::McpCatalogBuilder::default();
        for (name, server) in servers {
            catalog.register(McpServerRegistration::from_config(name, server));
        }
        Self {
            catalog: catalog.build(),
        }
    }

    pub(crate) fn configured_servers(&self) -> HashMap<String, McpServerConfig> {
        self.catalog.configured_servers()
    }
}

#[derive(Clone)]
pub struct McpManager {
    plugins_manager: Arc<PluginsManager>,
    extensions: Arc<ExtensionRegistry<Config>>,
}

impl McpManager {
    pub fn new(plugins_manager: Arc<PluginsManager>) -> Self {
        Self {
            plugins_manager,
            extensions: codex_extension_api::empty_extension_registry(),
        }
    }

    /// Creates a manager that resolves host-installed MCP contributions.
    pub fn new_with_extensions(
        plugins_manager: Arc<PluginsManager>,
        extensions: Arc<ExtensionRegistry<Config>>,
    ) -> Self {
        Self {
            plugins_manager,
            extensions,
        }
    }

    /// Returns the MCP config after applying runtime extension overlays.
    pub async fn runtime_config(&self, config: &Config) -> McpConfig {
        self.runtime_config_with_context(
            McpServerContributionContext::global(config),
            /*configured_base*/ None,
        )
        .await
        .0
    }

    pub(crate) async fn runtime_config_for_step(
        &self,
        config: &Config,
        thread_init: &ExtensionDataInit,
        thread_store: &ExtensionData,
        available_environment_ids: &[String],
    ) -> McpConfig {
        self.runtime_config_for_step_with_base_and_revision(
            config,
            thread_init,
            thread_store,
            available_environment_ids,
        )
        .await
        .0
    }

    pub(crate) async fn runtime_config_for_step_with_base_and_revision(
        &self,
        config: &Config,
        thread_init: &ExtensionDataInit,
        thread_store: &ExtensionData,
        available_environment_ids: &[String],
    ) -> (McpConfig, McpConfiguredBase, McpContributorsRevision) {
        self.runtime_config_with_context(
            McpServerContributionContext::for_step(
                config,
                thread_init,
                thread_store,
                available_environment_ids,
            ),
            /*configured_base*/ None,
        )
        .await
    }

    pub(crate) async fn runtime_config_for_step_from_base_with_revision(
        &self,
        config: &Config,
        thread_init: &ExtensionDataInit,
        thread_store: &ExtensionData,
        available_environment_ids: &[String],
        configured_base: &McpConfiguredBase,
    ) -> (McpConfig, McpContributorsRevision) {
        let (mcp_config, _, contributors_revision) = self
            .runtime_config_with_context(
                McpServerContributionContext::for_step(
                    config,
                    thread_init,
                    thread_store,
                    available_environment_ids,
                ),
                Some(configured_base),
            )
            .await;
        (mcp_config, contributors_revision)
    }

    pub(crate) async fn refresh_runtime_config_for_step_with_revision(
        &self,
        config: &Config,
        thread_init: &ExtensionDataInit,
        thread_store: &ExtensionData,
        available_environment_ids: &[String],
        configured_base: &McpConfiguredBase,
    ) -> (McpConfig, McpContributorsRevision) {
        let context = McpServerContributionContext::for_step(
            config,
            thread_init,
            thread_store,
            available_environment_ids,
        );
        for contributor in self.extensions.mcp_server_contributors() {
            contributor.refresh(context).await;
        }
        let (mcp_config, _, contributors_revision) = self
            .runtime_config_with_context(context, Some(configured_base))
            .await;
        (mcp_config, contributors_revision)
    }

    pub(crate) fn contributors_revision(&self) -> McpContributorsRevision {
        self.extensions
            .mcp_server_contributors()
            .iter()
            .map(|contributor| (contributor.id(), contributor.revision()))
            .collect()
    }

    async fn runtime_config_with_context(
        &self,
        context: McpServerContributionContext<'_, Config>,
        configured_base: Option<&McpConfiguredBase>,
    ) -> (McpConfig, McpConfiguredBase, McpContributorsRevision) {
        let config = context.config();
        let mut selected_plugin_registrations = Vec::new();
        let mut overlays = Vec::new();
        let mut contributors_revision = Vec::new();
        // A contributor can emit multiple ordered actions, so order each action globally rather
        // than enumerating contributors.
        let mut contribution_order = 0;
        for contributor in self.extensions.mcp_server_contributors() {
            let contributed = contributor.contribute_with_revision(context).await;
            contributors_revision.push((contributor.id(), contributed.revision));
            for contribution in contributed.contributions {
                match contribution {
                    McpServerContribution::Set { name, config } => {
                        overlays.push(OrderedMcpOverlay::Set {
                            contributor_id: contributor.id(),
                            contribution_order,
                            name,
                            config,
                        });
                    }
                    McpServerContribution::SetEffective { name, server } => {
                        overlays.push(OrderedMcpOverlay::SetEffective {
                            contributor_id: contributor.id(),
                            contribution_order,
                            name,
                            server,
                        });
                    }
                    McpServerContribution::SelectedPlugin {
                        name,
                        plugin_id,
                        plugin_display_name,
                        selection_order,
                        config,
                    } => selected_plugin_registrations.push(
                        McpServerRegistration::from_selected_plugin(
                            name,
                            McpPluginAttribution::new(plugin_id, plugin_display_name),
                            selection_order,
                            *config,
                        ),
                    ),
                    McpServerContribution::Remove { name } => {
                        overlays.push(OrderedMcpOverlay::Remove {
                            contributor_id: contributor.id(),
                            contribution_order,
                            name,
                        });
                    }
                }
                contribution_order += 1;
            }
        }

        let mut mcp_config = config.to_mcp_config(self.plugins_manager.as_ref()).await;
        let configured_base = match configured_base {
            Some(configured_base) => configured_base.clone(),
            None => McpConfiguredBase {
                catalog: mcp_config.mcp_server_catalog.clone(),
            },
        };
        let mut selected_plugin_catalog = configured_base
            .catalog
            .to_builder_recomputing_disabled_vetoes();
        for registration in selected_plugin_registrations {
            selected_plugin_catalog.register(registration);
        }
        let mut catalog = selected_plugin_catalog.build().to_builder();

        for overlay in overlays {
            match overlay {
                OrderedMcpOverlay::Set {
                    contributor_id,
                    contribution_order,
                    name,
                    config,
                } => catalog.register(McpServerRegistration::from_extension(
                    name,
                    contributor_id,
                    contribution_order,
                    *config,
                )),
                OrderedMcpOverlay::SetEffective {
                    contributor_id,
                    contribution_order,
                    name,
                    server,
                } => catalog.register(McpServerRegistration::from_effective_extension(
                    name,
                    contributor_id,
                    contribution_order,
                    *server,
                )),
                OrderedMcpOverlay::Remove {
                    contributor_id,
                    contribution_order,
                    name,
                } => catalog.remove_extension(name, contributor_id, contribution_order),
            }
        }
        let catalog = catalog.build();
        for conflict in catalog.conflicts() {
            tracing::warn!(
                server = conflict.name,
                outcome = ?conflict.outcome,
                contenders = ?conflict.contenders,
                "conflicting MCP server actions; using resolved catalog outcome"
            );
        }
        mcp_config.mcp_server_catalog = catalog;
        (mcp_config, configured_base, contributors_revision)
    }

    /// Returns config- and plugin-backed servers without runtime contributions.
    pub async fn configured_servers(&self, config: &Config) -> HashMap<String, McpServerConfig> {
        self.configured_base(config).await.configured_servers()
    }

    /// Returns serializable runtime winners without initializing external discovery.
    pub async fn current_runtime_servers(
        &self,
        config: &Config,
    ) -> HashMap<String, McpServerConfig> {
        let (mcp_config, _, _) = self
            .runtime_config_with_context(
                McpServerContributionContext::global_current(config),
                /*configured_base*/ None,
            )
            .await;
        configured_mcp_servers(&mcp_config)
    }

    pub(crate) async fn configured_base(&self, config: &Config) -> McpConfiguredBase {
        let mcp_config = config.to_mcp_config(self.plugins_manager.as_ref()).await;
        McpConfiguredBase {
            catalog: mcp_config.mcp_server_catalog,
        }
    }

    /// Returns serializable configured and host-contributed servers before auth gating.
    /// Runtime-only effective contributions are excluded.
    pub async fn runtime_servers(&self, config: &Config) -> HashMap<String, McpServerConfig> {
        let mcp_config = self.runtime_config(config).await;
        configured_mcp_servers(&mcp_config)
    }

    /// Returns runtime servers after auth gating and extension overlays.
    pub async fn effective_servers(&self, config: &Config) -> HashMap<String, EffectiveMcpServer> {
        let mcp_config = self.runtime_config(config).await;
        effective_mcp_servers(&mcp_config)
    }
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod tests;
