use codex_core::config::Config;
use std::sync::Mutex;

use crate::catalog::SkillCatalog;
use crate::catalog::SkillCatalogEntry;

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

#[derive(Debug)]
pub(crate) struct SkillsThreadState {
    config: Mutex<SkillsExtensionConfig>,
}

impl SkillsThreadState {
    pub(crate) fn new(config: SkillsExtensionConfig) -> Self {
        Self {
            config: Mutex::new(config),
        }
    }

    pub(crate) fn config(&self) -> SkillsExtensionConfig {
        self.config
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub(crate) fn set_config(&self, config: SkillsExtensionConfig) {
        *self
            .config
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = config;
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SkillsTurnState {
    pub(crate) catalog: SkillCatalog,
    pub(crate) selected_entries: Vec<SkillCatalogEntry>,
    pub(crate) warnings: Vec<String>,
    pub(crate) main_prompts_injected: bool,
}
