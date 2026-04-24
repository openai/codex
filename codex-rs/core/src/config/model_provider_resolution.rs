use std::collections::HashMap;
use std::io;

use codex_config::config_toml::ConfigToml;
use codex_config::profile_toml::ConfigProfile;
use codex_model_provider_info::LEGACY_OLLAMA_CHAT_PROVIDER_ID;
use codex_model_provider_info::ModelProviderInfo;
use codex_model_provider_info::OLLAMA_CHAT_PROVIDER_REMOVED_ERROR;
use codex_model_provider_info::built_in_model_providers;
use codex_model_provider_info::merge_configured_model_providers;

pub(crate) struct ResolvedModelProvider {
    pub(crate) id: String,
    pub(crate) info: ModelProviderInfo,
    pub(crate) all: HashMap<String, ModelProviderInfo>,
}

pub(crate) fn resolve_model_provider_from_config_toml(
    cfg: &ConfigToml,
    config_profile: &ConfigProfile,
    explicit_model_provider: Option<String>,
) -> io::Result<ResolvedModelProvider> {
    let openai_base_url = cfg
        .openai_base_url
        .clone()
        .filter(|value| !value.is_empty());
    let model_providers = merge_configured_model_providers(
        built_in_model_providers(openai_base_url),
        cfg.model_providers.clone(),
    )
    .map_err(|message| io::Error::new(io::ErrorKind::InvalidData, message))?;

    let model_provider_id = explicit_model_provider
        .or_else(|| config_profile.model_provider.clone())
        .or_else(|| cfg.model_provider.clone())
        .unwrap_or_else(|| "openai".to_string());
    let model_provider = model_providers
        .get(&model_provider_id)
        .ok_or_else(|| {
            let message = if model_provider_id == LEGACY_OLLAMA_CHAT_PROVIDER_ID {
                OLLAMA_CHAT_PROVIDER_REMOVED_ERROR.to_string()
            } else {
                format!("Model provider `{model_provider_id}` not found")
            };
            io::Error::new(io::ErrorKind::NotFound, message)
        })?
        .clone();

    Ok(ResolvedModelProvider {
        id: model_provider_id,
        info: model_provider,
        all: model_providers,
    })
}
