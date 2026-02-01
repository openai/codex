//! Plugin integration with runtime components.
//!
//! This module provides the entry point for integrating plugins with the
//! session runtime (SkillManager, HookRegistry, SubagentManager).

use std::path::PathBuf;

use cocode_hooks::HookRegistry;
use cocode_skill::SkillManager;
use cocode_subagent::SubagentManager;
use tracing::info;

use crate::loader::load_plugins_from_roots;
use crate::registry::PluginRegistry;
use crate::scope::PluginScope;

/// Configuration for plugin integration.
#[derive(Debug, Clone, Default)]
pub struct PluginIntegrationConfig {
    /// Directory for managed (system-installed) plugins.
    pub managed_dir: Option<PathBuf>,

    /// Directory for user-global plugins.
    pub user_dir: Option<PathBuf>,

    /// Directory for project-local plugins.
    pub project_dir: Option<PathBuf>,
}

impl PluginIntegrationConfig {
    /// Create a new configuration with default directories.
    ///
    /// - User directory: `~/.config/cocode/plugins/`
    /// - Project directory: `.cocode/plugins/`
    pub fn with_defaults(project_root: Option<&std::path::Path>) -> Self {
        let user_dir = dirs::config_dir().map(|d| d.join("cocode").join("plugins"));

        let project_dir = project_root.map(|p| p.join(".cocode").join("plugins"));

        Self {
            managed_dir: None,
            user_dir,
            project_dir,
        }
    }

    /// Set the managed plugin directory.
    pub fn with_managed_dir(mut self, dir: PathBuf) -> Self {
        self.managed_dir = Some(dir);
        self
    }

    /// Set the user plugin directory.
    pub fn with_user_dir(mut self, dir: PathBuf) -> Self {
        self.user_dir = Some(dir);
        self
    }

    /// Set the project plugin directory.
    pub fn with_project_dir(mut self, dir: PathBuf) -> Self {
        self.project_dir = Some(dir);
        self
    }

    /// Build the list of plugin roots with their scopes.
    fn plugin_roots(&self) -> Vec<(PathBuf, PluginScope)> {
        let mut roots = Vec::new();

        if let Some(dir) = &self.managed_dir {
            roots.push((dir.clone(), PluginScope::Managed));
        }
        if let Some(dir) = &self.user_dir {
            roots.push((dir.clone(), PluginScope::User));
        }
        if let Some(dir) = &self.project_dir {
            roots.push((dir.clone(), PluginScope::Project));
        }

        roots
    }
}

/// Integrate plugins with runtime components.
///
/// This function:
/// 1. Discovers plugins from configured directories
/// 2. Loads all plugins and their contributions
/// 3. Applies skills to the skill manager
/// 4. Applies hooks to the hook registry
/// 5. Applies agents to the subagent manager
///
/// Returns the populated plugin registry.
pub fn integrate_plugins(
    config: &PluginIntegrationConfig,
    skill_manager: &mut SkillManager,
    hook_registry: &mut HookRegistry,
    subagent_manager: &mut SubagentManager,
) -> PluginRegistry {
    let roots = config.plugin_roots();

    info!(
        roots = roots.len(),
        managed = config.managed_dir.is_some(),
        user = config.user_dir.is_some(),
        project = config.project_dir.is_some(),
        "Integrating plugins"
    );

    // Load all plugins
    let plugins = load_plugins_from_roots(&roots);

    // Build registry
    let mut registry = PluginRegistry::new();
    registry.register_all(plugins);

    // Apply contributions
    registry.apply_skills_to(skill_manager);
    registry.apply_hooks_to(hook_registry);
    registry.apply_agents_to(subagent_manager);

    info!(
        plugins = registry.len(),
        skills = registry.skill_contributions().len(),
        hooks = registry.hook_contributions().len(),
        agents = registry.agent_contributions().len(),
        commands = registry.command_contributions().len(),
        mcp_servers = registry.mcp_server_contributions().len(),
        "Plugin integration complete"
    );

    registry
}

/// Load plugins without applying to runtime components.
///
/// Use this when you need to inspect plugins before integration.
pub fn load_plugins(config: &PluginIntegrationConfig) -> PluginRegistry {
    let roots = config.plugin_roots();
    let plugins = load_plugins_from_roots(&roots);

    let mut registry = PluginRegistry::new();
    registry.register_all(plugins);

    registry
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_test_plugin(dir: &std::path::Path) {
        fs::create_dir_all(dir).expect("mkdir");
        fs::write(
            dir.join("PLUGIN.toml"),
            r#"
[plugin]
name = "test-plugin"
version = "1.0.0"
description = "A test plugin"

[contributions]
skills = ["skills/"]
agents = ["agents/"]
"#,
        )
        .expect("write");

        // Create a skill
        let skills_dir = dir.join("skills").join("test-skill");
        fs::create_dir_all(&skills_dir).expect("mkdir");
        fs::write(
            skills_dir.join("SKILL.toml"),
            r#"
name = "test-skill"
description = "A test skill"
prompt_inline = "Do something"
"#,
        )
        .expect("write");

        // Create an agent
        let agents_dir = dir.join("agents").join("test-agent");
        fs::create_dir_all(&agents_dir).expect("mkdir");
        fs::write(
            agents_dir.join("AGENT.toml"),
            r#"
name = "test-agent"
description = "A test agent"
agent_type = "test-agent"
tools = ["Read"]
"#,
        )
        .expect("write");
    }

    #[test]
    fn test_integration_config_defaults() {
        let config = PluginIntegrationConfig::default();
        assert!(config.managed_dir.is_none());
        assert!(config.user_dir.is_none());
        assert!(config.project_dir.is_none());
    }

    #[test]
    fn test_integration_config_with_project() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let config = PluginIntegrationConfig::with_defaults(Some(tmp.path()));

        assert!(config.managed_dir.is_none());
        assert!(config.user_dir.is_some());
        assert!(config.project_dir.is_some());

        let project_dir = config.project_dir.unwrap();
        assert!(project_dir.ends_with(".cocode/plugins"));
    }

    #[test]
    fn test_load_plugins() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let plugins_dir = tmp.path().join("plugins");
        let plugin_dir = plugins_dir.join("test-plugin");
        setup_test_plugin(&plugin_dir);

        let config = PluginIntegrationConfig::default().with_project_dir(plugins_dir);

        let registry = load_plugins(&config);
        assert_eq!(registry.len(), 1);
        assert!(registry.has("test-plugin"));
    }

    #[test]
    fn test_integrate_plugins() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let plugins_dir = tmp.path().join("plugins");
        let plugin_dir = plugins_dir.join("test-plugin");
        setup_test_plugin(&plugin_dir);

        let config = PluginIntegrationConfig::default().with_project_dir(plugins_dir);

        let mut skill_manager = SkillManager::new();
        let mut hook_registry = HookRegistry::default();
        let mut subagent_manager = SubagentManager::new();

        let registry = integrate_plugins(
            &config,
            &mut skill_manager,
            &mut hook_registry,
            &mut subagent_manager,
        );

        assert_eq!(registry.len(), 1);
        assert_eq!(registry.skill_contributions().len(), 1);
        assert_eq!(registry.agent_contributions().len(), 1);

        // Verify skill was applied
        assert!(skill_manager.get("test-skill").is_some());
    }

    #[test]
    fn test_integrate_empty_config() {
        let config = PluginIntegrationConfig::default();

        let mut skill_manager = SkillManager::new();
        let mut hook_registry = HookRegistry::default();
        let mut subagent_manager = SubagentManager::new();

        let registry = integrate_plugins(
            &config,
            &mut skill_manager,
            &mut hook_registry,
            &mut subagent_manager,
        );

        assert!(registry.is_empty());
    }
}
