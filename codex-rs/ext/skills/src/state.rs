use codex_core::config::Config;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use std::future::Future;
use std::sync::Mutex;
use tokio::sync::OnceCell;

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
    selected_roots: Vec<SelectedCapabilityRoot>,
    remote_catalog: OnceCell<SkillCatalog>,
}

impl SkillsThreadState {
    pub(crate) fn new(
        config: SkillsExtensionConfig,
        selected_roots: Vec<SelectedCapabilityRoot>,
    ) -> Self {
        Self {
            config: Mutex::new(config),
            selected_roots,
            remote_catalog: OnceCell::new(),
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

    pub(crate) fn selected_roots(&self) -> &[SelectedCapabilityRoot] {
        &self.selected_roots
    }

    pub(crate) async fn remote_catalog_snapshot(
        &self,
        initialize: impl Future<Output = SkillCatalog> + Send,
    ) -> SkillCatalog {
        self.remote_catalog.get_or_init(|| initialize).await.clone()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SkillsTurnState {
    pub(crate) catalog: SkillCatalog,
    pub(crate) selected_entries: Vec<SkillCatalogEntry>,
    pub(crate) warnings: Vec<String>,
    pub(crate) main_prompts_injected: bool,
}
