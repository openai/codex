//! Component injection into Codex systems.

mod agents;
mod hooks;
mod mcp;
mod output_styles;
mod skills;

pub use agents::*;
pub use hooks::*;
pub use mcp::*;
pub use output_styles::*;
pub use skills::*;

use crate::loader::LoadedPlugin;
use std::collections::HashMap;
use tracing::info;

/// Plugin injector - orchestrates injection of components into Codex systems.
pub struct PluginInjector;

impl PluginInjector {
    /// Create a new plugin injector.
    pub fn new() -> Self {
        Self
    }

    /// Inject all components from loaded plugins.
    pub fn inject_all(&self, plugins: &[LoadedPlugin]) -> InjectionReport {
        let mut report = InjectionReport::default();

        for plugin in plugins {
            // Skills
            for skill in &plugin.skills {
                if let Ok(converted) = convert_skill(skill) {
                    report.skills.push(converted);
                    report.skills_injected += 1;
                }
            }

            // Agents
            for agent in &plugin.agents {
                if let Ok(converted) = convert_agent(agent) {
                    report.agents.push(converted);
                    report.agents_injected += 1;
                }
            }

            // Hooks
            for hook in &plugin.hooks {
                if let Ok(converted) = convert_hook(hook) {
                    report.hooks.push(converted);
                    report.hooks_injected += 1;
                }
            }

            // MCP Servers
            for server in &plugin.mcp_servers {
                if let Ok(converted) = convert_mcp_server(server) {
                    report.mcp_servers.insert(converted.0, converted.1);
                    report.mcp_servers_injected += 1;
                }
            }

            // Commands
            for cmd in &plugin.commands {
                report.commands.push(InjectedCommand {
                    name: cmd.name.clone(),
                    description: cmd.description.clone(),
                    path: cmd.path.clone(),
                    source_plugin: cmd.source_plugin.clone(),
                });
                report.commands_injected += 1;
            }

            // Output Styles
            for style in &plugin.output_styles {
                if let Ok(converted) = convert_output_style(style) {
                    report.output_styles.push(converted);
                    report.output_styles_injected += 1;
                }
            }
        }

        info!(
            "Injection complete: {} skills, {} agents, {} hooks, {} mcp, {} commands, {} styles",
            report.skills_injected,
            report.agents_injected,
            report.hooks_injected,
            report.mcp_servers_injected,
            report.commands_injected,
            report.output_styles_injected
        );

        report
    }

    /// Get skill metadata ready for injection into SkillsManager.
    pub fn get_skill_metadata(&self, plugins: &[LoadedPlugin]) -> Vec<InjectedSkill> {
        plugins
            .iter()
            .flat_map(|p| p.skills.iter())
            .filter_map(|s| convert_skill(s).ok())
            .collect()
    }

    /// Get agent definitions ready for injection into AgentRegistry.
    pub fn get_agent_definitions(&self, plugins: &[LoadedPlugin]) -> Vec<InjectedAgent> {
        plugins
            .iter()
            .flat_map(|p| p.agents.iter())
            .filter_map(|a| convert_agent(a).ok())
            .collect()
    }

    /// Get hook configs ready for injection into HookRegistry.
    pub fn get_hook_configs(&self, plugins: &[LoadedPlugin]) -> Vec<InjectedHook> {
        plugins
            .iter()
            .flat_map(|p| p.hooks.iter())
            .filter_map(|h| convert_hook(h).ok())
            .collect()
    }

    /// Get MCP server configs for Config merging.
    pub fn get_mcp_server_configs(
        &self,
        plugins: &[LoadedPlugin],
    ) -> HashMap<String, InjectedMcpServer> {
        plugins
            .iter()
            .flat_map(|p| p.mcp_servers.iter())
            .filter_map(|s| convert_mcp_server(s).ok())
            .collect()
    }

    /// Get output style configs for rendering.
    pub fn get_output_styles(&self, plugins: &[LoadedPlugin]) -> Vec<InjectedOutputStyle> {
        plugins
            .iter()
            .flat_map(|p| p.output_styles.iter())
            .filter_map(|s| convert_output_style(s).ok())
            .collect()
    }
}

impl Default for PluginInjector {
    fn default() -> Self {
        Self::new()
    }
}

/// Report of injection results.
#[derive(Debug, Default, Clone)]
pub struct InjectionReport {
    /// Injected skills.
    pub skills: Vec<InjectedSkill>,
    pub skills_injected: i32,

    /// Injected agents.
    pub agents: Vec<InjectedAgent>,
    pub agents_injected: i32,

    /// Injected hooks.
    pub hooks: Vec<InjectedHook>,
    pub hooks_injected: i32,

    /// Injected MCP servers.
    pub mcp_servers: HashMap<String, InjectedMcpServer>,
    pub mcp_servers_injected: i32,

    /// Injected commands.
    pub commands: Vec<InjectedCommand>,
    pub commands_injected: i32,

    /// Injected output styles.
    pub output_styles: Vec<InjectedOutputStyle>,
    pub output_styles_injected: i32,

    /// Errors encountered.
    pub errors: Vec<String>,
}

/// Injected command.
#[derive(Debug, Clone)]
pub struct InjectedCommand {
    pub name: String,
    pub description: String,
    pub path: std::path::PathBuf,
    pub source_plugin: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_injection_report_default() {
        let report = InjectionReport::default();
        assert_eq!(report.skills_injected, 0);
        assert!(report.errors.is_empty());
    }
}
