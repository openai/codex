use super::super::*;
use crate::migration_source::MarketplaceImportSource;
use crate::source_cla;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn detect_home_lists_enabled_plugins_from_settings() {
    let (_root, external_agent_home, codex_home) = fixture_paths();
    fs::create_dir_all(&external_agent_home).expect("create external agent home");
    fs::write(
        external_agent_home.join("settings.json"),
        r#"{
          "enabledPlugins": {
            "formatter@acme-tools": true,
            "deployer@acme-tools": true,
            "analyzer@security-plugins": false
          },
          "extraKnownMarketplaces": {
            "acme-tools": {
              "source": "acme-corp/external-agent-plugins"
            }
          }
        }"#,
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
                    marketplace_name: "acme-tools".to_string(),
                    plugin_names: vec!["deployer".to_string(), "formatter".to_string()],
                }],
                ..Default::default()
            }),
        }]
    );
}

#[tokio::test]
async fn detect_home_uses_materialized_known_marketplace_for_inline_npm_source() {
    let (_root, external_agent_home, codex_home) = fixture_paths();
    let marketplace_root = external_agent_home
        .join("plugins")
        .join("marketplaces")
        .join("acme-tools");
    fs::create_dir_all(external_agent_home.join("plugins"))
        .expect("create external agent plugins dir");
    fs::create_dir_all(&marketplace_root).expect("create installed marketplace dir");
    fs::write(
        external_agent_home.join("settings.json"),
        r#"{
          "enabledPlugins": {
            "formatter@acme-tools": true
          },
          "extraKnownMarketplaces": {
            "acme-tools": {
              "source": {
                "source": "settings",
                "name": "acme-tools",
                "plugins": [{
                  "name": "formatter",
                  "source": {
                    "source": "npm",
                    "package": "@acme/formatter"
                  }
                }]
              }
            }
          }
        }"#,
    )
    .expect("write settings");
    fs::write(
        external_agent_home.join(EXTERNAL_AGENT_KNOWN_MARKETPLACES_PATH),
        serde_json::to_string_pretty(&serde_json::json!({
            "acme-tools": {
                "source": {
                    "source": "settings",
                    "name": "acme-tools",
                    "plugins": [{
                        "name": "formatter",
                        "source": {
                            "source": "npm",
                            "package": "@acme/formatter",
                        },
                    }],
                },
                "installLocation": "plugins/marketplaces/acme-tools",
                "lastUpdated": "2026-07-09T00:16:23.611Z",
            }
        }))
        .expect("serialize known marketplaces"),
    )
    .expect("write known marketplaces");

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
                    marketplace_name: "acme-tools".to_string(),
                    plugin_names: vec!["formatter".to_string()],
                }],
                ..Default::default()
            }),
        }]
    );
}

#[test]
fn marketplace_import_sources_prefers_scoped_source_over_registry_name_collision() {
    let (root, external_agent_home, _codex_home) = fixture_paths();
    let source_root = root.path().join("repo");
    let scoped_marketplace = source_root.join("repo-marketplace");
    let cached_marketplace = external_agent_home.join("plugins/marketplaces/debug");
    fs::create_dir_all(&scoped_marketplace).expect("create scoped marketplace");
    fs::create_dir_all(&cached_marketplace).expect("create cached marketplace");
    fs::write(
        external_agent_home.join(EXTERNAL_AGENT_KNOWN_MARKETPLACES_PATH),
        serde_json::to_string_pretty(&serde_json::json!({
            "debug": {
                "source": {
                    "source": "github",
                    "repo": "acme/global-marketplace",
                },
                "installLocation": cached_marketplace,
            }
        }))
        .expect("serialize known marketplaces"),
    )
    .expect("write known marketplaces");
    let settings = serde_json::json!({
        "extraKnownMarketplaces": {
            "debug": {
                "source": {
                    "source": "directory",
                    "path": "./repo-marketplace",
                }
            }
        }
    });

    let import_sources =
        source_cla::marketplace_import_sources(&settings, &external_agent_home, &source_root);

    assert_eq!(
        import_sources.get("debug"),
        Some(&MarketplaceImportSource {
            source: source_root.join("./repo-marketplace").display().to_string(),
            ref_name: None,
        })
    );
}

#[test]
fn marketplace_import_sources_prefers_supported_declaration_over_materialization() {
    let (_root, external_agent_home, _codex_home) = fixture_paths();
    let cached_marketplace = external_agent_home.join("plugins/marketplaces/acme-tools");
    fs::create_dir_all(&cached_marketplace).expect("create cached marketplace");
    fs::write(
        external_agent_home.join(EXTERNAL_AGENT_KNOWN_MARKETPLACES_PATH),
        serde_json::to_string_pretty(&serde_json::json!({
            "acme-tools": {
                "source": {
                    "source": "git",
                    "url": "https://git.example.com/acme/tools.git",
                    "ref": "release",
                },
                "installLocation": cached_marketplace,
            }
        }))
        .expect("serialize known marketplaces"),
    )
    .expect("write known marketplaces");

    let import_sources = source_cla::marketplace_import_sources(
        &serde_json::json!({}),
        &external_agent_home,
        &external_agent_home,
    );

    assert_eq!(
        import_sources.get("acme-tools"),
        Some(&MarketplaceImportSource {
            source: "https://git.example.com/acme/tools.git".to_string(),
            ref_name: Some("release".to_string()),
        })
    );
}

#[tokio::test]
async fn detect_home_plugins_uses_local_settings_over_project_settings() {
    let (_root, external_agent_home, codex_home) = fixture_paths();
    fs::create_dir_all(&external_agent_home).expect("create external agent home");
    fs::write(
        external_agent_home.join("settings.json"),
        r#"{
          "enabledPlugins": {
            "formatter@acme-tools": true,
            "legacy@acme-tools": true
          },
          "extraKnownMarketplaces": {
            "acme-tools": {
              "source": "acme-corp/external-agent-plugins"
            }
          }
        }"#,
    )
    .expect("write project settings");
    fs::write(
        external_agent_home.join("settings.local.json"),
        r#"{
          "enabledPlugins": {
            "formatter@acme-tools": false,
            "deployer@acme-tools": true
          }
        }"#,
    )
    .expect("write local settings");

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
                    marketplace_name: "acme-tools".to_string(),
                    plugin_names: vec!["deployer".to_string(), "legacy".to_string()],
                }],
                ..Default::default()
            }),
        }]
    );
}

#[tokio::test]
async fn detect_repo_skips_plugins_that_are_already_configured_in_codex() {
    let root = TempDir::new().expect("create tempdir");
    let external_agent_home = root.path().join(EXTERNAL_AGENT_DIR);
    let codex_home = root.path().join(".codex");
    let repo_root = root.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).expect("create git dir");
    fs::create_dir_all(repo_root.join(EXTERNAL_AGENT_DIR)).expect("create repo external agent dir");
    fs::create_dir_all(&codex_home).expect("create codex home");
    fs::write(
        repo_root.join(EXTERNAL_AGENT_DIR).join("settings.json"),
        r#"{
          "enabledPlugins": {
            "formatter@acme-tools": true,
            "deployer@acme-tools": true
          },
          "extraKnownMarketplaces": {
            "acme-tools": {
              "source": "acme-corp/external-agent-plugins"
            }
          }
        }"#,
    )
    .expect("write repo settings");
    fs::write(
        codex_home.join("config.toml"),
        r#"
[plugins."formatter@acme-tools"]
enabled = true
"#,
    )
    .expect("write codex config");

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
                    marketplace_name: "acme-tools".to_string(),
                    plugin_names: vec!["deployer".to_string()],
                }],
                ..Default::default()
            }),
        }]
    );
}

#[tokio::test]
async fn detect_repo_skips_plugins_that_are_disabled_in_codex() {
    let root = TempDir::new().expect("create tempdir");
    let external_agent_home = root.path().join(EXTERNAL_AGENT_DIR);
    let codex_home = root.path().join(".codex");
    let repo_root = root.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).expect("create git dir");
    fs::create_dir_all(repo_root.join(EXTERNAL_AGENT_DIR)).expect("create repo external agent dir");
    fs::create_dir_all(&codex_home).expect("create codex home");
    fs::write(
        repo_root.join(EXTERNAL_AGENT_DIR).join("settings.json"),
        r#"{
          "enabledPlugins": {
            "formatter@acme-tools": true
          },
          "extraKnownMarketplaces": {
            "acme-tools": {
              "source": "acme-corp/external-agent-plugins"
            }
          }
        }"#,
    )
    .expect("write repo settings");
    fs::write(
        codex_home.join("config.toml"),
        r#"
[plugins."formatter@acme-tools"]
enabled = false
"#,
    )
    .expect("write codex config");

    let items = service_for_paths(external_agent_home, codex_home)
        .detect(ExternalAgentConfigDetectOptions {
            include_home: false,
            include_memory: false,
            cwds: Some(vec![repo_root]),
        })
        .await
        .expect("detect");

    assert_eq!(items, Vec::<ExternalAgentConfigMigrationItem>::new());
}

#[tokio::test]
async fn detect_repo_skips_plugins_without_explicit_enabled_in_codex() {
    let root = TempDir::new().expect("create tempdir");
    let external_agent_home = root.path().join(EXTERNAL_AGENT_DIR);
    let codex_home = root.path().join(".codex");
    let repo_root = root.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).expect("create git dir");
    fs::create_dir_all(repo_root.join(EXTERNAL_AGENT_DIR)).expect("create repo external agent dir");
    fs::create_dir_all(&codex_home).expect("create codex home");
    fs::write(
        repo_root.join(EXTERNAL_AGENT_DIR).join("settings.json"),
        r#"{
          "enabledPlugins": {
            "formatter@acme-tools": true
          },
          "extraKnownMarketplaces": {
            "acme-tools": {
              "source": "acme-corp/external-agent-plugins"
            }
          }
        }"#,
    )
    .expect("write repo settings");
    fs::write(
        codex_home.join("config.toml"),
        r#"
[plugins."formatter@acme-tools"]
"#,
    )
    .expect("write codex config");

    let items = service_for_paths(external_agent_home, codex_home)
        .detect(ExternalAgentConfigDetectOptions {
            include_home: false,
            include_memory: false,
            cwds: Some(vec![repo_root]),
        })
        .await
        .expect("detect");

    assert_eq!(items, Vec::<ExternalAgentConfigMigrationItem>::new());
}

#[tokio::test]
async fn import_plugins_requires_details() {
    let (_root, external_agent_home, codex_home) = fixture_paths();

    let err = service_for_paths(external_agent_home, codex_home)
        .import_plugins(/*cwd*/ None, /*details*/ None)
        .await
        .expect_err("expected missing details error");

    assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    assert_eq!(err.to_string(), "plugins migration item is missing details");
}

#[tokio::test]
async fn detect_repo_does_not_skip_plugins_only_configured_in_project_codex() {
    let root = TempDir::new().expect("create tempdir");
    let external_agent_home = root.path().join(EXTERNAL_AGENT_DIR);
    let codex_home = root.path().join(".codex");
    let repo_root = root.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).expect("create git dir");
    fs::create_dir_all(repo_root.join(EXTERNAL_AGENT_DIR)).expect("create repo external agent dir");
    fs::create_dir_all(repo_root.join(".codex")).expect("create repo codex dir");
    fs::create_dir_all(&codex_home).expect("create codex home");
    fs::write(
        repo_root.join(EXTERNAL_AGENT_DIR).join("settings.json"),
        r#"{
          "enabledPlugins": {
            "formatter@acme-tools": true
          },
          "extraKnownMarketplaces": {
            "acme-tools": {
              "source": "acme-corp/external-agent-plugins"
            }
          }
        }"#,
    )
    .expect("write repo settings");
    fs::write(
        repo_root.join(".codex").join("config.toml"),
        r#"
[plugins."formatter@acme-tools"]
enabled = true
"#,
    )
    .expect("write project codex config");

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
                    marketplace_name: "acme-tools".to_string(),
                    plugin_names: vec!["formatter".to_string()],
                }],
                ..Default::default()
            }),
        }]
    );
}

#[tokio::test]
async fn detect_home_skips_plugins_without_marketplace_source() {
    let (_root, external_agent_home, codex_home) = fixture_paths();
    fs::create_dir_all(&external_agent_home).expect("create external agent home");
    fs::write(
        external_agent_home.join("settings.json"),
        r#"{
          "enabledPlugins": {
            "formatter@acme-tools": true
          }
        }"#,
    )
    .expect("write settings");

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
async fn detect_home_skips_plugins_with_invalid_marketplace_source() {
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
              "source": "github"
            }
          }
        }"#,
    )
    .expect("write settings");

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
async fn detect_repo_filters_plugins_against_installed_marketplace() {
    let root = TempDir::new().expect("create tempdir");
    let external_agent_home = root.path().join(EXTERNAL_AGENT_DIR);
    let codex_home = root.path().join(".codex");
    let repo_root = root.path().join("repo");
    let marketplace_root = codex_home.join(".tmp").join("marketplaces").join("debug");
    fs::create_dir_all(repo_root.join(".git")).expect("create git dir");
    fs::create_dir_all(repo_root.join(EXTERNAL_AGENT_DIR)).expect("create repo external agent dir");
    fs::create_dir_all(marketplace_root.join(".agents").join("plugins"))
        .expect("create marketplace manifest dir");
    fs::create_dir_all(
        marketplace_root
            .join("plugins")
            .join("sample")
            .join(".codex-plugin"),
    )
    .expect("create sample plugin");
    fs::create_dir_all(
        marketplace_root
            .join("plugins")
            .join("available")
            .join(".codex-plugin"),
    )
    .expect("create available plugin");
    fs::write(
        repo_root.join(EXTERNAL_AGENT_DIR).join("settings.json"),
        r#"{
          "enabledPlugins": {
            "sample@debug": true,
            "available@debug": true,
            "missing@debug": true
          },
          "extraKnownMarketplaces": {
            "debug": {
              "source": "owner/debug-marketplace"
            }
          }
        }"#,
    )
    .expect("write repo settings");
    fs::write(
        codex_home.join("config.toml"),
        r#"
[marketplaces.debug]
source_type = "git"
source = "owner/debug-marketplace"
"#,
    )
    .expect("write codex config");
    fs::write(
        marketplace_root
            .join(".agents")
            .join("plugins")
            .join("marketplace.json"),
        r#"{
  "name": "debug",
  "plugins": [
    {
      "name": "sample",
      "source": {
        "source": "local",
        "path": "./plugins/sample"
      },
      "policy": {
        "installation": "NOT_AVAILABLE"
      }
    },
    {
      "name": "available",
      "source": {
        "source": "local",
        "path": "./plugins/available"
      }
    }
  ]
}"#,
    )
    .expect("write marketplace manifest");
    fs::write(
        marketplace_root
            .join("plugins")
            .join("sample")
            .join(".codex-plugin")
            .join("plugin.json"),
        r#"{"name":"sample"}"#,
    )
    .expect("write sample plugin manifest");
    fs::write(
        marketplace_root
            .join("plugins")
            .join("available")
            .join(".codex-plugin")
            .join("plugin.json"),
        r#"{"name":"available"}"#,
    )
    .expect("write available plugin manifest");

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
                    marketplace_name: "debug".to_string(),
                    plugin_names: vec!["available".to_string()],
                }],
                ..Default::default()
            }),
        }]
    );
}
