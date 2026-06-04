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
    last_catalog_body: Mutex<Option<String>>,
}

impl SkillsThreadState {
    pub(crate) fn new(config: SkillsExtensionConfig) -> Self {
        Self {
            config: Mutex::new(config),
            last_catalog_body: Mutex::new(None),
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

    pub(crate) fn mark_catalog_emitted_if_changed(&self, body: &str) -> bool {
        let mut last_catalog_body = self
            .last_catalog_body
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if last_catalog_body.as_deref() == Some(body) {
            return false;
        }

        *last_catalog_body = Some(body.to_string());
        true
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SkillsTurnState {
    pub(crate) catalog: SkillCatalog,
    pub(crate) selected_entries: Vec<SkillCatalogEntry>,
    pub(crate) warnings: Vec<String>,
    pub(crate) main_prompts_injected: bool,
}
