use std::collections::HashMap;
use std::collections::HashSet;

use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_plugins::PluginSkillRoot;

use crate::AppConnectorId;
use crate::AppDeclaration;
use crate::PluginCapabilitySummary;
use crate::PluginHookSource;
use crate::app_connector_ids_from_declarations;

const MAX_CAPABILITY_SUMMARY_DESCRIPTION_LEN: usize = 1024;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirstPartyPluginRoot {
    pub plugin_id: String,
    pub plugin_root: AbsolutePathBuf,
}

/// A plugin that was loaded from disk, including merged MCP server definitions.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedPlugin<M> {
    pub config_name: String,
    pub manifest_name: Option<String>,
    pub manifest_description: Option<String>,
    pub is_first_party: bool,
    pub root: AbsolutePathBuf,
    pub enabled: bool,
    pub skill_roots: Vec<AbsolutePathBuf>,
    pub disabled_skill_paths: HashSet<AbsolutePathBuf>,
    pub has_enabled_skills: bool,
    pub mcp_servers: HashMap<String, M>,
    pub apps: Vec<AppDeclaration>,
    pub hook_sources: Vec<PluginHookSource>,
    pub hook_load_warnings: Vec<String>,
    pub error: Option<String>,
}

impl<M> LoadedPlugin<M> {
    pub fn is_active(&self) -> bool {
        self.enabled && self.error.is_none()
    }

    pub fn display_name(&self) -> &str {
        self.manifest_name.as_deref().unwrap_or(&self.config_name)
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
        display_name: plugin.display_name().to_string(),
        description: prompt_safe_plugin_description(plugin.manifest_description.as_deref()),
        has_skills: plugin.has_enabled_skills,
        mcp_server_names,
        app_connector_ids: app_connector_ids_from_declarations(&plugin.apps),
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

/// Runtime view of loaded plugins and their derived capability summaries.
///
/// Callers must apply any runtime capability policies before constructing this outcome.
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

    pub fn effective_plugin_skill_roots(&self) -> Vec<PluginSkillRoot> {
        let mut skill_roots = Vec::new();
        let mut seen_paths = HashSet::new();
        for plugin in self.plugins.iter().filter(|plugin| plugin.is_active()) {
            for path in &plugin.skill_roots {
                if seen_paths.insert(path.clone()) {
                    skill_roots.push(PluginSkillRoot {
                        path: path.clone(),
                        plugin_id: plugin.config_name.clone(),
                        plugin_root: plugin.root.clone(),
                    });
                }
            }
        }

        skill_roots.sort_unstable_by(|a, b| a.path.cmp(&b.path));
        skill_roots
    }

    pub fn effective_first_party_plugin_roots(&self) -> Vec<FirstPartyPluginRoot> {
        let mut plugin_roots = Vec::new();
        let mut seen_paths = HashSet::new();
        for plugin in self
            .plugins
            .iter()
            .filter(|plugin| plugin.is_active() && plugin.is_first_party)
        {
            if seen_paths.insert(plugin.root.clone()) {
                plugin_roots.push(FirstPartyPluginRoot {
                    plugin_id: plugin.config_name.clone(),
                    plugin_root: plugin.root.clone(),
                });
            }
        }

        plugin_roots.sort_unstable_by(|a, b| a.plugin_root.cmp(&b.plugin_root));
        plugin_roots
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
        app_connector_ids_from_declarations(
            self.plugins
                .iter()
                .filter(|plugin| plugin.is_active())
                .flat_map(|plugin| plugin.apps.iter()),
        )
    }

    pub fn effective_plugin_hook_sources(&self) -> Vec<PluginHookSource> {
        self.plugins
            .iter()
            .filter(|plugin| plugin.is_active())
            .flat_map(|plugin| plugin.hook_sources.iter().cloned())
            .collect()
    }

    pub fn effective_plugin_hook_warnings(&self) -> Vec<String> {
        self.plugins
            .iter()
            .filter(|plugin| plugin.is_active())
            .flat_map(|plugin| plugin.hook_load_warnings.iter().cloned())
            .collect()
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

    fn effective_plugin_skill_roots(&self) -> Vec<PluginSkillRoot>;
}

impl<M: Clone> EffectiveSkillRoots for PluginLoadOutcome<M> {
    fn effective_skill_roots(&self) -> Vec<AbsolutePathBuf> {
        PluginLoadOutcome::effective_skill_roots(self)
    }

    fn effective_plugin_skill_roots(&self) -> Vec<PluginSkillRoot> {
        PluginLoadOutcome::effective_plugin_skill_roots(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_path(name: &str) -> AbsolutePathBuf {
        AbsolutePathBuf::from_absolute_path_checked(std::env::temp_dir().join(name))
            .expect("absolute temp path")
    }

    fn loaded_plugin(config_name: &str, skill_roots: Vec<AbsolutePathBuf>) -> LoadedPlugin<()> {
        LoadedPlugin {
            config_name: config_name.to_string(),
            manifest_name: None,
            manifest_description: None,
            is_first_party: false,
            root: test_path(config_name),
            enabled: true,
            skill_roots,
            disabled_skill_paths: HashSet::new(),
            has_enabled_skills: true,
            mcp_servers: HashMap::new(),
            apps: Vec::new(),
            hook_sources: Vec::new(),
            hook_load_warnings: Vec::new(),
            error: None,
        }
    }

    #[test]
    fn effective_plugin_skill_roots_preserves_first_plugin_for_shared_root() {
        let shared_root = test_path("shared-skills");
        let outcome = PluginLoadOutcome::from_plugins(vec![
            loaded_plugin("zeta@test", vec![shared_root.clone()]),
            loaded_plugin("alpha@test", vec![shared_root.clone()]),
        ]);

        assert_eq!(
            outcome.effective_plugin_skill_roots(),
            vec![PluginSkillRoot {
                path: shared_root,
                plugin_id: "zeta@test".to_string(),
                plugin_root: test_path("zeta@test"),
            }]
        );
    }

    #[test]
    fn effective_first_party_plugin_roots_only_includes_active_openai_plugins() {
        let shared_root = test_path("shared-plugin");
        let mut first_party = loaded_plugin("first@test", Vec::new());
        first_party.root = shared_root.clone();
        first_party.is_first_party = true;
        let mut duplicate = loaded_plugin("duplicate@test", Vec::new());
        duplicate.root = shared_root.clone();
        duplicate.is_first_party = true;
        let third_party = loaded_plugin("third@test", Vec::new());
        let mut disabled = loaded_plugin("disabled@test", Vec::new());
        disabled.is_first_party = true;
        disabled.enabled = false;
        let mut broken = loaded_plugin("broken@test", Vec::new());
        broken.is_first_party = true;
        broken.error = Some("broken".to_string());

        let outcome = PluginLoadOutcome::from_plugins(vec![
            first_party,
            duplicate,
            third_party,
            disabled,
            broken,
        ]);

        assert_eq!(
            outcome.effective_first_party_plugin_roots(),
            vec![FirstPartyPluginRoot {
                plugin_id: "first@test".to_string(),
                plugin_root: shared_root,
            }]
        );
    }
}
