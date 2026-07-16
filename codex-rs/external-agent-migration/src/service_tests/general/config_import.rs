use super::super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn import_repo_mcp_preserves_existing_same_named_server() {
    let root = TempDir::new().expect("create tempdir");
    let repo_root = root.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).expect("create git dir");
    fs::write(
        repo_root.join(".mcp.json"),
        r#"{
          "mcpServers": {
            "mixedTransport": {
              "command": "mcp-remote-proxy",
              "args": [
                "https://example.com/mixed-transport",
                "--transport",
                "http"
              ],
              "url": "https://example.com/mixed-transport"
            }
          }
        }"#,
    )
    .expect("write mcp");
    fs::create_dir_all(repo_root.join(".codex")).expect("create codex dir");
    let existing_config = r#"[mcp_servers.mixedTransport]
url = "https://example.com/mixed-transport"
"#;
    fs::write(
        repo_root.join(".codex").join("config.toml"),
        existing_config,
    )
    .expect("write config");

    let service = service_for_paths(
        root.path().join(EXTERNAL_AGENT_DIR),
        root.path().join(".codex"),
    );
    assert_eq!(
        service
            .detect(ExternalAgentConfigDetectOptions {
                include_home: false,
                include_memory: false,
                cwds: Some(vec![repo_root.clone()]),
            })
            .await
            .expect("detect"),
        Vec::<ExternalAgentConfigMigrationItem>::new()
    );

    service
        .import(vec![ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::McpServerConfig,
            description: String::new(),
            cwd: Some(repo_root.clone()),
            details: None,
        }])
        .await;

    assert_eq!(
        fs::read_to_string(repo_root.join(".codex").join("config.toml")).expect("read config"),
        existing_config
    );
}

#[tokio::test]
async fn detect_repo_mcp_lists_only_missing_servers() {
    let root = TempDir::new().expect("create tempdir");
    let repo_root = root.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).expect("create git dir");
    fs::write(
        repo_root.join(".mcp.json"),
        r#"{
          "mcpServers": {
            "docs": {"command": "docs-server"},
            "mixedTransport": {"command": "mcp-remote-proxy"}
          }
        }"#,
    )
    .expect("write mcp");
    fs::create_dir_all(repo_root.join(".codex")).expect("create codex dir");
    fs::write(
        repo_root.join(".codex").join("config.toml"),
        r#"[mcp_servers.mixedTransport]
url = "https://example.com/mixed-transport"
"#,
    )
    .expect("write config");

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
            item_type: ExternalAgentConfigMigrationItemType::McpServerConfig,
            description: format!(
                "Migrate MCP servers from {} into {}",
                repo_root.display(),
                repo_root.join(".codex").join("config.toml").display()
            ),
            cwd: Some(repo_root),
            details: Some(MigrationDetails {
                mcp_servers: vec![NamedMigration {
                    name: "docs".to_string(),
                }],
                ..Default::default()
            }),
        }]
    );
}

#[tokio::test]
async fn import_home_migrates_supported_config_fields_skills_and_agents_md() {
    let (_root, external_agent_home, codex_home) = fixture_paths();
    let agents_skills = codex_home
        .parent()
        .map(|parent| parent.join(".agents").join("skills"))
        .unwrap_or_else(|| PathBuf::from(".agents").join("skills"));
    fs::create_dir_all(external_agent_home.join("skills").join("skill-a")).expect("create skills");
    fs::write(
            external_agent_home.join("settings.json"),
            format!(r#"{{"model":"{SOURCE_EXTERNAL_AGENT_NAME}","permissions":{{"ask":["git push"]}},"env":{{"FOO":"bar","CI":false,"MAX_RETRIES":3,"MY_TEAM":"codex","IGNORED":null,"LIST":["a","b"],"MAP":{{"x":1}}}},"sandbox":{{"enabled":true,"network":{{"allowLocalBinding":true}}}}}}"#),
        )
        .expect("write settings");
    fs::write(
        external_agent_home
            .join("skills")
            .join("skill-a")
            .join("SKILL.md"),
        format!(
            "Use {SOURCE_EXTERNAL_AGENT_PRODUCT_NAME} and {SOURCE_EXTERNAL_AGENT_UPPER_NAME} utilities."
        ),
    )
    .expect("write skill");
    fs::write(
        external_agent_home.join(EXTERNAL_AGENT_CONFIG_MD),
        format!("{SOURCE_EXTERNAL_AGENT_DISPLAY_NAME} code guidance"),
    )
    .expect("write agents");

    service_for_paths(external_agent_home, codex_home.clone())
        .import(vec![
            ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::AgentsMd,
                description: String::new(),
                cwd: None,
                details: None,
            },
            ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::Config,
                description: String::new(),
                cwd: None,
                details: None,
            },
            ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::Skills,
                description: String::new(),
                cwd: None,
                details: None,
            },
        ])
        .await;

    assert_eq!(
        fs::read_to_string(codex_home.join("AGENTS.md")).expect("read agents"),
        "Codex guidance"
    );

    let config: TomlValue =
        toml::from_str(&fs::read_to_string(codex_home.join("config.toml")).expect("read config"))
            .expect("parse config");
    let expected: TomlValue = toml::from_str(
        r#"
sandbox_mode = "workspace-write"

[shell_environment_policy]
inherit = "core"

[shell_environment_policy.set]
CI = "false"
FOO = "bar"
MAX_RETRIES = "3"
MY_TEAM = "codex"
"#,
    )
    .expect("parse expected config");
    assert_eq!(config, expected);
    assert_eq!(
        fs::read_to_string(agents_skills.join("skill-a").join("SKILL.md"))
            .expect("read copied skill"),
        "Use Codex and Codex utilities."
    );
}

#[tokio::test]
async fn import_home_config_uses_local_settings_over_project_settings() {
    let (_root, external_agent_home, codex_home) = fixture_paths();
    fs::create_dir_all(&external_agent_home).expect("create external agent home");
    fs::write(
        external_agent_home.join("settings.json"),
        r#"{"env":{"FOO":"project","PROJECT_ONLY":"yes"},"sandbox":{"enabled":false}}"#,
    )
    .expect("write project settings");
    fs::write(
        external_agent_home.join("settings.local.json"),
        r#"{"env":{"FOO":"local","LOCAL_ONLY":true},"sandbox":{"enabled":true}}"#,
    )
    .expect("write local settings");

    service_for_paths(external_agent_home, codex_home.clone())
        .import(vec![ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::Config,
            description: String::new(),
            cwd: None,
            details: None,
        }])
        .await;

    let config: TomlValue =
        toml::from_str(&fs::read_to_string(codex_home.join("config.toml")).expect("read config"))
            .expect("parse config");
    let expected: TomlValue = toml::from_str(
        r#"
sandbox_mode = "workspace-write"

[shell_environment_policy]
inherit = "core"

[shell_environment_policy.set]
FOO = "local"
LOCAL_ONLY = "true"
PROJECT_ONLY = "yes"
"#,
    )
    .expect("parse expected config");
    assert_eq!(config, expected);
}

#[tokio::test]
async fn import_home_config_ignores_invalid_local_settings() {
    let (_root, external_agent_home, codex_home) = fixture_paths();
    fs::create_dir_all(&external_agent_home).expect("create external agent home");
    fs::write(
        external_agent_home.join("settings.json"),
        r#"{"env":{"FOO":"project"},"sandbox":{"enabled":false}}"#,
    )
    .expect("write project settings");
    fs::write(
        external_agent_home.join("settings.local.json"),
        "{invalid json",
    )
    .expect("write local settings");

    service_for_paths(external_agent_home, codex_home.clone())
        .import(vec![ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::Config,
            description: String::new(),
            cwd: None,
            details: None,
        }])
        .await;

    assert_eq!(
        fs::read_to_string(codex_home.join("config.toml")).expect("read config"),
        "[shell_environment_policy]\ninherit = \"core\"\n\n[shell_environment_policy.set]\nFOO = \"project\"\n"
    );
}

#[tokio::test]
async fn import_home_skips_empty_config_migration() {
    let (_root, external_agent_home, codex_home) = fixture_paths();
    fs::create_dir_all(&external_agent_home).expect("create external agent home");
    fs::write(
        external_agent_home.join("settings.json"),
        format!(r#"{{"model":"{SOURCE_EXTERNAL_AGENT_NAME}","sandbox":{{"enabled":false}}}}"#),
    )
    .expect("write settings");

    let outcome = service_for_paths(external_agent_home, codex_home.clone())
        .import(vec![ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::Config,
            description: String::new(),
            cwd: None,
            details: None,
        }])
        .await;

    assert_eq!(
        outcome.item_results,
        vec![ExternalAgentConfigImportItemResult {
            item_type: ExternalAgentConfigMigrationItemType::Config,
            description: String::new(),
            cwd: None,
            success_count: 0,
            error_count: 0,
            successes: Vec::new(),
            raw_errors: Vec::new(),
        }]
    );
    assert!(!codex_home.join("config.toml").exists());
}

#[tokio::test]
async fn import_local_plugins_returns_completed_status() {
    let (_root, external_agent_home, codex_home) = fixture_paths();
    let marketplace_root = external_agent_home.join("my-marketplace");
    let plugin_root = marketplace_root.join("plugins").join("cloudflare");
    fs::create_dir_all(marketplace_root.join(EXTERNAL_AGENT_PLUGIN_MANIFEST_DIR))
        .expect("create marketplace manifest dir");
    fs::create_dir_all(plugin_root.join(".codex-plugin")).expect("create plugin manifest dir");
    fs::create_dir_all(&codex_home).expect("create codex home");

    fs::write(
        external_agent_home.join("settings.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "enabledPlugins": {
                "cloudflare@my-plugins": true
            },
            "extraKnownMarketplaces": {
                "my-plugins": {
                    "source": "local",
                    "path": marketplace_root
                }
            }
        }))
        .expect("serialize settings"),
    )
    .expect("write settings");
    fs::write(
        marketplace_root
            .join(EXTERNAL_AGENT_PLUGIN_MANIFEST_DIR)
            .join("marketplace.json"),
        r#"{
          "name": "my-plugins",
          "plugins": [
            {
              "name": "cloudflare",
              "source": "./plugins/cloudflare"
            }
          ]
        }"#,
    )
    .expect("write marketplace manifest");
    fs::write(
        plugin_root.join(".codex-plugin").join("plugin.json"),
        r#"{"name":"cloudflare","version":"0.1.0"}"#,
    )
    .expect("write plugin manifest");

    let outcome = service_for_paths(external_agent_home, codex_home.clone())
        .import(vec![ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::Plugins,
            description: String::new(),
            cwd: None,
            details: Some(MigrationDetails {
                plugins: vec![PluginsMigration {
                    marketplace_name: "my-plugins".to_string(),
                    plugin_names: vec!["cloudflare".to_string()],
                }],
                ..Default::default()
            }),
        }])
        .await;

    assert_eq!(
        outcome.pending_plugin_imports,
        Vec::<PendingPluginImport>::new()
    );
    assert_eq!(
        outcome.item_results,
        vec![ExternalAgentConfigImportItemResult {
            item_type: ExternalAgentConfigMigrationItemType::Plugins,
            description: String::new(),
            cwd: None,
            success_count: 1,
            error_count: 0,
            successes: vec![ExternalAgentConfigImportSuccess {
                item_type: ExternalAgentConfigMigrationItemType::Plugins,
                cwd: None,
                source: Some("cloudflare@my-plugins".to_string()),
                target: Some("cloudflare@my-plugins".to_string()),
            }],
            raw_errors: Vec::new(),
        }]
    );
    let config = fs::read_to_string(codex_home.join("config.toml")).expect("read config");
    assert!(config.contains(r#"[plugins."cloudflare@my-plugins"]"#));
    assert!(config.contains("enabled = true"));
}

#[tokio::test]
async fn import_git_plugins_returns_pending_async_status() {
    let (_root, external_agent_home, codex_home) = fixture_paths();
    fs::create_dir_all(&external_agent_home).expect("create external agent home");
    fs::write(
        external_agent_home.join("settings.json"),
        r#"{
          "enabledPlugins": {
            "formatter@acme-tools": true
          },
          "extraKnownMarketplaces": {
            "acme-tools": {
              "source": "owner/debug-marketplace"
            }
          }
        }"#,
    )
    .expect("write settings");

    let outcome = service_for_paths(external_agent_home, codex_home.clone())
        .import(vec![ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::Plugins,
            description: String::new(),
            cwd: None,
            details: Some(MigrationDetails {
                plugins: vec![PluginsMigration {
                    marketplace_name: "acme-tools".to_string(),
                    plugin_names: vec!["formatter".to_string()],
                }],
                ..Default::default()
            }),
        }])
        .await;

    assert_eq!(
        outcome.pending_plugin_imports,
        vec![PendingPluginImport {
            cwd: None,
            description: String::new(),
            details: MigrationDetails {
                plugins: vec![PluginsMigration {
                    marketplace_name: "acme-tools".to_string(),
                    plugin_names: vec!["formatter".to_string()],
                }],
                ..Default::default()
            },
        }]
    );
    assert_eq!(
        outcome.item_results,
        vec![ExternalAgentConfigImportItemResult {
            item_type: ExternalAgentConfigMigrationItemType::Plugins,
            description: String::new(),
            cwd: None,
            success_count: 0,
            error_count: 0,
            successes: Vec::new(),
            raw_errors: Vec::new(),
        }]
    );
    assert!(!codex_home.join("config.toml").exists());
}

#[tokio::test]
async fn detect_home_skips_config_when_target_already_has_supported_fields() {
    let (_root, external_agent_home, codex_home) = fixture_paths();
    fs::create_dir_all(&external_agent_home).expect("create external agent home");
    fs::create_dir_all(&codex_home).expect("create codex home");
    fs::write(
        external_agent_home.join("settings.json"),
        r#"{"env":{"FOO":"bar"},"sandbox":{"enabled":true}}"#,
    )
    .expect("write settings");
    fs::write(
        codex_home.join("config.toml"),
        r#"
            sandbox_mode = "workspace-write"

            [shell_environment_policy]
            inherit = "core"

            [shell_environment_policy.set]
            FOO = "bar"
            "#,
    )
    .expect("write config");

    let items = service_for_paths(external_agent_home, codex_home)
        .detect(ExternalAgentConfigDetectOptions {
            include_home: true,
            include_memory: false,
            cwds: None,
        })
        .await
        .expect("detect");

    assert_eq!(items, Vec::<ExternalAgentConfigMigrationItem>::new());
}

#[tokio::test]
async fn detect_home_skips_skills_when_all_skill_directories_exist() {
    let (_root, external_agent_home, codex_home) = fixture_paths();
    let agents_skills = codex_home
        .parent()
        .map(|parent| parent.join(".agents").join("skills"))
        .unwrap_or_else(|| PathBuf::from(".agents").join("skills"));
    fs::create_dir_all(external_agent_home.join("skills").join("skill-a")).expect("create source");
    fs::create_dir_all(agents_skills.join("skill-a")).expect("create target");

    let items = service_for_paths(external_agent_home, codex_home)
        .detect(ExternalAgentConfigDetectOptions {
            include_home: true,
            include_memory: false,
            cwds: None,
        })
        .await
        .expect("detect");

    assert_eq!(items, Vec::<ExternalAgentConfigMigrationItem>::new());
}
