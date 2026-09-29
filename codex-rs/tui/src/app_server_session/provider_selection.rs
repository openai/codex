//! Provider request overrides honor managed requirements over explicit invocation choices.
//! Omission lets the server resolve the provider for thread creation, forks, and history lookup.

use super::AppServerSession;
use crate::legacy_core::config::Config;
use codex_config::ConfigLayerSource;

pub(crate) fn explicit_provider(config: &Config) -> Option<String> {
    config
        .config_layer_stack
        .layers_high_to_low()
        .find(|layer| layer.config.get("model_provider").is_some())
        .filter(|layer| {
            matches!(
                layer.name,
                ConfigLayerSource::SessionFlags
                    | ConfigLayerSource::User {
                        profile: Some(_),
                        ..
                    }
            )
        })
        .map(|_| config.model_provider_id.clone())
}

impl AppServerSession {
    pub(crate) fn explicit_model_provider(&self, config: &Config) -> Option<String> {
        self.model_provider_override
            .as_ref()
            .map(|provider| {
                config
                    .config_layer_stack
                    .required_model_provider()
                    .unwrap_or(provider)
                    .to_owned()
            })
            .or_else(|| explicit_provider(config))
    }

    pub(crate) async fn history_model_provider(
        &self,
        config: &Config,
    ) -> color_eyre::Result<Option<String>> {
        if let Some(provider) = self.explicit_model_provider(config) {
            return Ok(Some(provider));
        }
        let Some(cwd) = super::thread_cwd_from_config(
            config,
            self.thread_params_mode(),
            self.remote_cwd_override(),
        ) else {
            return Ok(None);
        };
        Ok(crate::config_update::read_effective_config_if_supported(
            self.request_handle(),
            std::path::Path::new(&cwd),
        )
        .await?
        .map(|config| {
            config
                .config
                .model_provider
                .unwrap_or_else(|| "openai".to_string())
        }))
    }
}
