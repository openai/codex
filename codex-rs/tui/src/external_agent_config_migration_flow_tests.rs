use super::*;
use codex_app_server_protocol::ExternalAgentConfigImportItemTypeFailure;
use codex_app_server_protocol::ExternalAgentConfigImportItemTypeSuccess;
use codex_app_server_protocol::ExternalAgentConfigImportTypeResult;
use codex_app_server_protocol::ExternalAgentConfigMigrationItemType;
use std::path::PathBuf;

#[test]
fn external_agent_config_migration_messages_snapshot() {
    let cases = [0, 1, 2];
    let selected_items = [ExternalAgentConfigMigrationItem {
        item_type: ExternalAgentConfigMigrationItemType::Config,
        description: "Import settings".to_string(),
        cwd: None,
        details: None,
    }];
    let completed_notification = ExternalAgentConfigImportCompletedNotification {
        import_id: "import-1".to_string(),
        item_type_results: vec![
            ExternalAgentConfigImportTypeResult {
                item_type: ExternalAgentConfigMigrationItemType::Config,
                successes: vec![ExternalAgentConfigImportItemTypeSuccess {
                    item_type: ExternalAgentConfigMigrationItemType::Config,
                    cwd: None,
                    source: Some("settings.json".to_string()),
                    target: Some("config.toml".to_string()),
                }],
                failures: Vec::new(),
            },
            ExternalAgentConfigImportTypeResult {
                item_type: ExternalAgentConfigMigrationItemType::Plugins,
                successes: vec![ExternalAgentConfigImportItemTypeSuccess {
                    item_type: ExternalAgentConfigMigrationItemType::Plugins,
                    cwd: None,
                    source: Some("formatter@example".to_string()),
                    target: Some("formatter@example".to_string()),
                }],
                failures: vec![ExternalAgentConfigImportItemTypeFailure {
                    item_type: ExternalAgentConfigMigrationItemType::Plugins,
                    error_type: Some("plugin_install_failed".to_string()),
                    failure_stage: "plugin_import".to_string(),
                    message: "install failed".to_string(),
                    cwd: Some(PathBuf::from("/workspace/project")),
                    source: Some("deployer@example".to_string()),
                }],
            },
        ],
    };

    let messages = cases
        .map(|remaining_item_count| {
            external_agent_config_migration_success_message(&selected_items, remaining_item_count)
        })
        .into_iter()
        .chain(external_agent_config_migration_finished_lines(
            &completed_notification,
        ))
        .chain([
            EXTERNAL_AGENT_CONFIG_MIGRATION_NO_ITEMS_MESSAGE.to_string(),
            EXTERNAL_AGENT_CONFIG_MIGRATION_REMOTE_UNAVAILABLE_MESSAGE.to_string(),
            EXTERNAL_AGENT_CONFIG_MIGRATION_DAEMON_UNAVAILABLE_MESSAGE.to_string(),
            EXTERNAL_AGENT_CONFIG_IMPORT_IN_PROGRESS_MESSAGE.to_string(),
        ])
        .collect::<Vec<_>>()
        .join("\n");

    insta::assert_snapshot!("external_agent_config_migration_messages", messages);
}
