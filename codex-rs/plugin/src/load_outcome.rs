use std::collections::HashMap;
use std::collections::HashSet;

use codex_utils_absolute_path::AbsolutePathBuf;

use crate::AppConnectorId;
use crate::PluginCapabilitySummary;

const MAX_CAPABILITY_SUMMARY_DESCRIPTION_LEN: usize = 1024;

/// A plugin that was loaded from disk, including merged MCP server definitions.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedPlugin<M> {
    pub config_name: String,
    pub manifest_name: Option<String>,
    pub manifest_description: Option<String>,
    pub root: AbsolutePathBuf,
    pub enabled: bool,
    pub skill_roots: Vec<AbsolutePathBuf>,
    pub disabled_skill_paths: HashSet<AbsolutePathBuf>,
    pub has_enabled_skills: bool,
    pub mcp_servers: HashMap<String, M>,
    pub apps: Vec<AppConnectorId>,
    pub error: Option<String>,
}

impl<M> LoadedPlugin<M> {
    pub fn is_active(&self) -> bool {
        self.enabled && self.error.is_none()
    }
}

fn plugin_capability_summary_from_loaded<M>(
    plugin: &LoadedPlugin<M>,
) -> Option<PluginCapabilitySummary> {
    if !plugin.is_active() {
        return None;
    }

    let mut mcp_server_names: Vec<String> = plugin.mcp_servers.keys().cloned().collect();
    mcp_server_names.sort_unstable();

    let summary = PluginCapabilitySummary {
        config_name: plugin.config_name.clone(),
        display_name: plugin
            .manifest_name
            .clone()
            .unwrap_or_else(|| plugin.config_name.clone()),
        description: prompt_safe_plugin_description(plugin.manifest_description.as_deref()),
        has_skills: plugin.has_enabled_skills,
        mcp_server_names,
        app_connector_ids: plugin.apps.clone(),
    };

    (summary.has_skills
        || !summary.mcp_server_names.is_empty()
        || !summary.app_connector_ids.is_empty())
    .then_some(summary)
}

/// Normalizes plugin descriptions for inclusion in model-facing capability summaries.
pub fn prompt_safe_plugin_description(description: Option<&str>) -> Option<String> {
    let description = description?
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if description.is_empty() {
        return None;
    }

    Some(
        description
            .chars()
            .take(MAX_CAPABILITY_SUMMARY_DESCRIPTION_LEN)
            .collect(),
    )
}

/// Outcome of loading configured plugins (skills roots, MCP, apps, errors).
#[derive(Debug, Clone, PartialEq)]
pub struct PluginLoadOutcome<M> {
    plugins: Vec<LoadedPlugin<M>>,
    capability_summaries: Vec<PluginCapabilitySummary>,
}

impl<M: Clone> Default for PluginLoadOutcome<M> {
    fn default() -> Self {
        Self::from_plugins(Vec::new())
    }
}

impl<M: Clone> PluginLoadOutcome<M> {
    pub fn from_plugins(plugins: Vec<LoadedPlugin<M>>) -> Self {
        let capability_summaries = plugins
            .iter()
            .filter_map(plugin_capability_summary_from_loaded)
            .collect::<Vec<_>>();
        Self {
            plugins,
            capability_summaries,
        }
    }

    pub fn effective_skill_roots(&self) -> Vec<AbsolutePathBuf> {
        let mut skill_roots: Vec<AbsolutePathBuf> = self
            .plugins
            .iter()
            .filter(|plugin| plugin.is_active())
            .flat_map(|plugin| plugin.skill_roots.iter().cloned())
            .collect();
        skill_roots.sort_unstable();
        skill_roots.dedup();
        skill_roots
    }

    pub fn effective_mcp_servers(&self) -> HashMap<String, M> {
        let mut mcp_servers = HashMap::new();
        for plugin in self.plugins.iter().filter(|plugin| plugin.is_active()) {
            for (name, config) in &plugin.mcp_servers {
                mcp_servers
                    .entry(name.clone())
                    .or_insert_with(|| config.clone());
            }
        }
        mcp_servers
    }

    pub fn effective_apps(&self) -> Vec<AppConnectorId> {
        let mut apps = Vec::new();
        let mut seen_connector_ids = HashSet::new();

        for plugin in self.plugins.iter().filter(|plugin| plugin.is_active()) {
            for connector_id in &plugin.apps {
                if seen_connector_ids.insert(connector_id.clone()) {
                    apps.push(connector_id.clone());
                }
            }
        }

        apps
    }

    pub fn capability_summaries(&self) -> &[PluginCapabilitySummary] {
        &self.capability_summaries
    }

    pub fn plugins(&self) -> &[LoadedPlugin<M>] {
        &self.plugins
    }
}

/// Implemented by [`PluginLoadOutcome`] so callers (e.g. skills) can depend on `codex-plugin`
/// without naming the MCP config type parameter.
pub trait EffectiveSkillRoots {
    fn effective_skill_roots(&self) -> Vec<AbsolutePathBuf>;
}

impl<M: Clone> EffectiveSkillRoots for PluginLoadOutcome<M> {
    fn effective_skill_roots(&self) -> Vec<AbsolutePathBuf> {
        PluginLoadOutcome::effective_skill_roots(self)
    }
}
