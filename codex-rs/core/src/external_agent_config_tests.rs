use super::*;
use pretty_assertions::assert_eq;
use std::io;
use tempfile::TempDir;

fn fixture_paths() -> (TempDir, PathBuf, PathBuf) {
    let root = TempDir::new().expect("create tempdir");
    let claude_home = root.path().join(".claude");
    let codex_home = root.path().join(".codex");
    (root, claude_home, codex_home)
}

fn service_for_paths(claude_home: PathBuf, codex_home: PathBuf) -> ExternalAgentConfigService {
    ExternalAgentConfigService::new_for_test(codex_home, claude_home)
}

fn github_plugin_details() -> MigrationDetails {
    MigrationDetails {
        plugins: vec![PluginsMigration {
            marketplace_name: "acme-tools".to_string(),
            plugin_names: vec!["formatter".to_string()],
        }],
    }
}

#[test]
fn detect_home_lists_config_skills_and_agents_md() {
    let (_root, claude_home, codex_home) = fixture_paths();
    let agents_skills = codex_home
        .parent()
        .map(|parent| parent.join(".agents").join("skills"))
        .unwrap_or_else(|| PathBuf::from(".agents").join("skills"));
    fs::create_dir_all(claude_home.join("skills").join("skill-a")).expect("create skills");
    fs::write(claude_home.join("CLAUDE.md"), "claude rules").expect("write claude md");
    fs::write(
        claude_home.join("settings.json"),
        r#"{"model":"claude","env":{"FOO":"bar"}}"#,
    )
    .expect("write settings");

    let items = service_for_paths(claude_home.clone(), codex_home.clone())
        .detect(ExternalAgentConfigDetectOptions {
            include_home: true,
            cwds: None,
        })
        .expect("detect");

    let expected = vec![
        ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::Config,
            description: format!(
                "Migrate {} into {}",
                claude_home.join("settings.json").display(),
                codex_home.join("config.toml").display()
            ),
            cwd: None,
            details: None,
        },
        ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::Skills,
            description: format!(
                "Migrate skills from {} to {}",
                claude_home.join("skills").display(),
                agents_skills.display()
            ),
            cwd: None,
            details: None,
        },
        ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::AgentsMd,
            description: format!(
                "Import {} to {}",
                claude_home.join("CLAUDE.md").display(),
                codex_home.join("AGENTS.md").display()
            ),
            cwd: None,
            details: None,
        },
    ];

    assert_eq!(items, expected);
}

#[test]
fn detect_repo_lists_agents_md_for_each_cwd() {
    let root = TempDir::new().expect("create tempdir");
    let repo_root = root.path().join("repo");
    let nested = repo_root.join("nested").join("child");
    fs::create_dir_all(repo_root.join(".git")).expect("create git dir");
    fs::create_dir_all(&nested).expect("create nested");
    fs::write(repo_root.join("CLAUDE.md"), "Claude code guidance").expect("write source");

    let items = service_for_paths(root.path().join(".claude"), root.path().join(".codex"))
        .detect(ExternalAgentConfigDetectOptions {
            include_home: false,
            cwds: Some(vec![nested, repo_root.clone()]),
        })
        .expect("detect");

    let expected = vec![
        ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::AgentsMd,
            description: format!(
                "Import {} to {}",
                repo_root.join("CLAUDE.md").display(),
                repo_root.join("AGENTS.md").display(),
            ),
            cwd: Some(repo_root.clone()),
            details: None,
        },
        ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::AgentsMd,
            description: format!(
                "Import {} to {}",
                repo_root.join("CLAUDE.md").display(),
                repo_root.join("AGENTS.md").display(),
            ),
            cwd: Some(repo_root),
            details: None,
        },
    ];

    assert_eq!(items, expected);
}

#[tokio::test]
async fn import_home_migrates_supported_config_fields_skills_and_agents_md() {
    let (_root, claude_home, codex_home) = fixture_paths();
    let agents_skills = codex_home
        .parent()
        .map(|parent| parent.join(".agents").join("skills"))
        .unwrap_or_else(|| PathBuf::from(".agents").join("skills"));
    fs::create_dir_all(claude_home.join("skills").join("skill-a")).expect("create skills");
    fs::write(
            claude_home.join("settings.json"),
            r#"{"model":"claude","permissions":{"ask":["git push"]},"env":{"FOO":"bar","CI":false,"MAX_RETRIES":3,"MY_TEAM":"codex","IGNORED":null,"LIST":["a","b"],"MAP":{"x":1}},"sandbox":{"enabled":true,"network":{"allowLocalBinding":true}}}"#,
        )
        .expect("write settings");
    fs::write(
        claude_home.join("skills").join("skill-a").join("SKILL.md"),
        "Use Claude Code and CLAUDE utilities.",
    )
    .expect("write skill");
    fs::write(claude_home.join("CLAUDE.md"), "Claude code guidance").expect("write agents");

    service_for_paths(claude_home, codex_home.clone())
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
        .await
        .expect("import");

    assert_eq!(
        fs::read_to_string(codex_home.join("AGENTS.md")).expect("read agents"),
        "Codex guidance"
    );

    assert_eq!(
        fs::read_to_string(codex_home.join("config.toml")).expect("read config"),
        "sandbox_mode = \"workspace-write\"\n\n[shell_environment_policy]\ninherit = \"core\"\n\n[shell_environment_policy.set]\nCI = \"false\"\nFOO = \"bar\"\nMAX_RETRIES = \"3\"\nMY_TEAM = \"codex\"\n"
    );
    assert_eq!(
        fs::read_to_string(agents_skills.join("skill-a").join("SKILL.md"))
            .expect("read copied skill"),
        "Use Codex and Codex utilities."
    );
}

#[tokio::test]
async fn import_home_skips_empty_config_migration() {
    let (_root, claude_home, codex_home) = fixture_paths();
    fs::create_dir_all(&claude_home).expect("create claude home");
    fs::write(
        claude_home.join("settings.json"),
        r#"{"model":"claude","sandbox":{"enabled":false}}"#,
    )
    .expect("write settings");

    service_for_paths(claude_home, codex_home.clone())
        .import(vec![ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::Config,
            description: String::new(),
            cwd: None,
            details: None,
        }])
        .await
        .expect("import");

    assert!(!codex_home.join("config.toml").exists());
}

#[test]
fn detect_home_skips_config_when_target_already_has_supported_fields() {
    let (_root, claude_home, codex_home) = fixture_paths();
    fs::create_dir_all(&claude_home).expect("create claude home");
    fs::create_dir_all(&codex_home).expect("create codex home");
    fs::write(
        claude_home.join("settings.json"),
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

    let items = service_for_paths(claude_home, codex_home)
        .detect(ExternalAgentConfigDetectOptions {
            include_home: true,
            cwds: None,
        })
        .expect("detect");

    assert_eq!(items, Vec::<ExternalAgentConfigMigrationItem>::new());
}

#[test]
fn detect_home_skips_skills_when_all_skill_directories_exist() {
    let (_root, claude_home, codex_home) = fixture_paths();
    let agents_skills = codex_home
        .parent()
        .map(|parent| parent.join(".agents").join("skills"))
        .unwrap_or_else(|| PathBuf::from(".agents").join("skills"));
    fs::create_dir_all(claude_home.join("skills").join("skill-a")).expect("create source");
    fs::create_dir_all(agents_skills.join("skill-a")).expect("create target");

    let items = service_for_paths(claude_home, codex_home)
        .detect(ExternalAgentConfigDetectOptions {
            include_home: true,
            cwds: None,
        })
        .expect("detect");

    assert_eq!(items, Vec::<ExternalAgentConfigMigrationItem>::new());
}

#[tokio::test]
async fn import_repo_agents_md_rewrites_terms_and_skips_non_empty_targets() {
    let root = TempDir::new().expect("create tempdir");
    let repo_root = root.path().join("repo-a");
    let repo_with_existing_target = root.path().join("repo-b");
    fs::create_dir_all(repo_root.join(".git")).expect("create git");
    fs::create_dir_all(repo_with_existing_target.join(".git")).expect("create git");
    fs::write(
        repo_root.join("CLAUDE.md"),
        "Claude code\nclaude\nCLAUDE-CODE\nSee CLAUDE.md\n",
    )
    .expect("write source");
    fs::write(repo_with_existing_target.join("CLAUDE.md"), "new source").expect("write source");
    fs::write(
        repo_with_existing_target.join("AGENTS.md"),
        "keep existing target",
    )
    .expect("write target");

    service_for_paths(root.path().join(".claude"), root.path().join(".codex"))
        .import(vec![
            ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::AgentsMd,
                description: String::new(),
                cwd: Some(repo_root.clone()),
                details: None,
            },
            ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::AgentsMd,
                description: String::new(),
                cwd: Some(repo_with_existing_target.clone()),
                details: None,
            },
        ])
        .await
        .expect("import");

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
    fs::write(repo_root.join("CLAUDE.md"), "Claude code guidance").expect("write source");
    fs::write(repo_root.join("AGENTS.md"), " \n\t").expect("write empty target");

    service_for_paths(root.path().join(".claude"), root.path().join(".codex"))
        .import(vec![ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::AgentsMd,
            description: String::new(),
            cwd: Some(repo_root.clone()),
            details: None,
        }])
        .await
        .expect("import");

    assert_eq!(
        fs::read_to_string(repo_root.join("AGENTS.md")).expect("read target"),
        "Codex guidance"
    );
}

#[test]
fn detect_repo_prefers_non_empty_dot_claude_agents_source() {
    let root = TempDir::new().expect("create tempdir");
    let repo_root = root.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).expect("create git");
    fs::create_dir_all(repo_root.join(".claude")).expect("create dot claude");
    fs::write(repo_root.join("CLAUDE.md"), " \n\t").expect("write empty root source");
    fs::write(
        repo_root.join(".claude").join("CLAUDE.md"),
        "Claude code guidance",
    )
    .expect("write dot claude source");

    let items = service_for_paths(root.path().join(".claude"), root.path().join(".codex"))
        .detect(ExternalAgentConfigDetectOptions {
            include_home: false,
            cwds: Some(vec![repo_root.clone()]),
        })
        .expect("detect");

    assert_eq!(
        items,
        vec![ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::AgentsMd,
            description: format!(
                "Import {} to {}",
                repo_root.join(".claude").join("CLAUDE.md").display(),
                repo_root.join("AGENTS.md").display(),
            ),
            cwd: Some(repo_root),
            details: None,
        }]
    );
}

#[tokio::test]
async fn import_repo_uses_non_empty_dot_claude_agents_source() {
    let root = TempDir::new().expect("create tempdir");
    let repo_root = root.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).expect("create git");
    fs::create_dir_all(repo_root.join(".claude")).expect("create dot claude");
    fs::write(repo_root.join("CLAUDE.md"), "").expect("write empty root source");
    fs::write(
        repo_root.join(".claude").join("CLAUDE.md"),
        "Claude code guidance",
    )
    .expect("write dot claude source");

    service_for_paths(root.path().join(".claude"), root.path().join(".codex"))
        .import(vec![ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::AgentsMd,
            description: String::new(),
            cwd: Some(repo_root.clone()),
            details: None,
        }])
        .await
        .expect("import");

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

#[test]
fn detect_home_lists_enabled_plugins_from_settings() {
    let (_root, claude_home, codex_home) = fixture_paths();
    fs::create_dir_all(&claude_home).expect("create claude home");
    fs::write(
        claude_home.join("settings.json"),
        r#"{
          "enabledPlugins": {
            "formatter@acme-tools": true,
            "deployer@acme-tools": true,
            "analyzer@security-plugins": false
          },
          "extraKnownMarketplaces": {
            "acme-tools": {
              "source": "acme-corp/claude-plugins"
            }
          }
        }"#,
    )
    .expect("write settings");

    let items = service_for_paths(claude_home.clone(), codex_home)
        .detect(ExternalAgentConfigDetectOptions {
            include_home: true,
            cwds: None,
        })
        .expect("detect");

    assert_eq!(
        items,
        vec![ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::Plugins,
            description: format!(
                "Import enabled plugins from {}",
                claude_home.join("settings.json").display()
            ),
            cwd: None,
            details: Some(MigrationDetails {
                plugins: vec![PluginsMigration {
                    marketplace_name: "acme-tools".to_string(),
                    plugin_names: vec!["deployer".to_string(), "formatter".to_string()],
                }],
            }),
        }]
    );
}

#[test]
fn detect_repo_skips_plugins_that_are_already_configured_in_codex() {
    let root = TempDir::new().expect("create tempdir");
    let claude_home = root.path().join(".claude");
    let codex_home = root.path().join(".codex");
    let repo_root = root.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).expect("create git dir");
    fs::create_dir_all(repo_root.join(".claude")).expect("create repo claude dir");
    fs::create_dir_all(&codex_home).expect("create codex home");
    fs::write(
        repo_root.join(".claude").join("settings.json"),
        r#"{
          "enabledPlugins": {
            "formatter@acme-tools": true,
            "deployer@acme-tools": true
          },
          "extraKnownMarketplaces": {
            "acme-tools": {
              "source": "acme-corp/claude-plugins"
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

    let items = service_for_paths(claude_home, codex_home)
        .detect(ExternalAgentConfigDetectOptions {
            include_home: false,
            cwds: Some(vec![repo_root.clone()]),
        })
        .expect("detect");

    assert_eq!(
        items,
        vec![ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::Plugins,
            description: format!(
                "Import enabled plugins from {}",
                repo_root.join(".claude").join("settings.json").display()
            ),
            cwd: Some(repo_root),
            details: Some(MigrationDetails {
                plugins: vec![PluginsMigration {
                    marketplace_name: "acme-tools".to_string(),
                    plugin_names: vec!["deployer".to_string()],
                }],
            }),
        }]
    );
}

#[test]
fn detect_repo_skips_plugins_that_are_disabled_in_codex() {
    let root = TempDir::new().expect("create tempdir");
    let claude_home = root.path().join(".claude");
    let codex_home = root.path().join(".codex");
    let repo_root = root.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).expect("create git dir");
    fs::create_dir_all(repo_root.join(".claude")).expect("create repo claude dir");
    fs::create_dir_all(&codex_home).expect("create codex home");
    fs::write(
        repo_root.join(".claude").join("settings.json"),
        r#"{
          "enabledPlugins": {
            "formatter@acme-tools": true
          },
          "extraKnownMarketplaces": {
            "acme-tools": {
              "source": "acme-corp/claude-plugins"
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

    let items = service_for_paths(claude_home, codex_home)
        .detect(ExternalAgentConfigDetectOptions {
            include_home: false,
            cwds: Some(vec![repo_root]),
        })
        .expect("detect");

    assert_eq!(items, Vec::<ExternalAgentConfigMigrationItem>::new());
}

#[test]
fn detect_repo_skips_plugins_without_explicit_enabled_in_codex() {
    let root = TempDir::new().expect("create tempdir");
    let claude_home = root.path().join(".claude");
    let codex_home = root.path().join(".codex");
    let repo_root = root.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).expect("create git dir");
    fs::create_dir_all(repo_root.join(".claude")).expect("create repo claude dir");
    fs::create_dir_all(&codex_home).expect("create codex home");
    fs::write(
        repo_root.join(".claude").join("settings.json"),
        r#"{
          "enabledPlugins": {
            "formatter@acme-tools": true
          },
          "extraKnownMarketplaces": {
            "acme-tools": {
              "source": "acme-corp/claude-plugins"
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

    let items = service_for_paths(claude_home, codex_home)
        .detect(ExternalAgentConfigDetectOptions {
            include_home: false,
            cwds: Some(vec![repo_root]),
        })
        .expect("detect");

    assert_eq!(items, Vec::<ExternalAgentConfigMigrationItem>::new());
}

#[tokio::test]
async fn import_plugins_requires_details() {
    let (_root, claude_home, codex_home) = fixture_paths();

    let err = service_for_paths(claude_home, codex_home)
        .import_plugins(/*cwd*/ None, /*details*/ None)
        .await
        .expect_err("expected missing details error");

    assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    assert_eq!(err.to_string(), "plugins migration item is missing details");
}

#[test]
fn detect_repo_does_not_skip_plugins_only_configured_in_project_codex() {
    let root = TempDir::new().expect("create tempdir");
    let claude_home = root.path().join(".claude");
    let codex_home = root.path().join(".codex");
    let repo_root = root.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).expect("create git dir");
    fs::create_dir_all(repo_root.join(".claude")).expect("create repo claude dir");
    fs::create_dir_all(repo_root.join(".codex")).expect("create repo codex dir");
    fs::create_dir_all(&codex_home).expect("create codex home");
    fs::write(
        repo_root.join(".claude").join("settings.json"),
        r#"{
          "enabledPlugins": {
            "formatter@acme-tools": true
          },
          "extraKnownMarketplaces": {
            "acme-tools": {
              "source": "acme-corp/claude-plugins"
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

    let items = service_for_paths(claude_home, codex_home)
        .detect(ExternalAgentConfigDetectOptions {
            include_home: false,
            cwds: Some(vec![repo_root.clone()]),
        })
        .expect("detect");

    assert_eq!(
        items,
        vec![ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::Plugins,
            description: format!(
                "Import enabled plugins from {}",
                repo_root.join(".claude").join("settings.json").display()
            ),
            cwd: Some(repo_root),
            details: Some(MigrationDetails {
                plugins: vec![PluginsMigration {
                    marketplace_name: "acme-tools".to_string(),
                    plugin_names: vec!["formatter".to_string()],
                }],
            }),
        }]
    );
}

#[test]
fn detect_home_skips_plugins_without_marketplace_source() {
    let (_root, claude_home, codex_home) = fixture_paths();
    fs::create_dir_all(&claude_home).expect("create claude home");
    fs::write(
        claude_home.join("settings.json"),
        r#"{
          "enabledPlugins": {
            "formatter@acme-tools": true
          }
        }"#,
    )
    .expect("write settings");

    let items = service_for_paths(claude_home, codex_home)
        .detect(ExternalAgentConfigDetectOptions {
            include_home: true,
            cwds: None,
        })
        .expect("detect");

    assert_eq!(items, Vec::<ExternalAgentConfigMigrationItem>::new());
}

#[test]
fn detect_home_skips_plugins_with_invalid_marketplace_source() {
    let (_root, claude_home, codex_home) = fixture_paths();
    fs::create_dir_all(&claude_home).expect("create claude home");
    fs::write(
        claude_home.join("settings.json"),
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

    let items = service_for_paths(claude_home, codex_home)
        .detect(ExternalAgentConfigDetectOptions {
            include_home: true,
            cwds: None,
        })
        .expect("detect");

    assert_eq!(items, Vec::<ExternalAgentConfigMigrationItem>::new());
}

#[test]
fn detect_repo_filters_plugins_against_installed_marketplace() {
    let root = TempDir::new().expect("create tempdir");
    let claude_home = root.path().join(".claude");
    let codex_home = root.path().join(".codex");
    let repo_root = root.path().join("repo");
    let marketplace_root = codex_home.join(".tmp").join("marketplaces").join("debug");
    fs::create_dir_all(repo_root.join(".git")).expect("create git dir");
    fs::create_dir_all(repo_root.join(".claude")).expect("create repo claude dir");
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
        repo_root.join(".claude").join("settings.json"),
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

    let items = service_for_paths(claude_home, codex_home)
        .detect(ExternalAgentConfigDetectOptions {
            include_home: false,
            cwds: Some(vec![repo_root.clone()]),
        })
        .expect("detect");

    assert_eq!(
        items,
        vec![ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::Plugins,
            description: format!(
                "Import enabled plugins from {}",
                repo_root.join(".claude").join("settings.json").display()
            ),
            cwd: Some(repo_root),
            details: Some(MigrationDetails {
                plugins: vec![PluginsMigration {
                    marketplace_name: "debug".to_string(),
                    plugin_names: vec!["available".to_string()],
                }],
            }),
        }]
    );
}

#[tokio::test]
async fn import_plugins_requires_source_marketplace_details() {
    let (_root, claude_home, codex_home) = fixture_paths();
    fs::create_dir_all(&claude_home).expect("create claude home");
    fs::write(
        claude_home.join("settings.json"),
        r#"{
          "enabledPlugins": {
            "formatter@acme-tools": true
          },
          "extraKnownMarketplaces": {
            "acme-tools": {
              "source": "github",
              "repo": "acme-corp/claude-plugins"
            }
          }
        }"#,
    )
    .expect("write settings");

    let outcome = service_for_paths(claude_home, codex_home)
        .import_plugins(
            /*cwd*/ None,
            Some(MigrationDetails {
                plugins: vec![PluginsMigration {
                    marketplace_name: "other-tools".to_string(),
                    plugin_names: github_plugin_details().plugins[0].plugin_names.clone(),
                }],
            }),
        )
        .await
        .expect("import plugins");

    assert_eq!(
        outcome,
        PluginImportOutcome {
            succeeded_marketplaces: Vec::new(),
            succeeded_plugin_ids: Vec::new(),
            failed_marketplaces: vec!["other-tools".to_string()],
            failed_plugin_ids: vec!["formatter@other-tools".to_string()],
        }
    );
}

#[tokio::test]
async fn import_plugins_defers_marketplace_source_validation_to_add_marketplace() {
    let (_root, claude_home, codex_home) = fixture_paths();
    fs::create_dir_all(&claude_home).expect("create claude home");
    fs::write(
        claude_home.join("settings.json"),
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

    let outcome = service_for_paths(claude_home, codex_home)
        .import_plugins(/*cwd*/ None, Some(github_plugin_details()))
        .await
        .expect("import plugins");

    assert_eq!(
        outcome,
        PluginImportOutcome {
            succeeded_marketplaces: Vec::new(),
            succeeded_plugin_ids: Vec::new(),
            failed_marketplaces: vec!["acme-tools".to_string()],
            failed_plugin_ids: vec!["formatter@acme-tools".to_string()],
        }
    );
}

#[tokio::test]
async fn import_plugins_supports_claude_plugin_marketplace_layout() {
    let (_root, claude_home, codex_home) = fixture_paths();
    let marketplace_root = claude_home.join("my-marketplace");
    let plugin_root = marketplace_root.join("plugins").join("cloudflare");
    fs::create_dir_all(marketplace_root.join(".claude-plugin"))
        .expect("create marketplace manifest dir");
    fs::create_dir_all(plugin_root.join(".codex-plugin")).expect("create plugin manifest dir");
    fs::create_dir_all(&codex_home).expect("create codex home");

    fs::write(
        claude_home.join("settings.json"),
        format!(
            r#"{{
              "enabledPlugins": {{
                "cloudflare@my-plugins": true
              }},
              "extraKnownMarketplaces": {{
                "my-plugins": {{
                  "source": "local",
                  "path": "{}"
                }}
              }}
            }}"#,
            marketplace_root.display()
        ),
    )
    .expect("write settings");
    fs::write(
        marketplace_root
            .join(".claude-plugin")
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

    let outcome = service_for_paths(claude_home, codex_home.clone())
        .import_plugins(
            /*cwd*/ None,
            Some(MigrationDetails {
                plugins: vec![PluginsMigration {
                    marketplace_name: "my-plugins".to_string(),
                    plugin_names: vec!["cloudflare".to_string()],
                }],
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
        }
    );
    let config = fs::read_to_string(codex_home.join("config.toml")).expect("read config");
    assert!(config.contains(r#"[plugins."cloudflare@my-plugins"]"#));
    assert!(config.contains("enabled = true"));
}

#[test]
fn import_skills_returns_only_new_skill_directory_count() {
    let (_root, claude_home, codex_home) = fixture_paths();
    let agents_skills = codex_home
        .parent()
        .map(|parent| parent.join(".agents").join("skills"))
        .unwrap_or_else(|| PathBuf::from(".agents").join("skills"));
    fs::create_dir_all(claude_home.join("skills").join("skill-a")).expect("create source a");
    fs::create_dir_all(claude_home.join("skills").join("skill-b")).expect("create source b");
    fs::create_dir_all(agents_skills.join("skill-a")).expect("create existing target");

    let copied_count = service_for_paths(claude_home, codex_home)
        .import_skills(/*cwd*/ None)
        .expect("import skills");

    assert_eq!(copied_count, 1);
}
