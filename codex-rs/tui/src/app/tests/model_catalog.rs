use super::*;
use assert_matches::assert_matches;
use codex_config::types::ModelAvailabilityNuxConfig;
use codex_protocol::openai_models::ModelAvailabilityNux;
use pretty_assertions::assert_eq;
use tokio::sync::mpsc::unbounded_channel;

fn all_model_presets() -> Vec<ModelPreset> {
    crate::legacy_core::test_support::all_model_presets().clone()
}

fn first_visible_model_with_upgrade(presets: &[ModelPreset]) -> (String, String) {
    presets
        .iter()
        .find_map(|preset| {
            let upgrade = preset.upgrade.as_ref()?;
            presets
                .iter()
                .any(|candidate| candidate.model == upgrade.id && candidate.show_in_picker)
                .then(|| (preset.model.clone(), upgrade.id.clone()))
        })
        .expect("preset with visible upgrade target present")
}

fn first_two_visible_models(presets: &[ModelPreset]) -> (String, String) {
    let visible_models: Vec<String> = presets
        .iter()
        .filter(|preset| preset.show_in_picker)
        .map(|preset| preset.model.clone())
        .collect();
    assert!(
        visible_models.len() >= 2,
        "at least two visible models are required for availability nux tests"
    );
    (visible_models[0].clone(), visible_models[1].clone())
}

fn model_availability_nux_config(shown_count: &[(&str, u32)]) -> ModelAvailabilityNuxConfig {
    ModelAvailabilityNuxConfig {
        shown_count: shown_count
            .iter()
            .map(|(model, count)| ((*model).to_string(), *count))
            .collect(),
    }
}

fn model_migration_copy_to_plain_text(copy: &crate::model_migration::ModelMigrationCopy) -> String {
    if let Some(markdown) = copy.markdown.as_ref() {
        return markdown.clone();
    }
    let mut s = String::new();
    for span in &copy.heading {
        s.push_str(&span.content);
    }
    s.push('\n');
    s.push('\n');
    for line in &copy.content {
        for span in &line.spans {
            s.push_str(&span.content);
        }
        s.push('\n');
    }
    s
}

#[tokio::test]
async fn model_migration_prompt_only_shows_for_deprecated_models() {
    let presets = all_model_presets();
    let (current_model, target_model) = first_visible_model_with_upgrade(&presets);
    let seen = BTreeMap::new();
    assert!(should_show_model_migration_prompt(
        &current_model,
        &target_model,
        &seen,
        &presets
    ));
    assert!(!should_show_model_migration_prompt(
        &target_model,
        &target_model,
        &seen,
        &presets
    ));
}

#[test]
fn select_model_availability_nux_picks_only_eligible_model() {
    let mut presets = all_model_presets();
    let (target_model, _) = first_two_visible_models(&presets);
    presets.iter_mut().for_each(|preset| {
        preset.availability_nux = None;
    });
    let target = presets
        .iter_mut()
        .find(|preset| preset.model == target_model)
        .expect("target preset present");
    target.availability_nux = Some(ModelAvailabilityNux {
        message: format!("{} is available", target.model),
    });

    let selected = select_model_availability_nux(&presets, &model_availability_nux_config(&[]));

    assert_eq!(
        selected,
        Some(StartupTooltipOverride {
            model_slug: target_model.clone(),
            message: format!("{target_model} is available"),
        })
    );
}

#[test]
fn select_model_availability_nux_skips_missing_and_exhausted_models() {
    let mut presets = all_model_presets();
    let (first_model, second_model) = first_two_visible_models(&presets);
    presets.iter_mut().for_each(|preset| {
        preset.availability_nux = None;
    });
    let first = presets
        .iter_mut()
        .find(|preset| preset.model == first_model)
        .expect("first preset present");
    first.availability_nux = Some(ModelAvailabilityNux {
        message: format!("{} is available", first.model),
    });
    let second = presets
        .iter_mut()
        .find(|preset| preset.model == second_model)
        .expect("second preset present");
    second.availability_nux = Some(ModelAvailabilityNux {
        message: format!("{} is available", second.model),
    });

    let selected = select_model_availability_nux(
        &presets,
        &model_availability_nux_config(&[(&first_model, MODEL_AVAILABILITY_NUX_MAX_SHOW_COUNT)]),
    );

    assert_eq!(
        selected,
        Some(StartupTooltipOverride {
            model_slug: second_model.clone(),
            message: format!("{second_model} is available"),
        })
    );
}

#[test]
fn select_model_availability_nux_uses_existing_model_order_as_priority() {
    let mut presets = all_model_presets();
    let (higher_priority_model, lower_priority_model) = first_two_visible_models(&presets);
    presets.iter_mut().for_each(|preset| {
        preset.availability_nux = None;
    });
    let lower_priority = presets
        .iter_mut()
        .find(|preset| preset.model == lower_priority_model)
        .expect("lower-priority preset present");
    lower_priority.availability_nux = Some(ModelAvailabilityNux {
        message: "lower-priority".to_string(),
    });
    let higher_priority = presets
        .iter_mut()
        .find(|preset| preset.model == higher_priority_model)
        .expect("higher-priority preset present");
    higher_priority.availability_nux = Some(ModelAvailabilityNux {
        message: "higher-priority".to_string(),
    });

    let selected = select_model_availability_nux(&presets, &model_availability_nux_config(&[]));

    assert_eq!(
        selected,
        Some(StartupTooltipOverride {
            model_slug: higher_priority_model.clone(),
            message: "higher-priority".to_string(),
        })
    );
}

#[test]
fn select_model_availability_nux_returns_none_when_all_models_are_exhausted() {
    let mut presets = all_model_presets();
    let (target_model, _) = first_two_visible_models(&presets);
    presets.iter_mut().for_each(|preset| {
        preset.availability_nux = None;
    });
    let target = presets
        .iter_mut()
        .find(|preset| preset.model == target_model)
        .expect("target preset present");
    target.availability_nux = Some(ModelAvailabilityNux {
        message: format!("{} is available", target.model),
    });

    let selected = select_model_availability_nux(
        &presets,
        &model_availability_nux_config(&[(&target_model, MODEL_AVAILABILITY_NUX_MAX_SHOW_COUNT)]),
    );

    assert_eq!(selected, None);
}

#[tokio::test]
async fn accepted_model_migration_persists_target_default_reasoning_effort() {
    let codex_home = tempdir().expect("temp codex home");
    let mut config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .build()
        .await
        .expect("config");
    config.model = Some("gpt-5.2".to_string());
    config.model_reasoning_effort = Some(ReasoningEffortConfig::XHigh);

    let (tx_raw, mut rx) = unbounded_channel();
    let app_event_tx = AppEventSender::new(tx_raw);

    apply_accepted_model_migration(
        &mut config,
        &app_event_tx,
        "gpt-5.2".to_string(),
        "gpt-5.4".to_string(),
        ReasoningEffortConfig::Medium,
    );

    assert_eq!(config.model.as_deref(), Some("gpt-5.4"));
    assert_eq!(
        config.model_reasoning_effort,
        Some(ReasoningEffortConfig::Medium)
    );

    let acknowledged = rx.try_recv().expect("acknowledged event");
    assert_matches!(
        acknowledged,
        AppEvent::PersistModelMigrationPromptAcknowledged { from_model, to_model }
            if from_model == "gpt-5.2" && to_model == "gpt-5.4"
    );

    let update_model = rx.try_recv().expect("update model event");
    assert_matches!(
        update_model,
        AppEvent::UpdateModel(model) if model == "gpt-5.4"
    );

    let update_effort = rx.try_recv().expect("update effort event");
    assert_matches!(
        update_effort,
        AppEvent::UpdateReasoningEffort(Some(ReasoningEffortConfig::Medium))
    );

    let persist_selection = rx.try_recv().expect("persist model selection event");
    assert_matches!(
        persist_selection,
        AppEvent::PersistModelSelection { model, effort }
            if model == "gpt-5.4" && effort == Some(ReasoningEffortConfig::Medium)
    );
}

#[tokio::test]
async fn model_migration_prompt_respects_hide_flag_and_self_target() {
    let presets = all_model_presets();
    let (current_model, target_model) = first_visible_model_with_upgrade(&presets);
    let mut seen = BTreeMap::new();
    seen.insert(current_model.clone(), target_model.clone());
    assert!(!should_show_model_migration_prompt(
        &current_model,
        &target_model,
        &seen,
        &presets
    ));
    assert!(!should_show_model_migration_prompt(
        &target_model,
        &target_model,
        &seen,
        &presets
    ));
}

#[tokio::test]
async fn model_migration_prompt_skips_when_target_missing_or_hidden() {
    let mut available = all_model_presets();
    let (current_model, target_model) = first_visible_model_with_upgrade(&available);
    let mut current = available
        .iter()
        .find(|preset| preset.model == current_model)
        .cloned()
        .expect("preset present");
    let migration_config_key = current
        .upgrade
        .as_ref()
        .map(|upgrade| upgrade.migration_config_key.clone())
        .unwrap_or_else(|| HIDE_GPT5_1_MIGRATION_PROMPT_CONFIG.to_string());
    current.upgrade = Some(ModelUpgrade {
        id: "missing-target".to_string(),
        reasoning_effort_mapping: None,
        migration_config_key,
        model_link: None,
        upgrade_copy: None,
        migration_markdown: None,
    });
    available.retain(|preset| preset.model != current_model);
    available.push(current.clone());

    assert!(!should_show_model_migration_prompt(
        &current.model,
        "missing-target",
        &BTreeMap::new(),
        &available,
    ));

    assert!(target_preset_for_upgrade(&available, "missing-target").is_none());

    let mut with_hidden_target = all_model_presets();
    let target = with_hidden_target
        .iter_mut()
        .find(|preset| preset.model == target_model)
        .expect("target preset present");
    target.show_in_picker = false;

    assert!(!should_show_model_migration_prompt(
        &current_model,
        &target_model,
        &BTreeMap::new(),
        &with_hidden_target,
    ));
    assert!(target_preset_for_upgrade(&with_hidden_target, &target_model).is_none());
}

#[tokio::test]
async fn model_migration_prompt_shows_for_hidden_model() {
    let codex_home = tempdir().expect("temp codex home");
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .build()
        .await
        .expect("config");

    let mut available_models = all_model_presets();
    let (current_model, _) = first_visible_model_with_upgrade(&available_models);
    let current = available_models
        .iter_mut()
        .find(|preset| preset.model == current_model)
        .expect("migration source preset present");
    current.show_in_picker = false;
    let current = current.clone();
    assert!(
        !current.show_in_picker,
        "expected migration source model to be hidden from picker for this test"
    );

    let upgrade = current.upgrade.as_ref().expect("upgrade configured");
    available_models
        .iter_mut()
        .find(|preset| preset.model == upgrade.id)
        .expect("upgrade target present")
        .show_in_picker = true;
    assert!(
        should_show_model_migration_prompt(
            &current.model,
            &upgrade.id,
            &config.notices.model_migrations,
            &available_models,
        ),
        "expected migration prompt to be eligible for hidden model"
    );

    let target =
        target_preset_for_upgrade(&available_models, &upgrade.id).expect("upgrade target present");
    let target_description = (!target.description.is_empty()).then(|| target.description.clone());
    let can_opt_out = true;
    let copy = migration_copy_for_models(
        &current.model,
        &upgrade.id,
        upgrade.model_link.clone(),
        upgrade.upgrade_copy.clone(),
        upgrade.migration_markdown.clone(),
        target.display_name.clone(),
        target_description,
        can_opt_out,
    );

    assert_snapshot!(
        "model_migration_prompt_shows_for_hidden_model",
        model_migration_copy_to_plain_text(&copy)
    );
}
