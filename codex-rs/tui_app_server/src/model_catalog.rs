use codex_models::CollaborationModesConfig;
use codex_models::builtin_collaboration_mode_presets;
use codex_protocol::config_types::CollaborationModeMask;
use codex_protocol::openai_models::ModelPreset;
use std::convert::Infallible;

#[derive(Debug, Clone)]
pub(crate) struct ModelCatalog {
    models: Vec<ModelPreset>,
    collaboration_modes_config: CollaborationModesConfig,
}

impl ModelCatalog {
    pub(crate) fn new(
        models: Vec<ModelPreset>,
        collaboration_modes_config: CollaborationModesConfig,
    ) -> Self {
        Self {
            models,
            collaboration_modes_config,
        }
    }

    pub(crate) fn try_list_models(&self) -> Result<Vec<ModelPreset>, Infallible> {
        Ok(self.models.clone())
    }

    pub(crate) fn list_collaboration_modes(&self) -> Vec<CollaborationModeMask> {
        builtin_collaboration_mode_presets(self.collaboration_modes_config)
    }
}
