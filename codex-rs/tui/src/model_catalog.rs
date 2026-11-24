//! Cache models by provider within one backend and account. Collaboration modes are shared.

use crate::backend_id::BackendId;
use codex_protocol::config_types::CollaborationModeMask;
use codex_protocol::openai_models::ModelPreset;
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Held during the config RPC. A confirmed or possible switch makes this backend's untagged model list
/// unsafe to apply for the rest of this TUI session, including after reconnecting.
pub(crate) type ProviderWrite = Arc<Mutex<bool>>;

pub(crate) const LUNA_RESERVE_MODEL: &str = "gpt-reserve";
pub(crate) const LUNA_MODEL: &str = "gpt-6-luna";

pub(crate) fn model_display_name(model: &str) -> &str {
    if model.eq_ignore_ascii_case(LUNA_RESERVE_MODEL) {
        "Luna Reserve"
    } else {
        model
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ModelCatalog {
    pub(crate) backend: Option<BackendId>,
    pub(crate) provider_write: ProviderWrite,
    // None keeps Cloud and unsupported or unidentified server catalogs unscoped.
    provider: Option<String>,
    default_provider: Option<String>,
    models_by_provider: HashMap<Option<String>, Arc<[ModelPreset]>>,
    pub(crate) collaboration_modes: Vec<CollaborationModeMask>,
}

impl ModelCatalog {
    pub(crate) fn new(models: Vec<ModelPreset>, provider: Option<String>) -> Self {
        Self {
            default_provider: provider.clone(),
            models_by_provider: HashMap::from([(provider.clone(), models.into())]),
            provider,
            backend: None,
            provider_write: ProviderWrite::default(),
            collaboration_modes: Vec::new(),
        }
    }

    pub(crate) fn models(&self) -> &[ModelPreset] {
        self.models_for(&self.provider)
    }

    pub(crate) fn models_mut(&mut self) -> &mut [ModelPreset] {
        self.models_by_provider
            .get_mut(&self.provider)
            .map(Arc::make_mut)
            .unwrap_or_default()
    }

    pub(crate) fn set_models(&mut self, models: Vec<ModelPreset>) {
        self.models_by_provider
            .insert(self.provider.clone(), models.into());
    }

    pub(crate) fn provider(&self) -> Option<&str> {
        self.provider.as_deref()
    }

    pub(crate) fn select_provider(&mut self, provider: String) {
        self.provider = Some(provider);
    }

    pub(crate) fn is_default_provider(&self) -> bool {
        self.provider == self.default_provider
    }

    pub(crate) fn default_models(&self) -> &[ModelPreset] {
        self.models_for(&self.default_provider)
    }

    fn models_for(&self, provider: &Option<String>) -> &[ModelPreset] {
        self.models_by_provider
            .get(provider)
            .map(AsRef::as_ref)
            .unwrap_or_default()
    }

    pub(crate) fn clear_models(&mut self) {
        self.models_by_provider.clear();
    }

    pub(crate) fn for_backend(
        mut self,
        backend: BackendId,
        writes: &mut HashMap<BackendId, ProviderWrite>,
    ) -> Self {
        self.backend = Some(backend);
        self.provider_write = writes.entry(backend).or_default().clone();
        self
    }

    pub(crate) fn provider_may_have_changed(&self) -> bool {
        self.provider_write
            .try_lock()
            .map_or(true, |changed| *changed)
    }

    pub(crate) fn with_collaboration_modes(mut self, modes: Vec<CollaborationModeMask>) -> Self {
        self.collaboration_modes = modes;
        self
    }

    pub(crate) fn try_list_models(&self) -> Result<Vec<ModelPreset>, Infallible> {
        Ok(self.models().to_vec())
    }

    pub(crate) fn display_name<'a>(&'a self, model: &'a str) -> &'a str {
        self.models()
            .iter()
            .find(|preset| preset.model == model)
            .map(|preset| preset.display_name.as_str())
            .unwrap_or_else(|| model_display_name(model))
    }
}
