use crate::error_code::internal_error;
use codex_app_server_protocol::CommandMigration;
use codex_app_server_protocol::ExternalAgentConfigDetectResponse;
use codex_app_server_protocol::ExternalAgentConfigImportCompletedNotification;
use codex_app_server_protocol::ExternalAgentConfigImportHistory;
use codex_app_server_protocol::ExternalAgentConfigImportItemTypeFailure as ProtocolImportFailure;
use codex_app_server_protocol::ExternalAgentConfigImportItemTypeSuccess as ProtocolImportSuccess;
use codex_app_server_protocol::ExternalAgentConfigImportTypeResult as ProtocolImportTypeResult;
use codex_app_server_protocol::ExternalAgentConfigMigrationItem;
use codex_app_server_protocol::ExternalAgentConfigMigrationItemType;
use codex_app_server_protocol::HookMigration;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::McpServerMigration;
use codex_app_server_protocol::MigrationDetails;
use codex_app_server_protocol::PluginsMigration;
use codex_app_server_protocol::SkillMigration;
use codex_app_server_protocol::SubagentMigration;
use codex_external_agent_migration::ExternalAgentConfigImportItemResult as CoreImportItemResult;
use codex_external_agent_migration::ExternalAgentConfigImportRawError as CoreImportRawError;
use codex_external_agent_migration::ExternalAgentConfigImportSuccess;
use codex_external_agent_migration::ExternalAgentConfigMigrationItem as CoreMigrationItem;
use codex_external_agent_migration::ExternalAgentConfigMigrationItemType as CoreMigrationItemType;
use codex_external_agent_migration::MigrationDetails as CoreMigrationDetails;
use codex_external_agent_migration::NamedMigration;
use codex_external_agent_migration::PluginsMigration as CorePluginsMigration;
use codex_external_agent_migration::sessions::ExternalAgentSessionMigration;
use codex_state::ExternalAgentConfigImportFailureRecord;
use codex_state::ExternalAgentConfigImportSuccessRecord;

pub(super) fn detect_response(items: Vec<CoreMigrationItem>) -> ExternalAgentConfigDetectResponse {
    ExternalAgentConfigDetectResponse {
        items: items.into_iter().map(protocol_migration_item).collect(),
    }
}

fn protocol_migration_item(item: CoreMigrationItem) -> ExternalAgentConfigMigrationItem {
    ExternalAgentConfigMigrationItem {
        item_type: protocol_migration_item_type(item.item_type),
        description: item.description,
        cwd: item.cwd,
        details: item.details.map(protocol_migration_details),
    }
}

fn protocol_migration_details(details: CoreMigrationDetails) -> MigrationDetails {
    MigrationDetails {
        plugins: details
            .plugins
            .into_iter()
            .map(|plugin| PluginsMigration {
                marketplace_name: plugin.marketplace_name,
                plugin_names: plugin.plugin_names,
            })
            .collect(),
        skills: details
            .skills
            .into_iter()
            .map(|skill| SkillMigration { name: skill.name })
            .collect(),
        sessions: details
            .sessions
            .into_iter()
            .map(|session| codex_app_server_protocol::SessionMigration {
                path: session.path,
                cwd: session.cwd,
                title: session.title,
            })
            .collect(),
        mcp_servers: details
            .mcp_servers
            .into_iter()
            .map(|server| McpServerMigration { name: server.name })
            .collect(),
        hooks: details
            .hooks
            .into_iter()
            .map(|hook| HookMigration { name: hook.name })
            .collect(),
        subagents: details
            .subagents
            .into_iter()
            .map(|subagent| SubagentMigration {
                name: subagent.name,
            })
            .collect(),
        commands: details
            .commands
            .into_iter()
            .map(|command| CommandMigration { name: command.name })
            .collect(),
        memory: details.memory,
    }
}

pub(super) fn core_migration_items(
    items: Vec<ExternalAgentConfigMigrationItem>,
) -> Vec<CoreMigrationItem> {
    items
        .into_iter()
        .map(|item| CoreMigrationItem {
            item_type: core_migration_item_type(item.item_type),
            description: item.description,
            cwd: item.cwd,
            details: item.details.map(core_migration_details),
        })
        .collect()
}

fn core_migration_details(details: MigrationDetails) -> CoreMigrationDetails {
    CoreMigrationDetails {
        plugins: details
            .plugins
            .into_iter()
            .map(|plugin| CorePluginsMigration {
                marketplace_name: plugin.marketplace_name,
                plugin_names: plugin.plugin_names,
            })
            .collect(),
        skills: details
            .skills
            .into_iter()
            .map(|skill| NamedMigration { name: skill.name })
            .collect(),
        sessions: details
            .sessions
            .into_iter()
            .map(|session| ExternalAgentSessionMigration {
                path: session.path,
                cwd: session.cwd,
                title: session.title,
            })
            .collect(),
        mcp_servers: details
            .mcp_servers
            .into_iter()
            .map(|server| NamedMigration { name: server.name })
            .collect(),
        hooks: details
            .hooks
            .into_iter()
            .map(|hook| NamedMigration { name: hook.name })
            .collect(),
        subagents: details
            .subagents
            .into_iter()
            .map(|subagent| NamedMigration {
                name: subagent.name,
            })
            .collect(),
        commands: details
            .commands
            .into_iter()
            .map(|command| NamedMigration { name: command.name })
            .collect(),
        memory: details.memory,
    }
}

pub(super) fn protocol_migration_item_type(
    item_type: CoreMigrationItemType,
) -> ExternalAgentConfigMigrationItemType {
    match item_type {
        CoreMigrationItemType::Config => ExternalAgentConfigMigrationItemType::Config,
        CoreMigrationItemType::Skills => ExternalAgentConfigMigrationItemType::Skills,
        CoreMigrationItemType::AgentsMd => ExternalAgentConfigMigrationItemType::AgentsMd,
        CoreMigrationItemType::Plugins => ExternalAgentConfigMigrationItemType::Plugins,
        CoreMigrationItemType::McpServerConfig => {
            ExternalAgentConfigMigrationItemType::McpServerConfig
        }
        CoreMigrationItemType::Subagents => ExternalAgentConfigMigrationItemType::Subagents,
        CoreMigrationItemType::Hooks => ExternalAgentConfigMigrationItemType::Hooks,
        CoreMigrationItemType::Commands => ExternalAgentConfigMigrationItemType::Commands,
        CoreMigrationItemType::Memory => ExternalAgentConfigMigrationItemType::Memory,
        CoreMigrationItemType::Sessions => ExternalAgentConfigMigrationItemType::Sessions,
    }
}

fn core_migration_item_type(
    item_type: ExternalAgentConfigMigrationItemType,
) -> CoreMigrationItemType {
    match item_type {
        ExternalAgentConfigMigrationItemType::Config => CoreMigrationItemType::Config,
        ExternalAgentConfigMigrationItemType::Skills => CoreMigrationItemType::Skills,
        ExternalAgentConfigMigrationItemType::AgentsMd => CoreMigrationItemType::AgentsMd,
        ExternalAgentConfigMigrationItemType::Plugins => CoreMigrationItemType::Plugins,
        ExternalAgentConfigMigrationItemType::McpServerConfig => {
            CoreMigrationItemType::McpServerConfig
        }
        ExternalAgentConfigMigrationItemType::Subagents => CoreMigrationItemType::Subagents,
        ExternalAgentConfigMigrationItemType::Hooks => CoreMigrationItemType::Hooks,
        ExternalAgentConfigMigrationItemType::Commands => CoreMigrationItemType::Commands,
        ExternalAgentConfigMigrationItemType::Memory => CoreMigrationItemType::Memory,
        ExternalAgentConfigMigrationItemType::Sessions => CoreMigrationItemType::Sessions,
    }
}

pub(super) fn protocol_import_history(
    record: codex_state::ExternalAgentConfigImportHistoryRecord,
) -> Result<ExternalAgentConfigImportHistory, JSONRPCErrorError> {
    let successes = record
        .successes
        .into_iter()
        .map(protocol_import_success_record)
        .collect::<Result<Vec<_>, _>>()?;
    let failures = record
        .failures
        .into_iter()
        .map(protocol_import_failure_record)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ExternalAgentConfigImportHistory {
        import_id: record.import_id,
        provider_id: record.provider_id,
        completed_at_ms: record.completed_at_ms,
        successes,
        failures,
    })
}

fn protocol_import_success_record(
    record: ExternalAgentConfigImportSuccessRecord,
) -> Result<ProtocolImportSuccess, JSONRPCErrorError> {
    Ok(ProtocolImportSuccess {
        item_type: protocol_import_record_item_type(record.item_type)?,
        cwd: record.cwd,
        source: record.source,
        target: record.target,
    })
}

fn protocol_import_failure_record(
    record: ExternalAgentConfigImportFailureRecord,
) -> Result<ProtocolImportFailure, JSONRPCErrorError> {
    Ok(ProtocolImportFailure {
        item_type: protocol_import_record_item_type(record.item_type)?,
        error_type: record.error_type,
        sub_error_type: record.sub_error_type,
        failure_stage: record.failure_stage,
        message: record.message,
        cwd: record.cwd,
        source: record.source,
    })
}

fn protocol_import_record_item_type(
    item_type: String,
) -> Result<ExternalAgentConfigMigrationItemType, JSONRPCErrorError> {
    serde_json::from_value(serde_json::Value::String(item_type.clone())).map_err(|err| {
        internal_error(format!(
            "failed to decode import item type {item_type}: {err}"
        ))
    })
}

pub(super) fn completed_notification(
    import_id: String,
    item_results: &[CoreImportItemResult],
) -> ExternalAgentConfigImportCompletedNotification {
    let mut protocol_type_results: Vec<ProtocolImportTypeResult> = Vec::new();
    for item_result in item_results {
        let item_raw_errors = item_result
            .raw_errors
            .iter()
            .map(protocol_import_raw_error)
            .collect::<Vec<_>>();
        let item_successes = item_result
            .successes
            .iter()
            .map(protocol_import_success)
            .collect::<Vec<_>>();
        let item_type = protocol_migration_item_type(item_result.item_type);
        if let Some(type_result) = protocol_type_results
            .iter_mut()
            .find(|type_result| type_result.item_type == item_type)
        {
            type_result.successes.extend(item_successes);
            type_result.failures.extend(item_raw_errors);
        } else {
            protocol_type_results.push(ProtocolImportTypeResult {
                item_type,
                successes: item_successes,
                failures: item_raw_errors,
            });
        }
    }
    protocol_type_results.sort_by_key(|type_result| match type_result.item_type {
        ExternalAgentConfigMigrationItemType::Config => 0,
        ExternalAgentConfigMigrationItemType::Skills => 1,
        ExternalAgentConfigMigrationItemType::AgentsMd => 2,
        ExternalAgentConfigMigrationItemType::Plugins => 3,
        ExternalAgentConfigMigrationItemType::McpServerConfig => 4,
        ExternalAgentConfigMigrationItemType::Subagents => 5,
        ExternalAgentConfigMigrationItemType::Hooks => 6,
        ExternalAgentConfigMigrationItemType::Commands => 7,
        ExternalAgentConfigMigrationItemType::Sessions => 8,
        ExternalAgentConfigMigrationItemType::Memory => 9,
    });

    ExternalAgentConfigImportCompletedNotification {
        import_id,
        item_type_results: protocol_type_results,
    }
}

pub(super) fn protocol_import_type_result(
    item_result: &CoreImportItemResult,
) -> ProtocolImportTypeResult {
    ProtocolImportTypeResult {
        item_type: protocol_migration_item_type(item_result.item_type),
        successes: item_result
            .successes
            .iter()
            .map(protocol_import_success)
            .collect(),
        failures: item_result
            .raw_errors
            .iter()
            .map(protocol_import_raw_error)
            .collect(),
    }
}

fn protocol_import_success(success: &ExternalAgentConfigImportSuccess) -> ProtocolImportSuccess {
    ProtocolImportSuccess {
        item_type: protocol_migration_item_type(success.item_type),
        cwd: success.cwd.clone(),
        source: success.source.clone(),
        target: success.target.clone(),
    }
}

fn protocol_import_raw_error(raw_error: &CoreImportRawError) -> ProtocolImportFailure {
    ProtocolImportFailure {
        item_type: protocol_migration_item_type(raw_error.item_type),
        error_type: raw_error.error_type.clone(),
        sub_error_type: raw_error.sub_error_type.clone(),
        failure_stage: raw_error.failure_stage.clone(),
        message: raw_error.message.clone(),
        cwd: raw_error.cwd.clone(),
        source: raw_error.source.clone(),
    }
}
