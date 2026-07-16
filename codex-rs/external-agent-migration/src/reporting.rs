use std::path::Path;

use crate::model::ExternalAgentConfigImportItemResult;
use crate::model::ExternalAgentConfigImportRawError;
use crate::model::ExternalAgentConfigMigrationItemType;
use crate::model::PluginImportOutcome;

fn migration_item_type_label(item_type: ExternalAgentConfigMigrationItemType) -> &'static str {
    match item_type {
        ExternalAgentConfigMigrationItemType::Config => "config",
        ExternalAgentConfigMigrationItemType::Skills => "skills",
        ExternalAgentConfigMigrationItemType::AgentsMd => "agents_md",
        ExternalAgentConfigMigrationItemType::Plugins => "plugins",
        ExternalAgentConfigMigrationItemType::McpServerConfig => "mcp_server_config",
        ExternalAgentConfigMigrationItemType::Subagents => "subagents",
        ExternalAgentConfigMigrationItemType::Hooks => "hooks",
        ExternalAgentConfigMigrationItemType::Commands => "commands",
        ExternalAgentConfigMigrationItemType::Memory => "memory",
        ExternalAgentConfigMigrationItemType::Sessions => "sessions",
    }
}

pub fn record_import_error(
    result: &mut ExternalAgentConfigImportItemResult,
    failure_stage: &'static str,
    message: impl Into<String>,
    source: Option<String>,
) {
    result.record_error(ExternalAgentConfigImportRawError {
        item_type: result.item_type,
        error_type: None,
        sub_error_type: None,
        failure_stage: failure_stage.to_string(),
        message: message.into(),
        cwd: result.cwd.clone(),
        source,
    });
}

pub(super) fn record_plugin_import_errors(
    outcome: &mut PluginImportOutcome,
    cwd: Option<&Path>,
    plugin_ids: &[String],
    failure_stage: &'static str,
    message: impl Into<String>,
) {
    let message = message.into();
    outcome
        .raw_errors
        .extend(plugin_ids.iter().map(|plugin_id| {
            plugin_import_raw_error(cwd, failure_stage, message.clone(), Some(plugin_id.clone()))
        }));
}

pub(super) fn plugin_import_raw_error(
    cwd: Option<&Path>,
    failure_stage: &'static str,
    message: String,
    source: Option<String>,
) -> ExternalAgentConfigImportRawError {
    ExternalAgentConfigImportRawError {
        item_type: ExternalAgentConfigMigrationItemType::Plugins,
        error_type: None,
        sub_error_type: None,
        failure_stage: failure_stage.to_string(),
        message,
        cwd: cwd.map(Path::to_path_buf),
        source,
    }
}

pub(super) fn migration_metric_tags(
    item_type: ExternalAgentConfigMigrationItemType,
    skills_count: Option<usize>,
) -> Vec<(&'static str, String)> {
    let mut tags = vec![(
        "migration_type",
        migration_item_type_label(item_type).to_string(),
    )];
    if matches!(
        item_type,
        ExternalAgentConfigMigrationItemType::Skills
            | ExternalAgentConfigMigrationItemType::Subagents
            | ExternalAgentConfigMigrationItemType::Commands
    ) {
        tags.push(("skills_count", skills_count.unwrap_or(0).to_string()));
    }
    tags
}

pub(super) fn emit_migration_metric(
    metric_name: &str,
    item_type: ExternalAgentConfigMigrationItemType,
    skills_count: Option<usize>,
) {
    let Some(metrics) = codex_otel::global() else {
        return;
    };
    let tags = migration_metric_tags(item_type, skills_count);
    let tag_refs = tags
        .iter()
        .map(|(key, value)| (*key, value.as_str()))
        .collect::<Vec<_>>();
    let _ = metrics.counter(metric_name, /*inc*/ 1, &tag_refs);
}
