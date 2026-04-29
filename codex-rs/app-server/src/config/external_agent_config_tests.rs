use super::*;
use pretty_assertions::assert_eq;
use std::io;
use tempfile::TempDir;

const EXTERNAL_AGENT_PROJECT_CONFIG_FILE: &str = ".claude.json";
const EXTERNAL_AGENT_PLUGIN_MANIFEST_DIR: &str = ".claude-plugin";
const SOURCE_EXTERNAL_AGENT_NAME: &str = "claude";
const SOURCE_EXTERNAL_AGENT_DISPLAY_NAME: &str = "Claude";
const SOURCE_EXTERNAL_AGENT_PRODUCT_NAME: &str = "Claude Code";
const SOURCE_EXTERNAL_AGENT_UPPER_NAME: &str = "CLAUDE";
const SOURCE_EXTERNAL_AGENT_UPPER_PRODUCT_NAME: &str = "CLAUDE-CODE";

fn fixture_paths() -> (TempDir, PathBuf, PathBuf) {
    let root = TempDir::new().expect("create tempdir");
    let external_agent_home = root.path().join(EXTERNAL_AGENT_DIR);
    let codex_home = root.path().join(".codex");
    (root, external_agent_home, codex_home)
}

fn service_for_paths(
    external_agent_home: PathBuf,
    codex_home: PathBuf,
) -> ExternalAgentConfigService {
    ExternalAgentConfigService::new_for_test(codex_home, external_agent_home)
}

fn github_plugin_details() -> MigrationDetails {
    MigrationDetails {
        plugins: vec![PluginsMigration {
            marketplace_name: "acme-tools".to_string(),
            plugin_names: vec!["formatter".to_string()],
        }],
        ..Default::default()
    }
}

#[tokio::test]
async fn detect_home_lists_config_skills_and_agents_md() {
    let (_root, external_agent_home, codex_home) = fixture_paths();
    let agents_skills = codex_home
        .parent()
        .map(|parent| parent.join(".agents").join("skills"))
        .unwrap_or_else(|| PathBuf::from(".agents").join("skills"));
    fs::create_dir_all(external_agent_home.join("skills").join("skill-a")).expect("create skills");
    fs::write(
        external_agent_home.join(EXTERNAL_AGENT_CONFIG_MD),
        format!("{SOURCE_EXTERNAL_AGENT_NAME} rules"),
    )
    .expect("write external agent md");
    fs::write(
        external_agent_home.join("settings.json"),
        format!(r#"{{"model":"{SOURCE_EXTERNAL_AGENT_NAME}","env":{{"FOO":"bar"}}}}"#),
    )
    .expect("write settings");

    let items = service_for_paths(external_agent_home.clone(), codex_home.clone())
        .detect(ExternalAgentConfigDetectOptions {
            include_home: true,
            cwds: None,
        })
        .await
        .expect("detect");

    let expected = vec![
        ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::Config,
            description: format!(
                "Migrate {} into {}",
                external_agent_home.join("settings.json").display(),
                codex_home.join("config.toml").display()
            ),
            cwd: None,
            details: None,
        },
        ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::Skills,
            description: format!(
                "Migrate skills from {} to {}",
                external_agent_home.join("skills").display(),
                agents_skills.display()
            ),
            cwd: None,
            details: None,
        },
        ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::AgentsMd,
            description: format!(
                "Migrate {} to {}",
                external_agent_home.join(EXTERNAL_AGENT_CONFIG_MD).display(),
                codex_home.join("AGENTS.md").display()
            ),
            cwd: None,
            details: None,
        },
    ];

    assert_eq!(items, expected);
}

#[tokio::test]
async fn detect_home_lists_recent_sessions() {
    let (root, external_agent_home, codex_home) = fixture_paths();
    let project_root = root.path().join("repo");
    let recent_timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let session_path = external_agent_home
        .join("projects")
        .join("repo")
        .join("session.jsonl");
    fs::create_dir_all(&project_root).expect("create project root");
    fs::create_dir_all(session_path.parent().expect("session parent")).expect("create sessions");
    fs::write(
        &session_path,
        serde_json::json!({
            "type": "user",
            "cwd": &project_root,
            "timestamp": &recent_timestamp,
            "message": { "content": "first request" },
        })
        .to_string(),
    )
    .expect("write session");

    let items = service_for_paths(external_agent_home.clone(), codex_home)
        .detect(ExternalAgentConfigDetectOptions {
            include_home: true,
            cwds: None,
        })
        .await
        .expect("detect");

    assert_eq!(
        items,
        vec![ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::Sessions,
            description: format!(
                "Migrate recent sessions from {}",
                external_agent_home.join("projects").display()
            ),
            cwd: None,
            details: Some(MigrationDetails {
                plugins: Vec::new(),
                sessions: vec![ExternalAgentSessionMigration {
                    path: session_path,
                    cwd: project_root,
                    title: Some("first request".to_string()),
                }],
                ..Default::default()
            }),
        }]
    );
}

#[tokio::test]
async fn detect_repo_lists_agents_md_for_each_cwd() {
    let root = TempDir::new().expect("create tempdir");
    let repo_root = root.path().join("repo");
    let nested = repo_root.join("nested").join("child");
    fs::create_dir_all(repo_root.join(".git")).expect("create git dir");
    fs::create_dir_all(&nested).expect("create nested");
    fs::write(
        repo_root.join(EXTERNAL_AGENT_CONFIG_MD),
        format!("{SOURCE_EXTERNAL_AGENT_DISPLAY_NAME} code guidance"),
    )
    .expect("write source");

    let items = service_for_paths(
        root.path().join(EXTERNAL_AGENT_DIR),
        root.path().join(".codex"),
    )
    .detect(ExternalAgentConfigDetectOptions {
        include_home: false,
        cwds: Some(vec![nested, repo_root.clone()]),
    })
    .await
    .expect("detect");

    let expected = vec![
        ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::AgentsMd,
            description: format!(
                "Migrate {} to {}",
                repo_root.join(EXTERNAL_AGENT_CONFIG_MD).display(),
                repo_root.join("AGENTS.md").display(),
            ),
            cwd: Some(repo_root.clone()),
            details: None,
        },
        ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::AgentsMd,
            description: format!(
                "Migrate {} to {}",
                repo_root.join(EXTERNAL_AGENT_CONFIG_MD).display(),
                repo_root.join("AGENTS.md").display(),
            ),
            cwd: Some(repo_root),
            details: None,
        },
    ];

    assert_eq!(items, expected);
}

#[tokio::test]
async fn detect_repo_still_reports_non_plugin_items_when_home_config_is_invalid() {
    let root = TempDir::new().expect("create tempdir");
    let repo_root = root.path().join("repo");
    let codex_home = root.path().join(".codex");
    fs::create_dir_all(repo_root.join(".git")).expect("create git dir");
    fs::create_dir_all(
        repo_root
            .join(EXTERNAL_AGENT_DIR)
            .join("skills")
            .join("skill-a"),
    )
    .expect("create repo skills");
    fs::create_dir_all(&codex_home).expect("create codex home");
    fs::write(codex_home.join("config.toml"), "this is not valid = [toml")
        .expect("write invalid codex config");
    fs::write(
        repo_root.join(EXTERNAL_AGENT_DIR).join("settings.json"),
        r#"{"env":{"FOO":"bar"}}"#,
    )
    .expect("write settings");
    fs::write(
        repo_root
            .join(EXTERNAL_AGENT_DIR)
            .join("skills")
            .join("skill-a")
            .join("SKILL.md"),
        format!(
            "Use {SOURCE_EXTERNAL_AGENT_PRODUCT_NAME} and {SOURCE_EXTERNAL_AGENT_UPPER_NAME} utilities."
        ),
    )
    .expect("write skill");
    fs::write(
        repo_root
            .join(EXTERNAL_AGENT_DIR)
            .join(EXTERNAL_AGENT_CONFIG_MD),
        format!("{SOURCE_EXTERNAL_AGENT_DISPLAY_NAME} code guidance"),
    )
    .expect("write agents");

    let items = service_for_paths(root.path().join(EXTERNAL_AGENT_DIR), codex_home)
        .detect(ExternalAgentConfigDetectOptions {
            include_home: false,
            cwds: Some(vec![repo_root.clone()]),
        })
        .await
        .expect("detect");

    assert_eq!(
        items,
        vec![
            ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::Config,
                description: format!(
                    "Migrate {} into {}",
                    repo_root
                        .join(EXTERNAL_AGENT_DIR)
                        .join("settings.json")
                        .display(),
                    repo_root.join(".codex").join("config.toml").display()
                ),
                cwd: Some(repo_root.clone()),
                details: None,
            },
            ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::Skills,
                description: format!(
                    "Migrate skills from {} to {}",
                    repo_root.join(EXTERNAL_AGENT_DIR).join("skills").display(),
                    repo_root.join(".agents").join("skills").display()
                ),
                cwd: Some(repo_root.clone()),
                details: None,
            },
            ExternalAgentConfigMigrationItem {
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
            },
        ]
    );
}

#[tokio::test]
async fn detect_repo_lists_mcp_hooks_commands_and_subagents() {
    let root = TempDir::new().expect("create tempdir");
    let repo_root = root.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).expect("create git dir");
    fs::create_dir_all(
        repo_root
            .join(EXTERNAL_AGENT_DIR)
            .join("commands")
            .join("pr"),
    )
    .expect("create commands");
    fs::create_dir_all(repo_root.join(EXTERNAL_AGENT_DIR).join("agents")).expect("create agents");
    fs::write(
        repo_root.join(".mcp.json"),
        r#"{"mcpServers":{"docs":{"command":"docs-server"}}}"#,
    )
    .expect("write mcp");
    fs::write(
        repo_root.join(EXTERNAL_AGENT_DIR).join("settings.json"),
        r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"echo external-agent","timeout":3},{"type":"http","url":"https://example.invalid/hook"}]}]}}"#,
    )
    .expect("write hooks");
    fs::write(
        repo_root
            .join(EXTERNAL_AGENT_DIR)
            .join("commands")
            .join("pr")
            .join("review.md"),
        "---\ndescription: Review PR\n---\nReview the pull request carefully.\n",
    )
    .expect("write command");
    fs::write(
        repo_root
            .join(EXTERNAL_AGENT_DIR)
            .join("agents")
            .join("researcher.md"),
        "---\nname: researcher\ndescription: Research role\n---\nResearch carefully.\n",
    )
    .expect("write subagent");

    let items = service_for_paths(
        root.path().join(EXTERNAL_AGENT_DIR),
        root.path().join(".codex"),
    )
    .detect(ExternalAgentConfigDetectOptions {
        include_home: false,
        cwds: Some(vec![repo_root.clone()]),
    })
    .await
    .expect("detect");

    assert_eq!(
        items,
        vec![
            ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::McpServerConfig,
                description: format!(
                    "Migrate MCP servers from {} into {}",
                    repo_root.display(),
                    repo_root.join(".codex").join("config.toml").display()
                ),
                cwd: Some(repo_root.clone()),
                details: Some(MigrationDetails {
                    mcp_servers: vec![NamedMigration {
                        name: "docs".to_string(),
                    }],
                    ..Default::default()
                }),
            },
            ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::Hooks,
                description: format!(
                    "Migrate hooks from {} to {}",
                    repo_root.join(EXTERNAL_AGENT_DIR).display(),
                    repo_root.join(".codex").join("hooks.json").display()
                ),
                cwd: Some(repo_root.clone()),
                details: Some(MigrationDetails {
                    hooks: vec![NamedMigration {
                        name: "PreToolUse".to_string(),
                    }],
                    ..Default::default()
                }),
            },
            ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::Commands,
                description: format!(
                    "Migrate commands from {} to {}",
                    repo_root
                        .join(EXTERNAL_AGENT_DIR)
                        .join("commands")
                        .display(),
                    repo_root.join(".agents").join("skills").display()
                ),
                cwd: Some(repo_root.clone()),
                details: Some(MigrationDetails {
                    commands: vec![NamedMigration {
                        name: "source-command-pr-review".to_string(),
                    }],
                    ..Default::default()
                }),
            },
            ExternalAgentConfigMigrationItem {
                item_type: ExternalAgentConfigMigrationItemType::Subagents,
                description: format!(
                    "Migrate subagents from {} to {}",
                    repo_root.join(EXTERNAL_AGENT_DIR).join("agents").display(),
                    repo_root.join(".codex").join("agents").display()
                ),
                cwd: Some(repo_root),
                details: Some(MigrationDetails {
                    subagents: vec![NamedMigration {
                        name: "researcher".to_string(),
                    }],
                    ..Default::default()
                }),
            },
        ]
    );
}

#[tokio::test]
async fn detect_repo_skips_hooks_when_only_unsupported_hooks_exist() {
    let root = TempDir::new().expect("create tempdir");
    let repo_root = root.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).expect("create git dir");
    fs::create_dir_all(repo_root.join(EXTERNAL_AGENT_DIR)).expect("create external agent dir");
    fs::write(
        repo_root.join(EXTERNAL_AGENT_DIR).join("settings.json"),
        r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","if":"Bash(rm *)","command":"echo blocked"}]}],"SubagentStart":[{"matcher":"worker","hooks":[{"type":"command","command":"echo started"}]}]}}"#,
    )
    .expect("write hooks");

    let items = service_for_paths(
        root.path().join(EXTERNAL_AGENT_DIR),
        root.path().join(".codex"),
    )
    .detect(ExternalAgentConfigDetectOptions {
        include_home: false,
        cwds: Some(vec![repo_root]),
    })
    .await
    .expect("detect");

    assert_eq!(items, Vec::<ExternalAgentConfigMigrationItem>::new());
}

#[tokio::test]
async fn import_repo_migrates_mcp_hooks_commands_and_subagents() {
    let root = TempDir::new().expect("create tempdir");
    let repo_root = root.path().join("repo");
    fs::create_dir_all(repo_root.join(".git")).expect("create git dir");
    fs::create_dir_all(
        repo_root
            .join(EXTERNAL_AGENT_DIR)
            .join("commands")
            .join("pr"),
    )
    .expect("create commands");
    fs::create_dir_all(repo_root.join(EXTERNAL_AGENT_DIR).join("agents")).expect("create agents");
    fs::write(
        repo_root.join(".mcp.json"),
        r#"{
          "mcpServers": {
            "docs": {
              "command": "docs-server",
              "args": ["--stdio"],
              "headers": {"X-Ignored": "unsupported for stdio"},
              "env": {"DOCS_TOKEN": "${DOCS_TOKEN}", "STATIC": "yes"}
            },
            "api": {
              "url": "https://example.com/mcp",
              "args": ["ignored-for-http"],
              "env": {"IGNORED": "unsupported for http"},
              "headers": {
                "Authorization": "Bearer ${API_TOKEN}",
                "X-Team": "${TEAM}"
              }
            }
          }
        }"#,
    )
    .expect("write mcp");
    fs::write(
        repo_root.join(EXTERNAL_AGENT_DIR).join("settings.json"),
        r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"echo external-agent","timeout":3},{"type":"prompt","prompt":"skip"}]}],"Stop":[{"matcher":"ignored","hooks":[{"command":"echo done"}]}]}}"#,
    )
    .expect("write hooks");
    fs::write(
        repo_root
            .join(EXTERNAL_AGENT_DIR)
            .join("commands")
            .join("pr")
            .join("review.md"),
        "---\ndescription: Review PR\n---\nReview the pull request carefully.\n",
    )
    .expect("write command");
    fs::write(
        repo_root
            .join(EXTERNAL_AGENT_DIR)
            .join("agents")
            .join("researcher.md"),
        format!("---\nname: researcher\ndescription: Research role\npermissionMode: acceptEdits\nskills: [deep-research]\ntools: Bash, Read\ndisallowedTools: WebFetch\neffort: high\n---\nResearch with {SOURCE_EXTERNAL_AGENT_PRODUCT_NAME} carefully.\n"),
    )
    .expect("write subagent");

    service_for_paths(
        root.path().join(EXTERNAL_AGENT_DIR),
        root.path().join(".codex"),
    )
    .import(vec![
        ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::McpServerConfig,
            description: String::new(),
            cwd: Some(repo_root.clone()),
            details: None,
        },
        ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::Hooks,
            description: String::new(),
            cwd: Some(repo_root.clone()),
            details: None,
        },
        ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::Commands,
            description: String::new(),
            cwd: Some(repo_root.clone()),
            details: None,
        },
        ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::Subagents,
            description: String::new(),
            cwd: Some(repo_root.clone()),
            details: None,
        },
    ])
    .await
    .expect("import");

    let config: TomlValue = toml::from_str(
        &fs::read_to_string(repo_root.join(".codex").join("config.toml")).expect("read config"),
    )
    .expect("parse config");
    let expected_config: TomlValue = toml::from_str(
        r#"
[mcp_servers.api]
url = "https://example.com/mcp"
bearer_token_env_var = "API_TOKEN"

[mcp_servers.api.env_http_headers]
X-Team = "TEAM"

[mcp_servers.docs]
command = "docs-server"
args = ["--stdio"]
env_vars = ["DOCS_TOKEN"]

[mcp_servers.docs.env]
STATIC = "yes"
"#,
    )
    .expect("parse expected config");
    assert_eq!(config, expected_config);
    let mcp_servers = config
        .get("mcp_servers")
        .cloned()
        .ok_or_else(|| io::Error::other("missing mcp_servers"))
        .expect("mcp servers");
    let _supported_mcp_config: std::collections::HashMap<
        String,
        codex_config::types::McpServerConfig,
    > = mcp_servers
        .try_into()
        .expect("migrated MCP config should be supported");

    let hooks: JsonValue = serde_json::from_str(
        &fs::read_to_string(repo_root.join(".codex").join("hooks.json")).expect("read hooks"),
    )
    .expect("parse hooks");
    let _supported_hooks: codex_config::HooksFile =
        serde_json::from_value(hooks.clone()).expect("migrated hooks should be supported");
    assert_eq!(
        hooks,
        serde_json::json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "Bash",
                    "hooks": [{
                        "type": "command",
                        "command": "echo external-agent",
                        "timeout": 3
                    }]
                }],
                "Stop": [{
                    "hooks": [{
                        "type": "command",
                        "command": "echo done"
                    }]
                }]
            }
        })
    );
    assert!(
        !repo_root
            .join(".codex")
            .join("hooks.migration-notes.md")
            .exists()
    );

    assert_eq!(
        fs::read_to_string(
            repo_root
                .join(".agents")
                .join("skills")
                .join("source-command-pr-review")
                .join("SKILL.md")
        )
        .expect("read command skill"),
        "---\nname: \"source-command-pr-review\"\ndescription: \"Review PR\"\n---\n\n# source-command-pr-review\n\nUse this skill when the user asks to run the migrated source command `pr-review`.\n\n## Command Template\n\nReview the pull request carefully.\n"
    );

    let agent: TomlValue = toml::from_str(
        &fs::read_to_string(
            repo_root
                .join(".codex")
                .join("agents")
                .join("researcher.toml"),
        )
        .expect("read agent"),
    )
    .expect("parse agent");
    let expected_agent: TomlValue = toml::from_str(
        r#"
name = "researcher"
description = "Research role"
model_reasoning_effort = "high"
sandbox_mode = "workspace-write"
developer_instructions = """
Research with Codex carefully."""
"#,
    )
    .expect("parse expected agent");
    assert_eq!(agent, expected_agent);
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
        .await
        .expect("import");

    assert_eq!(
        fs::read_to_string(codex_home.join("config.toml")).expect("read config"),
        "sandbox_mode = \"workspace-write\"\n\n[shell_environment_policy]\ninherit = \"core\"\n\n[shell_environment_policy.set]\nFOO = \"local\"\nLOCAL_ONLY = \"true\"\nPROJECT_ONLY = \"yes\"\n"
    );
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
        .await
        .expect("import");

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

    service_for_paths(external_agent_home, codex_home.clone())
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
        .await
        .expect("import");

    assert_eq!(outcome, Vec::<PendingPluginImport>::new());
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
        .await
        .expect("import");

    assert_eq!(
        outcome,
        vec![PendingPluginImport {
            cwd: None,
            details: MigrationDetails {
                plugins: vec![PluginsMigration {
                    marketplace_name: "acme-tools".to_string(),
                    plugin_names: vec!["formatter".to_string()],
                }],
                ..Default::default()
            },
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
            cwds: None,
        })
        .await
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

    service_for_paths(
        root.path().join(EXTERNAL_AGENT_DIR),
        root.path().join(".codex"),
    )
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
    fs::write(
        repo_root.join(EXTERNAL_AGENT_CONFIG_MD),
        format!("{SOURCE_EXTERNAL_AGENT_DISPLAY_NAME} code guidance"),
    )
    .expect("write source");
    fs::write(repo_root.join("AGENTS.md"), " \n\t").expect("write empty target");

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
    .await
    .expect("import");

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

    service_for_paths(
        root.path().join(EXTERNAL_AGENT_DIR),
        root.path().join(".codex"),
    )
    .import(vec![ExternalAgentConfigMigrationItem {
        item_type: ExternalAgentConfigMigrationItemType::Hooks,
        description: String::new(),
        cwd: Some(repo_root.clone()),
        details: None,
    }])
    .await
    .expect("import");

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

    service_for_paths(external_agent_home, root.path().join(".codex"))
        .import(vec![ExternalAgentConfigMigrationItem {
            item_type: ExternalAgentConfigMigrationItemType::McpServerConfig,
            description: String::new(),
            cwd: Some(repo_root.clone()),
            details: None,
        }])
        .await
        .expect("import");

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
        .await
        .expect("import");

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
        .await
        .expect("import");

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
        }
    );
    let config = fs::read_to_string(codex_home.join("config.toml")).expect("read config");
    assert!(config.contains(r#"[plugins."cloudflare@my-plugins"]"#));
    assert!(config.contains("enabled = true"));
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
        outcome,
        PluginImportOutcome {
            succeeded_marketplaces: vec![EXTERNAL_OFFICIAL_MARKETPLACE_NAME.to_string()],
            succeeded_plugin_ids: Vec::new(),
            failed_marketplaces: Vec::new(),
            failed_plugin_ids: vec![format!("sample@{EXTERNAL_OFFICIAL_MARKETPLACE_NAME}")],
        }
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
        }
    );
    let config = fs::read_to_string(codex_home.join("config.toml")).expect("read config");
    assert!(config.contains(r#"[plugins."cloudflare@my-plugins"]"#));
    assert!(config.contains("enabled = true"));
}

#[test]
fn import_skills_returns_only_new_skill_directory_count() {
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

    let copied_count = service_for_paths(external_agent_home, codex_home)
        .import_skills(/*cwd*/ None)
        .expect("import skills");

    assert_eq!(copied_count, 1);
}
