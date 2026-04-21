//! Startup warnings, model migration prompts, and bootstrap prompt helpers.
//!
//! These helpers run before or during `App::run` bootstrap. They translate configuration and model
//! catalog state into one-time TUI prompts or warning cells without owning the main event loop.

use super::*;

pub(super) fn emit_skill_load_warnings(app_event_tx: &AppEventSender, errors: &[SkillErrorInfo]) {
    if errors.is_empty() {
        return;
    }

    let error_count = errors.len();
    app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
        crate::history_cell::new_warning_event(format!(
            "Skipped loading {error_count} skill(s) due to invalid SKILL.md files."
        )),
    )));

    for error in errors {
        let path = error.path.display();
        let message = error.message.as_str();
        app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
            crate::history_cell::new_warning_event(format!("{path}: {message}")),
        )));
    }
}

pub(super) fn emit_project_config_warnings(app_event_tx: &AppEventSender, config: &Config) {
    let mut disabled_folders = Vec::new();

    for layer in config.config_layer_stack.get_layers(
        ConfigLayerStackOrdering::LowestPrecedenceFirst,
        /*include_disabled*/ true,
    ) {
        let ConfigLayerSource::Project { dot_codex_folder } = &layer.name else {
            continue;
        };
        let Some(disabled_reason) = &layer.disabled_reason else {
            continue;
        };
        disabled_folders.push((
            dot_codex_folder.as_path().display().to_string(),
            disabled_reason.clone(),
        ));
    }

    if disabled_folders.is_empty() {
        return;
    }

    let mut message = concat!(
        "Project-local config, hooks, and exec policies are disabled in the following folders ",
        "until the project is trusted, but skills still load.\n",
    )
    .to_string();
    for (index, (folder, reason)) in disabled_folders.iter().enumerate() {
        let display_index = index + 1;
        message.push_str(&format!("    {display_index}. {folder}\n"));
        message.push_str(&format!("       {reason}\n"));
    }

    app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
        history_cell::new_warning_event(message),
    )));
}

pub(super) fn emit_system_bwrap_warning(app_event_tx: &AppEventSender, config: &Config) {
    let Some(message) =
        crate::legacy_core::config::system_bwrap_warning(config.permissions.sandbox_policy.get())
    else {
        return;
    };

    app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
        history_cell::new_warning_event(message),
    )));
}

pub(super) fn should_show_model_migration_prompt(
    current_model: &str,
    target_model: &str,
    seen_migrations: &BTreeMap<String, String>,
    available_models: &[ModelPreset],
) -> bool {
    if target_model == current_model {
        return false;
    }

    if let Some(seen_target) = seen_migrations.get(current_model)
        && seen_target == target_model
    {
        return false;
    }

    if !available_models
        .iter()
        .any(|preset| preset.model == target_model && preset.show_in_picker)
    {
        return false;
    }

    if available_models
        .iter()
        .any(|preset| preset.model == current_model && preset.upgrade.is_some())
    {
        return true;
    }

    if available_models
        .iter()
        .any(|preset| preset.upgrade.as_ref().map(|u| u.id.as_str()) == Some(target_model))
    {
        return true;
    }

    false
}

pub(super) fn migration_prompt_hidden(config: &Config, migration_config_key: &str) -> bool {
    match migration_config_key {
        HIDE_GPT_5_1_CODEX_MAX_MIGRATION_PROMPT_CONFIG => config
            .notices
            .hide_gpt_5_1_codex_max_migration_prompt
            .unwrap_or(false),
        HIDE_GPT5_1_MIGRATION_PROMPT_CONFIG => {
            config.notices.hide_gpt5_1_migration_prompt.unwrap_or(false)
        }
        _ => false,
    }
}

pub(super) fn target_preset_for_upgrade<'a>(
    available_models: &'a [ModelPreset],
    target_model: &str,
) -> Option<&'a ModelPreset> {
    available_models
        .iter()
        .find(|preset| preset.model == target_model && preset.show_in_picker)
}

pub(super) const MODEL_AVAILABILITY_NUX_MAX_SHOW_COUNT: u32 = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StartupTooltipOverride {
    pub(super) model_slug: String,
    pub(super) message: String,
}

pub(super) fn select_model_availability_nux(
    available_models: &[ModelPreset],
    nux_config: &ModelAvailabilityNuxConfig,
) -> Option<StartupTooltipOverride> {
    available_models.iter().find_map(|preset| {
        let ModelAvailabilityNux { message } = preset.availability_nux.as_ref()?;
        let shown_count = nux_config
            .shown_count
            .get(&preset.model)
            .copied()
            .unwrap_or_default();
        (shown_count < MODEL_AVAILABILITY_NUX_MAX_SHOW_COUNT).then(|| StartupTooltipOverride {
            model_slug: preset.model.clone(),
            message: message.clone(),
        })
    })
}

pub(super) async fn prepare_startup_tooltip_override(
    config: &mut Config,
    available_models: &[ModelPreset],
    is_first_run: bool,
) -> Option<String> {
    if is_first_run || !config.show_tooltips {
        return None;
    }

    let tooltip_override =
        select_model_availability_nux(available_models, &config.model_availability_nux)?;

    let shown_count = config
        .model_availability_nux
        .shown_count
        .get(&tooltip_override.model_slug)
        .copied()
        .unwrap_or_default();
    let next_count = shown_count.saturating_add(1);
    let mut updated_shown_count = config.model_availability_nux.shown_count.clone();
    updated_shown_count.insert(tooltip_override.model_slug.clone(), next_count);

    if let Err(err) = ConfigEditsBuilder::for_config(config)
        .set_model_availability_nux_count(&updated_shown_count)
        .apply()
        .await
    {
        tracing::error!(
            error = %err,
            model = %tooltip_override.model_slug,
            "failed to persist model availability nux count"
        );
        return Some(tooltip_override.message);
    }

    config.model_availability_nux.shown_count = updated_shown_count;
    Some(tooltip_override.message)
}

pub(super) async fn handle_model_migration_prompt_if_needed(
    tui: &mut tui::Tui,
    config: &mut Config,
    model: &str,
    app_event_tx: &AppEventSender,
    available_models: &[ModelPreset],
) -> Option<AppExitInfo> {
    let upgrade = available_models
        .iter()
        .find(|preset| preset.model == model)
        .and_then(|preset| preset.upgrade.as_ref());

    if let Some(ModelUpgrade {
        id: target_model,
        reasoning_effort_mapping,
        migration_config_key,
        model_link,
        upgrade_copy,
        migration_markdown,
    }) = upgrade
    {
        if migration_prompt_hidden(config, migration_config_key.as_str()) {
            return None;
        }

        let target_model = target_model.to_string();
        if !should_show_model_migration_prompt(
            model,
            &target_model,
            &config.notices.model_migrations,
            available_models,
        ) {
            return None;
        }

        let current_preset = available_models.iter().find(|preset| preset.model == model);
        let target_preset = target_preset_for_upgrade(available_models, &target_model);
        let target_preset = target_preset?;
        let target_display_name = target_preset.display_name.clone();
        let heading_label = if target_display_name == model {
            target_model.clone()
        } else {
            target_display_name.clone()
        };
        let target_description =
            (!target_preset.description.is_empty()).then(|| target_preset.description.clone());
        let can_opt_out = current_preset.is_some();
        let prompt_copy = migration_copy_for_models(
            model,
            &target_model,
            model_link.clone(),
            upgrade_copy.clone(),
            migration_markdown.clone(),
            heading_label,
            target_description,
            can_opt_out,
        );
        match run_model_migration_prompt(tui, prompt_copy).await {
            ModelMigrationOutcome::Accepted => {
                app_event_tx.send(AppEvent::PersistModelMigrationPromptAcknowledged {
                    from_model: model.to_string(),
                    to_model: target_model.clone(),
                });

                let mapped_effort = if let Some(reasoning_effort_mapping) = reasoning_effort_mapping
                    && let Some(reasoning_effort) = config.model_reasoning_effort
                {
                    reasoning_effort_mapping
                        .get(&reasoning_effort)
                        .cloned()
                        .or(config.model_reasoning_effort)
                } else {
                    config.model_reasoning_effort
                };

                config.model = Some(target_model.clone());
                config.model_reasoning_effort = mapped_effort;
                app_event_tx.send(AppEvent::UpdateModel(target_model.clone()));
                app_event_tx.send(AppEvent::UpdateReasoningEffort(mapped_effort));
                app_event_tx.send(AppEvent::PersistModelSelection {
                    model: target_model.clone(),
                    effort: mapped_effort,
                });
            }
            ModelMigrationOutcome::Rejected => {
                app_event_tx.send(AppEvent::PersistModelMigrationPromptAcknowledged {
                    from_model: model.to_string(),
                    to_model: target_model.clone(),
                });
            }
            ModelMigrationOutcome::Exit => {
                return Some(AppExitInfo {
                    token_usage: TokenUsage::default(),
                    thread_id: None,
                    thread_name: None,
                    update_action: None,
                    exit_reason: ExitReason::UserRequested,
                });
            }
        }
    }

    None
}
pub(super) fn normalize_harness_overrides_for_cwd(
    mut overrides: ConfigOverrides,
    base_cwd: &AbsolutePathBuf,
) -> Result<ConfigOverrides> {
    if overrides.additional_writable_roots.is_empty() {
        return Ok(overrides);
    }

    let mut normalized = Vec::with_capacity(overrides.additional_writable_roots.len());
    for root in overrides.additional_writable_roots.drain(..) {
        let absolute = base_cwd.join(root);
        normalized.push(absolute.into_path_buf());
    }
    overrides.additional_writable_roots = normalized;
    Ok(overrides)
}
