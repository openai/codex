use crate::legacy_core::config::Config;
use crate::legacy_core::config::ConfigOverrides;
use codex_app_server_protocol::NewThreadModelDefaults;
use codex_protocol::config_types::ServiceTier;
use toml::Value as TomlValue;

pub(crate) fn apply_managed_new_thread_defaults(
    config: &mut Config,
    defaults: Option<&NewThreadModelDefaults>,
    cli_kv_overrides: &[(String, TomlValue)],
    harness_overrides: &ConfigOverrides,
) {
    let Some(defaults) = defaults else {
        return;
    };
    let has_cli_override = |key: &str| cli_kv_overrides.iter().any(|(path, _value)| path == key);

    if harness_overrides.model.is_none()
        && !has_cli_override("model")
        && let Some(model) = defaults.model.as_ref()
    {
        config.model = Some(model.clone());
    }
    if !has_cli_override("model_reasoning_effort")
        && let Some(reasoning_effort) = defaults.model_reasoning_effort.as_ref()
    {
        config.model_reasoning_effort = Some(reasoning_effort.clone());
    }
    if harness_overrides.service_tier.is_none()
        && !has_cli_override("service_tier")
        && let Some(service_tier) = defaults.service_tier.as_ref()
    {
        config.service_tier = Some(
            ServiceTier::from_request_value(service_tier)
                .map(|tier| tier.request_value().to_string())
                .unwrap_or_else(|| service_tier.clone()),
        );
    }
}

#[cfg(test)]
#[path = "managed_new_thread_defaults_tests.rs"]
mod tests;
