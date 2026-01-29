//! Plugin contribution types.
//!
//! Plugins can contribute various types of extensions to cocode:
//! - Skills (slash commands)
//! - Hooks (lifecycle interceptors)
//! - Agents (specialized subagents)

use cocode_hooks::HookDefinition;
use cocode_skill::SkillPromptCommand;
use serde::{Deserialize, Serialize};

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

    /// Paths to agent configuration files.
    #[serde(default)]
    pub agents: Vec<String>,
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

    /// An agent contribution (placeholder for future implementation).
    Agent {
        /// Agent name.
        name: String,
        /// The plugin that contributed this agent.
        plugin_name: String,
    },
}

impl PluginContribution {
    /// Get the name of this contribution.
    pub fn name(&self) -> &str {
        match self {
            Self::Skill { skill, .. } => &skill.name,
            Self::Hook { hook, .. } => &hook.name,
            Self::Agent { name, .. } => name,
        }
    }

    /// Get the plugin that contributed this.
    pub fn plugin_name(&self) -> &str {
        match self {
            Self::Skill { plugin_name, .. } => plugin_name,
            Self::Hook { plugin_name, .. } => plugin_name,
            Self::Agent { plugin_name, .. } => plugin_name,
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
    }

    #[test]
    fn test_contribution_skill() {
        let skill = SkillPromptCommand {
            name: "test".to_string(),
            description: "Test skill".to_string(),
            prompt: "Do something".to_string(),
            allowed_tools: None,
        };

        let contrib = PluginContribution::Skill {
            skill,
            plugin_name: "my-plugin".to_string(),
        };

        assert_eq!(contrib.name(), "test");
        assert_eq!(contrib.plugin_name(), "my-plugin");
        assert!(contrib.is_skill());
        assert!(!contrib.is_hook());
    }

    #[test]
    fn test_contributions_serialize() {
        let contrib = PluginContributions {
            skills: vec!["skills/".to_string()],
            hooks: vec!["hooks.toml".to_string()],
            agents: vec![],
        };

        let toml = toml::to_string(&contrib).expect("serialize");
        assert!(toml.contains("skills"));
        assert!(toml.contains("hooks"));
    }
}
