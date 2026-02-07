//! Plugin contribution types.
//!
//! Plugins can contribute various types of extensions to cocode:
//! - Skills (slash commands)
//! - Hooks (lifecycle interceptors)
//! - Agents (specialized subagents)
//! - Commands (plugin-provided commands)
//! - MCP servers (Model Context Protocol servers)

use cocode_hooks::HookDefinition;
use cocode_skill::SkillPromptCommand;
use cocode_subagent::AgentDefinition;
use serde::Deserialize;
use serde::Serialize;

use crate::command::PluginCommand;
use crate::mcp::McpServerConfig;

/// Contributions declared in a plugin manifest.
///
/// Each field is a list of paths (relative to the plugin directory) that
/// contain contribution definitions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginContributions {
    /// Paths to skill directories (containing SKILL.toml files).
    #[serde(default)]
    pub skills: Vec<String>,

    /// Paths to hook configuration files (TOML).
    #[serde(default)]
    pub hooks: Vec<String>,

    /// Paths to agent directories (containing AGENT.toml files).
    #[serde(default)]
    pub agents: Vec<String>,

    /// Paths to command directories (containing COMMAND.toml files).
    #[serde(default)]
    pub commands: Vec<String>,

    /// Paths to MCP server configuration files.
    #[serde(default)]
    pub mcp_servers: Vec<String>,
}

/// A contribution from a plugin.
///
/// This represents a loaded contribution with its source plugin tracked.
#[derive(Debug, Clone)]
pub enum PluginContribution {
    /// A skill contribution.
    Skill {
        /// The loaded skill.
        skill: SkillPromptCommand,
        /// The plugin that contributed this skill.
        plugin_name: String,
    },

    /// A hook contribution.
    Hook {
        /// The loaded hook definition.
        hook: HookDefinition,
        /// The plugin that contributed this hook.
        plugin_name: String,
    },

    /// An agent contribution.
    Agent {
        /// The loaded agent definition.
        definition: AgentDefinition,
        /// The plugin that contributed this agent.
        plugin_name: String,
    },

    /// A command contribution.
    Command {
        /// The loaded command.
        command: PluginCommand,
        /// The plugin that contributed this command.
        plugin_name: String,
    },

    /// An MCP server contribution.
    McpServer {
        /// The MCP server configuration.
        config: McpServerConfig,
        /// The plugin that contributed this server.
        plugin_name: String,
    },
}

impl PluginContribution {
    /// Get the name of this contribution.
    pub fn name(&self) -> &str {
        match self {
            Self::Skill { skill, .. } => &skill.name,
            Self::Hook { hook, .. } => &hook.name,
            Self::Agent { definition, .. } => &definition.name,
            Self::Command { command, .. } => &command.name,
            Self::McpServer { config, .. } => &config.name,
        }
    }

    /// Get the plugin that contributed this.
    pub fn plugin_name(&self) -> &str {
        match self {
            Self::Skill { plugin_name, .. }
            | Self::Hook { plugin_name, .. }
            | Self::Agent { plugin_name, .. }
            | Self::Command { plugin_name, .. }
            | Self::McpServer { plugin_name, .. } => plugin_name,
        }
    }

    /// Check if this is a skill contribution.
    pub fn is_skill(&self) -> bool {
        matches!(self, Self::Skill { .. })
    }

    /// Check if this is a hook contribution.
    pub fn is_hook(&self) -> bool {
        matches!(self, Self::Hook { .. })
    }

    /// Check if this is an agent contribution.
    pub fn is_agent(&self) -> bool {
        matches!(self, Self::Agent { .. })
    }

    /// Check if this is a command contribution.
    pub fn is_command(&self) -> bool {
        matches!(self, Self::Command { .. })
    }

    /// Check if this is an MCP server contribution.
    pub fn is_mcp_server(&self) -> bool {
        matches!(self, Self::McpServer { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contributions_default() {
        let contrib = PluginContributions::default();
        assert!(contrib.skills.is_empty());
        assert!(contrib.hooks.is_empty());
        assert!(contrib.agents.is_empty());
        assert!(contrib.commands.is_empty());
        assert!(contrib.mcp_servers.is_empty());
    }

    #[test]
    fn test_contribution_skill() {
        let skill = SkillPromptCommand {
            name: "test".to_string(),
            description: "Test skill".to_string(),
            prompt: "Do something".to_string(),
            allowed_tools: None,
            interface: None,
        };

        let contrib = PluginContribution::Skill {
            skill,
            plugin_name: "my-plugin".to_string(),
        };

        assert_eq!(contrib.name(), "test");
        assert_eq!(contrib.plugin_name(), "my-plugin");
        assert!(contrib.is_skill());
        assert!(!contrib.is_hook());
        assert!(!contrib.is_agent());
        assert!(!contrib.is_command());
        assert!(!contrib.is_mcp_server());
    }

    #[test]
    fn test_contribution_agent() {
        let definition = AgentDefinition {
            name: "test-agent".to_string(),
            description: "A test agent".to_string(),
            agent_type: "test-agent".to_string(),
            tools: vec!["Read".to_string()],
            disallowed_tools: vec![],
            identity: None,
            max_turns: None,
            permission_mode: None,
        };

        let contrib = PluginContribution::Agent {
            definition,
            plugin_name: "my-plugin".to_string(),
        };

        assert_eq!(contrib.name(), "test-agent");
        assert_eq!(contrib.plugin_name(), "my-plugin");
        assert!(contrib.is_agent());
        assert!(!contrib.is_skill());
    }

    #[test]
    fn test_contributions_serialize() {
        let contrib = PluginContributions {
            skills: vec!["skills/".to_string()],
            hooks: vec!["hooks.toml".to_string()],
            agents: vec!["agents/".to_string()],
            commands: vec!["commands/".to_string()],
            mcp_servers: vec![],
        };

        let toml_str = toml::to_string(&contrib).expect("serialize");
        assert!(toml_str.contains("skills"));
        assert!(toml_str.contains("hooks"));
        assert!(toml_str.contains("agents"));
        assert!(toml_str.contains("commands"));
    }
}
