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

fn github_plugin_details() -> PluginsMigrationDetails {
    PluginsMigrationDetails {
        marketplaces: vec![PluginMarketplaceMigration {
            name: "acme-tools".to_string(),
            source: PluginMarketplaceSource::Github,
            repo: "acme-corp/claude-plugins".to_string(),
            ref_name: Some("main".to_string()),
        }],
        plugin_ids: vec!["formatter@acme-tools".to_string()],
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
                "Copy skill folders from {} to {}",
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
fn detect_home_lists_enabled_github_plugins_from_extra_known_marketplaces() {
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
              "source": "github",
              "repo": "acme-corp/claude-plugins",
              "ref": "main"
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
            details: Some(PluginsMigrationDetails {
                marketplaces: vec![PluginMarketplaceMigration {
                    name: "acme-tools".to_string(),
                    source: PluginMarketplaceSource::Github,
                    repo: "acme-corp/claude-plugins".to_string(),
                    ref_name: Some("main".to_string()),
                }],
                plugin_ids: vec![
                    "deployer@acme-tools".to_string(),
                    "formatter@acme-tools".to_string(),
                ],
            }),
        }]
    );
}

#[tokio::test]
async fn import_plugins_requires_details() {
    let (_root, claude_home, codex_home) = fixture_paths();

    let err = service_for_paths(claude_home, codex_home)
        .import(vec![ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::Plugins,
            description: String::new(),
            cwd: None,
            details: None,
        }])
        .await
        .expect_err("expected missing details error");

    assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    assert_eq!(err.to_string(), "plugins migration item is missing details");
}

#[tokio::test]
async fn import_plugins_requires_matching_marketplace_details() {
    let (_root, claude_home, codex_home) = fixture_paths();

    let err = service_for_paths(claude_home, codex_home)
        .import(vec![ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::Plugins,
            description: String::new(),
            cwd: None,
            details: Some(PluginsMigrationDetails {
                marketplaces: Vec::new(),
                plugin_ids: github_plugin_details().plugin_ids,
            }),
        }])
        .await
        .expect_err("expected missing marketplace details error");

    assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    assert_eq!(
        err.to_string(),
        "missing marketplace details for plugin `formatter@acme-tools`"
    );
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
