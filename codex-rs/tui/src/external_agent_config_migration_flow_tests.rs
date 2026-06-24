use super::*;
use codex_app_server_protocol::ExternalAgentConfigImportItemTypeFailure;
use codex_app_server_protocol::ExternalAgentConfigImportItemTypeSuccess;
use codex_app_server_protocol::ExternalAgentConfigImportTypeResult;
use codex_app_server_protocol::ExternalAgentConfigMigrationItemType;
use codex_app_server_protocol::McpServerMigration;
use codex_app_server_protocol::MigrationDetails;
use codex_app_server_protocol::PluginsMigration;
use codex_app_server_protocol::SessionMigration;
use codex_app_server_protocol::SkillMigration;
use pretty_assertions::assert_eq;
use ratatui::text::Line;
use std::path::PathBuf;

fn selected_items() -> Vec<ExternalAgentConfigMigrationItem> {
    vec![
        ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::Config,
            description: "Import settings".to_string(),
            cwd: None,
            details: None,
        },
        ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::Skills,
            description: "Import skills".to_string(),
            cwd: None,
            details: Some(MigrationDetails {
                skills: vec![
                    SkillMigration {
                        name: "triage".to_string(),
                    },
                    SkillMigration {
                        name: "release-notes".to_string(),
                    },
                    SkillMigration {
                        name: "risk-check".to_string(),
                    },
                    SkillMigration {
                        name: "incident-review".to_string(),
                    },
                ],
                ..Default::default()
            }),
        },
        ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::McpServerConfig,
            description: "Import MCP servers".to_string(),
            cwd: None,
            details: Some(MigrationDetails {
                mcp_servers: vec![
                    McpServerMigration {
                        name: "docs".to_string(),
                    },
                    McpServerMigration {
                        name: "issues".to_string(),
                    },
                ],
                ..Default::default()
            }),
        },
        ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::Sessions,
            description: "Import chat sessions".to_string(),
            cwd: None,
            details: Some(MigrationDetails {
                sessions: vec![
                    SessionMigration {
                        path: PathBuf::from("/sessions/alpha.jsonl"),
                        cwd: PathBuf::from("/workspace/project"),
                        title: Some("Alpha rollout".to_string()),
                    },
                    SessionMigration {
                        path: PathBuf::from("/sessions/beta.jsonl"),
                        cwd: PathBuf::from("/workspace/project"),
                        title: Some("Beta review".to_string()),
                    },
                    SessionMigration {
                        path: PathBuf::from("/sessions/gamma.jsonl"),
                        cwd: PathBuf::from("/workspace/project"),
                        title: Some("Gamma notes".to_string()),
                    },
                ],
                ..Default::default()
            }),
        },
        ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::Plugins,
            description: "Import plugins".to_string(),
            cwd: None,
            details: Some(MigrationDetails {
                plugins: vec![PluginsMigration {
                    marketplace_name: "example".to_string(),
                    plugin_names: vec!["formatter".to_string(), "reviewer".to_string()],
                }],
                ..Default::default()
            }),
        },
    ]
}

fn completed_notification() -> ExternalAgentConfigImportCompletedNotification {
    ExternalAgentConfigImportCompletedNotification {
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
    }
}

#[test]
fn external_agent_config_migration_messages_snapshot() {
    let selected_items = selected_items();
    let completed_notification = completed_notification();
    let messages = [0, 1, 2]
        .into_iter()
        .flat_map(|remaining_item_count| {
            external_agent_config_migration_started_lines(&selected_items, remaining_item_count)
        })
        .chain(external_agent_config_migration_finished_lines(
            &completed_notification,
        ))
        .chain([
            Line::from(EXTERNAL_AGENT_CONFIG_MIGRATION_NO_ITEMS_MESSAGE),
            Line::from(EXTERNAL_AGENT_CONFIG_MIGRATION_REMOTE_UNAVAILABLE_MESSAGE),
            Line::from(EXTERNAL_AGENT_CONFIG_MIGRATION_DAEMON_UNAVAILABLE_MESSAGE),
            Line::from(EXTERNAL_AGENT_CONFIG_IMPORT_IN_PROGRESS_MESSAGE),
        ])
        .map(|line| {
            line.spans
                .into_iter()
                .map(|span| span.content.into_owned())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    insta::assert_snapshot!("external_agent_config_migration_messages", messages);
}

#[test]
fn external_agent_config_migration_status_lines_use_semantic_colors() {
    assert_eq!(
        external_agent_config_migration_started_lines(
            &selected_items(),
            /*remaining_item_count*/ 0,
        ),
        vec![
            Line::from(vec![
                "• ".dim(),
                "Claude Code import started.".cyan(),
                " You can keep working while it finishes.".into(),
            ]),
            Line::from(vec![
                "  ".into(),
                "Imported setup will apply to new chats.".dim(),
            ]),
            Line::from(vec!["  ".into(), "Importing:".cyan().bold()]),
            Line::from(vec![
                "    ".into(),
                "Settings".cyan(),
                ": ".into(),
                "1".green(),
            ]),
            Line::from(vec![
                "    ".into(),
                "Skills".cyan(),
                ": ".into(),
                "4".green(),
                " — ".dim(),
                "triage, release-notes, risk-check, +1 more".into(),
            ]),
            Line::from(vec![
                "    ".into(),
                "MCP servers".cyan(),
                ": ".into(),
                "2".green(),
                " — ".dim(),
                "docs, issues".into(),
            ]),
            Line::from(vec![
                "    ".into(),
                "Chat sessions".cyan(),
                ": ".into(),
                "3".green(),
                " — ".dim(),
                "Alpha rollout, Beta review, Gamma notes".into(),
            ]),
            Line::from(vec![
                "    ".into(),
                "Plugins".cyan(),
                ": ".into(),
                "2".green(),
                " — ".dim(),
                "formatter, reviewer".into(),
            ]),
        ]
    );

    assert_eq!(
        external_agent_config_migration_finished_lines(&completed_notification()),
        vec![
            Line::from(vec![
                "• ".dim(),
                "Claude Code import finished: ".into(),
                "2 imported".green(),
                ", ".into(),
                "1 failed".red(),
                ".".into(),
            ]),
            Line::from(vec!["  ".into(), "Results by type:".cyan().bold()]),
            Line::from(vec![
                "    ".into(),
                "Settings".cyan(),
                ": ".into(),
                "1 imported".green(),
                ", ".into(),
                "0 failed".green(),
            ]),
            Line::from(vec![
                "    ".into(),
                "Plugins".cyan(),
                ": ".into(),
                "1 imported".green(),
                ", ".into(),
                "1 failed".red(),
            ]),
            Line::from(vec![
                "  ".into(),
                "Run /import again to check for additional items.".dim(),
            ]),
        ]
    );
}
