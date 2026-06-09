use anyhow::Context;
use codex_app_server_protocol::PluginAvailability;
use codex_app_server_protocol::PluginInstallPolicy;
use codex_login::CodexAuth;
use codex_plugin::PluginCapabilitySummary;
use std::collections::HashSet;
use std::path::Component;
use std::path::Path;
use tracing::warn;

use crate::OPENAI_BUNDLED_MARKETPLACE_NAME;
use crate::OPENAI_CURATED_MARKETPLACE_NAME;
use crate::PluginsConfigInput;
use crate::PluginsManager;
use crate::marketplace::MarketplacePluginInstallPolicy;
use crate::remote::REMOTE_GLOBAL_MARKETPLACE_NAME;

const TOOL_SUGGEST_DISCOVERABLE_PLUGIN_ALLOWLIST: &[&str] = &[
    "github@openai-curated",
    "notion@openai-curated",
    "slack@openai-curated",
    "gmail@openai-curated",
    "google-calendar@openai-curated",
    "google-drive@openai-curated",
    "openai-developers@openai-curated",
    "canva@openai-curated",
    "teams@openai-curated",
    "sharepoint@openai-curated",
    "outlook-email@openai-curated",
    "outlook-calendar@openai-curated",
    "linear@openai-curated",
    "figma@openai-curated",
    "github@openai-curated-remote",
    "notion@openai-curated-remote",
    "slack@openai-curated-remote",
    "gmail@openai-curated-remote",
    "google-calendar@openai-curated-remote",
    "google-drive@openai-curated-remote",
    "openai-developers@openai-curated-remote",
    "canva@openai-curated-remote",
    "teams@openai-curated-remote",
    "sharepoint@openai-curated-remote",
    "outlook-email@openai-curated-remote",
    "outlook-calendar@openai-curated-remote",
    "linear@openai-curated-remote",
    "figma@openai-curated-remote",
    "chrome@openai-bundled",
    "computer-use@openai-bundled",
];

const TOOL_SUGGEST_DISCOVERABLE_MARKETPLACE_ALLOWLIST: &[&str] = &[
    OPENAI_BUNDLED_MARKETPLACE_NAME,
    OPENAI_CURATED_MARKETPLACE_NAME,
    REMOTE_GLOBAL_MARKETPLACE_NAME,
];

const OPENAI_CURATED_MARKETPLACE_PATH_SUFFIX: &str =
    ".tmp/plugins/.agents/plugins/marketplace.json";

#[derive(Debug, Clone)]
pub struct ToolSuggestPluginDiscoveryInput {
    pub plugins: PluginsConfigInput,
    pub configured_plugin_ids: HashSet<String>,
    pub disabled_plugin_ids: HashSet<String>,
    pub loaded_plugin_app_connector_ids: HashSet<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolSuggestDiscoverablePlugin {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub has_skills: bool,
    pub mcp_server_names: Vec<String>,
    pub app_connector_ids: Vec<String>,
}

impl PluginsManager {
    pub async fn list_tool_suggest_discoverable_plugins(
        &self,
        input: &ToolSuggestPluginDiscoveryInput,
        auth: Option<&CodexAuth>,
    ) -> anyhow::Result<Vec<ToolSuggestDiscoverablePlugin>> {
        if !input.plugins.plugins_enabled {
            return Ok(Vec::new());
        }

        let marketplaces = self
            .list_marketplaces_for_config(&input.plugins, &[])
            .context("failed to list plugin marketplaces for tool suggestions")?
            .marketplaces;
        let mut installed_app_connector_ids = self
            .plugins_for_config(&input.plugins)
            .await
            .capability_summaries()
            .iter()
            .flat_map(|plugin| plugin.app_connector_ids.iter())
            .map(|connector_id| connector_id.0.clone())
            .collect::<HashSet<_>>();
        installed_app_connector_ids.extend(input.loaded_plugin_app_connector_ids.iter().cloned());
        let remote_installed_marketplaces = if input.plugins.remote_plugin_enabled {
            self.build_remote_installed_plugin_marketplaces_from_cache(&[
                REMOTE_GLOBAL_MARKETPLACE_NAME,
            ])
        } else {
            None
        };

        let mut discoverable_plugins = Vec::<ToolSuggestDiscoverablePlugin>::new();
        for marketplace in marketplaces {
            let marketplace_name = marketplace.name;
            if input.plugins.remote_plugin_enabled
                && marketplace_name == OPENAI_CURATED_MARKETPLACE_NAME
            {
                continue;
            }
            let use_legacy_local_curated_filter = should_use_legacy_local_curated_discovery_filter(
                &marketplace_name,
                marketplace.path.as_path(),
            );
            let is_allowlisted_marketplace = TOOL_SUGGEST_DISCOVERABLE_MARKETPLACE_ALLOWLIST
                .contains(&marketplace_name.as_str());

            for plugin in marketplace.plugins {
                let is_configured_plugin = input.configured_plugin_ids.contains(plugin.id.as_str());
                let is_fallback_plugin =
                    TOOL_SUGGEST_DISCOVERABLE_PLUGIN_ALLOWLIST.contains(&plugin.id.as_str());
                if plugin.installed
                    || plugin.policy.installation == MarketplacePluginInstallPolicy::NotAvailable
                    || input.disabled_plugin_ids.contains(plugin.id.as_str())
                    || (!is_allowlisted_marketplace && !is_configured_plugin)
                {
                    continue;
                }

                // On Windows-backed WSL mounts, keep local curated discovery bounded to the
                // legacy fallback/configured set instead of reading every plugin detail for app
                // ids. Remote curated has cached app ids and still expands by installed apps.
                if use_legacy_local_curated_filter && !is_configured_plugin && !is_fallback_plugin {
                    continue;
                }

                let plugin_id = plugin.id.clone();

                match self
                    .read_plugin_detail_for_marketplace_plugin(
                        &input.plugins,
                        &marketplace_name,
                        plugin,
                    )
                    .await
                {
                    Ok(plugin) => {
                        let plugin: PluginCapabilitySummary = plugin.into();
                        let matches_installed_app =
                            plugin.app_connector_ids.iter().any(|connector_id| {
                                installed_app_connector_ids.contains(connector_id.0.as_str())
                            });
                        if !is_configured_plugin && !is_fallback_plugin && !matches_installed_app {
                            continue;
                        }

                        discoverable_plugins.push(ToolSuggestDiscoverablePlugin {
                            id: plugin.config_name,
                            name: plugin.display_name,
                            description: plugin.description,
                            has_skills: plugin.has_skills,
                            mcp_server_names: plugin.mcp_server_names,
                            app_connector_ids: plugin
                                .app_connector_ids
                                .into_iter()
                                .map(|connector_id| connector_id.0)
                                .collect(),
                        });
                    }
                    Err(err) => {
                        warn!("failed to load discoverable plugin suggestion {plugin_id}: {err:#}")
                    }
                }
            }
        }
        if let Some(remote_installed_marketplaces) = remote_installed_marketplaces.as_ref() {
            let installed_remote_plugin_ids = remote_installed_marketplaces
                .iter()
                .flat_map(|marketplace| marketplace.plugins.iter())
                .map(|plugin| plugin.remote_plugin_id.clone())
                .collect::<HashSet<_>>();
            for plugin in
                self.cached_global_remote_discoverable_plugins_for_config(&input.plugins, auth)
            {
                let is_configured_plugin = input
                    .configured_plugin_ids
                    .contains(plugin.config_id.as_str())
                    || input
                        .configured_plugin_ids
                        .contains(plugin.remote_plugin_id.as_str());
                let is_fallback_plugin =
                    TOOL_SUGGEST_DISCOVERABLE_PLUGIN_ALLOWLIST.contains(&plugin.config_id.as_str());
                let matches_installed_app = plugin
                    .app_ids
                    .iter()
                    .any(|app_id| installed_app_connector_ids.contains(app_id.as_str()));
                let is_disabled = input
                    .disabled_plugin_ids
                    .contains(plugin.config_id.as_str())
                    || input
                        .disabled_plugin_ids
                        .contains(plugin.remote_plugin_id.as_str());
                if installed_remote_plugin_ids.contains(&plugin.remote_plugin_id)
                    || plugin.install_policy == PluginInstallPolicy::NotAvailable
                    || plugin.availability == PluginAvailability::DisabledByAdmin
                    || is_disabled
                    || (!is_configured_plugin && !is_fallback_plugin && !matches_installed_app)
                {
                    continue;
                }

                discoverable_plugins.push(ToolSuggestDiscoverablePlugin {
                    id: plugin.config_id,
                    name: plugin.name,
                    description: plugin.description,
                    has_skills: plugin.has_skills,
                    mcp_server_names: Vec::new(),
                    app_connector_ids: plugin.app_ids,
                });
            }
        }
        discoverable_plugins.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(discoverable_plugins)
    }
}

fn should_use_legacy_local_curated_discovery_filter(
    marketplace_name: &str,
    marketplace_path: &Path,
) -> bool {
    marketplace_name == OPENAI_CURATED_MARKETPLACE_NAME
        && is_wsl_windows_drive_path(marketplace_path)
        && marketplace_path.ends_with(Path::new(OPENAI_CURATED_MARKETPLACE_PATH_SUFFIX))
}

fn is_wsl_windows_drive_path(path: &Path) -> bool {
    let mut components = path.components();
    if components.next() != Some(Component::RootDir) {
        return false;
    }
    if components.next().and_then(|part| part.as_os_str().to_str()) != Some("mnt") {
        return false;
    }
    let Some(drive) = components.next().and_then(|part| part.as_os_str().to_str()) else {
        return false;
    };
    drive.len() == 1 && drive.as_bytes()[0].is_ascii_alphabetic()
}

#[cfg(test)]
#[path = "discoverable_tests.rs"]
mod tests;
