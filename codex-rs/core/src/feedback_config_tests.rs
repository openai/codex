//! Check configuration changes and feature toggles are represented without stale overrides.

use super::*;
use codex_models_manager::model_info::model_info_from_slug;
use codex_models_manager::model_info::with_config_overrides;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn configured_and_effective_context_are_distinct_and_unset_is_explicit() {
    let mut config = crate::config::test_config().await;
    config.model_context_window = Some(872_000);
    let mut model = model_info_from_slug("test-model");
    model.context_window = Some(272_000);
    model.max_context_window = Some(400_000);
    model.effective_context_window_percent = 95;
    let resolved = with_config_overrides(model.clone(), &config.to_models_manager_config());
    let tags = usage_tags(&config, &config.features, &resolved, Some("priority"));
    assert_eq!(
        [
            tags["model_context_window"].as_str(),
            tags["model_context_window_override"].as_str(),
            tags["model_context_window_effective"].as_str(),
            tags["service_tier"].as_str(),
        ],
        ["872000", "true", "380000", "priority"]
    );

    config.model_context_window = None;
    let mut updated = tags;
    updated.extend(usage_tags(
        &config,
        &config.features,
        &model,
        /*service_tier*/ None,
    ));
    assert_eq!(
        [
            updated["model_context_window"].as_str(),
            updated["model_context_window_override"].as_str(),
            updated["model_context_window_effective"].as_str(),
            updated["service_tier"].as_str(),
        ],
        ["unset", "false", "258400", "unset"]
    );
}

#[tokio::test]
async fn reported_features_have_boolean_tags_that_change_with_their_state() {
    let config = crate::config::test_config().await;
    let model = model_info_from_slug("test-model");
    let mut features = Features::with_defaults();
    for spec in FEATURES {
        features.disable(spec.id);
    }
    let disabled = usage_tags(&config, &features, &model, /*service_tier*/ None);
    for spec in FEATURES {
        features.enable(spec.id);
    }
    let enabled = usage_tags(&config, &features, &model, /*service_tier*/ None);
    let actual: BTreeMap<_, _> = disabled
        .iter()
        .filter(|(key, _)| key.starts_with("feature."))
        .map(|(key, value)| (key.as_str(), (value.as_str(), enabled[key].as_str())))
        .collect();
    let mut expected: BTreeMap<_, _> = FEATURES
        .iter()
        .map(|spec| (format!("feature.{}", spec.key), ("false", "true")))
        .collect();
    expected.remove("feature.multi_agent");
    expected.remove("feature.enable_fanout");
    assert_eq!(
        actual,
        expected
            .iter()
            .map(|(key, value)| (key.as_str(), *value))
            .collect()
    );
}
