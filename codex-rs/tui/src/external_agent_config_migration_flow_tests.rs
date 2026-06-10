use super::*;

#[test]
fn external_agent_config_migration_messages_snapshot() {
    let cases = [0, 1, 2];

    let messages = cases
        .map(external_agent_config_migration_success_message)
        .into_iter()
        .chain([
            EXTERNAL_AGENT_CONFIG_MIGRATION_FINISHED_MESSAGE.to_string(),
            EXTERNAL_AGENT_CONFIG_MIGRATION_NO_ITEMS_MESSAGE.to_string(),
            EXTERNAL_AGENT_CONFIG_MIGRATION_REMOTE_UNAVAILABLE_MESSAGE.to_string(),
            EXTERNAL_AGENT_CONFIG_MIGRATION_DAEMON_UNAVAILABLE_MESSAGE.to_string(),
            EXTERNAL_AGENT_CONFIG_IMPORT_IN_PROGRESS_MESSAGE.to_string(),
        ])
        .collect::<Vec<_>>()
        .join("\n");

    insta::assert_snapshot!("external_agent_config_migration_messages", messages);
}
