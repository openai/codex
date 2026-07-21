use crate::app_server_session::AppServerSession;
use crate::app_server_session::EXTERNAL_AGENT_CONFIG_IMPORT_IN_PROGRESS_MESSAGE;
use crate::external_agent_config_migration::ExternalAgentConfigMigrationOutcome;
use crate::external_agent_config_migration::run_external_agent_config_migration_prompt;
use crate::external_agent_config_migration_model::external_agent_config_migration_item_count;
use crate::external_agent_config_migration_model::external_agent_config_migration_type_label;
use crate::external_agent_config_migration_source::ExternalAgentConfigMigrationSource;
use crate::external_agent_config_migration_source::run_external_agent_config_source_prompt;
use crate::legacy_core::config::Config;
use crate::tui;
use codex_app_server_protocol::ExternalAgentConfigDetectParams;
use codex_app_server_protocol::ExternalAgentConfigImportCompletedNotification;
use codex_app_server_protocol::ExternalAgentConfigMigrationItem;
use codex_app_server_protocol::ExternalAgentConfigMigrationItemType;
use ratatui::prelude::Stylize as _;
use ratatui::text::Line;

pub(crate) const EXTERNAL_AGENT_CONFIG_MIGRATION_NO_ITEMS_MESSAGE: &str =
    "No compatible setup was found to import.";
pub(crate) const EXTERNAL_AGENT_CONFIG_MIGRATION_REMOTE_UNAVAILABLE_MESSAGE: &str = "Import from other apps is unavailable in remote sessions. Start Codex locally and run /import.";
pub(crate) const EXTERNAL_AGENT_CONFIG_MIGRATION_DAEMON_UNAVAILABLE_MESSAGE: &str = "Import from other apps is unavailable while Codex is connected to the local app-server daemon. Stop the daemon, restart Codex, and run /import.";

pub(crate) enum ExternalAgentConfigMigrationFlowOutcome {
    Started(Vec<Line<'static>>),
    NoItems,
    Cancelled,
}

struct DetectedExternalAgentConfigSource {
    source: ExternalAgentConfigMigrationSource,
    items: Vec<ExternalAgentConfigMigrationItem>,
}

#[derive(Default)]
struct ExternalAgentConfigDetection {
    sources: Vec<DetectedExternalAgentConfigSource>,
    errors: Vec<String>,
}

enum ExternalAgentConfigDetectionOutcome {
    Sources(Vec<DetectedExternalAgentConfigSource>),
    NoItems,
    Failed(String),
}

impl ExternalAgentConfigDetection {
    fn record_items(
        &mut self,
        source: ExternalAgentConfigMigrationSource,
        items: Vec<ExternalAgentConfigMigrationItem>,
    ) {
        if !items.is_empty() {
            self.sources
                .push(DetectedExternalAgentConfigSource { source, items });
        }
    }

    fn record_error(
        &mut self,
        source: ExternalAgentConfigMigrationSource,
        error: impl std::fmt::Display,
    ) {
        self.errors.push(format!("{}: {error}", source.label()));
    }

    fn finish(self) -> ExternalAgentConfigDetectionOutcome {
        if !self.sources.is_empty() {
            ExternalAgentConfigDetectionOutcome::Sources(self.sources)
        } else if self.errors.is_empty() {
            ExternalAgentConfigDetectionOutcome::NoItems
        } else {
            ExternalAgentConfigDetectionOutcome::Failed(format!(
                "Could not check for importable setup: {}",
                self.errors.join("; ")
            ))
        }
    }
}

fn external_agent_config_migration_started_lines(
    selected_items: &[ExternalAgentConfigMigrationItem],
    remaining_item_count: usize,
) -> Vec<Line<'static>> {
    let mut import_summaries =
        Vec::<(ExternalAgentConfigMigrationItemType, usize, Vec<&str>)>::new();
    for item in selected_items {
        let names = item
            .details
            .as_ref()
            .map_or_else(Vec::new, |details| match item.item_type {
                ExternalAgentConfigMigrationItemType::Plugins => details
                    .plugins
                    .iter()
                    .flat_map(|plugin_group| plugin_group.plugin_names.iter())
                    .map(String::as_str)
                    .collect(),
                ExternalAgentConfigMigrationItemType::Skills => details
                    .skills
                    .iter()
                    .map(|skill| skill.name.as_str())
                    .collect(),
                ExternalAgentConfigMigrationItemType::McpServerConfig => details
                    .mcp_servers
                    .iter()
                    .map(|server| server.name.as_str())
                    .collect(),
                ExternalAgentConfigMigrationItemType::Subagents => details
                    .subagents
                    .iter()
                    .map(|agent| agent.name.as_str())
                    .collect(),
                ExternalAgentConfigMigrationItemType::Hooks => details
                    .hooks
                    .iter()
                    .map(|hook| hook.name.as_str())
                    .collect(),
                ExternalAgentConfigMigrationItemType::Commands => details
                    .commands
                    .iter()
                    .map(|command| command.name.as_str())
                    .collect(),
                ExternalAgentConfigMigrationItemType::Memory => {
                    details.memory.iter().map(String::as_str).collect()
                }
                ExternalAgentConfigMigrationItemType::Sessions => details
                    .sessions
                    .iter()
                    .filter_map(|session| session.title.as_deref())
                    .collect(),
                ExternalAgentConfigMigrationItemType::AgentsMd
                | ExternalAgentConfigMigrationItemType::Config => Vec::new(),
            });
        let count = external_agent_config_migration_item_count(item);
        if let Some((_, type_count, type_names)) = import_summaries
            .iter_mut()
            .find(|(item_type, _, _)| *item_type == item.item_type)
        {
            *type_count += count;
            type_names.extend(names);
        } else {
            import_summaries.push((item.item_type, count, names));
        }
    }

    let mut lines = vec![
        vec![
            "• ".dim(),
            "Import started.".cyan(),
            " You can keep working while it finishes.".into(),
        ]
        .into(),
        vec!["  ".into(), "Imported setup will apply to new chats.".dim()].into(),
        vec!["  ".into(), "Importing:".cyan().bold()].into(),
    ];
    lines.extend(
        import_summaries
            .into_iter()
            .map(|(item_type, count, names)| {
                let mut line = vec![
                    "    ".into(),
                    external_agent_config_migration_type_label(item_type).cyan(),
                    ": ".into(),
                    count.to_string().green(),
                ];
                if !names.is_empty() {
                    let shown_names = names.iter().take(3).copied().collect::<Vec<_>>();
                    let mut name_summary = shown_names.join(", ");
                    if names.len() > shown_names.len() {
                        name_summary
                            .push_str(&format!(", +{} more", names.len() - shown_names.len()));
                    }
                    line.extend([" — ".dim(), name_summary.into()]);
                }
                line.into()
            }),
    );
    if let Some(remaining_items_handoff) = remaining_items_handoff(remaining_item_count) {
        lines.push(vec!["  ".into(), remaining_items_handoff.dim()].into());
    }
    lines
}

pub(crate) fn external_agent_config_migration_finished_lines(
    notification: &ExternalAgentConfigImportCompletedNotification,
) -> Vec<Line<'static>> {
    let imported_count = notification
        .item_type_results
        .iter()
        .map(|type_result| type_result.successes.len())
        .sum::<usize>();
    let failed_count = notification
        .item_type_results
        .iter()
        .map(|type_result| type_result.failures.len())
        .sum::<usize>();
    let failed_count = if failed_count == 0 {
        format!("{failed_count} failed").green()
    } else {
        format!("{failed_count} failed").red()
    };
    let mut lines = vec![
        vec![
            "• ".dim(),
            "Import finished: ".into(),
            format!("{imported_count} imported").green(),
            ", ".into(),
            failed_count,
            ".".into(),
        ]
        .into(),
    ];
    if !notification.item_type_results.is_empty() {
        lines.push(vec!["  ".into(), "Results by type:".cyan().bold()].into());
        lines.extend(notification.item_type_results.iter().map(|type_result| {
            let failed_count = format!("{} failed", type_result.failures.len());
            let failed_count = if type_result.failures.is_empty() {
                failed_count.green()
            } else {
                failed_count.red()
            };
            vec![
                "    ".into(),
                external_agent_config_migration_type_label(type_result.item_type).cyan(),
                ": ".into(),
                format!("{} imported", type_result.successes.len()).green(),
                ", ".into(),
                failed_count,
            ]
            .into()
        }));
    }
    lines.push(
        vec![
            "  ".into(),
            "Run /import again to check for additional items.".dim(),
        ]
        .into(),
    );
    lines
}

fn remaining_items_handoff(remaining_item_count: usize) -> Option<String> {
    match remaining_item_count {
        0 => None,
        1 => Some(
            "1 additional item remains. After it finishes, run /import again to review it."
                .to_string(),
        ),
        _ => Some(format!(
            "{remaining_item_count} additional items remain. After it finishes, run /import again to review them."
        )),
    }
}

pub(crate) async fn handle_external_agent_config_migration_prompt(
    tui: &mut tui::Tui,
    app_server: &mut AppServerSession,
    config: &Config,
) -> Result<ExternalAgentConfigMigrationFlowOutcome, String> {
    if app_server.uses_remote_workspace() {
        return Err(EXTERNAL_AGENT_CONFIG_MIGRATION_REMOTE_UNAVAILABLE_MESSAGE.to_string());
    }
    if !app_server.uses_embedded_app_server() {
        return Err(EXTERNAL_AGENT_CONFIG_MIGRATION_DAEMON_UNAVAILABLE_MESSAGE.to_string());
    }
    if app_server.external_agent_config_import_in_progress() {
        return Err(EXTERNAL_AGENT_CONFIG_IMPORT_IN_PROGRESS_MESSAGE.to_string());
    }

    let cwd = config.cwd.to_path_buf();
    let mut detection = ExternalAgentConfigDetection::default();
    for source in ExternalAgentConfigMigrationSource::ALL {
        let response = match app_server
            .external_agent_config_detect(ExternalAgentConfigDetectParams {
                include_home: true,
                cwds: Some(vec![cwd.clone()]),
                max_session_age_days: None,
                max_sessions: None,
                source: None,
                migration_source: Some(source.migration_source().to_string()),
            })
            .await
        {
            Ok(response) => response,
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    cwd = %cwd.display(),
                    source = source.label(),
                    "failed to detect external agent config migrations"
                );
                detection.record_error(source, err);
                continue;
            }
        };
        detection.record_items(source, response.items);
    }

    let detected_sources = match detection.finish() {
        ExternalAgentConfigDetectionOutcome::Sources(sources) => sources,
        ExternalAgentConfigDetectionOutcome::NoItems => {
            return Ok(ExternalAgentConfigMigrationFlowOutcome::NoItems);
        }
        ExternalAgentConfigDetectionOutcome::Failed(error) => return Err(error),
    };

    let selected_source = if detected_sources.len() == 1 {
        detected_sources[0].source
    } else {
        let sources = detected_sources
            .iter()
            .map(|detected| detected.source)
            .collect::<Vec<_>>();
        let Some(source) = run_external_agent_config_source_prompt(tui, &sources).await else {
            return Ok(ExternalAgentConfigMigrationFlowOutcome::Cancelled);
        };
        source
    };
    let Some(detected_source) = detected_sources
        .into_iter()
        .find(|detected| detected.source == selected_source)
    else {
        return Err("Selected import source is no longer available.".to_string());
    };
    let detected_items = detected_source.items;

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
                match app_server
                    .external_agent_config_import(
                        items,
                        selected_source.migration_source().to_string(),
                    )
                    .await
                {
                    Ok(()) => {
                        let remaining_item_count =
                            detected_items.len().saturating_sub(selected_items.len());
                        let started_lines = external_agent_config_migration_started_lines(
                            &selected_items,
                            remaining_item_count,
                        );
                        return Ok(ExternalAgentConfigMigrationFlowOutcome::Started(
                            started_lines,
                        ));
                    }
                    Err(err) => {
                        tracing::warn!(
                            error = %err,
                            cwd = %cwd.display(),
                            "failed to import external agent config migration items"
                        );
                        error = Some(format!("Import failed: {err}"));
                    }
                }
            }
            ExternalAgentConfigMigrationOutcome::Skip => {
                return Ok(ExternalAgentConfigMigrationFlowOutcome::Cancelled);
            }
        }
    }
}

#[cfg(test)]
#[path = "external_agent_config_migration_flow_tests.rs"]
mod tests;
