//! Loads the model catalog for the current effective provider configuration.

use std::path::PathBuf;
use std::sync::Arc;

use codex_config::ConfigLayerStack;
use codex_core::config::Config;
use codex_features::Feature;
use codex_login::AuthManager;
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
    auth_manager: Arc<AuthManager>,
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
        models_manager: SharedModelsManager,
        auth_manager: Arc<AuthManager>,
    ) -> Self {
        Self {
            config_manager,
            auth_manager,
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

    /// Marks catalog configuration stale after an account transition.
    pub(crate) async fn invalidate(&self) {
        self.state.lock().await.needs_reload = true;
    }

    pub(crate) async fn set_api_key_model_discovery_enabled(&self, enabled: bool) {
        self.state
            .lock()
            .await
            .models_manager
            .set_api_key_model_discovery_enabled(enabled);
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
            {
                state.models_manager =
                    codex_core::build_models_manager(&config, Arc::clone(&self.auth_manager));
            }
            state.models_manager.set_api_key_model_discovery_enabled(
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
        self.list_models_from_snapshot(config, models_manager, refresh_strategy)
            .await
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
        Ok(models_manager
            .list_models(refresh_strategy, config.http_client_factory())
            .await)
    }
}
