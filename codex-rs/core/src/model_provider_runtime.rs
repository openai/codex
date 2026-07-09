use crate::config::Config;
use arc_swap::ArcSwap;
use codex_login::AuthManager;
use codex_model_provider::ProviderCapabilities;
use codex_model_provider::SharedModelProvider;
use codex_model_provider::create_model_provider;
use codex_model_provider_info::ModelProviderInfo;
use codex_models_manager::manager::SharedModelsManager;
use codex_protocol::openai_models::ModelsResponse;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

/// The config inputs that determine which model manager is valid for a runtime.
#[derive(Debug, Clone, PartialEq)]
struct ModelProviderRuntimeIdentity {
    provider_id: String,
    provider_info: ModelProviderInfo,
    codex_home: PathBuf,
    config_model_catalog: Option<ModelsResponse>,
    auth_revision: u64,
}

impl ModelProviderRuntimeIdentity {
    fn from_config(config: &Config, auth_manager: &AuthManager) -> Self {
        Self {
            provider_id: config.model_provider_id.clone(),
            provider_info: config.model_provider.clone(),
            codex_home: config.codex_home.to_path_buf(),
            config_model_catalog: config.model_catalog.clone(),
            auth_revision: auth_revision(auth_manager),
        }
    }

    fn matches_config(&self, config: &Config, auth_manager: &AuthManager) -> bool {
        self.provider_id == config.model_provider_id
            && self.provider_info == config.model_provider
            && self.codex_home == config.codex_home.to_path_buf()
            && self.config_model_catalog == config.model_catalog
            && self.auth_revision == auth_revision(auth_manager)
    }
}

fn auth_revision(auth_manager: &AuthManager) -> u64 {
    let receiver = auth_manager.auth_change_receiver();
    *receiver.borrow()
}

/// Immutable process-default model-provider state published as one coherent value.
pub(crate) struct ModelProviderRuntimeSnapshot {
    generation: u64,
    identity: ModelProviderRuntimeIdentity,
    provider: SharedModelProvider,
    models_manager: SharedModelsManager,
    capabilities: ProviderCapabilities,
}

impl ModelProviderRuntimeSnapshot {
    fn new(
        generation: u64,
        identity: ModelProviderRuntimeIdentity,
        auth_manager: Arc<AuthManager>,
    ) -> Self {
        let provider = create_model_provider(
            identity.provider_info.clone(),
            Some(Arc::clone(&auth_manager)),
        );
        let models_manager = provider.models_manager(
            identity.codex_home.clone(),
            identity.config_model_catalog.clone(),
        );
        Self::from_provider(generation, identity, provider, models_manager)
    }

    fn from_provider(
        generation: u64,
        identity: ModelProviderRuntimeIdentity,
        provider: SharedModelProvider,
        models_manager: SharedModelsManager,
    ) -> Self {
        let capabilities = provider.capabilities();
        Self {
            generation,
            identity,
            provider,
            models_manager,
            capabilities,
        }
    }

    fn matches_config(&self, config: &Config, auth_manager: &AuthManager) -> bool {
        self.identity.matches_config(config, auth_manager)
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) fn provider_info(&self) -> &ModelProviderInfo {
        &self.identity.provider_info
    }

    pub(crate) fn provider(&self) -> SharedModelProvider {
        Arc::clone(&self.provider)
    }

    pub(crate) fn models_manager(&self) -> SharedModelsManager {
        Arc::clone(&self.models_manager)
    }

    pub(crate) fn apply_to_config(&self, config: &mut Config) {
        config.model_provider_id = self.identity.provider_id.clone();
        config.model_provider = self.identity.provider_info.clone();
        config.model_catalog = self.identity.config_model_catalog.clone();
    }
}

#[derive(Clone)]
pub(crate) enum ModelProviderRuntimeSource {
    RuntimeDefault(Arc<DefaultModelProviderRuntime>),
    Explicit,
}

#[derive(Clone)]
pub(crate) struct InitialModelProviderRuntime {
    pub(crate) source: ModelProviderRuntimeSource,
    pub(crate) snapshot: Arc<ModelProviderRuntimeSnapshot>,
}

/// Owns the atomically replaceable process-default model-provider snapshot.
pub(crate) struct DefaultModelProviderRuntime {
    snapshot: ArcSwap<ModelProviderRuntimeSnapshot>,
    refresh_lock: Mutex<()>,
}

impl DefaultModelProviderRuntime {
    pub(crate) fn new(config: &Config, auth_manager: Arc<AuthManager>) -> Self {
        Self::from_identity(
            ModelProviderRuntimeIdentity::from_config(config, &auth_manager),
            auth_manager,
        )
    }

    pub(crate) fn from_parts(
        provider_id: String,
        provider_info: ModelProviderInfo,
        codex_home: PathBuf,
        config_model_catalog: Option<ModelsResponse>,
        auth_manager: Arc<AuthManager>,
    ) -> Self {
        Self::from_identity(
            ModelProviderRuntimeIdentity {
                provider_id,
                provider_info,
                codex_home,
                config_model_catalog,
                auth_revision: auth_revision(&auth_manager),
            },
            auth_manager,
        )
    }

    fn from_identity(
        identity: ModelProviderRuntimeIdentity,
        auth_manager: Arc<AuthManager>,
    ) -> Self {
        Self {
            snapshot: ArcSwap::from_pointee(ModelProviderRuntimeSnapshot::new(
                /*generation*/ 0,
                identity,
                auth_manager,
            )),
            refresh_lock: Mutex::new(()),
        }
    }

    pub(crate) fn refresh(&self, config: &Config, auth_manager: Arc<AuthManager>) -> bool {
        let _refresh_guard = self
            .refresh_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let current = self.snapshot.load_full();
        if current.matches_config(config, &auth_manager) {
            return false;
        }

        let next = ModelProviderRuntimeSnapshot::new(
            current.generation.saturating_add(1),
            ModelProviderRuntimeIdentity::from_config(config, &auth_manager),
            auth_manager,
        );
        self.snapshot.store(Arc::new(next));
        true
    }

    pub(crate) fn models_manager(&self) -> SharedModelsManager {
        Arc::clone(&self.snapshot.load().models_manager)
    }

    pub(crate) fn snapshot(&self) -> Arc<ModelProviderRuntimeSnapshot> {
        self.snapshot.load_full()
    }

    pub(crate) fn provider_id(&self) -> String {
        self.snapshot.load().identity.provider_id.clone()
    }

    pub(crate) fn capabilities(&self) -> ProviderCapabilities {
        self.snapshot.load().capabilities
    }

    pub(crate) fn generation(&self) -> u64 {
        self.snapshot.load().generation
    }
}

pub(crate) fn build_explicit_model_provider_runtime(
    config: &Config,
    auth_manager: Arc<AuthManager>,
) -> InitialModelProviderRuntime {
    InitialModelProviderRuntime {
        source: ModelProviderRuntimeSource::Explicit,
        snapshot: Arc::new(ModelProviderRuntimeSnapshot::new(
            /*generation*/ 0,
            ModelProviderRuntimeIdentity::from_config(config, &auth_manager),
            auth_manager,
        )),
    }
}

pub(crate) fn build_explicit_model_provider_runtime_with_models_manager(
    config: &Config,
    auth_manager: Arc<AuthManager>,
    models_manager: SharedModelsManager,
) -> InitialModelProviderRuntime {
    let identity = ModelProviderRuntimeIdentity::from_config(config, &auth_manager);
    let provider = create_model_provider(identity.provider_info.clone(), Some(auth_manager));
    InitialModelProviderRuntime {
        source: ModelProviderRuntimeSource::Explicit,
        snapshot: Arc::new(ModelProviderRuntimeSnapshot::from_provider(
            /*generation*/ 0,
            identity,
            provider,
            models_manager,
        )),
    }
}

pub(crate) fn build_models_manager(
    config: &Config,
    auth_manager: Arc<AuthManager>,
) -> SharedModelsManager {
    let provider = create_model_provider(config.model_provider.clone(), Some(auth_manager));
    provider.models_manager(
        config.codex_home.to_path_buf(),
        config.model_catalog.clone(),
    )
}
