use super::*;
use crate::ModelsManagerConfig;
use pretty_assertions::assert_eq;

#[test]
fn reasoning_summaries_override_true_enables_support() {
    let model = model_info_from_slug("unknown-model");
    let config = ModelsManagerConfig {
        model_supports_reasoning_summaries: Some(true),
        ..Default::default()
    };

    let updated = with_config_overrides(model.clone(), &config);
    let mut expected = model;
    expected.supports_reasoning_summaries = true;

    assert_eq!(updated, expected);
}

#[test]
fn reasoning_summaries_override_false_does_not_disable_support() {
    let mut model = model_info_from_slug("unknown-model");
    model.supports_reasoning_summaries = true;
    let config = ModelsManagerConfig {
        model_supports_reasoning_summaries: Some(false),
        ..Default::default()
    };

    let updated = with_config_overrides(model.clone(), &config);

    assert_eq!(updated, model);
}

#[test]
fn reasoning_summaries_override_false_is_noop_when_model_is_false() {
    let model = model_info_from_slug("unknown-model");
    let config = ModelsManagerConfig {
        model_supports_reasoning_summaries: Some(false),
        ..Default::default()
    };

    let updated = with_config_overrides(model.clone(), &config);

    assert_eq!(updated, model);
}

#[test]
fn model_context_window_override_clamps_to_max_context_window() {
    let mut model = model_info_from_slug("unknown-model");
    model.max_context_window = Some(1_000_000);
    let config = ModelsManagerConfig {
        model_context_window: Some(2_000_000),
        ..Default::default()
    };

    let updated = with_config_overrides(model.clone(), &config);
    let mut expected = model;
    expected.context_window = Some(1_000_000);

    assert_eq!(updated, expected);
}

#[test]
fn model_context_window_override_keeps_values_under_max_context_window() {
    let mut model = model_info_from_slug("unknown-model");
    model.max_context_window = Some(1_000_000);
    let config = ModelsManagerConfig {
        model_context_window: Some(500_000),
        ..Default::default()
    };

    let updated = with_config_overrides(model.clone(), &config);
    let mut expected = model;
    expected.context_window = Some(500_000);

    assert_eq!(updated, expected);
}
