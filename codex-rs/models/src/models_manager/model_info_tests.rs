use super::*;
use pretty_assertions::assert_eq;

#[test]
fn reasoning_summaries_override_true_enables_support() {
    let model = model_info_from_slug("unknown-model");
    let overrides = ModelInfoConfigOverrides {
        model_supports_reasoning_summaries: Some(true),
        ..Default::default()
    };

    let updated = with_config_overrides(model.clone(), &overrides);
    let mut expected = model;
    expected.supports_reasoning_summaries = true;

    assert_eq!(updated, expected);
}

#[test]
fn reasoning_summaries_override_false_does_not_disable_support() {
    let mut model = model_info_from_slug("unknown-model");
    model.supports_reasoning_summaries = true;
    let overrides = ModelInfoConfigOverrides {
        model_supports_reasoning_summaries: Some(false),
        ..Default::default()
    };

    let updated = with_config_overrides(model.clone(), &overrides);

    assert_eq!(updated, model);
}

#[test]
fn reasoning_summaries_override_false_is_noop_when_model_is_false() {
    let model = model_info_from_slug("unknown-model");
    let overrides = ModelInfoConfigOverrides {
        model_supports_reasoning_summaries: Some(false),
        ..Default::default()
    };

    let updated = with_config_overrides(model.clone(), &overrides);

    assert_eq!(updated, model);
}
