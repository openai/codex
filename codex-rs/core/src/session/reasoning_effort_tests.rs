//! Request-effort pins across initial replay and compaction lookups.

use super::RequestEffortUsage;
use crate::session::step_settings::ResolvedStepSettings;
use crate::session::tests::make_session_and_context_with_auth_and_config_and_rx;
use codex_features::Feature;
use codex_history::InitialHistory;
use codex_history::ResumedHistory;
use codex_login::CodexAuth;
use codex_protocol::ThreadId;
use codex_protocol::openai_models::ModelOrigin;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use test_case::test_case;

#[test_case(ReasoningEffort::High, ReasoningEffort::Low; "disallowed_high")]
#[test_case(ReasoningEffort::Ultra, ReasoningEffort::Low; "valid_ultra")]
#[test_case(ReasoningEffort::Persistent, ReasoningEffort::Low; "valid_persistent")]
#[tokio::test]
async fn fresh_step_metadata_retires_only_incompatible_pins(
    previous: ReasoningEffort,
    selected: ReasoningEffort,
) {
    let (session, turn, _events) = make_session_and_context_with_auth_and_config_and_rx(
        CodexAuth::from_api_key("Test API Key"),
        Vec::new(),
        |config| {
            config
                .features
                .enable(Feature::ReasoningEffortOverride)
                .unwrap();
        },
    )
    .await;
    let mut model = (*turn.initial_settings.model_info).clone();
    model.origin = ModelOrigin::Provider;
    model.supports_reasoning_effort_updates = true;
    model.supported_reasoning_levels = [previous.clone(), selected.clone(), ReasoningEffort::Max]
        .into_iter()
        .map(|effort| ReasoningEffortPreset {
            description: effort.to_string(),
            effort,
        })
        .collect();
    let original = model.resolve_reasoning_effort(previous.clone());
    let mut choice = turn.initial_settings.selected().clone();
    choice.collaboration_mode.settings.reasoning_effort = Some(previous.clone());
    let old_step = ResolvedStepSettings::new(
        Arc::new(choice.clone()),
        Arc::new(model.clone()),
        &Default::default(),
    );
    assert_eq!(
        session
            .reasoning_effort_for_request(&old_step, RequestEffortUsage::Sampling)
            .await,
        Some(original.clone())
    );
    if previous == ReasoningEffort::High {
        model.supported_reasoning_levels = vec![ReasoningEffortPreset {
            description: selected.to_string(),
            effort: selected.clone(),
        }];
    }
    choice.collaboration_mode.settings.reasoning_effort = Some(selected.clone());
    let expected = if previous == ReasoningEffort::High {
        model.resolve_reasoning_effort(selected)
    } else {
        original.clone()
    };
    let fresh_step =
        ResolvedStepSettings::new(Arc::new(choice), Arc::new(model), &Default::default());
    assert_eq!(
        session
            .reasoning_effort_for_request(&fresh_step, RequestEffortUsage::Compaction)
            .await,
        Some(expected.clone())
    );
    assert_eq!(
        session
            .state
            .lock()
            .await
            .reasoning_effort_pin
            .get(&old_step.model_info.slug),
        Some(original.clone())
    );
    assert_eq!(
        session
            .reasoning_effort_for_request(&old_step, RequestEffortUsage::Sampling)
            .await,
        Some(original)
    );
    assert_eq!(
        session
            .reasoning_effort_for_request(&fresh_step, RequestEffortUsage::Sampling)
            .await,
        Some(expected)
    );
}

#[test_case(InitialHistory::Forked(Vec::new()); "fork")]
#[test_case(InitialHistory::Resumed(ResumedHistory {
history_revision: None,
    conversation_id: ThreadId::default(),
    history: Arc::new(Vec::new()),
    rollout_path: None,
}); "resume")]
#[tokio::test]
async fn initial_replay_preserves_prewarmed_effort(history: InitialHistory) {
    let (session, turn_context, _events) = make_session_and_context_with_auth_and_config_and_rx(
        CodexAuth::from_api_key("Test API Key"),
        Vec::new(),
        |config| {
            config
                .features
                .enable(Feature::ReasoningEffortOverride)
                .unwrap();
        },
    )
    .await;
    let mut model_info = Arc::clone(&turn_context.initial_settings.model_info);
    Arc::make_mut(&mut model_info).supports_reasoning_effort_updates = true;
    let mut selected = turn_context.initial_settings.selected().clone();
    selected.collaboration_mode.settings.reasoning_effort = Some(ReasoningEffort::Medium);
    let prewarm_settings = ResolvedStepSettings::new(
        Arc::new(selected.clone()),
        Arc::clone(&model_info),
        &codex_features::Features::default(),
    );
    // Force the ordering where prewarm pins its request before initial replay finishes.
    assert_eq!(
        session
            .reasoning_effort_for_request(&prewarm_settings, RequestEffortUsage::Sampling)
            .await,
        Some(ReasoningEffort::Medium),
    );
    session.record_initial_history(history).await;

    selected.collaboration_mode.settings.reasoning_effort = Some(ReasoningEffort::High);
    let turn_settings = ResolvedStepSettings::new(
        Arc::new(selected),
        model_info,
        &codex_features::Features::default(),
    );
    assert_eq!(
        session
            .reasoning_effort_for_request(&turn_settings, RequestEffortUsage::Sampling)
            .await,
        Some(ReasoningEffort::Medium),
    );
}

#[tokio::test]
async fn compaction_effort_lookup_preserves_pin_for_fallback_models() {
    let (session, turn_context, _events) = make_session_and_context_with_auth_and_config_and_rx(
        CodexAuth::from_api_key("Test API Key"),
        Vec::new(),
        |config| {
            config
                .features
                .enable(Feature::ReasoningEffortOverride)
                .unwrap();
        },
    )
    .await;
    session
        .state
        .lock()
        .await
        .reasoning_effort_pin
        .pin("original", ReasoningEffort::Low);
    let mut settings = (*turn_context.initial_settings).clone();
    let effort = ReasoningEffort::Medium;
    let model = Arc::make_mut(&mut settings.model_info);
    model.slug = "fallback".to_string();
    model.supports_reasoning_effort_updates = true;
    model.default_reasoning_level = Some(effort.clone());

    assert_eq!(
        session
            .reasoning_effort_for_request(&settings, RequestEffortUsage::Compaction)
            .await,
        Some(effort)
    );
    assert_eq!(
        session
            .state
            .lock()
            .await
            .reasoning_effort_pin
            .get("original"),
        Some(ReasoningEffort::Low)
    );
}

#[tokio::test]
async fn unsupported_model_compaction_uses_selected_effort_without_mutating_pin() {
    let (session, turn_context, _events) = make_session_and_context_with_auth_and_config_and_rx(
        CodexAuth::from_api_key("Test API Key"),
        Vec::new(),
        |config| {
            config
                .features
                .enable(Feature::ReasoningEffortOverride)
                .unwrap();
            config.model_reasoning_effort = Some(ReasoningEffort::High);
        },
    )
    .await;
    let mut settings = (*turn_context.initial_settings).clone();
    Arc::make_mut(&mut settings.model_info).supports_reasoning_effort_updates = false;
    session
        .state
        .lock()
        .await
        .reasoning_effort_pin
        .pin(&settings.model_info.slug, ReasoningEffort::Low);

    assert_eq!(
        session
            .reasoning_effort_for_request(&settings, RequestEffortUsage::Compaction)
            .await,
        Some(ReasoningEffort::High),
    );
    assert_eq!(
        session
            .state
            .lock()
            .await
            .reasoning_effort_pin
            .get(&settings.model_info.slug),
        Some(ReasoningEffort::Low),
    );
    // Sampling retires the old baseline, so a later supported request starts afresh.
    assert_eq!(
        session
            .reasoning_effort_for_request(&settings, RequestEffortUsage::Sampling)
            .await,
        Some(ReasoningEffort::High),
    );
    Arc::make_mut(&mut settings.model_info).supports_reasoning_effort_updates = true;
    assert_eq!(
        session
            .reasoning_effort_for_request(&settings, RequestEffortUsage::Sampling)
            .await,
        Some(ReasoningEffort::High),
    );
}
