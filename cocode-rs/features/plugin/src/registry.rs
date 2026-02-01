//! Plugin registry for runtime plugin management.
//!
//! The registry tracks loaded plugins and provides access to their contributions.

use crate::command::PluginCommand;
use crate::contribution::PluginContribution;
#[cfg(test)]
use crate::error::PluginError;
use crate::error::Result;
use crate::error::plugin_error::AlreadyRegisteredSnafu;
use crate::loader::LoadedPlugin;
use crate::mcp::McpServerConfig;
use crate::scope::PluginScope;

use cocode_hooks::HookDefinition;
use cocode_hooks::HookRegistry;
use cocode_skill::SkillManager;
use cocode_skill::SkillPromptCommand;
use cocode_subagent::AgentDefinition;
use cocode_subagent::SubagentManager;
use std::collections::HashMap;
use tracing::debug;
use tracing::info;

/// Registry for managing loaded plugins.
///
/// The registry tracks plugins and provides access to their contributions.
/// It can also integrate with the skill manager and hook registry.
#[derive(Default)]
pub struct PluginRegistry {
    /// Loaded plugins indexed by name.
    plugins: HashMap<String, LoadedPlugin>,
}

impl PluginRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a loaded plugin.
    ///
    /// Returns an error if a plugin with the same name is already registered.
    pub fn register(&mut self, plugin: LoadedPlugin) -> Result<()> {
        let name = plugin.name().to_string();

        if self.plugins.contains_key(&name) {
            return Err(AlreadyRegisteredSnafu { name }.build());
        }

        debug!(
            name = %name,
            scope = %plugin.scope,
            contributions = plugin.contributions.len(),
            "Registered plugin"
        );

        self.plugins.insert(name, plugin);
        Ok(())
    }

    /// Register multiple plugins.
    ///
    /// Plugins are registered in order. If a duplicate is found, it is skipped
    /// with a warning.
    pub fn register_all(&mut self, plugins: Vec<LoadedPlugin>) {
        for plugin in plugins {
            if let Err(e) = self.register(plugin) {
                tracing::warn!(error = %e, "Skipping duplicate plugin");
            }
        }
    }

    /// Unregister a plugin by name.
    pub fn unregister(&mut self, name: &str) -> Option<LoadedPlugin> {
        self.plugins.remove(name)
    }

    /// Get a plugin by name.
    pub fn get(&self, name: &str) -> Option<&LoadedPlugin> {
        self.plugins.get(name)
    }

    /// Check if a plugin is registered.
    pub fn has(&self, name: &str) -> bool {
        self.plugins.contains_key(name)
    }

    /// Get all plugin names.
    pub fn names(&self) -> Vec<&str> {
        let mut names: Vec<_> = self.plugins.keys().map(String::as_str).collect();
        names.sort();
        names
    }

    /// Get all plugins.
    pub fn all(&self) -> impl Iterator<Item = &LoadedPlugin> {
        self.plugins.values()
    }

    /// Get the number of registered plugins.
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    /// Get all skill contributions.
    pub fn skill_contributions(&self) -> Vec<(&SkillPromptCommand, &str)> {
        self.plugins
            .values()
            .flat_map(|plugin| {
                plugin.contributions.iter().filter_map(|c| {
                    if let PluginContribution::Skill { skill, plugin_name } = c {
                        Some((skill, plugin_name.as_str()))
                    } else {
                        None
                    }
                })
            })
            .collect()
    }

    /// Get all hook contributions.
    pub fn hook_contributions(&self) -> Vec<(&HookDefinition, &str)> {
        self.plugins
            .values()
            .flat_map(|plugin| {
                plugin.contributions.iter().filter_map(|c| {
                    if let PluginContribution::Hook { hook, plugin_name } = c {
                        Some((hook, plugin_name.as_str()))
                    } else {
                        None
                    }
                })
            })
            .collect()
    }

    /// Get all agent contributions.
    pub fn agent_contributions(&self) -> Vec<(&AgentDefinition, &str)> {
        self.plugins
            .values()
            .flat_map(|plugin| {
                plugin.contributions.iter().filter_map(|c| {
                    if let PluginContribution::Agent {
                        definition,
                        plugin_name,
                    } = c
                    {
                        Some((definition, plugin_name.as_str()))
                    } else {
                        None
                    }
                })
            })
            .collect()
    }

    /// Get all command contributions.
    pub fn command_contributions(&self) -> Vec<(&PluginCommand, &str)> {
        self.plugins
            .values()
            .flat_map(|plugin| {
                plugin.contributions.iter().filter_map(|c| {
                    if let PluginContribution::Command {
                        command,
                        plugin_name,
                    } = c
                    {
                        Some((command, plugin_name.as_str()))
                    } else {
                        None
                    }
                })
            })
            .collect()
    }

    /// Get all MCP server contributions.
    pub fn mcp_server_contributions(&self) -> Vec<(&McpServerConfig, &str)> {
        self.plugins
            .values()
            .flat_map(|plugin| {
                plugin.contributions.iter().filter_map(|c| {
                    if let PluginContribution::McpServer {
                        config,
                        plugin_name,
                    } = c
                    {
                        Some((config, plugin_name.as_str()))
                    } else {
                        None
                    }
                })
            })
            .collect()
    }

    /// Apply all skill contributions to a skill manager.
    pub fn apply_skills_to(&self, manager: &mut SkillManager) {
        let skills = self.skill_contributions();
        let count = skills.len();

        for (skill, plugin_name) in skills {
            debug!(
                skill = %skill.name,
                plugin = %plugin_name,
                "Applying skill from plugin"
            );
            manager.register(skill.clone());
        }

        if count > 0 {
            info!(count = count, "Applied skills from plugins");
        }
    }

    /// Apply all hook contributions to a hook registry.
    pub fn apply_hooks_to(&self, registry: &mut HookRegistry) {
        let hooks = self.hook_contributions();
        let count = hooks.len();

        for (hook, plugin_name) in hooks {
            debug!(
                hook = %hook.name,
                plugin = %plugin_name,
                "Applying hook from plugin"
            );
            registry.register(hook.clone());
        }

        if count > 0 {
            info!(count = count, "Applied hooks from plugins");
        }
    }

    /// Apply all agent contributions to a subagent manager.
    pub fn apply_agents_to(&self, manager: &mut SubagentManager) {
        let agents = self.agent_contributions();
        let count = agents.len();

        for (definition, plugin_name) in agents {
            debug!(
                agent = %definition.name,
                agent_type = %definition.agent_type,
                plugin = %plugin_name,
                "Applying agent from plugin"
            );
            manager.register_agent_type(definition.clone());
        }

        if count > 0 {
            info!(count = count, "Applied agents from plugins");
        }
    }

    /// Get plugins by scope.
    pub fn by_scope(&self, scope: PluginScope) -> Vec<&LoadedPlugin> {
        self.plugins.values().filter(|p| p.scope == scope).collect()
    }

    /// Clear all registered plugins.
    pub fn clear(&mut self) {
        self.plugins.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contribution::PluginContributions;
    use crate::manifest::PluginManifest;
    use crate::manifest::PluginMetadata;
    use std::path::PathBuf;

    fn make_plugin(name: &str, scope: PluginScope) -> LoadedPlugin {
        LoadedPlugin {
            manifest: PluginManifest {
                plugin: PluginMetadata {
                    name: name.to_string(),
                    version: "1.0.0".to_string(),
                    description: "Test plugin".to_string(),
                    author: None,
                    repository: None,
                    license: None,
                    min_cocode_version: None,
                },
                contributions: PluginContributions::default(),
            },
            path: PathBuf::from(format!("/plugins/{name}")),
            scope,
            contributions: Vec::new(),
        }
    }

    #[test]
    fn test_register_and_get() {
        let mut registry = PluginRegistry::new();
        let plugin = make_plugin("test", PluginScope::User);

        registry.register(plugin).expect("register");

        assert!(registry.has("test"));
        assert!(!registry.has("other"));

        let plugin = registry.get("test").expect("get");
        assert_eq!(plugin.name(), "test");
    }

    #[test]
    fn test_duplicate_registration() {
        let mut registry = PluginRegistry::new();

        registry
            .register(make_plugin("test", PluginScope::User))
            .expect("first");
        let result = registry.register(make_plugin("test", PluginScope::Project));

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            PluginError::AlreadyRegistered { .. }
        ));
    }

    #[test]
    fn test_names() {
        let mut registry = PluginRegistry::new();
        registry
            .register(make_plugin("beta", PluginScope::User))
            .expect("register");
        registry
            .register(make_plugin("alpha", PluginScope::Project))
            .expect("register");

        let names = registry.names();
        assert_eq!(names, vec!["alpha", "beta"]);
    }

    #[test]
    fn test_by_scope() {
        let mut registry = PluginRegistry::new();
        registry
            .register(make_plugin("user1", PluginScope::User))
            .expect("register");
        registry
            .register(make_plugin("user2", PluginScope::User))
            .expect("register");
        registry
            .register(make_plugin("project1", PluginScope::Project))
            .expect("register");

        let user_plugins = registry.by_scope(PluginScope::User);
        assert_eq!(user_plugins.len(), 2);

        let project_plugins = registry.by_scope(PluginScope::Project);
        assert_eq!(project_plugins.len(), 1);
    }

    #[test]
    fn test_unregister() {
        let mut registry = PluginRegistry::new();
        registry
            .register(make_plugin("test", PluginScope::User))
            .expect("register");

        let removed = registry.unregister("test");
        assert!(removed.is_some());
        assert!(!registry.has("test"));
    }
}
