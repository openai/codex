use crate::legacy_core::config::Config;
use crate::legacy_core::config::ConfigOverrides;
use codex_config::TomlValue;
use codex_protocol::config_types::ServiceTier;

pub(crate) fn config_with_managed_new_thread_defaults(
    mut config: Config,
    cli_overrides: &[(String, TomlValue)],
    harness_overrides: &ConfigOverrides,
) -> Config {
    // `Config` has already merged persisted defaults with launch overrides. Consult the original
    // override inputs so an explicit TUI launch choice still wins over the managed default.
    let has_cli_override = |key: &str| cli_overrides.iter().any(|(path, _)| path == key);
    let Some(defaults) = config
        .config_layer_stack
        .requirements_toml()
        .models
        .as_ref()
        .and_then(|models| models.new_thread.as_ref())
        .cloned()
    else {
        return config;
    };

    if harness_overrides.model.is_none()
        && !has_cli_override("model")
        && let Some(model) = defaults.model
    {
        config.model = Some(model);
    }
    if !has_cli_override("model_reasoning_effort")
        && let Some(reasoning_effort) = defaults.model_reasoning_effort
    {
        config.model_reasoning_effort = Some(reasoning_effort);
    }
    if harness_overrides.service_tier.is_none()
        && !has_cli_override("service_tier")
        && let Some(service_tier) = defaults.service_tier
    {
        config.service_tier = Some(
            ServiceTier::from_request_value(&service_tier)
                .map(|tier| tier.request_value().to_string())
                .unwrap_or(service_tier),
        );
    }

    config
}

#[cfg(test)]
#[path = "managed_new_thread_defaults_tests.rs"]
mod tests;
