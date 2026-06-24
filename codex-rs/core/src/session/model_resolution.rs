use super::Session;
use super::session::SessionSettingsUpdate;
use crate::config::Config;
use codex_models_manager::UnsupportedModelFallback;
use codex_models_manager::manager::RefreshStrategy;
use codex_models_manager::manager::SharedModelsManager;

pub(super) async fn resolve_configured_model(
    config: &mut Config,
    models_manager: &SharedModelsManager,
    refresh_strategy: RefreshStrategy,
) -> String {
    let resolution = models_manager
        .resolve_model(&config.model, refresh_strategy)
        .await;
    if let Some(fallback) = resolution.fallback.as_ref() {
        record_model_fallback(config.model_provider_id.as_str(), fallback);
        config.startup_warnings.push(model_fallback_warning(
            config.model_provider_id.as_str(),
            fallback,
        ));
        config.model = Some(resolution.model.clone());
    }
    resolution.model
}

impl Session {
    pub(super) async fn resolve_settings_update_model(&self, updates: &mut SessionSettingsUpdate) {
        let Some(collaboration_mode) = updates.collaboration_mode.clone() else {
            return;
        };
        let requested_model = Some(collaboration_mode.model().to_string());
        let resolution = self
            .services
            .models_manager
            .resolve_model(&requested_model, RefreshStrategy::OnlineIfUncached)
            .await;
        let Some(fallback) = resolution.fallback else {
            return;
        };
        updates.collaboration_mode = Some(collaboration_mode.with_updates(
            Some(resolution.model),
            /*effort*/ None,
            /*developer_instructions*/ None,
        ));
        let provider_id = {
            let state = self.state.lock().await;
            state
                .session_configuration
                .original_config_do_not_use
                .model_provider_id
                .clone()
        };
        record_model_fallback(provider_id.as_str(), &fallback);
    }
}

fn model_fallback_warning(provider_id: &str, fallback: &UnsupportedModelFallback) -> String {
    let requested_model = fallback.requested_model.as_str();
    let fallback_model = fallback.fallback_model.as_str();
    format!(
        "Model `{requested_model}` is unavailable for provider `{provider_id}`; using \
         `{fallback_model}` instead."
    )
}

fn record_model_fallback(provider_id: &str, fallback: &UnsupportedModelFallback) {
    tracing::warn!(
        provider = provider_id,
        requested_model = fallback.requested_model.as_str(),
        fallback_model = fallback.fallback_model.as_str(),
        "requested model is unsupported by provider; using provider fallback"
    );
}
