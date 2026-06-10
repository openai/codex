use crate::app_server_session::AppServerSession;
use crate::app_server_session::EXTERNAL_AGENT_CONFIG_IMPORT_IN_PROGRESS_MESSAGE;
use crate::external_agent_config_migration::ExternalAgentConfigMigrationOutcome;
use crate::external_agent_config_migration::run_external_agent_config_migration_prompt;
use crate::legacy_core::config::Config;
use crate::tui;
use codex_app_server_protocol::ExternalAgentConfigDetectParams;

pub(crate) const EXTERNAL_AGENT_CONFIG_MIGRATION_FINISHED_MESSAGE: &str =
    "Agent import finished. Run /import again to check for additional items.";
pub(crate) const EXTERNAL_AGENT_CONFIG_MIGRATION_NO_ITEMS_MESSAGE: &str =
    "No supported agent setup was found to import.";
pub(crate) const EXTERNAL_AGENT_CONFIG_MIGRATION_REMOTE_UNAVAILABLE_MESSAGE: &str =
    "Agent import is unavailable in remote sessions. Start Codex locally and run /import.";
pub(crate) const EXTERNAL_AGENT_CONFIG_MIGRATION_DAEMON_UNAVAILABLE_MESSAGE: &str = "Agent import is unavailable while Codex is connected to the local app-server daemon. Stop the daemon, restart Codex, and run /import.";

pub(crate) enum ExternalAgentConfigMigrationFlowOutcome {
    Started(String),
    NoItems,
    Cancelled,
}

fn external_agent_config_migration_success_message(remaining_item_count: usize) -> String {
    let message = "Agent import started. You can keep working while it finishes. Imported setup will apply to new chats.";
    match remaining_items_handoff(remaining_item_count) {
        Some(remaining_items_handoff) => format!("{message} {remaining_items_handoff}"),
        None => message.to_string(),
    }
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
    let detected_items = match app_server
        .external_agent_config_detect(ExternalAgentConfigDetectParams {
            include_home: true,
            cwds: Some(vec![cwd.clone()]),
        })
        .await
    {
        Ok(response) => response.items,
        Err(err) => {
            tracing::warn!(
                error = %err,
                cwd = %cwd.display(),
                "failed to detect external agent config migrations"
            );
            return Err(format!("Could not check for agent setup: {err}"));
        }
    };

    if detected_items.is_empty() {
        return Ok(ExternalAgentConfigMigrationFlowOutcome::NoItems);
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
                    Ok(()) => {
                        let remaining_item_count =
                            detected_items.len().saturating_sub(selected_items.len());
                        let success_message =
                            external_agent_config_migration_success_message(remaining_item_count);
                        return Ok(ExternalAgentConfigMigrationFlowOutcome::Started(
                            success_message,
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
