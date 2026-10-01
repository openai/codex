//! First-thread model overrides come from invocation inputs, independently of local config.
//! Explicit profile launches retain their existing resolved-config path until server profile
//! selection is available for connected launches.

use crate::legacy_core::config::ConfigOverrides;
use codex_app_server_protocol::ThreadStartParams;
use codex_config::LoaderOverrides;

#[derive(Default)]
pub(crate) struct StartupLaunchChoices {
    pub(crate) model: Option<String>,
    pub(crate) effort: Option<serde_json::Value>,
    provider: Option<String>,
    selected_profile: bool,
}

impl StartupLaunchChoices {
    pub(crate) fn from_launch(
        cli_overrides: &[(String, toml::Value)],
        harness_overrides: &ConfigOverrides,
        loader_overrides: &LoaderOverrides,
    ) -> Self {
        if loader_overrides.user_config_profile.is_some() {
            return Self {
                provider: harness_overrides.model_provider.clone(),
                selected_profile: true,
                ..Self::default()
            };
        }
        let value = |key| {
            cli_overrides
                .iter()
                .rev()
                .find(|(path, _)| path == key)
                .map(|(_, value)| value)
        };
        Self {
            model: harness_overrides.model.clone().or_else(|| {
                value("model")
                    .and_then(toml::Value::as_str)
                    .map(str::to_owned)
            }),
            effort: value("model_reasoning_effort")
                .and_then(|value| serde_json::to_value(value).ok()),
            provider: harness_overrides.model_provider.clone().or_else(|| {
                value("model_provider")
                    .and_then(toml::Value::as_str)
                    .map(str::to_owned)
            }),
            selected_profile: false,
        }
    }

    pub(super) fn configure(self, params: &mut ThreadStartParams) {
        if self.selected_profile {
            params.model_provider = self.provider.or(params.model_provider.take());
            return;
        }
        params.model = self.model;
        params.model_provider = self.provider;
        if let Some(overrides) = params.config.as_mut() {
            overrides.remove("model_reasoning_effort");
        }
        if let Some(effort) = self.effort {
            params
                .config
                .get_or_insert_default()
                .insert("model_reasoning_effort".into(), effort);
        }
    }
}
