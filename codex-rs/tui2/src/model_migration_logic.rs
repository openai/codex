use codex_core::config::Config;
use codex_core::models_manager::model_presets::HIDE_GPT_5_1_CODEX_MAX_MIGRATION_PROMPT_CONFIG;
use codex_core::models_manager::model_presets::HIDE_GPT5_1_MIGRATION_PROMPT_CONFIG;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ModelUpgrade;
use serde::Deserialize;
use serde::Serialize;
use std::io;
use std::path::PathBuf;

const PENDING_MODEL_MIGRATION_NOTICE_FILENAME: &str = "pending_model_migration_notice.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PendingModelMigrationNotice {
    pub(crate) from_model: String,
    pub(crate) to_model: String,
    // Used to respect hide flags even if config changes between scheduling and display.
    #[serde(default)]
    pub(crate) migration_config_key: Option<String>,
}

fn pending_model_migration_notice_path(config: &Config) -> PathBuf {
    config
        .codex_home
        .join(PENDING_MODEL_MIGRATION_NOTICE_FILENAME)
}

fn migration_prompt_hidden(config: &Config, migration_config_key: &str) -> bool {
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

fn should_show_model_migration_notice(
    current_model: &str,
    target_model: &str,
    available_models: &[ModelPreset],
    config: &Config,
) -> bool {
    if target_model == current_model {
        return false;
    }

    if let Some(seen_target) = config.notices.model_migrations.get(current_model)
        && seen_target == target_model
    {
        return false;
    }

    if available_models
        .iter()
        .any(|preset| preset.model == current_model && preset.upgrade.is_some())
    {
        return true;
    }

    available_models
        .iter()
        .any(|preset| preset.upgrade.as_ref().map(|u| u.id.as_str()) == Some(target_model))
}

/// Read and clear the one-shot migration notice file, returning the notice if it should be shown.
///
/// If the notice is returned, this also updates `config.notices.model_migrations` to prevent
/// re-scheduling within the current process.
pub(crate) fn take_pending_model_migration_notice(
    config: &mut Config,
) -> Option<PendingModelMigrationNotice> {
    let notice_path = pending_model_migration_notice_path(config);
    let contents = match std::fs::read_to_string(&notice_path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return None,
        Err(err) => {
            tracing::error!(
                error = %err,
                notice_path = %notice_path.display(),
                "failed to read pending model migration notice"
            );
            return None;
        }
    };

    let notice: PendingModelMigrationNotice = match serde_json::from_str(&contents) {
        Ok(notice) => notice,
        Err(err) => {
            tracing::error!(
                error = %err,
                notice_path = %notice_path.display(),
                "failed to parse pending model migration notice"
            );
            return None;
        }
    };

    if let Some(migration_config_key) = notice.migration_config_key.as_deref()
        && migration_prompt_hidden(config, migration_config_key)
    {
        let _ = std::fs::remove_file(&notice_path);
        return None;
    }

    if let Some(seen_target) = config.notices.model_migrations.get(&notice.from_model)
        && seen_target == &notice.to_model
    {
        let _ = std::fs::remove_file(&notice_path);
        return None;
    }

    // Best-effort: clear the one-shot file so it doesn't appear again.
    let _ = std::fs::remove_file(&notice_path);

    config
        .notices
        .model_migrations
        .insert(notice.from_model.clone(), notice.to_model.clone());

    Some(notice)
}

pub(crate) fn maybe_schedule_model_migration_notice(
    config: &Config,
    current_model: &str,
    available_models: &[ModelPreset],
) {
    let Some(ModelUpgrade {
        id: target_model,
        migration_config_key,
        ..
    }) = available_models
        .iter()
        .find(|preset| preset.model == current_model)
        .and_then(|preset| preset.upgrade.as_ref())
    else {
        return;
    };

    if migration_prompt_hidden(config, migration_config_key.as_str()) {
        return;
    }

    if available_models
        .iter()
        .all(|preset| preset.model != target_model.as_str())
    {
        return;
    }

    if !should_show_model_migration_notice(
        current_model,
        target_model.as_str(),
        available_models,
        config,
    ) {
        return;
    }

    let notice_path = pending_model_migration_notice_path(config);
    if notice_path.exists() {
        return;
    }

    let notice = PendingModelMigrationNotice {
        from_model: current_model.to_string(),
        to_model: target_model.to_string(),
        migration_config_key: Some(migration_config_key.to_string()),
    };
    let Ok(json_line) = serde_json::to_string(&notice).map(|json| format!("{json}\n")) else {
        return;
    };

    if let Some(parent) = notice_path.parent()
        && let Err(err) = std::fs::create_dir_all(parent)
    {
        tracing::error!(
            error = %err,
            notice_path = %notice_path.display(),
            "failed to create directory for pending model migration notice"
        );
        return;
    }

    if let Err(err) = std::fs::write(&notice_path, json_line) {
        tracing::error!(
            error = %err,
            notice_path = %notice_path.display(),
            "failed to persist pending model migration notice"
        );
    }
}
