//! Metrics derived from loaded configuration at session start.

use super::Config;
use codex_config::ConfigLayerSource;
use codex_config::ConfigLayerStack;
use codex_features::FEATURES;
use codex_features::Stage;
use codex_otel::SessionTelemetry;

pub(crate) fn emit_session_start_metrics(config: &Config, telemetry: &SessionTelemetry) {
    for feature in FEATURES {
        if matches!(feature.stage, Stage::Removed) {
            continue;
        }
        if config.features.enabled(feature.id) != feature.default_enabled {
            telemetry.counter(
                "codex.feature.state",
                /*inc*/ 1,
                &[
                    ("feature", feature.key),
                    ("value", &config.features.enabled(feature.id).to_string()),
                ],
            );
        }
    }
    #[cfg(windows)]
    crate::windows_system_config::emit_namespace_squatting_probe();
}

/// Report saved opt-outs even when a higher-precedence session setting overrides them.
pub(crate) fn legacy_multi_agent_disabled_sources(layers: &ConfigLayerStack) -> Vec<&'static str> {
    let mut user = None;
    let mut project = None;
    for layer in layers.layers_low_to_high() {
        let setting = match layer.name {
            ConfigLayerSource::User { .. } => &mut user,
            ConfigLayerSource::Project { .. } => &mut project,
            _ => continue,
        };
        if let Some(value) = layer
            .config
            .get("features")
            .and_then(|features| features.get("multi_agent"))
            .and_then(toml::Value::as_bool)
        {
            *setting = Some(value);
        }
    }
    let managed = layers
        .requirements()
        .feature_requirements
        .as_ref()
        .and_then(|requirements| requirements.value.entries.get("multi_agent"))
        .copied();
    [
        ("user", user),
        ("project", project),
        ("managed_requirement", managed),
    ]
    .into_iter()
    .filter_map(|(source, value)| (value == Some(false)).then_some(source))
    .collect()
}
