use codex_core::test_support::SpawnAgentTestSetup;
use codex_core::test_support::all_model_presets;
use codex_core::test_support::spawn_agent_snapshot_for_tests;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ReasoningEffort;
use pretty_assertions::assert_eq;

fn picker_model_with_reasoning(excluding_model: &str) -> &'static ModelPreset {
    let Some(model) = all_model_presets().iter().find(|preset| {
        preset.show_in_picker
            && preset.model != excluding_model
            && !preset.supported_reasoning_efforts.is_empty()
    }) else {
        panic!("expected a picker-visible model with reasoning efforts");
    };
    model
}

fn picker_model_with_multiple_reasoning_efforts() -> &'static ModelPreset {
    let Some(model) = all_model_presets()
        .iter()
        .find(|preset| preset.show_in_picker && preset.supported_reasoning_efforts.len() > 1)
    else {
        panic!("expected a picker-visible model with multiple reasoning efforts");
    };
    model
}

fn unsupported_reasoning_effort(model: &ModelPreset) -> ReasoningEffort {
    let Some(reasoning_effort) = [
        ReasoningEffort::None,
        ReasoningEffort::Minimal,
        ReasoningEffort::Low,
        ReasoningEffort::Medium,
        ReasoningEffort::High,
        ReasoningEffort::XHigh,
    ]
    .into_iter()
    .find(|effort| {
        !model
            .supported_reasoning_efforts
            .iter()
            .any(|preset| preset.effort == *effort)
    }) else {
        panic!("expected a reasoning effort unsupported by the selected model");
    };
    reasoning_effort
}

#[tokio::test]
async fn spawn_agent_role_model_takes_precedence_over_requested_model_and_reasoning() {
    let role_model = picker_model_with_multiple_reasoning_efforts();
    let requested_model = picker_model_with_reasoning(&role_model.model);
    let role_reasoning_effort = role_model
        .supported_reasoning_efforts
        .last()
        .map(|preset| preset.effort)
        .expect("role model should support reasoning efforts");

    let snapshot = spawn_agent_snapshot_for_tests(SpawnAgentTestSetup {
        requested_model: Some(requested_model.model.clone()),
        requested_reasoning_effort: Some(requested_model.default_reasoning_effort),
        role_name: Some("custom-role".to_string()),
        role_model: Some(role_model.model.clone()),
        role_reasoning_effort: Some(role_reasoning_effort),
        ..SpawnAgentTestSetup::default()
    })
    .await
    .expect("spawn_agent should succeed");

    assert_eq!(snapshot.model, role_model.model);
    assert_eq!(snapshot.reasoning_effort, Some(role_reasoning_effort));
}

#[tokio::test]
async fn spawn_agent_overrides_model_and_uses_selected_model_default_effort() {
    let inherited_model = picker_model_with_reasoning("").model.clone();
    let requested_model = picker_model_with_reasoning(&inherited_model);

    let snapshot = spawn_agent_snapshot_for_tests(SpawnAgentTestSetup {
        requested_model: Some(requested_model.model.clone()),
        inherited_model: Some(inherited_model),
        ..SpawnAgentTestSetup::default()
    })
    .await
    .expect("spawn_agent should succeed");

    assert_eq!(snapshot.model, requested_model.model);
    assert_eq!(
        snapshot.reasoning_effort,
        Some(requested_model.default_reasoning_effort)
    );
}

#[tokio::test]
async fn spawn_agent_overrides_reasoning_effort_for_inherited_model() {
    let inherited_model = picker_model_with_reasoning("").model.clone();

    let snapshot = spawn_agent_snapshot_for_tests(SpawnAgentTestSetup {
        requested_reasoning_effort: Some(ReasoningEffort::Low),
        inherited_model: Some(inherited_model),
        inherited_reasoning_effort: Some(ReasoningEffort::Medium),
        ..SpawnAgentTestSetup::default()
    })
    .await
    .expect("spawn_agent should succeed");

    assert_eq!(snapshot.reasoning_effort, Some(ReasoningEffort::Low));
}

#[tokio::test]
async fn spawn_agent_overrides_model_and_reasoning_effort_together() {
    let inherited_model = picker_model_with_reasoning("").model.clone();
    let requested_model = picker_model_with_reasoning(&inherited_model);

    let snapshot = spawn_agent_snapshot_for_tests(SpawnAgentTestSetup {
        requested_model: Some(requested_model.model.clone()),
        requested_reasoning_effort: Some(ReasoningEffort::Low),
        inherited_model: Some(inherited_model),
        ..SpawnAgentTestSetup::default()
    })
    .await
    .expect("spawn_agent should succeed");

    assert_eq!(snapshot.model, requested_model.model);
    assert_eq!(snapshot.reasoning_effort, Some(ReasoningEffort::Low));
}

#[tokio::test]
async fn spawn_agent_rejects_unknown_model_override() {
    let error = spawn_agent_snapshot_for_tests(SpawnAgentTestSetup {
        requested_model: Some("not-a-real-model".to_string()),
        ..SpawnAgentTestSetup::default()
    })
    .await
    .expect_err("unknown model should be rejected");

    assert!(error.contains("Unknown model `not-a-real-model` for spawn_agent"));
}

#[tokio::test]
async fn spawn_agent_rejects_unsupported_reasoning_effort_for_selected_model() {
    let inherited_model = picker_model_with_reasoning("").model.clone();
    let requested_model = picker_model_with_reasoning(&inherited_model);
    let requested_reasoning_effort = unsupported_reasoning_effort(requested_model);

    let error = spawn_agent_snapshot_for_tests(SpawnAgentTestSetup {
        requested_model: Some(requested_model.model.clone()),
        requested_reasoning_effort: Some(requested_reasoning_effort),
        inherited_model: Some(inherited_model),
        ..SpawnAgentTestSetup::default()
    })
    .await
    .expect_err("unsupported reasoning effort should be rejected");

    assert!(error.contains(&format!(
        "Reasoning effort `{requested_reasoning_effort}` is not supported"
    )));
}
