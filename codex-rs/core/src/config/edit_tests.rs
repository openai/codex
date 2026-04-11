use super::*;
use codex_config::types::AppToolApproval;
use codex_config::types::McpServerToolConfig;
use codex_config::types::McpServerTransportConfig;
use codex_protocol::openai_models::ReasoningEffort;
use pretty_assertions::assert_eq;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use tempfile::tempdir;
use toml::Value as TomlValue;

#[test]
fn blocking_set_model_top_level() {
    let tmp = tempdir().expect("tmpdir");
    let codex_home = tmp.path();

    apply_blocking(
        codex_home,
        /*profile*/ None,
        &[ConfigEdit::SetModel {
            model: Some("gpt-5.1-codex".to_string()),
            effort: Some(ReasoningEffort::High),
        }],
    )
    .expect("persist");

    let contents = std::fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).expect("read config");
    let expected = r#"model = "gpt-5.1-codex"
model_reasoning_effort = "high"
"#;
    assert_eq!(contents, expected);
}

#[test]
fn builder_with_edits_applies_custom_paths() {
    let tmp = tempdir().expect("tmpdir");
    let codex_home = tmp.path();

    ConfigEditsBuilder::new(codex_home)
        .with_edits(vec![ConfigEdit::SetPath {
            segments: vec!["enabled".to_string()],
            value: value(true),
        }])
        .apply_blocking()
        .expect("persist");

    let contents = std::fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).expect("read config");
    assert_eq!(contents, "enabled = true\n");
}

#[test]
fn set_model_availability_nux_count_writes_shown_count() {
    let tmp = tempdir().expect("tmpdir");
    let codex_home = tmp.path();
    let shown_count = HashMap::from([("gpt-foo".to_string(), 4)]);

    ConfigEditsBuilder::new(codex_home)
        .set_model_availability_nux_count(&shown_count)
        .apply_blocking()
        .expect("persist");

    let contents = std::fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).expect("read config");
    let expected = r#"[tui.model_availability_nux]
gpt-foo = 4
"#;
    assert_eq!(contents, expected);
}

#[test]
fn set_skill_config_writes_disabled_entry() {
    let tmp = tempdir().expect("tmpdir");
    let codex_home = tmp.path();

    ConfigEditsBuilder::new(codex_home)
        .with_edits([ConfigEdit::SetSkillConfig {
            path: PathBuf::from("/tmp/skills/demo/SKILL.md"),
            enabled: false,
        }])
        .apply_blocking()
        .expect("persist");

    let contents = std::fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).expect("read config");
    let expected = r#"[[skills.config]]
path = "/tmp/skills/demo/SKILL.md"
enabled = false
"#;
    assert_eq!(contents, expected);
}

#[test]
fn set_skill_config_removes_entry_when_enabled() {
    let tmp = tempdir().expect("tmpdir");
    let codex_home = tmp.path();
    std::fs::write(
        codex_home.join(CONFIG_TOML_FILE),
        r#"[[skills.config]]
path = "/tmp/skills/demo/SKILL.md"
enabled = false
"#,
    )
    .expect("seed config");

    ConfigEditsBuilder::new(codex_home)
        .with_edits([ConfigEdit::SetSkillConfig {
            path: PathBuf::from("/tmp/skills/demo/SKILL.md"),
            enabled: true,
        }])
        .apply_blocking()
        .expect("persist");

    let contents = std::fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).expect("read config");
    assert_eq!(contents, "");
}

#[test]
fn set_skill_config_writes_name_selector_entry() {
    let tmp = tempdir().expect("tmpdir");
    let codex_home = tmp.path();

    ConfigEditsBuilder::new(codex_home)
        .with_edits([ConfigEdit::SetSkillConfigByName {
            name: "github:yeet".to_string(),
            enabled: false,
        }])
        .apply_blocking()
        .expect("persist");

    let contents = std::fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).expect("read config");
    let expected = r#"[[skills.config]]
name = "github:yeet"
enabled = false
"#;
    assert_eq!(contents, expected);
}

#[test]
fn blocking_set_model_preserves_inline_table_contents() {
    let tmp = tempdir().expect("tmpdir");
    let codex_home = tmp.path();

    // Seed with inline tables for profiles to simulate common user config.
    std::fs::write(
        codex_home.join(CONFIG_TOML_FILE),
        r#"profile = "fast"

profiles = { fast = { model = "gpt-4o", sandbox_mode = "strict" } }
"#,
    )
    .expect("seed");

    apply_blocking(
        codex_home,
        /*profile*/ None,
        &[ConfigEdit::SetModel {
            model: Some("o4-mini".to_string()),
            effort: None,
        }],
    )
    .expect("persist");

    let raw = std::fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).expect("read config");
    let value: TomlValue = toml::from_str(&raw).expect("parse config");

    // Ensure sandbox_mode is preserved under profiles.fast and model updated.
    let profiles_tbl = value
        .get("profiles")
        .and_then(|v| v.as_table())
        .expect("profiles table");
    let fast_tbl = profiles_tbl
        .get("fast")
        .and_then(|v| v.as_table())
        .expect("fast table");
    assert_eq!(
        fast_tbl.get("sandbox_mode").and_then(|v| v.as_str()),
        Some("strict")
    );
    assert_eq!(
        fast_tbl.get("model").and_then(|v| v.as_str()),
        Some("o4-mini")
    );
}

#[cfg(unix)]
#[test]
fn blocking_set_model_writes_through_symlink_chain() {
    let tmp = tempdir().expect("tmpdir");
    let codex_home = tmp.path();
    let target_dir = tempdir().expect("target dir");
    let target_path = target_dir.path().join(CONFIG_TOML_FILE);
    let link_path = codex_home.join("config-link.toml");
    let config_path = codex_home.join(CONFIG_TOML_FILE);

    symlink(&target_path, &link_path).expect("symlink link");
    symlink("config-link.toml", &config_path).expect("symlink config");

    apply_blocking(
        codex_home,
        /*profile*/ None,
        &[ConfigEdit::SetModel {
            model: Some("gpt-5.1-codex".to_string()),
            effort: Some(ReasoningEffort::High),
        }],
    )
    .expect("persist");

    let meta = std::fs::symlink_metadata(&config_path).expect("config metadata");
    assert!(meta.file_type().is_symlink());

    let contents = std::fs::read_to_string(&target_path).expect("read target");
    let expected = r#"model = "gpt-5.1-codex"
model_reasoning_effort = "high"
"#;
    assert_eq!(contents, expected);
}

#[cfg(unix)]
#[test]
fn blocking_set_model_replaces_symlink_on_cycle() {
    let tmp = tempdir().expect("tmpdir");
    let codex_home = tmp.path();
    let link_a = codex_home.join("a.toml");
    let link_b = codex_home.join("b.toml");
    let config_path = codex_home.join(CONFIG_TOML_FILE);

    symlink("b.toml", &link_a).expect("symlink a");
    symlink("a.toml", &link_b).expect("symlink b");
    symlink("a.toml", &config_path).expect("symlink config");

    apply_blocking(
        codex_home,
        /*profile*/ None,
        &[ConfigEdit::SetModel {
            model: Some("gpt-5.1-codex".to_string()),
            effort: None,
        }],
    )
    .expect("persist");

    let meta = std::fs::symlink_metadata(&config_path).expect("config metadata");
    assert!(!meta.file_type().is_symlink());

    let contents = std::fs::read_to_string(&config_path).expect("read config");
    let expected = r#"model = "gpt-5.1-codex"
"#;
    assert_eq!(contents, expected);
}

#[test]
fn batch_write_table_upsert_preserves_inline_comments() {
    let tmp = tempdir().expect("tmpdir");
    let codex_home = tmp.path();
    let original = r#"approval_policy = "never"

[mcp_servers.linear]
name = "linear"
# ok
url = "https://linear.example"

[mcp_servers.linear.http_headers]
foo = "bar"

[sandbox_workspace_write]
# ok 3
network_access = false
"#;
    std::fs::write(codex_home.join(CONFIG_TOML_FILE), original).expect("seed config");

    apply_blocking(
        codex_home,
        /*profile*/ None,
        &[
            ConfigEdit::SetPath {
                segments: vec![
                    "mcp_servers".to_string(),
                    "linear".to_string(),
                    "url".to_string(),
                ],
                value: value("https://linear.example/v2"),
            },
            ConfigEdit::SetPath {
                segments: vec![
                    "sandbox_workspace_write".to_string(),
                    "network_access".to_string(),
                ],
                value: value(true),
            },
        ],
    )
    .expect("apply");

    let updated = std::fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).expect("read config");
    let expected = r#"approval_policy = "never"

[mcp_servers.linear]
name = "linear"
# ok
url = "https://linear.example/v2"

[mcp_servers.linear.http_headers]
foo = "bar"

[sandbox_workspace_write]
# ok 3
network_access = true
"#;
    assert_eq!(updated, expected);
}

#[test]
fn blocking_clear_model_removes_inline_table_entry() {
    let tmp = tempdir().expect("tmpdir");
    let codex_home = tmp.path();

    std::fs::write(
        codex_home.join(CONFIG_TOML_FILE),
        r#"profile = "fast"

profiles = { fast = { model = "gpt-4o", sandbox_mode = "strict" } }
"#,
    )
    .expect("seed");

    apply_blocking(
        codex_home,
        /*profile*/ None,
        &[ConfigEdit::SetModel {
            model: None,
            effort: Some(ReasoningEffort::High),
        }],
    )
    .expect("persist");

    let contents = std::fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).expect("read config");
    let expected = r#"profile = "fast"

[profiles.fast]
sandbox_mode = "strict"
model_reasoning_effort = "high"
"#;
    assert_eq!(contents, expected);
}

#[test]
fn blocking_set_model_scopes_to_active_profile() {
    let tmp = tempdir().expect("tmpdir");
    let codex_home = tmp.path();
    std::fs::write(
        codex_home.join(CONFIG_TOML_FILE),
        r#"profile = "team"

[profiles.team]
model_reasoning_effort = "low"
"#,
    )
    .expect("seed");

    apply_blocking(
        codex_home,
        /*profile*/ None,
        &[ConfigEdit::SetModel {
            model: Some("o5-preview".to_string()),
            effort: Some(ReasoningEffort::Minimal),
        }],
    )
    .expect("persist");

    let contents = std::fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).expect("read config");
    let expected = r#"profile = "team"

[profiles.team]
model_reasoning_effort = "minimal"
model = "o5-preview"
"#;
    assert_eq!(contents, expected);
}

#[test]
fn blocking_set_model_with_explicit_profile() {
    let tmp = tempdir().expect("tmpdir");
    let codex_home = tmp.path();
    std::fs::write(
        codex_home.join(CONFIG_TOML_FILE),
        r#"[profiles."team a"]
model = "gpt-5.1-codex"
"#,
    )
    .expect("seed");

    apply_blocking(
        codex_home,
        Some("team a"),
        &[ConfigEdit::SetModel {
            model: Some("o4-mini".to_string()),
            effort: None,
        }],
    )
    .expect("persist");

    let contents = std::fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).expect("read config");
    let expected = r#"[profiles."team a"]
model = "o4-mini"
"#;
    assert_eq!(contents, expected);
}

#[test]
fn blocking_set_hide_full_access_warning_preserves_table() {
    let tmp = tempdir().expect("tmpdir");
    let codex_home = tmp.path();
    std::fs::write(
        codex_home.join(CONFIG_TOML_FILE),
        r#"# Global comment

[notice]
# keep me
existing = "value"
"#,
    )
    .expect("seed");

    apply_blocking(
        codex_home,
        /*profile*/ None,
        &[ConfigEdit::SetNoticeHideFullAccessWarning(true)],
    )
    .expect("persist");

    let contents = std::fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).expect("read config");
    let expected = r#"# Global comment

[notice]
# keep me
existing = "value"
hide_full_access_warning = true
"#;
    assert_eq!(contents, expected);
}

#[test]
fn blocking_set_hide_rate_limit_model_nudge_preserves_table() {
    let tmp = tempdir().expect("tmpdir");
    let codex_home = tmp.path();
    std::fs::write(
        codex_home.join(CONFIG_TOML_FILE),
        r#"[notice]
existing = "value"
"#,
    )
    .expect("seed");

    apply_blocking(
        codex_home,
        /*profile*/ None,
        &[ConfigEdit::SetNoticeHideRateLimitModelNudge(true)],
    )
    .expect("persist");

    let contents = std::fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).expect("read config");
    let expected = r#"[notice]
existing = "value"
hide_rate_limit_model_nudge = true
"#;
    assert_eq!(contents, expected);
}

#[test]
fn blocking_set_hide_gpt5_1_migration_prompt_preserves_table() {
    let tmp = tempdir().expect("tmpdir");
    let codex_home = tmp.path();
    std::fs::write(
        codex_home.join(CONFIG_TOML_FILE),
        r#"[notice]
existing = "value"
"#,
    )
    .expect("seed");
    apply_blocking(
        codex_home,
        /*profile*/ None,
        &[ConfigEdit::SetNoticeHideModelMigrationPrompt(
            "hide_gpt5_1_migration_prompt".to_string(),
            true,
        )],
    )
    .expect("persist");

    let contents = std::fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).expect("read config");
    let expected = r#"[notice]
existing = "value"
hide_gpt5_1_migration_prompt = true
"#;
    assert_eq!(contents, expected);
}

#[test]
fn blocking_set_hide_gpt_5_1_codex_max_migration_prompt_preserves_table() {
    let tmp = tempdir().expect("tmpdir");
    let codex_home = tmp.path();
    std::fs::write(
        codex_home.join(CONFIG_TOML_FILE),
        r#"[notice]
existing = "value"
"#,
    )
    .expect("seed");
    apply_blocking(
        codex_home,
        /*profile*/ None,
        &[ConfigEdit::SetNoticeHideModelMigrationPrompt(
            "hide_gpt-5.1-codex-max_migration_prompt".to_string(),
            true,
        )],
    )
    .expect("persist");

    let contents = std::fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).expect("read config");
    let expected = r#"[notice]
existing = "value"
"hide_gpt-5.1-codex-max_migration_prompt" = true
"#;
    assert_eq!(contents, expected);
}

#[test]
fn blocking_record_model_migration_seen_preserves_table() {
    let tmp = tempdir().expect("tmpdir");
    let codex_home = tmp.path();
    std::fs::write(
        codex_home.join(CONFIG_TOML_FILE),
        r#"[notice]
existing = "value"
"#,
    )
    .expect("seed");
    apply_blocking(
        codex_home,
        /*profile*/ None,
        &[ConfigEdit::RecordModelMigrationSeen {
            from: "gpt-5".to_string(),
            to: "gpt-5.1".to_string(),
        }],
    )
    .expect("persist");

    let contents = std::fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).expect("read config");
    let expected = r#"[notice]
existing = "value"

[notice.model_migrations]
gpt-5 = "gpt-5.1"
"#;
    assert_eq!(contents, expected);
}

#[test]
fn blocking_replace_mcp_servers_round_trips() {
    let tmp = tempdir().expect("tmpdir");
    let codex_home = tmp.path();

    let mut servers = BTreeMap::new();
    servers.insert(
        "stdio".to_string(),
        McpServerConfig {
            transport: McpServerTransportConfig::Stdio {
                command: "cmd".to_string(),
                args: vec!["--flag".to_string()],
                env: Some(
                    [
                        ("B".to_string(), "2".to_string()),
                        ("A".to_string(), "1".to_string()),
                    ]
                    .into_iter()
                    .collect(),
                ),
                env_vars: vec!["FOO".to_string()],
                cwd: None,
            },
            enabled: true,
            required: false,
            disabled_reason: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            enabled_tools: Some(vec!["one".to_string(), "two".to_string()]),
            disabled_tools: None,
            scopes: None,
            oauth_resource: None,
            tools: HashMap::new(),
        },
    );

    servers.insert(
        "http".to_string(),
        McpServerConfig {
            transport: McpServerTransportConfig::StreamableHttp {
                url: "https://example.com".to_string(),
                bearer_token_env_var: Some("TOKEN".to_string()),
                http_headers: Some(
                    [("Z-Header".to_string(), "z".to_string())]
                        .into_iter()
                        .collect(),
                ),
                env_http_headers: None,
            },
            enabled: false,
            required: false,
            disabled_reason: None,
            startup_timeout_sec: Some(std::time::Duration::from_secs(5)),
            tool_timeout_sec: None,
            enabled_tools: None,
            disabled_tools: Some(vec!["forbidden".to_string()]),
            scopes: None,
            oauth_resource: Some("https://resource.example.com".to_string()),
            tools: HashMap::new(),
        },
    );

    apply_blocking(
        codex_home,
        /*profile*/ None,
        &[ConfigEdit::ReplaceMcpServers(servers.clone())],
    )
    .expect("persist");

    let raw = std::fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).expect("read config");
    let expected = "\
[mcp_servers.http]
url = \"https://example.com\"
bearer_token_env_var = \"TOKEN\"
enabled = false
startup_timeout_sec = 5.0
disabled_tools = [\"forbidden\"]
oauth_resource = \"https://resource.example.com\"

[mcp_servers.http.http_headers]
Z-Header = \"z\"

[mcp_servers.stdio]
command = \"cmd\"
args = [\"--flag\"]
env_vars = [\"FOO\"]
enabled_tools = [\"one\", \"two\"]

[mcp_servers.stdio.env]
A = \"1\"
B = \"2\"
";
    assert_eq!(raw, expected);
}

#[test]
fn blocking_replace_mcp_servers_serializes_tool_approval_overrides() {
    let tmp = tempdir().expect("tmpdir");
    let codex_home = tmp.path();

    let mut servers = BTreeMap::new();
    servers.insert(
        "docs".to_string(),
        McpServerConfig {
            transport: McpServerTransportConfig::Stdio {
                command: "docs-server".to_string(),
                args: Vec::new(),
                env: None,
                env_vars: Vec::new(),
                cwd: None,
            },
            enabled: true,
            required: false,
            disabled_reason: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            enabled_tools: None,
            disabled_tools: None,
            scopes: None,
            oauth_resource: None,
            tools: HashMap::from([(
                "search".to_string(),
                McpServerToolConfig {
                    approval_mode: Some(AppToolApproval::Approve),
                },
            )]),
        },
    );

    apply_blocking(
        codex_home,
        /*profile*/ None,
        &[ConfigEdit::ReplaceMcpServers(servers)],
    )
    .expect("persist");

    let raw = std::fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).expect("read config");
    let expected = "\
[mcp_servers.docs]
command = \"docs-server\"

[mcp_servers.docs.tools.search]
approval_mode = \"approve\"
";
    assert_eq!(raw, expected);
}

#[test]
fn blocking_replace_mcp_servers_preserves_inline_comments() {
    let tmp = tempdir().expect("tmpdir");
    let codex_home = tmp.path();
    std::fs::write(
        codex_home.join(CONFIG_TOML_FILE),
        r#"[mcp_servers]
# keep me
foo = { command = "cmd" }
"#,
    )
    .expect("seed");

    let mut servers = BTreeMap::new();
    servers.insert(
        "foo".to_string(),
        McpServerConfig {
            transport: McpServerTransportConfig::Stdio {
                command: "cmd".to_string(),
                args: Vec::new(),
                env: None,
                env_vars: Vec::new(),
                cwd: None,
            },
            enabled: true,
            required: false,
            disabled_reason: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            enabled_tools: None,
            disabled_tools: None,
            scopes: None,
            oauth_resource: None,
            tools: HashMap::new(),
        },
    );

    apply_blocking(
        codex_home,
        /*profile*/ None,
        &[ConfigEdit::ReplaceMcpServers(servers)],
    )
    .expect("persist");

    let contents = std::fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).expect("read config");
    let expected = r#"[mcp_servers]
# keep me
foo = { command = "cmd" }
"#;
    assert_eq!(contents, expected);
}

#[test]
fn blocking_replace_mcp_servers_preserves_inline_comment_suffix() {
    let tmp = tempdir().expect("tmpdir");
    let codex_home = tmp.path();
    std::fs::write(
        codex_home.join(CONFIG_TOML_FILE),
        r#"[mcp_servers]
foo = { command = "cmd" } # keep me
"#,
    )
    .expect("seed");

    let mut servers = BTreeMap::new();
    servers.insert(
        "foo".to_string(),
        McpServerConfig {
            transport: McpServerTransportConfig::Stdio {
                command: "cmd".to_string(),
                args: Vec::new(),
                env: None,
                env_vars: Vec::new(),
                cwd: None,
            },
            enabled: false,
            required: false,
            disabled_reason: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            enabled_tools: None,
            disabled_tools: None,
            scopes: None,
            oauth_resource: None,
            tools: HashMap::new(),
        },
    );

    apply_blocking(
        codex_home,
        /*profile*/ None,
        &[ConfigEdit::ReplaceMcpServers(servers)],
    )
    .expect("persist");

    let contents = std::fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).expect("read config");
    let expected = r#"[mcp_servers]
foo = { command = "cmd" , enabled = false } # keep me
"#;
    assert_eq!(contents, expected);
}

#[test]
fn blocking_replace_mcp_servers_preserves_inline_comment_after_removing_keys() {
    let tmp = tempdir().expect("tmpdir");
    let codex_home = tmp.path();
    std::fs::write(
        codex_home.join(CONFIG_TOML_FILE),
        r#"[mcp_servers]
foo = { command = "cmd", args = ["--flag"] } # keep me
"#,
    )
    .expect("seed");

    let mut servers = BTreeMap::new();
    servers.insert(
        "foo".to_string(),
        McpServerConfig {
            transport: McpServerTransportConfig::Stdio {
                command: "cmd".to_string(),
                args: Vec::new(),
                env: None,
                env_vars: Vec::new(),
                cwd: None,
            },
            enabled: true,
            required: false,
            disabled_reason: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            enabled_tools: None,
            disabled_tools: None,
            scopes: None,
            oauth_resource: None,
            tools: HashMap::new(),
        },
    );

    apply_blocking(
        codex_home,
        /*profile*/ None,
        &[ConfigEdit::ReplaceMcpServers(servers)],
    )
    .expect("persist");

    let contents = std::fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).expect("read config");
    let expected = r#"[mcp_servers]
foo = { command = "cmd"} # keep me
"#;
    assert_eq!(contents, expected);
}

#[test]
fn blocking_replace_mcp_servers_preserves_inline_comment_prefix_on_update() {
    let tmp = tempdir().expect("tmpdir");
    let codex_home = tmp.path();
    std::fs::write(
        codex_home.join(CONFIG_TOML_FILE),
        r#"[mcp_servers]
# keep me
foo = { command = "cmd" }
"#,
    )
    .expect("seed");

    let mut servers = BTreeMap::new();
    servers.insert(
        "foo".to_string(),
        McpServerConfig {
            transport: McpServerTransportConfig::Stdio {
                command: "cmd".to_string(),
                args: Vec::new(),
                env: None,
                env_vars: Vec::new(),
                cwd: None,
            },
            enabled: false,
            required: false,
            disabled_reason: None,
            startup_timeout_sec: None,
            tool_timeout_sec: None,
            enabled_tools: None,
            disabled_tools: None,
            scopes: None,
            oauth_resource: None,
            tools: HashMap::new(),
        },
    );

    apply_blocking(
        codex_home,
        /*profile*/ None,
        &[ConfigEdit::ReplaceMcpServers(servers)],
    )
    .expect("persist");

    let contents = std::fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).expect("read config");
    let expected = r#"[mcp_servers]
# keep me
foo = { command = "cmd" , enabled = false }
"#;
    assert_eq!(contents, expected);
}

#[test]
fn blocking_clear_path_noop_when_missing() {
    let tmp = tempdir().expect("tmpdir");
    let codex_home = tmp.path();

    apply_blocking(
        codex_home,
        /*profile*/ None,
        &[ConfigEdit::ClearPath {
            segments: vec!["missing".to_string()],
        }],
    )
    .expect("apply");

    assert!(
        !codex_home.join(CONFIG_TOML_FILE).exists(),
        "config.toml should not be created on noop"
    );
}

#[test]
fn blocking_set_path_updates_notifications() {
    let tmp = tempdir().expect("tmpdir");
    let codex_home = tmp.path();

    let item = value(false);
    apply_blocking(
        codex_home,
        /*profile*/ None,
        &[ConfigEdit::SetPath {
            segments: vec!["tui".to_string(), "notifications".to_string()],
            value: item,
        }],
    )
    .expect("apply");

    let raw = std::fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).expect("read config");
    let config: TomlValue = toml::from_str(&raw).expect("parse config");
    let notifications = config
        .get("tui")
        .and_then(|item| item.as_table())
        .and_then(|tbl| tbl.get("notifications"))
        .and_then(toml::Value::as_bool);
    assert_eq!(notifications, Some(false));
}

#[tokio::test]
async fn async_builder_set_model_persists() {
    let tmp = tempdir().expect("tmpdir");
    let codex_home = tmp.path().to_path_buf();

    ConfigEditsBuilder::new(&codex_home)
        .set_model(Some("gpt-5.1-codex"), Some(ReasoningEffort::High))
        .apply()
        .await
        .expect("persist");

    let contents = std::fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).expect("read config");
    let expected = r#"model = "gpt-5.1-codex"
model_reasoning_effort = "high"
"#;
    assert_eq!(contents, expected);
}

#[test]
fn blocking_builder_set_model_round_trips_back_and_forth() {
    let tmp = tempdir().expect("tmpdir");
    let codex_home = tmp.path();

    let initial_expected = r#"model = "o4-mini"
model_reasoning_effort = "low"
"#;
    ConfigEditsBuilder::new(codex_home)
        .set_model(Some("o4-mini"), Some(ReasoningEffort::Low))
        .apply_blocking()
        .expect("persist initial");
    let mut contents =
        std::fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).expect("read config");
    assert_eq!(contents, initial_expected);

    let updated_expected = r#"model = "gpt-5.1-codex"
model_reasoning_effort = "high"
"#;
    ConfigEditsBuilder::new(codex_home)
        .set_model(Some("gpt-5.1-codex"), Some(ReasoningEffort::High))
        .apply_blocking()
        .expect("persist update");
    contents = std::fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).expect("read config");
    assert_eq!(contents, updated_expected);

    ConfigEditsBuilder::new(codex_home)
        .set_model(Some("o4-mini"), Some(ReasoningEffort::Low))
        .apply_blocking()
        .expect("persist revert");
    contents = std::fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).expect("read config");
    assert_eq!(contents, initial_expected);
}

#[tokio::test]
async fn blocking_set_asynchronous_helpers_available() {
    let tmp = tempdir().expect("tmpdir");
    let codex_home = tmp.path().to_path_buf();

    ConfigEditsBuilder::new(&codex_home)
        .set_hide_full_access_warning(/*acknowledged*/ true)
        .apply()
        .await
        .expect("persist");

    let raw = std::fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).expect("read config");
    let notice = toml::from_str::<TomlValue>(&raw)
        .expect("parse config")
        .get("notice")
        .and_then(|item| item.as_table())
        .and_then(|tbl| tbl.get("hide_full_access_warning"))
        .and_then(toml::Value::as_bool);
    assert_eq!(notice, Some(true));
}

#[test]
fn blocking_builder_set_realtime_audio_persists_and_clears() {
    let tmp = tempdir().expect("tmpdir");
    let codex_home = tmp.path();

    ConfigEditsBuilder::new(codex_home)
        .set_realtime_microphone(Some("USB Mic"))
        .set_realtime_speaker(Some("Desk Speakers"))
        .apply_blocking()
        .expect("persist realtime audio");

    let raw = std::fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).expect("read config");
    let config: TomlValue = toml::from_str(&raw).expect("parse config");
    let realtime_audio = config
        .get("audio")
        .and_then(TomlValue::as_table)
        .expect("audio table should exist");
    assert_eq!(
        realtime_audio.get("microphone").and_then(TomlValue::as_str),
        Some("USB Mic")
    );
    assert_eq!(
        realtime_audio.get("speaker").and_then(TomlValue::as_str),
        Some("Desk Speakers")
    );

    ConfigEditsBuilder::new(codex_home)
        .set_realtime_microphone(/*microphone*/ None)
        .apply_blocking()
        .expect("clear realtime microphone");

    let raw = std::fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).expect("read config");
    let config: TomlValue = toml::from_str(&raw).expect("parse config");
    let realtime_audio = config
        .get("audio")
        .and_then(TomlValue::as_table)
        .expect("audio table should exist");
    assert_eq!(realtime_audio.get("microphone"), None);
    assert_eq!(
        realtime_audio.get("speaker").and_then(TomlValue::as_str),
        Some("Desk Speakers")
    );
}

#[test]
fn blocking_builder_set_realtime_voice_persists_and_clears() {
    let tmp = tempdir().expect("tmpdir");
    let codex_home = tmp.path();

    ConfigEditsBuilder::new(codex_home)
        .set_realtime_voice(Some("cedar"))
        .apply_blocking()
        .expect("persist realtime voice");

    let raw = std::fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).expect("read config");
    let config: TomlValue = toml::from_str(&raw).expect("parse config");
    let realtime = config
        .get("realtime")
        .and_then(TomlValue::as_table)
        .expect("realtime table should exist");
    assert_eq!(
        realtime.get("voice").and_then(TomlValue::as_str),
        Some("cedar")
    );

    ConfigEditsBuilder::new(codex_home)
        .set_realtime_voice(/*voice*/ None)
        .apply_blocking()
        .expect("clear realtime voice");

    let raw = std::fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).expect("read config");
    let config: TomlValue = toml::from_str(&raw).expect("parse config");
    let realtime = config
        .get("realtime")
        .and_then(TomlValue::as_table)
        .expect("realtime table should exist");
    assert_eq!(realtime.get("voice"), None);
}

#[test]
fn replace_mcp_servers_blocking_clears_table_when_empty() {
    let tmp = tempdir().expect("tmpdir");
    let codex_home = tmp.path();
    std::fs::write(
        codex_home.join(CONFIG_TOML_FILE),
        "[mcp_servers]\nfoo = { command = \"cmd\" }\n",
    )
    .expect("seed");

    apply_blocking(
        codex_home,
        /*profile*/ None,
        &[ConfigEdit::ReplaceMcpServers(BTreeMap::new())],
    )
    .expect("persist");

    let contents = std::fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).expect("read config");
    assert!(!contents.contains("mcp_servers"));
}
