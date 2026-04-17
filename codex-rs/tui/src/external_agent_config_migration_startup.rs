use crate::app_server_session::AppServerSession;
use crate::external_agent_config_migration::ExternalAgentConfigMigrationOutcome;
use crate::external_agent_config_migration::run_external_agent_config_migration_prompt;
use crate::legacy_core::config::Config;
use crate::legacy_core::config::ConfigBuilder;
use crate::legacy_core::config::ConfigOverrides;
use crate::legacy_core::config::edit::ConfigEdit;
use crate::legacy_core::config::edit::ConfigEditsBuilder;
use crate::tui;
use codex_app_server_protocol::ExternalAgentConfigDetectParams;
use codex_app_server_protocol::ExternalAgentConfigMigrationItem;
use codex_features::Feature;
use color_eyre::eyre::Result;
use color_eyre::eyre::WrapErr;
use std::collections::BTreeSet;
use std::path::Path;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use toml::Value as TomlValue;

const EXTERNAL_CONFIG_MIGRATION_PROMPT_COOLDOWN_SECS: i64 = 5 * 24 * 60 * 60;

pub(crate) enum ExternalAgentConfigMigrationStartupOutcome {
    Continue { success_message: Option<String> },
    ExitRequested,
}

fn external_config_migration_project_key(path: &Path) -> String {
    path.display().to_string()
}

fn is_external_config_migration_scope_hidden(config: &Config, cwd: Option<&Path>) -> bool {
    match cwd {
        Some(cwd) => config
            .notices
            .external_config_migration_prompts
            .projects
            .get(&external_config_migration_project_key(cwd))
            .copied()
            .unwrap_or(false),
        None => config
            .notices
            .external_config_migration_prompts
            .home
            .unwrap_or(false),
    }
}

fn external_config_migration_last_prompted_at(config: &Config, cwd: Option<&Path>) -> Option<i64> {
    match cwd {
        Some(cwd) => config
            .notices
            .external_config_migration_prompts
            .project_last_prompted_at
            .get(&external_config_migration_project_key(cwd))
            .copied(),
        None => {
            config
                .notices
                .external_config_migration_prompts
                .home_last_prompted_at
        }
    }
}

fn is_external_config_migration_scope_cooling_down(
    config: &Config,
    cwd: Option<&Path>,
    now_unix_seconds: i64,
) -> bool {
    external_config_migration_last_prompted_at(config, cwd).is_some_and(|last_prompted_at| {
        last_prompted_at.saturating_add(EXTERNAL_CONFIG_MIGRATION_PROMPT_COOLDOWN_SECS)
            > now_unix_seconds
    })
}

fn visible_external_agent_config_migration_items(
    config: &Config,
    items: Vec<ExternalAgentConfigMigrationItem>,
    now_unix_seconds: i64,
) -> Vec<ExternalAgentConfigMigrationItem> {
    items
        .into_iter()
        .filter(|item| {
            !is_external_config_migration_scope_hidden(config, item.cwd.as_deref())
                && !is_external_config_migration_scope_cooling_down(
                    config,
                    item.cwd.as_deref(),
                    now_unix_seconds,
                )
        })
        .collect()
}

fn external_agent_config_migration_success_message(
    items: &[ExternalAgentConfigMigrationItem],
) -> String {
    if items.iter().any(|item| {
        item.item_type == codex_app_server_protocol::ExternalAgentConfigMigrationItemType::Plugins
    }) {
        "External config migration completed. Plugin migration is still in progress and may take a few minutes."
            .to_string()
    } else {
        "External config migration completed successfully.".to_string()
    }
}

fn unix_seconds_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

async fn persist_external_agent_config_migration_prompt_shown(
    config: &mut Config,
    items: &[ExternalAgentConfigMigrationItem],
    now_unix_seconds: i64,
) -> Result<()> {
    let mut edits = Vec::new();
    if items.iter().any(|item| item.cwd.is_none()) {
        edits.push(
            ConfigEdit::SetNoticeExternalConfigMigrationPromptHomeLastPromptedAt(now_unix_seconds),
        );
    }

    for project in items
        .iter()
        .filter_map(|item| item.cwd.as_deref())
        .map(external_config_migration_project_key)
    {
        edits.push(
            ConfigEdit::SetNoticeExternalConfigMigrationPromptProjectLastPromptedAt(
                project,
                now_unix_seconds,
            ),
        );
    }

    if edits.is_empty() {
        return Ok(());
    }

    ConfigEditsBuilder::new(&config.codex_home)
        .with_edits(edits)
        .apply()
        .await
        .map_err(|err| color_eyre::eyre::eyre!("{err}"))
        .wrap_err("Failed to save external config migration prompt timestamp")?;

    if items.iter().any(|item| item.cwd.is_none()) {
        config
            .notices
            .external_config_migration_prompts
            .home_last_prompted_at = Some(now_unix_seconds);
    }
    for project in items
        .iter()
        .filter_map(|item| item.cwd.as_deref())
        .map(external_config_migration_project_key)
    {
        config
            .notices
            .external_config_migration_prompts
            .project_last_prompted_at
            .insert(project, now_unix_seconds);
    }

    Ok(())
}

async fn persist_external_agent_config_migration_prompt_dismissal(
    config: &mut Config,
    items: &[ExternalAgentConfigMigrationItem],
) -> Result<()> {
    let hide_home = items.iter().any(|item| item.cwd.is_none());
    let projects = items
        .iter()
        .filter_map(|item| item.cwd.as_deref())
        .map(external_config_migration_project_key)
        .collect::<BTreeSet<_>>();

    let mut edits = Vec::new();
    if hide_home
        && !config
            .notices
            .external_config_migration_prompts
            .home
            .unwrap_or(false)
    {
        edits.push(ConfigEdit::SetNoticeHideExternalConfigMigrationPromptHome(
            true,
        ));
    }
    for project in &projects {
        if !config
            .notices
            .external_config_migration_prompts
            .projects
            .get(project)
            .copied()
            .unwrap_or(false)
        {
            edits.push(
                ConfigEdit::SetNoticeHideExternalConfigMigrationPromptProject(
                    project.clone(),
                    true,
                ),
            );
        }
    }

    if edits.is_empty() {
        return Ok(());
    }

    ConfigEditsBuilder::new(&config.codex_home)
        .with_edits(edits)
        .apply()
        .await
        .map_err(|err| color_eyre::eyre::eyre!("{err}"))
        .wrap_err("Failed to save external config migration prompt preference")?;

    if hide_home {
        config.notices.external_config_migration_prompts.home = Some(true);
    }
    for project in projects {
        config
            .notices
            .external_config_migration_prompts
            .projects
            .insert(project, true);
    }

    Ok(())
}

pub(crate) async fn handle_external_agent_config_migration_prompt_if_needed(
    tui: &mut tui::Tui,
    app_server: &mut AppServerSession,
    config: &mut Config,
    cli_kv_overrides: &[(String, TomlValue)],
    harness_overrides: &ConfigOverrides,
) -> Result<ExternalAgentConfigMigrationStartupOutcome> {
    if !config.features.enabled(Feature::ExternalMigration) {
        return Ok(ExternalAgentConfigMigrationStartupOutcome::Continue {
            success_message: None,
        });
    }

    let now_unix_seconds = unix_seconds_now();
    let detected_items = match app_server
        .external_agent_config_detect(ExternalAgentConfigDetectParams {
            include_home: true,
            cwds: Some(vec![config.cwd.to_path_buf()]),
        })
        .await
    {
        Ok(response) => {
            visible_external_agent_config_migration_items(config, response.items, now_unix_seconds)
        }
        Err(err) => {
            tracing::warn!(
                error = %err,
                cwd = %config.cwd.display(),
                "failed to detect external agent config migrations; continuing startup"
            );
            return Ok(ExternalAgentConfigMigrationStartupOutcome::Continue {
                success_message: None,
            });
        }
    };

    if detected_items.is_empty() {
        return Ok(ExternalAgentConfigMigrationStartupOutcome::Continue {
            success_message: None,
        });
    }

    if let Err(err) = persist_external_agent_config_migration_prompt_shown(
        config,
        &detected_items,
        now_unix_seconds,
    )
    .await
    {
        tracing::warn!(
            error = %err,
            cwd = %config.cwd.display(),
            "failed to persist external config migration prompt timestamp"
        );
    }

    let mut selected_items = detected_items.clone();
    let mut error: Option<String> = None;

    loop {
        match run_external_agent_config_migration_prompt(
            tui,
            &detected_items,
            &selected_items,
            error.as_deref(),
        )
        .await
        {
            ExternalAgentConfigMigrationOutcome::Proceed(items) => {
                selected_items = items.clone();
                match app_server.external_agent_config_import(items).await {
                    Ok(_) => {
                        let success_message =
                            external_agent_config_migration_success_message(&selected_items);
                        *config = ConfigBuilder::default()
                            .codex_home(config.codex_home.to_path_buf())
                            .cli_overrides(cli_kv_overrides.to_vec())
                            .harness_overrides(harness_overrides.clone())
                            .build()
                            .await
                            .wrap_err("Failed to reload config after external agent migration")?;
                        return Ok(ExternalAgentConfigMigrationStartupOutcome::Continue {
                            success_message: Some(success_message),
                        });
                    }
                    Err(err) => {
                        tracing::warn!(
                            error = %err,
                            cwd = %config.cwd.display(),
                            "failed to import external agent config migration items"
                        );
                        error = Some(format!("Migration failed: {err}"));
                    }
                }
            }
            ExternalAgentConfigMigrationOutcome::Skip => {
                return Ok(ExternalAgentConfigMigrationStartupOutcome::Continue {
                    success_message: None,
                });
            }
            ExternalAgentConfigMigrationOutcome::SkipForever => {
                match persist_external_agent_config_migration_prompt_dismissal(
                    config,
                    &detected_items,
                )
                .await
                {
                    Ok(()) => {
                        return Ok(ExternalAgentConfigMigrationStartupOutcome::Continue {
                            success_message: None,
                        });
                    }
                    Err(err) => {
                        tracing::warn!(
                            error = %err,
                            cwd = %config.cwd.display(),
                            "failed to persist external config migration prompt dismissal"
                        );
                        error = Some(format!("Failed to save preference: {err}"));
                    }
                }
            }
            ExternalAgentConfigMigrationOutcome::Exit => {
                return Ok(ExternalAgentConfigMigrationStartupOutcome::ExitRequested);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_app_server_protocol::ExternalAgentConfigMigrationItemType;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[tokio::test]
    async fn visible_external_agent_config_migration_items_omits_hidden_scopes() {
        let codex_home = tempdir().expect("temp codex home");
        let mut config = ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .build()
            .await
            .expect("config");
        config.notices.external_config_migration_prompts.home = Some(true);
        config
            .notices
            .external_config_migration_prompts
            .projects
            .insert("/tmp/project".to_string(), true);

        let visible = visible_external_agent_config_migration_items(
            &config,
            vec![
                ExternalAgentConfigMigrationItem {
                    item_type: ExternalAgentConfigMigrationItemType::Config,
                    description: "home".to_string(),
                    cwd: None,
                    details: None,
                },
                ExternalAgentConfigMigrationItem {
                    item_type: ExternalAgentConfigMigrationItemType::AgentsMd,
                    description: "project".to_string(),
                    cwd: Some(PathBuf::from("/tmp/project")),
                    details: None,
                },
                ExternalAgentConfigMigrationItem {
                    item_type: ExternalAgentConfigMigrationItemType::Skills,
                    description: "other project".to_string(),
                    cwd: Some(PathBuf::from("/tmp/other")),
                    details: None,
                },
            ],
            /*now_unix_seconds*/ 1_760_000_000,
        );

        assert_eq!(
            visible,
            vec![ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::Skills,
                description: "other project".to_string(),
                cwd: Some(PathBuf::from("/tmp/other")),
                details: None,
            }]
        );
    }

    #[tokio::test]
    async fn visible_external_agent_config_migration_items_omits_recently_prompted_scopes() {
        let codex_home = tempdir().expect("temp codex home");
        let mut config = ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .build()
            .await
            .expect("config");
        config
            .notices
            .external_config_migration_prompts
            .home_last_prompted_at = Some(1_760_000_000);
        config
            .notices
            .external_config_migration_prompts
            .project_last_prompted_at
            .insert("/tmp/project".to_string(), 1_760_000_000);

        let visible = visible_external_agent_config_migration_items(
            &config,
            vec![
                ExternalAgentConfigMigrationItem {
                    item_type: ExternalAgentConfigMigrationItemType::Config,
                    description: "home".to_string(),
                    cwd: None,
                    details: None,
                },
                ExternalAgentConfigMigrationItem {
                    item_type: ExternalAgentConfigMigrationItemType::AgentsMd,
                    description: "project".to_string(),
                    cwd: Some(PathBuf::from("/tmp/project")),
                    details: None,
                },
                ExternalAgentConfigMigrationItem {
                    item_type: ExternalAgentConfigMigrationItemType::Skills,
                    description: "other project".to_string(),
                    cwd: Some(PathBuf::from("/tmp/other")),
                    details: None,
                },
            ],
            /*now_unix_seconds*/
            1_760_000_000 + EXTERNAL_CONFIG_MIGRATION_PROMPT_COOLDOWN_SECS - 1,
        );

        assert_eq!(
            visible,
            vec![ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::Skills,
                description: "other project".to_string(),
                cwd: Some(PathBuf::from("/tmp/other")),
                details: None,
            }]
        );
    }

    #[tokio::test]
    async fn external_config_migration_scope_cooldown_expires_after_five_days() {
        let codex_home = tempdir().expect("temp codex home");
        let mut config = ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .build()
            .await
            .expect("config");
        config
            .notices
            .external_config_migration_prompts
            .home_last_prompted_at = Some(1_760_000_000);

        assert!(is_external_config_migration_scope_cooling_down(
            &config,
            /*cwd*/ None,
            1_760_000_000 + EXTERNAL_CONFIG_MIGRATION_PROMPT_COOLDOWN_SECS - 1,
        ));
        assert!(!is_external_config_migration_scope_cooling_down(
            &config,
            /*cwd*/ None,
            1_760_000_000 + EXTERNAL_CONFIG_MIGRATION_PROMPT_COOLDOWN_SECS,
        ));
    }

    #[test]
    fn external_agent_config_migration_success_message_mentions_plugins_when_present() {
        let message = external_agent_config_migration_success_message(&[
            ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::Config,
                description: String::new(),
                cwd: None,
                details: None,
            },
            ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::Plugins,
                description: String::new(),
                cwd: None,
                details: None,
            },
        ]);

        assert_eq!(
            message,
            "External config migration completed. Plugin migration is still in progress and may take a few minutes."
        );
    }

    #[test]
    fn external_agent_config_migration_success_message_omits_plugins_copy_when_absent() {
        let message =
            external_agent_config_migration_success_message(&[ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::AgentsMd,
                description: String::new(),
                cwd: None,
                details: None,
            }]);

        assert_eq!(message, "External config migration completed successfully.");
    }
}
