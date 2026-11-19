//! Owns the host-selected Guardian inference route and its shared model catalog.
//! The actor thread resolves this once; both scoring and blocking review consume it.
//! An explicit Guardian route resolves inference settings from the selected provider
//! and trusted host defaults, independently of Actor inference configuration.

use std::sync::Arc;

use codex_extension_api::ExtensionData;
use codex_http_client::HttpClientFactory;
use codex_model_provider::ProviderAuthContext;
use codex_model_provider::SharedModelProvider;
use codex_models_manager::manager::SharedModelsManager;

use crate::config::Config;
use crate::config::GuardianInferenceProvider;

/// Resolved inference dependencies for a host-selected Guardian provider.
#[derive(Clone)]
pub struct GuardianInferenceContext {
    pub(crate) auth: ProviderAuthContext,
    pub provider_id: String,
    pub provider: SharedModelProvider,
    pub models_manager: SharedModelsManager,
    pub http_client_factory: HttpClientFactory,
    pub service_tier: Option<String>,
}

/// Inference dependencies supplied by Core to an internal Guardian reviewer.
pub(crate) struct GuardianReviewerInference(pub Arc<GuardianInferenceContext>);

impl GuardianInferenceContext {
    pub(crate) fn resolve(
        config: &Config,
        selected: &GuardianInferenceProvider,
        auth: ProviderAuthContext,
        provider: SharedModelProvider,
        models_manager: SharedModelsManager,
    ) -> Self {
        Self {
            auth,
            provider_id: selected.id.clone(),
            provider,
            models_manager,
            http_client_factory: config.http_client_factory_for_provider(&selected.info),
            // The actor's tier belongs to a different inference provider.
            service_tier: None,
        }
    }

    /// Returns the host-selected route, or an error if its startup state is missing.
    pub fn for_config(config: &Config, store: &ExtensionData) -> anyhow::Result<Option<Arc<Self>>> {
        if config.guardian_model_provider.is_none() {
            return Ok(None);
        }
        store.get::<Self>().map(Some).ok_or_else(|| {
            anyhow::anyhow!("host-selected Guardian inference context is not initialized")
        })
    }

    pub(crate) fn configure_reviewer(&self, config: &mut Config) {
        let mut provider = self.provider.info().clone();
        // The reviewer config already contains the host's blocking-review retry policy.
        provider.request_max_retries = config.model_provider.request_max_retries;
        provider.stream_max_retries = config.model_provider.stream_max_retries;
        config.model_providers.remove(&config.model_provider_id);
        config.model_provider_id.clone_from(&self.provider_id);
        config.model_provider = provider;
        config
            .model_providers
            .insert(self.provider_id.clone(), config.model_provider.clone());
        config.guardian_policy_config = config.managed_guardian_policy().map(str::to_owned);
        config.guardian_transcript_mode = Default::default();
        config.guardian_conversation_history_prompt = None;
        config.guardian_model_provider = None;
        config.model_catalog = None;
        config.model_context_window = None;
        config.async_compaction_threshold_percent =
            crate::config::DEFAULT_ASYNC_COMPACTION_THRESHOLD_PERCENT;
        config.model_auto_compact_token_limit = None;
        config.model_auto_compact_token_limit_scope = Default::default();
        config.tool_output_token_limit = None;
        config.model_verbosity = None;
        config.compact_prompt = None;
        config.service_tier.clone_from(&self.service_tier);
    }
}
