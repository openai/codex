//! The catalog correction policy preserves valid choices and avoids guessing special modes.

use super::normalize;
use crate::session::tests::make_session_configuration_for_tests;
use codex_models_manager::bundled_models_response;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;
use pretty_assertions::assert_eq;
use std::sync::Arc;

#[test_case::test_case("lower")]
#[test_case::test_case("valid")]
#[test_case::test_case("missing-model")]
#[test_case::test_case("no-efforts")]
#[test_case::test_case("only-higher")]
#[test_case::test_case("special")]
#[test_case::test_case("invalid-persistent")]
#[test_case::test_case("invalid-ultra")]
#[test_case::test_case("unknown")]
#[tokio::test]
async fn correction_changes_only_supported_bounded_choices(scenario: &str) {
    let mut settings = make_session_configuration_for_tests().await.step_settings;
    let previous = match scenario {
        "special" | "invalid-persistent" => ReasoningEffort::Persistent,
        "invalid-ultra" => ReasoningEffort::Ultra,
        "unknown" => ReasoningEffort::Custom("future-effort".into()),
        _ => ReasoningEffort::High,
    };
    Arc::make_mut(&mut settings)
        .collaboration_mode
        .settings
        .reasoning_effort = Some(previous);
    Arc::make_mut(&mut settings).service_tier = Some("priority".into());
    let mut expected = settings.clone();
    let mut model = bundled_models_response().unwrap().models.remove(0);
    model.slug = settings.collaboration_mode.model().into();
    model.service_tiers.clear();
    model.default_reasoning_level = Some(ReasoningEffort::Low);
    let allowed = match scenario {
        "valid" => ReasoningEffort::High,
        "only-higher" => ReasoningEffort::Max,
        "special" => ReasoningEffort::Persistent,
        _ => ReasoningEffort::Low,
    };
    model.supported_reasoning_levels = vec![ReasoningEffortPreset {
        description: allowed.to_string(),
        effort: allowed,
    }];
    if scenario == "missing-model" {
        model.slug = "another-model".into();
    }
    if scenario == "no-efforts" {
        model.supported_reasoning_levels.clear();
    }
    if matches!(scenario, "lower" | "invalid-persistent" | "invalid-ultra") {
        Arc::make_mut(&mut expected)
            .collaboration_mode
            .settings
            .reasoning_effort = Some(ReasoningEffort::Low);
    }
    if scenario != "missing-model" {
        Arc::make_mut(&mut expected).service_tier = Some("default".into());
    }
    normalize(&mut settings, &[model]);
    assert_eq!(settings, expected);
}
