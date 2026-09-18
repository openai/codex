//! Request-effort pins across initial replay and compaction lookups.

use super::RequestEffortUsage;
use crate::session::step_settings::ResolvedStepSettings;
use crate::session::tests::make_session_and_context_with_auth_and_config_and_rx;
use codex_features::Feature;
use codex_history::InitialHistory;
use codex_history::ResumedHistory;
use codex_login::CodexAuth;
use codex_protocol::ThreadId;
use codex_protocol::openai_models::ReasoningEffort;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use test_case::test_case;

#[test_case(InitialHistory::Forked(Vec::new()); "fork")]
#[test_case(InitialHistory::Resumed(ResumedHistory {
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
        /*fast_mode_enabled*/ false,
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
        /*fast_mode_enabled*/ false,
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
