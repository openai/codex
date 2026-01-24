//! Skill injection.

use crate::error::Result;
use crate::loader::PluginSkill;
use std::path::PathBuf;

/// Injected skill ready for SkillsManager.
#[derive(Debug, Clone)]
pub struct InjectedSkill {
    /// Skill name.
    pub name: String,
    /// Skill description.
    pub description: String,
    /// Short description (optional).
    pub short_description: Option<String>,
    /// Path to SKILL.md file.
    pub path: PathBuf,
    /// Scope (always Plugin for injected skills).
    pub scope: SkillScope,
    /// Source plugin ID.
    pub source_plugin: String,
}

/// Skill scope for injected skills.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillScope {
    /// Plugin-provided skill.
    Plugin,
}

/// Convert a plugin skill to injectable format.
pub fn convert_skill(skill: &PluginSkill) -> Result<InjectedSkill> {
    Ok(InjectedSkill {
        name: skill.name.clone(),
        description: skill.description.clone(),
        short_description: None,
        path: skill.path.clone(),
        scope: SkillScope::Plugin,
        source_plugin: skill.source_plugin.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_skill() {
        let plugin_skill = PluginSkill {
            name: "test-skill".to_string(),
            description: "A test skill".to_string(),
            path: PathBuf::from("/path/to/skill.md"),
            source_plugin: "test-plugin".to_string(),
        };

        let injected = convert_skill(&plugin_skill).unwrap();
        assert_eq!(injected.name, "test-skill");
        assert_eq!(injected.scope, SkillScope::Plugin);
    }
}
