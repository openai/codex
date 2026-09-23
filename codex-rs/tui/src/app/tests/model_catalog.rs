use super::*;
use assert_matches::assert_matches;
use codex_config::types::ModelAvailabilityNuxConfig;
use codex_protocol::config_types::ServiceTier;
use codex_protocol::openai_models::ModelAvailabilityNux;
use pretty_assertions::assert_eq;
use tokio::sync::mpsc::unbounded_channel;

fn all_model_presets() -> Vec<ModelPreset> {
    crate::test_support::TEST_MODEL_PRESETS.clone()
}

#[tokio::test]
async fn model_picker_refresh_updates_app_catalog_from_app_server() -> Result<()> {
    let (mut app, mut rx, _op_rx) = make_test_app_with_channels().await;
    let mut app_server = start_config_write_test_app_server(&app).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let fast = Some(ServiceTier::Fast.request_value());
    let thread_id = ThreadId::new();
    let mut session = test_thread_session(thread_id, test_path_buf("/tmp/project"));
    session.model = "gpt-5.5".to_string();
    app.primary_thread_id = Some(thread_id);
    app.active_thread_id = Some(thread_id);
    app.primary_session_configured = Some(session.clone());
    app.thread_event_channels.insert(
        thread_id,
        ThreadEventChannel::new_with_session(/*capacity*/ 4, session.clone(), Vec::new()),
    );
    app.chat_widget.handle_thread_session(session.clone());
    app.chat_widget
        .set_feature_enabled(Feature::FastMode, /*enabled*/ true);
    app.chat_widget.open_model_popup();
    let request_id = std::iter::from_fn(|| rx.try_recv().ok())
        .find_map(|event| match event {
            AppEvent::FetchModels { request_id } => Some(request_id),
            _ => None,
        })
        .expect("initial model request");
    let mut hidden = all_model_presets();
    for preset in &mut hidden {
        preset.show_in_picker = false;
    }
    Box::pin(app.handle_event(
        &mut tui,
        &mut app_server,
        AppEvent::ModelsLoaded {
            request_id,
            result: Ok(hidden),
        },
    ))
    .await?;
    app.chat_widget.open_model_popup();
    assert!(app.chat_widget.no_modal_or_popup_active());
    let request = std::iter::from_fn(|| rx.try_recv().ok())
        .find(|event| matches!(event, AppEvent::FetchModels { .. }))
        .expect("catalog request");

    Box::pin(app.handle_event(&mut tui, &mut app_server, request)).await?;
    let mut loaded = tokio::time::timeout(Duration::from_secs(/*secs*/ 10), async {
        loop {
            if let Some(event @ AppEvent::ModelsLoaded { .. }) = rx.recv().await {
                break event;
            }
        }
    })
    .await?;
    let AppEvent::ModelsLoaded {
        result: Ok(models), ..
    } = &mut loaded
    else {
        panic!("model/list should return models");
    };
    assert!(!models.is_empty());
    for model in models.iter_mut() {
        model.is_default = model.model == "gpt-5.5";
        model.default_service_tier = model
            .is_default
            .then(|| ServiceTier::Fast.request_value().to_string());
    }
    let expected = models.clone();
    Box::pin(app.handle_event(&mut tui, &mut app_server, loaded)).await?;
    assert!(app.chat_widget.no_modal_or_popup_active());
    app.chat_widget.open_model_popup();
    assert!(!app.chat_widget.no_modal_or_popup_active());

    session.service_tier = fast.map(str::to_string);
    let channel = &app.thread_event_channels[&thread_id];
    let cached = channel.store.lock().await.session.clone();
    assert_eq!(
        (&app.primary_session_configured, &cached),
        (&Some(session.clone()), &Some(session.clone()))
    );
    app.chat_widget.handle_thread_session(test_thread_session(
        ThreadId::new(),
        test_path_buf("/tmp/other"),
    ));
    app.chat_widget.handle_thread_session(cached.unwrap());
    assert_eq!(app.chat_widget.current_service_tier(), fast);
    let stale = AppEvent::ModelsLoaded {
        request_id: uuid::Uuid::new_v4(),
        result: Ok(all_model_presets()),
    };
    Box::pin(app.handle_event(&mut tui, &mut app_server, stale)).await?;
    assert_eq!(
        (
            app.model_catalog.try_list_models().unwrap(),
            app.chat_widget.model_catalog().try_list_models().unwrap(),
        ),
        (expected.clone(), expected)
    );
    let mut config = app.config.clone();
    config.service_tier = None;
    config.features.enable(Feature::FastMode)?;
    for (model, expected_tier) in [(None, fast), (Some("gpt-5.6-terra"), Some("default"))] {
        config.model = model.map(str::to_string);
        let started = app_server.start_thread(&config).await?;
        assert_eq!(started.session.service_tier.as_deref(), expected_tier);
    }
    app_server.shutdown().await?;
    Ok(())
}

fn model_presets_with_test_upgrades() -> Vec<ModelPreset> {
    let mut presets = all_model_presets();
    let mut preset = presets
        .iter()
        .find(|preset| preset.model == "gpt-5.5")
        .expect("current model template")
        .clone();
    // Catalog-provided migration metadata is independent of the retired model's prompts.
    preset.id = "gpt-5.2".to_string();
    preset.model = "gpt-5.2".to_string();
    preset.upgrade = Some(ModelUpgrade {
        id: "gpt-5.5".to_string(),
        migration_config_key: "hide_test_migration_prompt".to_string(),
        model_link: None,
        upgrade_copy: None,
        migration_markdown: None,
        retirement_at: None,
    });
    presets.push(preset);
    presets
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
    let seen = BTreeMap::new();
    let presets = model_presets_with_test_upgrades();
    assert!(should_show_model_migration_prompt(
        "gpt-5.2", "gpt-5.5", &seen, &presets
    ));
    assert!(should_show_model_migration_prompt(
        "gpt-5.4",
        "gpt-6-sol",
        &seen,
        &presets
    ));
    assert!(!should_show_model_migration_prompt(
        "gpt-5.4", "gpt-5.4", &seen, &presets
    ));
}

#[test]
fn retired_model_migration_respects_catalog_metadata() {
    let mut presets = model_presets_with_test_upgrades();
    let current = presets
        .iter_mut()
        .find(|preset| preset.model == "gpt-5.2")
        .expect("test preset");
    current.id = "gpt-5.4-mini".to_string();
    current.model = "gpt-5.4-mini".to_string();
    let expected = current.upgrade.clone();
    assert_eq!(
        model_upgrade_for_migration("gpt-5.4-mini", &presets),
        expected
    );

    presets
        .iter_mut()
        .find(|preset| preset.model == "gpt-5.4-mini")
        .expect("catalog preset")
        .upgrade = None;
    assert_eq!(model_upgrade_for_migration("gpt-5.4-mini", &presets), None);
}

#[test]
fn select_model_availability_nux_picks_only_eligible_model() {
    let mut presets = all_model_presets();
    presets.iter_mut().for_each(|preset| {
        preset.availability_nux = None;
    });
    let target = presets
        .iter_mut()
        .find(|preset| preset.model == "gpt-5.6-terra")
        .expect("target preset present");
    target.availability_nux = Some(ModelAvailabilityNux {
        message: "gpt-5.6-terra is available".to_string(),
    });

    let selected = select_model_availability_nux(&presets, &model_availability_nux_config(&[]));

    assert_eq!(
        selected,
        Some(StartupTooltipOverride {
            model_slug: "gpt-5.6-terra".to_string(),
            message: "gpt-5.6-terra is available".to_string(),
        })
    );
}

#[test]
fn select_model_availability_nux_does_not_fall_back_to_older_announcement() {
    let mut presets = all_model_presets();
    presets.iter_mut().for_each(|preset| {
        preset.availability_nux = None;
    });
    let gpt_5 = presets
        .iter_mut()
        .find(|preset| preset.model == "gpt-5.6-terra")
        .expect("gpt-5.6-terra preset present");
    gpt_5.availability_nux = Some(ModelAvailabilityNux {
        message: "gpt-5.6-terra is available".to_string(),
    });
    let gpt_5_2 = presets
        .iter_mut()
        .find(|preset| preset.model == "gpt-5.5")
        .expect("gpt-5.5 preset present");
    gpt_5_2.availability_nux = Some(ModelAvailabilityNux {
        message: "gpt-5.5 is available".to_string(),
    });

    let selected = select_model_availability_nux(
        &presets,
        &model_availability_nux_config(&[("gpt-5.6-terra", MODEL_AVAILABILITY_NUX_MAX_SHOW_COUNT)]),
    );

    assert_eq!(selected, None);
}

#[test]
fn select_model_availability_nux_uses_existing_model_order_as_priority() {
    let mut presets = all_model_presets();
    presets.iter_mut().for_each(|preset| {
        preset.availability_nux = None;
    });
    let first = presets
        .iter_mut()
        .find(|preset| preset.model == "gpt-5.5")
        .expect("gpt-5.5 preset present");
    first.availability_nux = Some(ModelAvailabilityNux {
        message: "first".to_string(),
    });
    let second = presets
        .iter_mut()
        .find(|preset| preset.model == "gpt-5.6-terra")
        .expect("gpt-5.6-terra preset present");
    second.availability_nux = Some(ModelAvailabilityNux {
        message: "second".to_string(),
    });

    let selected = select_model_availability_nux(&presets, &model_availability_nux_config(&[]));

    assert_eq!(
        selected,
        Some(StartupTooltipOverride {
            model_slug: "gpt-5.6-terra".to_string(),
            message: "second".to_string(),
        })
    );
}

#[test]
fn select_model_availability_nux_returns_none_when_all_models_are_exhausted() {
    let mut presets = all_model_presets();
    presets.iter_mut().for_each(|preset| {
        preset.availability_nux = None;
    });
    let target = presets
        .iter_mut()
        .find(|preset| preset.model == "gpt-5.6-terra")
        .expect("target preset present");
    target.availability_nux = Some(ModelAvailabilityNux {
        message: "gpt-5.6-terra is available".to_string(),
    });

    let selected = select_model_availability_nux(
        &presets,
        &model_availability_nux_config(&[("gpt-5.6-terra", MODEL_AVAILABILITY_NUX_MAX_SHOW_COUNT)]),
    );

    assert_eq!(selected, None);
}

#[tokio::test]
async fn prepare_startup_tooltip_override_persists_model_availability_nux_count() {
    let codex_home = tempdir().expect("temp codex home");
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .build()
        .await
        .expect("config");
    let mut presets = all_model_presets();
    presets.iter_mut().for_each(|preset| {
        preset.availability_nux = None;
    });
    let target = presets
        .iter_mut()
        .find(|preset| preset.model == "gpt-5.6-terra")
        .expect("target preset present");
    target.availability_nux = Some(ModelAvailabilityNux {
        message: "gpt-5.6-terra is available".to_string(),
    });

    let mut local_settings = crate::local_settings::LocalSettings::from(&config);
    let tooltip = prepare_startup_tooltip_override(
        &mut local_settings,
        &presets,
        /*is_first_run*/ false,
    )
    .await;

    assert_eq!(tooltip.as_deref(), Some("gpt-5.6-terra is available"));
    assert_eq!(
        local_settings.tui.model_availability_nux.shown_count,
        HashMap::from([("gpt-5.6-terra".to_string(), 1)])
    );

    let reloaded = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .build()
        .await
        .expect("reloaded config");
    assert_eq!(
        reloaded.model_availability_nux.shown_count,
        HashMap::from([("gpt-5.6-terra".to_string(), 1)])
    );
}

#[tokio::test]
async fn accepted_model_migration_persists_target_default_reasoning_effort() -> Result<()> {
    let presets = all_model_presets();
    let mut migration_copies = Vec::new();
    for (model, replacement) in [
        ("gpt-5.4", "gpt-6-sol"),
        ("gpt-5.4-mini", "gpt-6-luna"),
        ("gpt-5.5", "gpt-6-sol"),
        ("gpt-5.6-luna", "gpt-6-luna"),
        ("gpt-5.6-terra", "gpt-6-sol"),
        ("gpt-5.6-sol", "gpt-6-sol"),
    ] {
        let codex_home = tempdir()?;
        std::fs::write(
            codex_home.path().join("config.toml"),
            format!("model = \"{model}\"\nmodel_reasoning_effort = \"xhigh\"\n"),
        )?;
        let mut config = ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .build()
            .await?;
        let current_model = config.model.clone().expect("saved model");
        let upgrade = model_upgrade_for_migration(&current_model, &presets)
            .expect("catalog selection should have an upgrade");
        assert_eq!(upgrade.id, replacement);
        assert!(should_show_model_migration_prompt(
            &current_model,
            &upgrade.id,
            &BTreeMap::new(),
            &presets,
        ));
        let target =
            target_preset_for_upgrade(&presets, &upgrade.id).expect("available replacement");
        let target_effort = target.default_reasoning_effort.clone();
        let can_opt_out = presets.iter().any(|preset| preset.model == current_model);
        let copy = migration_copy_for_models(
            &current_model,
            &upgrade.id,
            upgrade.model_link,
            upgrade.upgrade_copy,
            upgrade.migration_markdown,
            target.display_name.clone(),
            Some(target.description.clone()),
            can_opt_out,
        );
        migration_copies.push(model_migration_copy_to_plain_text(&copy));

        let (tx_raw, mut rx) = unbounded_channel();
        let app_event_tx = AppEventSender::new(tx_raw);
        apply_accepted_model_migration(
            &mut config,
            &app_event_tx,
            current_model,
            upgrade.id,
            target_effort.clone(),
        );

        assert_eq!(config.model.as_deref(), Some(replacement));
        assert_eq!(config.model_reasoning_effort, Some(target_effort.clone()));

        let acknowledged = rx.try_recv().expect("acknowledged event");
        assert_matches!(
            acknowledged,
            AppEvent::PersistModelMigrationPromptAcknowledged { from_model, to_model }
                if from_model == model && to_model == replacement
        );

        let update_model = rx.try_recv().expect("update model event");
        assert_matches!(
            update_model,
            AppEvent::UpdateModel(updated_model) if updated_model == replacement
        );

        let update_effort = rx.try_recv().expect("update effort event");
        assert_matches!(
            update_effort,
            AppEvent::UpdateReasoningEffort(Some(effort)) if effort == target_effort
        );

        let persist_selection = rx.try_recv().expect("persist model selection event");
        assert_matches!(
            persist_selection,
            AppEvent::PersistModelSelection { model: selected_model, effort }
                if selected_model == replacement && effort == Some(target_effort)
        );
    }
    assert_snapshot!(migration_copies.join("\n"), @r"
    GPT-5.4 is no longer available

    Codex now uses GPT-6 Sol in place of GPT-5.4. Switch to GPT-6 Sol to continue.

    GPT-5.4 Mini is no longer available

    Codex now uses GPT-6 Luna in place of GPT-5.4 Mini. Switch to GPT-6 Luna to continue.

    Meet GPT-6 Sol

    Our latest Sol is more intelligent and more efficient so your usage limits go further. This model is a great daily driver for complex tasks, especially coding.

    Meet GPT-6 Luna

    Our latest Luna is significantly more efficient, so your usage limits go even further. Reach for it for any job that doesn't require frontier intelligence.

    Meet GPT-6 Sol

    Our latest Sol is more intelligent and more efficient so your usage limits go further. This model is a great daily driver for complex tasks, especially coding.

    Meet GPT-6 Sol

    Our latest Sol is more intelligent and more efficient so your usage limits go further. This model is a great daily driver for complex tasks, especially coding.
    ");
    Ok(())
}

#[tokio::test]
async fn model_migration_prompt_respects_hide_flag_and_self_target() {
    let presets = all_model_presets();
    let mut seen = BTreeMap::new();
    // A previously acknowledged intermediate upgrade must not hide the new target.
    seen.insert("gpt-5.4-mini".to_string(), "gpt-5.6-luna".to_string());
    assert!(should_show_model_migration_prompt(
        "gpt-5.4-mini",
        "gpt-6-luna",
        &seen,
        &presets
    ));
    seen.insert("gpt-5.4-mini".to_string(), "gpt-6-luna".to_string());
    assert!(!should_show_model_migration_prompt(
        "gpt-5.4-mini",
        "gpt-6-luna",
        &seen,
        &presets
    ));
    assert!(!should_show_model_migration_prompt(
        "gpt-6-luna",
        "gpt-6-luna",
        &seen,
        &presets
    ));
}

#[tokio::test]
async fn model_migration_prompt_skips_when_target_missing_or_hidden() {
    let mut available = all_model_presets();
    available.retain(|preset| preset.model != "gpt-6-luna");

    assert!(!should_show_model_migration_prompt(
        "gpt-5.4-mini",
        "gpt-6-luna",
        &BTreeMap::new(),
        &available,
    ));

    assert!(target_preset_for_upgrade(&available, "gpt-6-luna").is_none());

    let mut with_hidden_target = all_model_presets();
    let target = with_hidden_target
        .iter_mut()
        .find(|preset| preset.model == "gpt-6-luna")
        .expect("target preset present");
    target.show_in_picker = false;

    assert!(!should_show_model_migration_prompt(
        "gpt-5.4-mini",
        "gpt-6-luna",
        &BTreeMap::new(),
        &with_hidden_target,
    ));
    assert!(target_preset_for_upgrade(&with_hidden_target, "gpt-6-luna").is_none());
}

#[tokio::test]
async fn model_migration_prompt_shows_for_hidden_model() {
    let codex_home = tempdir().expect("temp codex home");
    let config = ConfigBuilder::default()
        .codex_home(codex_home.path().to_path_buf())
        .build()
        .await
        .expect("config");

    let mut available_models = model_presets_with_test_upgrades();
    let current = available_models
        .iter_mut()
        .find(|preset| preset.model == "gpt-5.2")
        .expect("gpt-5.2 preset present");
    current.show_in_picker = false;
    let current = current.clone();
    assert!(
        !current.show_in_picker,
        "expected gpt-5.2 to be hidden from picker for this test"
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
