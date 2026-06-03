use codex_core::config::Config;

use crate::catalog::SkillCatalog;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SkillsExtensionConfig {
    pub(crate) include_instructions: bool,
    pub(crate) bundled_skills_enabled: bool,
}

impl SkillsExtensionConfig {
    pub(crate) fn from_config(config: &Config) -> Self {
        Self {
            include_instructions: config.include_skill_instructions,
            bundled_skills_enabled: config.bundled_skills_enabled(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SkillsTurnState {
    pub(crate) catalog: SkillCatalog,
    pub(crate) entrypoints_injected: bool,
}
