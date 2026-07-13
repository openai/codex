use super::*;

fn migration_item(
    item_type: ExternalAgentConfigMigrationItemType,
) -> ExternalAgentConfigMigrationItem {
    ExternalAgentConfigMigrationItem {
        item_type,
        description: String::new(),
        cwd: None,
        details: None,
    }
}

#[test]
fn migration_items_that_update_runtime_sources_trigger_refresh() {
    assert!(migration_items_need_runtime_refresh(&[migration_item(
        ExternalAgentConfigMigrationItemType::Config,
    )]));
    assert!(migration_items_need_runtime_refresh(&[migration_item(
        ExternalAgentConfigMigrationItemType::Skills,
    )]));
    assert!(migration_items_need_runtime_refresh(&[migration_item(
        ExternalAgentConfigMigrationItemType::McpServerConfig,
    )]));
    assert!(migration_items_need_runtime_refresh(&[migration_item(
        ExternalAgentConfigMigrationItemType::Hooks,
    )]));
    assert!(migration_items_need_runtime_refresh(&[migration_item(
        ExternalAgentConfigMigrationItemType::Commands,
    )]));
    assert!(migration_items_need_runtime_refresh(&[migration_item(
        ExternalAgentConfigMigrationItemType::Plugins,
    )]));
    assert!(!migration_items_need_runtime_refresh(&[migration_item(
        ExternalAgentConfigMigrationItemType::Sessions,
    )]));
}
