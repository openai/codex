use super::super::*;
use crate::sessions::ExternalAgentSessionMigration;
use pretty_assertions::assert_eq;

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
            include_memory: false,
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
            details: Some(MigrationDetails {
                skills: named_migrations(vec!["skill-a".to_string()]),
                ..Default::default()
            }),
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
            include_memory: false,
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
        include_memory: false,
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
            include_memory: false,
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
                details: Some(MigrationDetails {
                    skills: named_migrations(vec!["skill-a".to_string()]),
                    ..Default::default()
                }),
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
        include_memory: false,
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
        r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","if":"Bash(rm *)","command":"echo blocked"}]}],"UnsupportedEvent":[{"matcher":"worker","hooks":[{"type":"command","command":"echo started"}]}]}}"#,
    )
    .expect("write hooks");

    let items = service_for_paths(
        root.path().join(EXTERNAL_AGENT_DIR),
        root.path().join(".codex"),
    )
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
    .await;

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
