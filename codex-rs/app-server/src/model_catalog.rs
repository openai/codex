//! Loads the model catalog for the current effective provider configuration.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use codex_config::ConfigLayerStack;
use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_features::Feature;
use codex_model_provider_info::OPENAI_PROVIDER_ID;
use codex_models_manager::ModelOrigin;
use codex_models_manager::manager::RefreshStrategy;
use codex_models_manager::manager::SharedModelsManager;
use codex_protocol::openai_models::ModelPreset;
use codex_utils_absolute_path::AbsolutePathBuf;
use tokio::sync::Mutex;

use crate::config_manager::ConfigManager;
use crate::config_manager::StartupConfig;

/// Refreshes catalog routing without changing the routes retained by existing threads.
pub(crate) struct ModelCatalog {
    config_manager: ConfigManager,
    thread_manager: Arc<ThreadManager>,
    startup_cwd: Option<PathBuf>,
    state: Mutex<CatalogState>,
}

struct CatalogState {
    needs_reload: bool,
    config: Arc<Config>,
    models_manager: SharedModelsManager,
}

impl ModelCatalog {
    pub(crate) fn new(
        config_manager: ConfigManager,
        startup_cwd: Option<PathBuf>,
        config: Arc<Config>,
        thread_manager: Arc<ThreadManager>,
    ) -> Self {
        let models_manager = thread_manager.get_models_manager();
        Self {
            config_manager,
            thread_manager,
            startup_cwd,
            state: Mutex::new(CatalogState {
                needs_reload: false,
                config,
                models_manager,
            }),
        }
    }

    /// Loads the same startup context used to reload the catalog after account setup.
    pub(crate) async fn load_startup_config_layers(&self) -> std::io::Result<ConfigLayerStack> {
        let cwd = self
            .startup_cwd
            .as_deref()
            .map(AbsolutePathBuf::from_absolute_path)
            .transpose()?;
        self.config_manager.load_config_layers(cwd).await
    }

    /// Reloads catalog configuration on the next read.
    pub(crate) async fn invalidate(&self) {
        self.state.lock().await.needs_reload = true;
    }

    pub(crate) async fn invalidate_if_current_provider(&self, provider_id: &str) {
        let mut state = self.state.lock().await;
        if state.config.model_provider_id == provider_id {
            state.needs_reload = true;
        }
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "catalog reloads are serialized with account invalidation; model fetching holds no lock"
    )]
    pub(crate) async fn current(&self) -> std::io::Result<(Arc<Config>, SharedModelsManager)> {
        let mut state = self.state.lock().await;
        if state.needs_reload {
            // Keep the large config-loading future off callers' stacks.
            let config = Box::pin(
                self.config_manager
                    .load_startup_config(self.startup_cwd.clone()),
            )
            .await?;
            let StartupConfig {
                config,
                used_defaults,
            } = config;
            if config.model_provider_id != state.config.model_provider_id
                || config.model_provider != state.config.model_provider
                || config.model_catalog != state.config.model_catalog
                || config.features.enabled(Feature::RuntimeAgentInstances)
                    != state
                        .config
                        .features
                        .enabled(Feature::RuntimeAgentInstances)
                || config.runtime_auth_mode != state.config.runtime_auth_mode
            {
                state.models_manager = self.thread_manager.models_manager_for_config(&config);
            }
            self.thread_manager.set_api_key_model_discovery_enabled(
                config.features.enabled(Feature::ApiKeyModelDiscovery),
            );
            state.config = Arc::new(config);
            // Try again on the next access so a temporary fallback cannot pin the provider.
            state.needs_reload = used_defaults;
        }
        Ok((Arc::clone(&state.config), Arc::clone(&state.models_manager)))
    }

    pub(crate) async fn list_models(
        &self,
        refresh_strategy: RefreshStrategy,
    ) -> std::io::Result<Vec<ModelPreset>> {
        let (config, models_manager) = self.current().await?;
        let models = self
            .list_models_from_snapshot(
                Arc::clone(&config),
                Arc::clone(&models_manager),
                refresh_strategy,
            )
            .await?;
        // The periodic worker also refreshes retained Work catalogs. Picker RPCs use
        // list_models_from_snapshot directly and do not wait on other originators.
        if refresh_strategy == RefreshStrategy::Online {
            models_manager
                .refresh_originator_catalogs(config.http_client_factory())
                .await;
        }
        Ok(models)
    }

    pub(crate) async fn list_models_from_snapshot(
        &self,
        config: Arc<Config>,
        models_manager: SharedModelsManager,
        refresh_strategy: RefreshStrategy,
    ) -> std::io::Result<Vec<ModelPreset>> {
        // Keep the existing policy check even for a warm catalog. A policy
        // edit alone must not make the retained provider silently permissible.
        self.config_manager
            .check_thread_model_provider(&config)
            .await?;
        let catalog = models_manager
            .model_catalog_with_origins(refresh_strategy, config.http_client_factory())
            .await;
        // The first-party catalog supplies picker rows in display order. Bundled,
        // configured and third-party catalogs rely on the manager's priority order.
        let use_catalog_order = config.model_provider_id == OPENAI_PROVIDER_ID
            && (config.model_provider.supports_codex_backend_routes()
                || config.model_provider.model_catalog_url.is_some())
            && catalog
                .iter()
                .all(|entry| entry.origin == ModelOrigin::Provider);
        let positions = use_catalog_order.then(|| {
            catalog
                .iter()
                .enumerate()
                .map(|(index, entry)| (entry.slug.clone(), index))
                .collect::<HashMap<_, _>>()
        });
        let models = catalog
            .into_iter()
            .map(|entry| Arc::unwrap_or_clone(entry.model))
            .collect();
        let mut presets = models_manager.build_available_models(models);
        if let Some(positions) = positions {
            // The manager has filtered the models and marked the default using priority.
            presets.sort_by_key(|preset| positions[&preset.model]);
        }
        Ok(presets)
    }
}
