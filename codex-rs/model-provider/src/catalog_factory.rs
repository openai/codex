//! Host catalog construction from the provider and credentials selected for a thread.

use crate::SharedModelProvider;
use codex_models_manager::manager::ModelCatalogSettings;
use codex_models_manager::manager::SharedModelsManager;
use codex_protocol::openai_models::ModelsResponse;

/// Builds catalogs for the selected provider while retaining the host's cache policy.
/// Provider-backed implementations must use the supplied provider, which carries the
/// current credential owner and runtime mode. Hosts may also supply independent metadata.
pub trait ModelCatalogFactory: Send + Sync {
    fn build(
        &self,
        provider: SharedModelProvider,
        config_model_catalog: Option<ModelsResponse>,
        api_key_model_discovery_default: bool,
    ) -> SharedModelsManager;

    /// Publish a live discovery setting to existing and future catalogs.
    /// Sources that do not perform API-key discovery may ignore it.
    fn set_api_key_model_discovery_enabled(&self, _enabled: bool) {}
}

impl ModelCatalogFactory for ModelCatalogSettings {
    fn build(
        &self,
        provider: SharedModelProvider,
        config_model_catalog: Option<ModelsResponse>,
        api_key_model_discovery_default: bool,
    ) -> SharedModelsManager {
        let mut settings = self.clone();
        settings.api_key_model_discovery_default = api_key_model_discovery_default;
        provider.models_manager_with_settings(config_model_catalog, settings)
    }

    fn set_api_key_model_discovery_enabled(&self, enabled: bool) {
        ModelCatalogSettings::set_api_key_model_discovery_enabled(self, enabled);
    }
}
