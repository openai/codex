use super::super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn import_repo_agents_md_from_nested_cwd_rewrites_terms_and_skips_non_empty_targets() {
    let root = TempDir::new().expect("create tempdir");
    let repo_root = root.path().join("repo-a");
    let nested_cwd = repo_root.join("nested");
    let repo_with_existing_target = root.path().join("repo-b");
    fs::create_dir_all(&nested_cwd).expect("create nested cwd");
    fs::create_dir_all(repo_root.join(".git")).expect("create git");
    fs::create_dir_all(repo_with_existing_target.join(".git")).expect("create git");
    fs::write(
        repo_root.join(EXTERNAL_AGENT_CONFIG_MD),
        format!(
            "{SOURCE_EXTERNAL_AGENT_PRODUCT_NAME}\n{SOURCE_EXTERNAL_AGENT_NAME}\n{SOURCE_EXTERNAL_AGENT_UPPER_PRODUCT_NAME}\nSee {EXTERNAL_AGENT_CONFIG_MD}\n"
        ),
    )
    .expect("write source");
    fs::write(
        repo_with_existing_target.join(EXTERNAL_AGENT_CONFIG_MD),
        "new source",
    )
    .expect("write source");
    fs::write(
        repo_with_existing_target.join("AGENTS.md"),
        "keep existing target",
    )
    .expect("write target");

    let outcome = service_for_paths(
        root.path().join(EXTERNAL_AGENT_DIR),
        root.path().join(".codex"),
    )
    .import(vec![
        ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::AgentsMd,
            description: String::new(),
            cwd: Some(nested_cwd.clone()),
            details: None,
        },
        ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::AgentsMd,
            description: String::new(),
            cwd: Some(repo_with_existing_target.clone()),
            details: None,
        },
    ])
    .await;

    assert_eq!(
        outcome.item_results,
        vec![
            ExternalAgentConfigImportItemResult {
                item_type: ExternalAgentConfigMigrationItemType::AgentsMd,
                description: String::new(),
                cwd: Some(nested_cwd.clone()),
                success_count: 1,
                error_count: 0,
                successes: vec![import_success(
                    ExternalAgentConfigMigrationItemType::AgentsMd,
                    Some(nested_cwd),
                    repo_root
                        .join(EXTERNAL_AGENT_CONFIG_MD)
                        .display()
                        .to_string(),
                    repo_root.join("AGENTS.md").display().to_string(),
                )],
                raw_errors: Vec::new(),
            },
            ExternalAgentConfigImportItemResult {
                item_type: ExternalAgentConfigMigrationItemType::AgentsMd,
                description: String::new(),
                cwd: Some(repo_with_existing_target.clone()),
                success_count: 0,
                error_count: 0,
                successes: Vec::new(),
                raw_errors: Vec::new(),
            },
        ]
    );
    assert_eq!(
        fs::read_to_string(repo_root.join("AGENTS.md")).expect("read target"),
        "Codex\nCodex\nCodex\nSee AGENTS.md\n"
    );
    assert_eq!(
        fs::read_to_string(repo_with_existing_target.join("AGENTS.md"))
            .expect("read existing target"),
        "keep existing target"
    );
}

#[tokio::test]
async fn import_repo_agents_md_overwrites_empty_targets() {
    let root = TempDir::new().expect("create tempdir");
    let repo_root = root.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).expect("create git");
    fs::write(
        repo_root.join(EXTERNAL_AGENT_CONFIG_MD),
        format!("{SOURCE_EXTERNAL_AGENT_DISPLAY_NAME} code guidance"),
    )
    .expect("write source");
    fs::write(repo_root.join("AGENTS.md"), " \n\t").expect("write empty target");

    let outcome = service_for_paths(
        root.path().join(EXTERNAL_AGENT_DIR),
        root.path().join(".codex"),
    )
    .import(vec![ExternalAgentConfigMigrationItem {
        item_type: ExternalAgentConfigMigrationItemType::AgentsMd,
        description: String::new(),
        cwd: Some(repo_root.clone()),
        details: None,
    }])
    .await;

    assert_eq!(
        outcome.item_results,
        vec![ExternalAgentConfigImportItemResult {
            item_type: ExternalAgentConfigMigrationItemType::AgentsMd,
            description: String::new(),
            cwd: Some(repo_root.clone()),
            success_count: 1,
            error_count: 0,
            successes: vec![import_success(
                ExternalAgentConfigMigrationItemType::AgentsMd,
                Some(repo_root.clone()),
                repo_root
                    .join(EXTERNAL_AGENT_CONFIG_MD)
                    .display()
                    .to_string(),
                repo_root.join("AGENTS.md").display().to_string(),
            )],
            raw_errors: Vec::new(),
        }]
    );
    assert_eq!(
        fs::read_to_string(repo_root.join("AGENTS.md")).expect("read target"),
        "Codex guidance"
    );
}

#[tokio::test]
async fn detect_repo_prefers_non_empty_external_agent_agents_source() {
    let root = TempDir::new().expect("create tempdir");
    let repo_root = root.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).expect("create git");
    fs::create_dir_all(repo_root.join(EXTERNAL_AGENT_DIR)).expect("create external agent dir");
    fs::write(repo_root.join(EXTERNAL_AGENT_CONFIG_MD), " \n\t").expect("write empty root source");
    fs::write(
        repo_root
            .join(EXTERNAL_AGENT_DIR)
            .join(EXTERNAL_AGENT_CONFIG_MD),
        format!("{SOURCE_EXTERNAL_AGENT_DISPLAY_NAME} code guidance"),
    )
    .expect("write external agent source");

    let items = service_for_paths(
        root.path().join(EXTERNAL_AGENT_DIR),
        root.path().join(".codex"),
    )
    .detect(ExternalAgentConfigDetectOptions {
        include_home: false,
        include_memory: false,
        cwds: Some(vec![repo_root.clone()]),
    })
    .await
    .expect("detect");

    assert_eq!(
        items,
        vec![ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::AgentsMd,
            description: format!(
                "Migrate {} to {}",
                repo_root
                    .join(EXTERNAL_AGENT_DIR)
                    .join(EXTERNAL_AGENT_CONFIG_MD)
                    .display(),
                repo_root.join("AGENTS.md").display(),
            ),
            cwd: Some(repo_root),
            details: None,
        }]
    );
}

#[tokio::test]
async fn import_repo_hooks_preserves_disabled_codex_hooks_feature() {
    let root = TempDir::new().expect("create tempdir");
    let repo_root = root.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).expect("create git dir");
    fs::create_dir_all(repo_root.join(EXTERNAL_AGENT_DIR)).expect("create external agent dir");
    fs::create_dir_all(repo_root.join(".codex")).expect("create codex dir");
    fs::write(
        repo_root.join(EXTERNAL_AGENT_DIR).join("settings.json"),
        r#"{"hooks":{"Stop":[{"hooks":[{"command":"echo done"}]}]}}"#,
    )
    .expect("write hooks");
    fs::write(
        repo_root.join(".codex").join("config.toml"),
        "[features]\ncodex_hooks = false\n",
    )
    .expect("write config");

    let outcome = service_for_paths(
        root.path().join(EXTERNAL_AGENT_DIR),
        root.path().join(".codex"),
    )
    .import(vec![ExternalAgentConfigMigrationItem {
        item_type: ExternalAgentConfigMigrationItemType::Hooks,
        description: String::new(),
        cwd: Some(repo_root.clone()),
        details: None,
    }])
    .await;

    assert_eq!(
        outcome.item_results,
        vec![ExternalAgentConfigImportItemResult {
            item_type: ExternalAgentConfigMigrationItemType::Hooks,
            description: String::new(),
            cwd: Some(repo_root.clone()),
            success_count: 1,
            error_count: 0,
            successes: vec![import_success(
                ExternalAgentConfigMigrationItemType::Hooks,
                Some(repo_root.clone()),
                "Stop",
                "Stop",
            )],
            raw_errors: Vec::new(),
        }]
    );
    assert_eq!(
        fs::read_to_string(repo_root.join(".codex").join("config.toml")).expect("read config"),
        "[features]\ncodex_hooks = false\n"
    );
    let hooks: JsonValue = serde_json::from_str(
        &fs::read_to_string(repo_root.join(".codex").join("hooks.json")).expect("read hooks"),
    )
    .expect("parse hooks");
    assert_eq!(
        hooks,
        serde_json::json!({
            "hooks": {
                "Stop": [{
                    "hooks": [{
                        "type": "command",
                        "command": "echo done"
                    }]
                }]
            }
        })
    );
}

#[tokio::test]
async fn import_repo_mcp_uses_home_settings_toggles_when_repo_settings_missing() {
    let root = TempDir::new().expect("create tempdir");
    let repo_root = root.path().join("repo");
    let external_agent_home = root.path().join(EXTERNAL_AGENT_DIR);
    fs::create_dir_all(repo_root.join(".git")).expect("create git dir");
    fs::create_dir_all(&external_agent_home).expect("create external agent home");
    fs::write(
        external_agent_home.join("settings.json"),
        r#"{"disabledMcpjsonServers":["blocked"]}"#,
    )
    .expect("write home settings");
    fs::write(
        root.path().join(EXTERNAL_AGENT_PROJECT_CONFIG_FILE),
        serde_json::json!({
            "projects": {
                repo_root.display().to_string(): {
                    "mcpServers": {
                        "allowed": {"command": "allowed-server"},
                        "blocked": {"command": "blocked-server"}
                    }
                }
            }
        })
        .to_string(),
    )
    .expect("write external agent project config");

    let outcome = service_for_paths(external_agent_home, root.path().join(".codex"))
        .import(vec![ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::McpServerConfig,
            description: String::new(),
            cwd: Some(repo_root.clone()),
            details: None,
        }])
        .await;

    assert_eq!(
        outcome.item_results,
        vec![ExternalAgentConfigImportItemResult {
            item_type: ExternalAgentConfigMigrationItemType::McpServerConfig,
            description: String::new(),
            cwd: Some(repo_root.clone()),
            success_count: 1,
            error_count: 0,
            successes: vec![import_success(
                ExternalAgentConfigMigrationItemType::McpServerConfig,
                Some(repo_root.clone()),
                "allowed",
                "allowed",
            )],
            raw_errors: Vec::new(),
        }]
    );
    let config: TomlValue = toml::from_str(
        &fs::read_to_string(repo_root.join(".codex").join("config.toml")).expect("read config"),
    )
    .expect("parse config");
    let expected: TomlValue = toml::from_str(
        r#"
[mcp_servers.allowed]
command = "allowed-server"
"#,
    )
    .expect("parse expected config");
    assert_eq!(config, expected);
}

#[tokio::test]
async fn import_repo_mcp_uses_local_settings_toggles_over_project_settings() {
    let root = TempDir::new().expect("create tempdir");
    let repo_root = root.path().join("repo");
    let external_agent_home = root.path().join(EXTERNAL_AGENT_DIR);
    fs::create_dir_all(repo_root.join(".git")).expect("create git dir");
    fs::create_dir_all(repo_root.join(EXTERNAL_AGENT_DIR)).expect("create external agent dir");
    fs::write(
        repo_root.join(".mcp.json"),
        r#"{
          "mcpServers": {
            "project-disabled": {"command": "project-disabled-server"},
            "local-disabled": {"command": "local-disabled-server"},
            "local-enabled": {"command": "local-enabled-server"}
          }
        }"#,
    )
    .expect("write mcp");
    fs::write(
        repo_root.join(EXTERNAL_AGENT_DIR).join("settings.json"),
        r#"{
          "enabledMcpjsonServers": ["project-disabled", "local-disabled"],
          "disabledMcpjsonServers": ["project-disabled"]
        }"#,
    )
    .expect("write project settings");
    fs::write(
        repo_root
            .join(EXTERNAL_AGENT_DIR)
            .join("settings.local.json"),
        r#"{
          "enabledMcpjsonServers": ["local-enabled", "local-disabled"],
          "disabledMcpjsonServers": ["local-disabled"]
        }"#,
    )
    .expect("write local settings");

    service_for_paths(external_agent_home, root.path().join(".codex"))
        .import(vec![ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::McpServerConfig,
            description: String::new(),
            cwd: Some(repo_root.clone()),
            details: None,
        }])
        .await;

    let config: TomlValue = toml::from_str(
        &fs::read_to_string(repo_root.join(".codex").join("config.toml")).expect("read config"),
    )
    .expect("parse config");
    let expected: TomlValue = toml::from_str(
        r#"
[mcp_servers.local-enabled]
command = "local-enabled-server"
"#,
    )
    .expect("parse expected config");
    assert_eq!(config, expected);
}

#[tokio::test]
async fn import_repo_mcp_ignores_invalid_home_settings_when_repo_settings_missing() {
    let root = TempDir::new().expect("create tempdir");
    let repo_root = root.path().join("repo");
    let external_agent_home = root.path().join(EXTERNAL_AGENT_DIR);
    fs::create_dir_all(repo_root.join(".git")).expect("create git dir");
    fs::create_dir_all(&external_agent_home).expect("create external agent home");
    fs::write(external_agent_home.join("settings.json"), "{ invalid json")
        .expect("write invalid home settings");
    fs::write(
        root.path().join(EXTERNAL_AGENT_PROJECT_CONFIG_FILE),
        serde_json::json!({
            "projects": {
                repo_root.display().to_string(): {
                    "mcpServers": {
                        "docs": {"command": "docs-server"}
                    }
                }
            }
        })
        .to_string(),
    )
    .expect("write external agent project config");

    service_for_paths(external_agent_home, root.path().join(".codex"))
        .import(vec![ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::McpServerConfig,
            description: String::new(),
            cwd: Some(repo_root.clone()),
            details: None,
        }])
        .await;

    let config: TomlValue = toml::from_str(
        &fs::read_to_string(repo_root.join(".codex").join("config.toml")).expect("read config"),
    )
    .expect("parse config");
    let expected: TomlValue = toml::from_str(
        r#"
[mcp_servers.docs]
command = "docs-server"
"#,
    )
    .expect("parse expected config");
    assert_eq!(config, expected);
}

#[tokio::test]
async fn import_repo_uses_non_empty_external_agent_agents_source() {
    let root = TempDir::new().expect("create tempdir");
    let repo_root = root.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).expect("create git");
    fs::create_dir_all(repo_root.join(EXTERNAL_AGENT_DIR)).expect("create external agent dir");
    fs::write(repo_root.join(EXTERNAL_AGENT_CONFIG_MD), "").expect("write empty root source");
    fs::write(
        repo_root
            .join(EXTERNAL_AGENT_DIR)
            .join(EXTERNAL_AGENT_CONFIG_MD),
        format!("{SOURCE_EXTERNAL_AGENT_DISPLAY_NAME} code guidance"),
    )
    .expect("write external agent source");

    service_for_paths(
        root.path().join(EXTERNAL_AGENT_DIR),
        root.path().join(".codex"),
    )
    .import(vec![ExternalAgentConfigMigrationItem {
        item_type: ExternalAgentConfigMigrationItemType::AgentsMd,
        description: String::new(),
        cwd: Some(repo_root.clone()),
        details: None,
    }])
    .await;

    assert_eq!(
        fs::read_to_string(repo_root.join("AGENTS.md")).expect("read target"),
        "Codex guidance"
    );
}

#[tokio::test]
async fn import_continues_after_failed_migration_item() {
    let root = TempDir::new().expect("create tempdir");
    let repo_root = root.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).expect("create git");
    fs::write(repo_root.join(EXTERNAL_AGENT_CONFIG_MD), "Claude guidance").expect("write source");

    service_for_paths(
        root.path().join(EXTERNAL_AGENT_DIR),
        root.path().join(".codex"),
    )
    .import(vec![
        ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::Plugins,
            description: "invalid plugin migration".to_string(),
            cwd: Some(repo_root.clone()),
            details: None,
        },
        ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::AgentsMd,
            description: "valid agents migration".to_string(),
            cwd: Some(repo_root.clone()),
            details: None,
        },
    ])
    .await;

    assert_eq!(
        fs::read_to_string(repo_root.join("AGENTS.md")).expect("read target"),
        "Codex guidance"
    );
}

#[test]
fn migration_metric_tags_for_skills_include_skills_count() {
    assert_eq!(
        migration_metric_tags(ExternalAgentConfigMigrationItemType::Skills, Some(3)),
        vec![
            ("migration_type", "skills".to_string()),
            ("skills_count", "3".to_string()),
        ]
    );
}
