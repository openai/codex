use super::super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn import_plugins_requires_source_marketplace_details() {
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
              "source": "github",
              "repo": "acme-corp/external-agent-plugins"
            }
          }
        }"#,
    )
    .expect("write settings");

    let outcome = service_for_paths(external_agent_home, codex_home)
        .import_plugins(
            /*cwd*/ None,
            Some(MigrationDetails {
                plugins: vec![PluginsMigration {
                    marketplace_name: "other-tools".to_string(),
                    plugin_names: github_plugin_details().plugins[0].plugin_names.clone(),
                }],
                ..Default::default()
            }),
        )
        .await
        .expect("import plugins");

    assert_eq!(outcome.succeeded_marketplaces, Vec::<String>::new());
    assert_eq!(outcome.succeeded_plugin_ids, Vec::<String>::new());
    assert_eq!(outcome.failed_marketplaces, vec!["other-tools".to_string()]);
    assert_eq!(
        outcome.failed_plugin_ids,
        vec!["formatter@other-tools".to_string()]
    );
    assert_single_plugin_raw_error(
        &outcome.raw_errors,
        "plugin_import",
        "formatter@other-tools",
        /*error_type*/ None,
    );
}

#[tokio::test]
async fn import_plugins_defers_marketplace_source_validation_to_add_marketplace() {
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
              "source": "local",
              "path": "./external_plugins/acme-tools"
            }
          }
        }"#,
    )
    .expect("write settings");

    let outcome = service_for_paths(external_agent_home, codex_home)
        .import_plugins(/*cwd*/ None, Some(github_plugin_details()))
        .await
        .expect("import plugins");

    assert_eq!(outcome.succeeded_marketplaces, Vec::<String>::new());
    assert_eq!(outcome.succeeded_plugin_ids, Vec::<String>::new());
    assert_eq!(outcome.failed_marketplaces, vec!["acme-tools".to_string()]);
    assert_eq!(
        outcome.failed_plugin_ids,
        vec!["formatter@acme-tools".to_string()]
    );
    assert_single_plugin_raw_error(
        &outcome.raw_errors,
        "plugin_import",
        "formatter@acme-tools",
        /*error_type*/ None,
    );
}

#[tokio::test]
async fn import_plugins_supports_external_agent_plugin_marketplace_layout() {
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
        .import_plugins(
            /*cwd*/ None,
            Some(MigrationDetails {
                plugins: vec![PluginsMigration {
                    marketplace_name: "my-plugins".to_string(),
                    plugin_names: vec!["cloudflare".to_string()],
                }],
                ..Default::default()
            }),
        )
        .await
        .expect("import plugins");

    assert_eq!(
        outcome,
        PluginImportOutcome {
            succeeded_marketplaces: vec!["my-plugins".to_string()],
            succeeded_plugin_ids: vec!["cloudflare@my-plugins".to_string()],
            failed_marketplaces: Vec::new(),
            failed_plugin_ids: Vec::new(),
            raw_errors: Vec::new(),
        }
    );
    let config = fs::read_to_string(codex_home.join("config.toml")).expect("read config");
    assert!(config.contains(r#"[plugins."cloudflare@my-plugins"]"#));
    assert!(config.contains("enabled = true"));
}

#[tokio::test]
async fn import_plugins_reuses_configured_marketplace_with_different_source() {
    let (_root, external_agent_home, codex_home) = fixture_paths();
    let configured_marketplace_root = external_agent_home.join("configured-marketplace");
    let source_marketplace_root = external_agent_home.join("source-marketplace");
    let configured_plugin_root = configured_marketplace_root.join("plugins/cloudflare");
    let source_plugin_root = source_marketplace_root.join("plugins/cloudflare");
    fs::create_dir_all(configured_marketplace_root.join(".agents/plugins"))
        .expect("create configured marketplace manifest dir");
    fs::create_dir_all(configured_plugin_root.join(".codex-plugin"))
        .expect("create configured plugin manifest dir");
    fs::create_dir_all(source_marketplace_root.join(EXTERNAL_AGENT_PLUGIN_MANIFEST_DIR))
        .expect("create source marketplace manifest dir");
    fs::create_dir_all(source_plugin_root.join(".codex-plugin"))
        .expect("create source plugin manifest dir");
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
                    "path": source_marketplace_root
                }
            }
        }))
        .expect("serialize settings"),
    )
    .expect("write settings");
    fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"[marketplaces.my-plugins]
source_type = "local"
source = {configured_marketplace_root:?}
"#
        ),
    )
    .expect("write Codex config");
    fs::write(
        configured_marketplace_root.join(".agents/plugins/marketplace.json"),
        r#"{
          "name": "my-plugins",
          "plugins": [{
            "name": "cloudflare",
            "source": {"source": "local", "path": "./plugins/cloudflare"}
          }]
        }"#,
    )
    .expect("write configured marketplace manifest");
    fs::write(
        source_marketplace_root
            .join(EXTERNAL_AGENT_PLUGIN_MANIFEST_DIR)
            .join("marketplace.json"),
        r#"{
          "name": "my-plugins",
          "plugins": [{"name": "cloudflare", "source": "./plugins/cloudflare"}]
        }"#,
    )
    .expect("write source marketplace manifest");
    fs::write(
        configured_plugin_root.join(".codex-plugin/plugin.json"),
        r#"{"name":"cloudflare","version":"0.1.0"}"#,
    )
    .expect("write configured plugin manifest");
    fs::write(
        source_plugin_root.join(".codex-plugin/plugin.json"),
        r#"{"name":"cloudflare","version":"0.2.0"}"#,
    )
    .expect("write source plugin manifest");

    let outcome = service_for_paths(external_agent_home, codex_home.clone())
        .import_plugins(
            /*cwd*/ None,
            Some(MigrationDetails {
                plugins: vec![PluginsMigration {
                    marketplace_name: "my-plugins".to_string(),
                    plugin_names: vec!["cloudflare".to_string()],
                }],
                ..Default::default()
            }),
        )
        .await
        .expect("import plugins");

    assert_eq!(
        outcome,
        PluginImportOutcome {
            succeeded_marketplaces: vec!["my-plugins".to_string()],
            succeeded_plugin_ids: vec!["cloudflare@my-plugins".to_string()],
            failed_marketplaces: Vec::new(),
            failed_plugin_ids: Vec::new(),
            raw_errors: Vec::new(),
        }
    );
    let config: TomlValue =
        toml::from_str(&fs::read_to_string(codex_home.join("config.toml")).expect("read config"))
            .expect("parse config");
    let expected: TomlValue = toml::from_str(&format!(
        r#"[marketplaces.my-plugins]
source_type = "local"
source = {configured_marketplace_root:?}

[plugins."cloudflare@my-plugins"]
enabled = true
"#
    ))
    .expect("parse expected config");
    assert_eq!(config, expected);
}

#[tokio::test]
async fn detect_home_supports_relative_external_agent_plugin_marketplace_path() {
    let (_root, external_agent_home, codex_home) = fixture_paths();
    let marketplace_root = external_agent_home.join("my-marketplace");
    let plugin_root = marketplace_root.join("plugins").join("cloudflare");
    fs::create_dir_all(marketplace_root.join(EXTERNAL_AGENT_PLUGIN_MANIFEST_DIR))
        .expect("create marketplace manifest dir");
    fs::create_dir_all(plugin_root.join(".codex-plugin")).expect("create plugin manifest dir");
    fs::create_dir_all(&codex_home).expect("create codex home");

    fs::write(
        external_agent_home.join("settings.json"),
        r#"{
          "enabledPlugins": {
            "cloudflare@my-plugins": true
          },
          "extraKnownMarketplaces": {
            "my-plugins": {
              "source": "directory",
              "path": "./my-marketplace"
            }
          }
        }"#,
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

    let items = service_for_paths(external_agent_home.clone(), codex_home)
        .detect(ExternalAgentConfigDetectOptions {
            include_home: true,
            include_memory: false,
            cwds: None,
        })
        .await
        .expect("detect");

    assert_eq!(
        items,
        vec![ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::Plugins,
            description: format!(
                "Migrate enabled plugins from {}",
                external_agent_home.join("settings.json").display()
            ),
            cwd: None,
            details: Some(MigrationDetails {
                plugins: vec![PluginsMigration {
                    marketplace_name: "my-plugins".to_string(),
                    plugin_names: vec!["cloudflare".to_string()],
                }],
                ..Default::default()
            }),
        }]
    );
}

#[tokio::test]
async fn detect_home_infers_external_official_marketplace_when_missing_from_settings() {
    let (_root, external_agent_home, codex_home) = fixture_paths();
    fs::create_dir_all(&external_agent_home).expect("create external agent home");
    fs::create_dir_all(&codex_home).expect("create codex home");

    fs::write(
        external_agent_home.join("settings.json"),
        format!(
            r#"{{
          "enabledPlugins": {{
            "sample@{EXTERNAL_OFFICIAL_MARKETPLACE_NAME}": true
          }}
        }}"#
        ),
    )
    .expect("write settings");

    let items = service_for_paths(external_agent_home.clone(), codex_home)
        .detect(ExternalAgentConfigDetectOptions {
            include_home: true,
            include_memory: false,
            cwds: None,
        })
        .await
        .expect("detect");

    assert_eq!(
        items,
        vec![ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::Plugins,
            description: format!(
                "Migrate enabled plugins from {}",
                external_agent_home.join("settings.json").display()
            ),
            cwd: None,
            details: Some(MigrationDetails {
                plugins: vec![PluginsMigration {
                    marketplace_name: EXTERNAL_OFFICIAL_MARKETPLACE_NAME.to_string(),
                    plugin_names: vec!["sample".to_string()],
                }],
                ..Default::default()
            }),
        }]
    );
}

#[tokio::test]
async fn import_plugins_supports_relative_external_agent_plugin_marketplace_path() {
    let (_root, external_agent_home, codex_home) = fixture_paths();
    let marketplace_root = external_agent_home.join("my-marketplace");
    let plugin_root = marketplace_root.join("plugins").join("cloudflare");
    fs::create_dir_all(marketplace_root.join(EXTERNAL_AGENT_PLUGIN_MANIFEST_DIR))
        .expect("create marketplace manifest dir");
    fs::create_dir_all(plugin_root.join(".codex-plugin")).expect("create plugin manifest dir");
    fs::create_dir_all(&codex_home).expect("create codex home");

    fs::write(
        external_agent_home.join("settings.json"),
        r#"{
          "enabledPlugins": {
            "cloudflare@my-plugins": true
          },
          "extraKnownMarketplaces": {
            "my-plugins": {
              "source": "directory",
              "path": "./my-marketplace"
            }
          }
        }"#,
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
        .import_plugins(
            /*cwd*/ None,
            Some(MigrationDetails {
                plugins: vec![PluginsMigration {
                    marketplace_name: "my-plugins".to_string(),
                    plugin_names: vec!["cloudflare".to_string()],
                }],
                ..Default::default()
            }),
        )
        .await
        .expect("import plugins");

    assert_eq!(
        outcome,
        PluginImportOutcome {
            succeeded_marketplaces: vec!["my-plugins".to_string()],
            succeeded_plugin_ids: vec!["cloudflare@my-plugins".to_string()],
            failed_marketplaces: Vec::new(),
            failed_plugin_ids: Vec::new(),
            raw_errors: Vec::new(),
        }
    );
    let config = fs::read_to_string(codex_home.join("config.toml")).expect("read config");
    assert!(config.contains(r#"[plugins."cloudflare@my-plugins"]"#));
    assert!(config.contains("enabled = true"));
}

#[tokio::test]
async fn import_plugins_infers_external_official_marketplace_when_missing_from_settings() {
    let (_root, external_agent_home, codex_home) = fixture_paths();
    fs::create_dir_all(&external_agent_home).expect("create external agent home");
    fs::create_dir_all(&codex_home).expect("create codex home");

    fs::write(
        external_agent_home.join("settings.json"),
        format!(
            r#"{{
          "enabledPlugins": {{
            "sample@{EXTERNAL_OFFICIAL_MARKETPLACE_NAME}": true
          }}
        }}"#
        ),
    )
    .expect("write settings");

    let outcome = service_for_paths(external_agent_home, codex_home)
        .import_plugins(
            /*cwd*/ None,
            Some(MigrationDetails {
                plugins: vec![PluginsMigration {
                    marketplace_name: EXTERNAL_OFFICIAL_MARKETPLACE_NAME.to_string(),
                    plugin_names: vec!["sample".to_string()],
                }],
                ..Default::default()
            }),
        )
        .await
        .expect("import plugins");

    assert_eq!(
        outcome.succeeded_marketplaces,
        vec![EXTERNAL_OFFICIAL_MARKETPLACE_NAME.to_string()]
    );
    assert_eq!(outcome.succeeded_plugin_ids, Vec::<String>::new());
    assert_eq!(outcome.failed_marketplaces, Vec::<String>::new());
    assert_eq!(
        outcome.failed_plugin_ids,
        vec![format!("sample@{EXTERNAL_OFFICIAL_MARKETPLACE_NAME}")]
    );
    assert_single_plugin_raw_error(
        &outcome.raw_errors,
        "plugin_import",
        &format!("sample@{EXTERNAL_OFFICIAL_MARKETPLACE_NAME}"),
        Some("plugin_not_found"),
    );
}

#[tokio::test]
async fn detect_repo_supports_project_relative_external_agent_plugin_marketplace_path() {
    let root = TempDir::new().expect("create tempdir");
    let external_agent_home = root.path().join(EXTERNAL_AGENT_DIR);
    let codex_home = root.path().join(".codex");
    let repo_root = root.path().join("repo");
    let marketplace_root = repo_root.join("my-marketplace");
    let plugin_root = marketplace_root.join("plugins").join("cloudflare");
    fs::create_dir_all(repo_root.join(".git")).expect("create git dir");
    fs::create_dir_all(repo_root.join(EXTERNAL_AGENT_DIR)).expect("create repo external agent dir");
    fs::create_dir_all(marketplace_root.join(EXTERNAL_AGENT_PLUGIN_MANIFEST_DIR))
        .expect("create marketplace manifest dir");
    fs::create_dir_all(plugin_root.join(".codex-plugin")).expect("create plugin manifest dir");
    fs::create_dir_all(&codex_home).expect("create codex home");

    fs::write(
        repo_root.join(EXTERNAL_AGENT_DIR).join("settings.json"),
        r#"{
          "enabledPlugins": {
            "cloudflare@my-plugins": true
          },
          "extraKnownMarketplaces": {
            "my-plugins": {
              "source": "directory",
              "path": "./my-marketplace"
            }
          }
        }"#,
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

    let items = service_for_paths(external_agent_home, codex_home)
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
            item_type: ExternalAgentConfigMigrationItemType::Plugins,
            description: format!(
                "Migrate enabled plugins from {}",
                repo_root
                    .join(EXTERNAL_AGENT_DIR)
                    .join("settings.json")
                    .display()
            ),
            cwd: Some(repo_root),
            details: Some(MigrationDetails {
                plugins: vec![PluginsMigration {
                    marketplace_name: "my-plugins".to_string(),
                    plugin_names: vec!["cloudflare".to_string()],
                }],
                ..Default::default()
            }),
        }]
    );
}

#[tokio::test]
async fn import_plugins_supports_project_relative_external_agent_plugin_marketplace_path() {
    let root = TempDir::new().expect("create tempdir");
    let external_agent_home = root.path().join(EXTERNAL_AGENT_DIR);
    let codex_home = root.path().join(".codex");
    let repo_root = root.path().join("repo");
    let marketplace_root = repo_root.join("my-marketplace");
    let plugin_root = marketplace_root.join("plugins").join("cloudflare");
    fs::create_dir_all(repo_root.join(".git")).expect("create git dir");
    fs::create_dir_all(repo_root.join(EXTERNAL_AGENT_DIR)).expect("create repo external agent dir");
    fs::create_dir_all(marketplace_root.join(EXTERNAL_AGENT_PLUGIN_MANIFEST_DIR))
        .expect("create marketplace manifest dir");
    fs::create_dir_all(plugin_root.join(".codex-plugin")).expect("create plugin manifest dir");
    fs::create_dir_all(&codex_home).expect("create codex home");

    fs::write(
        repo_root.join(EXTERNAL_AGENT_DIR).join("settings.json"),
        r#"{
          "enabledPlugins": {
            "cloudflare@my-plugins": true
          },
          "extraKnownMarketplaces": {
            "my-plugins": {
              "source": "directory",
              "path": "./my-marketplace"
            }
          }
        }"#,
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
        .import_plugins(
            Some(repo_root.as_path()),
            Some(MigrationDetails {
                plugins: vec![PluginsMigration {
                    marketplace_name: "my-plugins".to_string(),
                    plugin_names: vec!["cloudflare".to_string()],
                }],
                ..Default::default()
            }),
        )
        .await
        .expect("import plugins");

    assert_eq!(
        outcome,
        PluginImportOutcome {
            succeeded_marketplaces: vec!["my-plugins".to_string()],
            succeeded_plugin_ids: vec!["cloudflare@my-plugins".to_string()],
            failed_marketplaces: Vec::new(),
            failed_plugin_ids: Vec::new(),
            raw_errors: Vec::new(),
        }
    );
    let config = fs::read_to_string(codex_home.join("config.toml")).expect("read config");
    assert!(config.contains(r#"[plugins."cloudflare@my-plugins"]"#));
    assert!(config.contains("enabled = true"));
}

#[test]
fn import_skills_returns_only_new_skill_directory_names() {
    let (_root, external_agent_home, codex_home) = fixture_paths();
    let agents_skills = codex_home
        .parent()
        .map(|parent| parent.join(".agents").join("skills"))
        .unwrap_or_else(|| PathBuf::from(".agents").join("skills"));
    fs::create_dir_all(external_agent_home.join("skills").join("skill-a"))
        .expect("create source a");
    fs::create_dir_all(external_agent_home.join("skills").join("skill-b"))
        .expect("create source b");
    fs::create_dir_all(agents_skills.join("skill-a")).expect("create existing target");

    let copied_names = service_for_paths(external_agent_home, codex_home)
        .import_skills(/*cwd*/ None)
        .expect("import skills");

    assert_eq!(copied_names, vec!["skill-b".to_string()]);
}
