use super::hooks_cla::append_convertible_hook_groups_cla;
use super::hooks_cla::hook_migration_cla;
use super::hooks_cla::rewrite_hook_command_cla;
use super::*;
use pretty_assertions::assert_eq;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use toml::Value as TomlValue;

const TEST_REWRITE_PROFILE: RewriteProfile = RewriteProfile::new(
    "CLAUDE.md",
    &[
        "claude code",
        "claude-code",
        "claude_code",
        "claudecode",
        "claude",
    ],
);

fn source_path(relative_path: &str) -> PathBuf {
    Path::new("/repo")
        .join(external_agent_config_dir())
        .join(relative_path)
}

fn source_hook_command(script_name: &str) -> String {
    format!(
        "python3 {}/{EXTERNAL_AGENT_HOOKS_SUBDIR}/{script_name}",
        external_agent_config_dir()
    )
}

fn source_hook_command_with_project_dir(script_name: &str) -> String {
    format!(
        "python3 \"${}\"/{}/{EXTERNAL_AGENT_HOOKS_SUBDIR}/{script_name}",
        external_agent_project_dir_env_var(),
        external_agent_config_dir()
    )
}

fn migrated_hook_command(script_name: &str) -> String {
    migrated_quoted_hook_command(script_name)
}

fn migrated_quoted_hook_command(script_name: &str) -> String {
    let hook_path = Path::new("/repo/.codex")
        .join(EXTERNAL_AGENT_MIGRATED_HOOKS_SUBDIR)
        .join(script_name);
    format!(
        "python3 {}",
        shell_single_quote(hook_path.to_string_lossy().as_ref())
    )
}

#[test]
fn env_placeholder_accepts_defaults() {
    assert_eq!(
        parse_env_placeholder("${TOKEN:-fallback}"),
        Some("TOKEN".to_string())
    );
}

#[test]
fn mcp_migration_skips_placeholder_args() {
    let root = tempfile::TempDir::new().expect("tempdir");
    fs::write(
        root.path().join(".mcp.json"),
        r#"{"mcpServers":{"db":{"command":"db-server","args":["${DATABASE_URL}"]}}}"#,
    )
    .expect("write mcp");

    assert_eq!(
        build_mcp_config_from_external(
            root.path(),
            /*external_agent_home*/ None,
            /*settings*/ None,
        )
        .unwrap(),
        TomlValue::Table(Default::default())
    );
}

#[test]
fn mcp_migration_prefers_command_transport_for_mixed_server_config() {
    let root = tempfile::TempDir::new().expect("tempdir");
    fs::write(
        root.path().join(".mcp.json"),
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

    assert_eq!(
        build_mcp_config_from_external(
            root.path(),
            /*external_agent_home*/ None,
            /*settings*/ None,
        )
        .unwrap(),
        toml::from_str(
            r#"
[mcp_servers.mixedTransport]
command = "mcp-remote-proxy"
args = [
  "https://example.com/mixed-transport",
  "--transport",
  "http",
]
"#
        )
        .unwrap()
    );
}

#[test]
fn mcp_migration_skips_unsupported_transports() {
    let root = tempfile::TempDir::new().expect("tempdir");
    fs::write(
        root.path().join(".mcp.json"),
        r#"{
          "mcpServers": {
            "legacy-sse": {"type": "sse", "url": "https://example.invalid/sse"},
            "vault": {
              "url": "https://example.invalid/vault",
              "headers": {"Authorization": "Bearer ${VAULT_TOKEN:-dev-token}"}
            }
          }
        }"#,
    )
    .expect("write mcp");

    assert_eq!(
        build_mcp_config_from_external(
            root.path(),
            /*external_agent_home*/ None,
            /*settings*/ None,
        )
        .unwrap(),
        toml::from_str(
            r#"
[mcp_servers.vault]
url = "https://example.invalid/vault"
bearer_token_env_var = "VAULT_TOKEN"
"#
        )
        .unwrap()
    );
}

#[test]
fn mcp_migration_reads_matching_project_entries_from_repo_external_project_config() {
    let root = tempfile::TempDir::new().expect("tempdir");
    let project = root.path().join("repo");
    fs::create_dir_all(&project).expect("create repo");
    let other = root.path().join("other");
    fs::create_dir_all(&other).expect("create other");
    fs::write(
        project.join(external_agent_project_config_file()),
        serde_json::json!({
            "mcpServers": {
                "top": {"command": "top-server"}
            },
            "projects": {
                project.display().to_string(): {
                    "mcpServers": {
                        "repo": {"command": "repo-server"}
                    }
                },
                other.display().to_string(): {
                    "mcpServers": {
                        "other": {"command": "other-server"}
                    }
                }
            }
        })
        .to_string(),
    )
    .expect("write external agent project config");

    assert_eq!(
        build_mcp_config_from_external(
            &project, /*external_agent_home*/ None, /*settings*/ None,
        )
        .unwrap(),
        toml::from_str(
            r#"
[mcp_servers.repo]
command = "repo-server"

[mcp_servers.top]
command = "top-server"
"#
        )
        .unwrap()
    );
}

#[test]
fn mcp_migration_reads_matching_project_entries_from_home_external_project_config() {
    let root = tempfile::TempDir::new().expect("tempdir");
    let project = root.path().join("repo");
    fs::create_dir_all(&project).expect("create repo");
    let external_agent_home = root.path().join(external_agent_config_dir());
    fs::create_dir_all(&external_agent_home).expect("create external agent home");
    fs::write(
        root.path().join(external_agent_project_config_file()),
        serde_json::json!({
            "projects": {
                project.display().to_string(): {
                    "mcpServers": {
                        "repo": {"command": "repo-server"}
                    }
                }
            }
        })
        .to_string(),
    )
    .expect("write external agent project config");

    assert_eq!(
        build_mcp_config_from_external(
            &project,
            Some(&external_agent_home),
            /*settings*/ None,
        )
        .unwrap(),
        toml::from_str(
            r#"
[mcp_servers.repo]
command = "repo-server"
"#
        )
        .unwrap()
    );
}

#[test]
fn mcp_migration_preserves_repo_servers_over_home_project_entries() {
    let root = tempfile::TempDir::new().expect("tempdir");
    let project = root.path().join("repo");
    fs::create_dir_all(&project).expect("create repo");
    let external_agent_home = root.path().join(external_agent_config_dir());
    fs::create_dir_all(&external_agent_home).expect("create external agent home");
    fs::write(
        project.join(EXTERNAL_AGENT_MCP_CONFIG_FILE),
        serde_json::json!({
            "mcpServers": {
                "shared": {"command": "repo-server"}
            }
        })
        .to_string(),
    )
    .expect("write repo mcp");
    fs::write(
        root.path().join(external_agent_project_config_file()),
        serde_json::json!({
            "projects": {
                project.display().to_string(): {
                    "mcpServers": {
                        "home-only": {"command": "home-only-server"},
                        "shared": {"command": "home-server"}
                    }
                }
            }
        })
        .to_string(),
    )
    .expect("write external agent project config");

    assert_eq!(
        build_mcp_config_from_external(
            &project,
            Some(&external_agent_home),
            /*settings*/ None,
        )
        .unwrap(),
        toml::from_str(
            r#"
[mcp_servers.home-only]
command = "home-only-server"

[mcp_servers.shared]
command = "repo-server"
"#
        )
        .unwrap()
    );
}

#[test]
fn mcp_migration_skips_disabled_servers() {
    let root = tempfile::TempDir::new().expect("tempdir");
    fs::write(
        root.path().join(".mcp.json"),
        r#"{
          "mcpServers": {
            "enabled": {"command": "enabled-server"},
            "explicit-disabled": {"command": "disabled-server", "disabled": true},
            "not-enabled": {"command": "not-enabled-server"}
          }
        }"#,
    )
    .expect("write mcp");
    let settings = serde_json::json!({
        "enabledMcpjsonServers": ["enabled"],
        "disabledMcpjsonServers": ["explicit-disabled"]
    });

    assert_eq!(
        build_mcp_config_from_external(
            root.path(),
            /*external_agent_home*/ None,
            Some(&settings),
        )
        .unwrap(),
        toml::from_str(
            r#"
[mcp_servers.enabled]
command = "enabled-server"
"#
        )
        .unwrap()
    );
}

#[test]
fn subagent_accepts_yaml_block_lists_by_ignoring_unsupported_fields() {
    let document = parse_document_content(
        "---\nname: cloud-incident\ndescription: Debug incidents\nskills:\n  - runbook-reader\ntools:\n  - Read\n  - Bash\ndisallowedTools:\n  - Write\n---\nInvestigate carefully.\n",
    );

    assert!(agent_metadata(&document).is_some());
}

#[test]
fn subagent_requires_minimum_codex_agent_fields() {
    let missing_description =
        parse_document_content("---\nname: incomplete\n---\nInvestigate carefully.\n");
    let missing_body =
        parse_document_content("---\nname: incomplete\ndescription: Missing body\n---\n");

    assert!(agent_metadata(&missing_description).is_none());
    assert!(agent_metadata(&missing_body).is_none());
}

#[test]
fn subagent_preserves_default_model_when_source_model_is_present() {
    let document = parse_document_content(
        "---\nname: reviewer\ndescription: Review code\nmodel: source-opus\neffort: max\n---\nReview carefully.\n",
    );
    let metadata = agent_metadata(&document).expect("metadata");
    let rendered: TomlValue = toml::from_str(
        &render_agent_toml(&document.body, &metadata, TEST_REWRITE_PROFILE).expect("render agent"),
    )
    .expect("parse rendered agent");
    let expected: TomlValue = toml::from_str(
        r#"
name = "reviewer"
description = "Review code"
model_reasoning_effort = "xhigh"
developer_instructions = """
Review carefully."""
"#,
    )
    .expect("parse expected agent");

    assert_eq!(rendered, expected);
}

#[test]
fn subagent_target_preserves_dotted_file_stem() {
    let target_agents = Path::new("/repo/.codex/agents");
    let source_file = source_path("agents/security.audit.md");

    assert_eq!(
        subagent_target_file(&source_file, target_agents),
        Some(PathBuf::from("/repo/.codex/agents/security.audit.toml"))
    );
}

#[test]
fn frontmatter_accepts_crlf_delimiters() {
    let document = parse_document_content(
        "---\r\nname: reviewer\r\ndescription: Review code\r\n---\r\nReview carefully.\r\n",
    );

    assert_eq!(
        (
            document
                .frontmatter
                .get("name")
                .and_then(FrontmatterValue::as_scalar),
            document
                .frontmatter
                .get("description")
                .and_then(FrontmatterValue::as_scalar),
            document.body.as_str(),
        ),
        (
            Some("reviewer"),
            Some("Review code"),
            "Review carefully.\r\n"
        )
    );
}

#[test]
fn hook_migration_ignores_unsupported_handlers() {
    let settings = serde_json::json!({
        "hooks": {
            "PreToolUse": [{
                "matcher": "Bash",
                "if": "tool_input.command contains 'rm'",
                "hooks": [{
                    "type": "command",
                    "command": source_hook_command("policy_gate.py")
                }]
            }, {
                "matcher": "Edit",
                "hooks": [
                    {
                        "type": "command",
                        "if": "Bash(rm *)",
                        "command": source_hook_command("policy_gate.py")
                    },
                    {
                        "type": "http",
                        "url": "https://example.invalid/hook"
                    }
                ]
            }],
            "PermissionRequest": [{
                "matcher": "Bash",
                "hooks": [{
                    "type": "command",
                    "command": source_hook_command("approve.py")
                }]
            }],
            "SubagentStart": [{
                "matcher": "worker",
                "hooks": [{"type": "prompt", "prompt": "check"}]
            }]
        }
    });
    let mut migration = serde_json::Map::new();
    append_convertible_hook_groups_cla(
        &settings,
        &mut migration,
        Some(Path::new("/repo/.codex")),
        TEST_REWRITE_PROFILE,
    );

    assert_eq!(
        migration,
        serde_json::json!({
            "PermissionRequest": [{
                "matcher": "Bash",
                "hooks": [{
                    "type": "command",
                    "command": migrated_hook_command("approve.py")
                }]
            }]
        })
        .as_object()
        .cloned()
        .expect("object")
    );
}

#[test]
fn hook_migration_honors_disable_all_hooks() {
    let root = tempfile::TempDir::new().expect("tempdir");
    fs::write(
        root.path().join("settings.json"),
        r#"{
          "disableAllHooks": true,
          "hooks": {
            "SessionStart": [{
              "matcher": "startup",
              "hooks": [{"type": "command", "command": "echo setup"}]
            }]
          }
        }"#,
    )
    .expect("write settings");

    assert_eq!(
        hook_migration_cla(
            root.path(),
            /*target_config_dir*/ None,
            TEST_REWRITE_PROFILE,
        )
        .unwrap(),
        serde_json::Map::new()
    );
}

#[test]
fn hook_migration_honors_settings_local_disable_override() {
    let root = tempfile::TempDir::new().expect("tempdir");
    fs::write(
        root.path().join("settings.json"),
        r#"{
          "disableAllHooks": true,
          "hooks": {
            "SessionStart": [{
              "matcher": "project",
              "hooks": [{"type": "command", "command": "echo project"}]
            }]
          }
        }"#,
    )
    .expect("write project settings");
    fs::write(
        root.path().join("settings.local.json"),
        r#"{
          "disableAllHooks": false,
          "hooks": {
            "SessionStart": [{
              "matcher": "local",
              "hooks": [{"type": "command", "command": "echo local"}]
            }]
          }
        }"#,
    )
    .expect("write local settings");

    assert_eq!(
        hook_migration_cla(
            root.path(),
            /*target_config_dir*/ None,
            TEST_REWRITE_PROFILE,
        )
        .unwrap(),
        serde_json::json!({
            "SessionStart": [{
                "matcher": "project",
                "hooks": [{
                    "type": "command",
                    "command": "echo project"
                }]
            }, {
                "matcher": "local",
                "hooks": [{
                    "type": "command",
                    "command": "echo local"
                }]
            }]
        })
        .as_object()
        .cloned()
        .expect("object")
    );
}

#[test]
fn hook_command_paths_rewrite_to_target_hook_dir() {
    let project_dir_env_var = external_agent_project_dir_env_var();
    let plugin_root_env_var = format!(
        "{}_PLUGIN_ROOT",
        SOURCE_EXTERNAL_AGENT_NAME.to_ascii_uppercase()
    );
    let source_hooks_path = format!(
        "{}/{EXTERNAL_AGENT_HOOKS_SUBDIR}",
        external_agent_config_dir()
    );
    assert_eq!(
        rewrite_hook_command_cla(
            &source_hook_command_with_project_dir("check.py"),
            Some(Path::new("/repo/.codex")),
        ),
        migrated_hook_command("check.py")
    );
    assert_eq!(
        rewrite_hook_command_cla(
            &format!("\"${project_dir_env_var}\"/{source_hooks_path}/check-style.sh"),
            Some(Path::new("/repo/.codex")),
        ),
        shell_single_quote(
            Path::new("/repo/.codex")
                .join(EXTERNAL_AGENT_MIGRATED_HOOKS_SUBDIR)
                .join("check-style.sh")
                .to_string_lossy()
                .as_ref()
        )
    );
    assert_eq!(
        rewrite_hook_command_cla(
            &source_hook_command("check.py"),
            Some(Path::new("/repo/.codex")),
        ),
        migrated_hook_command("check.py")
    );
    assert_eq!(
        rewrite_hook_command_cla(
            &format!("python3 ./{source_hooks_path}/check.py"),
            Some(Path::new("/repo/.codex")),
        ),
        migrated_hook_command("check.py")
    );
    assert_eq!(
        rewrite_hook_command_cla(
            &format!("python3 '${{{project_dir_env_var}}}/{source_hooks_path}/check.py'"),
            Some(Path::new("/repo/.codex")),
        ),
        migrated_quoted_hook_command("check.py")
    );
    assert_eq!(
        rewrite_hook_command_cla(
            &format!("python3 \"${{{project_dir_env_var}}}/{source_hooks_path}/check.py\""),
            Some(Path::new("/repo/.codex")),
        ),
        migrated_quoted_hook_command("check.py")
    );
    assert_eq!(
        rewrite_hook_command_cla(
            &format!("bash -lc \"python3 {source_hooks_path}/check.py\""),
            Some(Path::new("/repo/.codex")),
        ),
        format!("bash -lc \"python3 {source_hooks_path}/check.py\"")
    );
    assert_eq!(
        rewrite_hook_command_cla(
            &format!(
                "HOOK=${{{project_dir_env_var}}}/{source_hooks_path}/check.py python3 \"$HOOK\""
            ),
            Some(Path::new("/repo/.codex")),
        ),
        format!("HOOK=${{{project_dir_env_var}}}/{source_hooks_path}/check.py python3 \"$HOOK\"")
    );
    assert_eq!(
        rewrite_hook_command_cla(
            &format!("python3 {source_hooks_path}/${{SCRIPT}}.py"),
            Some(Path::new("/repo/.codex")),
        ),
        format!("python3 {source_hooks_path}/${{SCRIPT}}.py")
    );
    assert_eq!(
        rewrite_hook_command_cla(
            &format!("python3 {source_hooks_path}/{{lint,fmt}}.sh"),
            Some(Path::new("/repo/.codex")),
        ),
        format!("python3 {source_hooks_path}/{{lint,fmt}}.sh")
    );
    assert_eq!(
        rewrite_hook_command_cla(
            &format!("python3 {source_hooks_path}/my\\ script.py"),
            Some(Path::new("/repo/.codex")),
        ),
        format!("python3 {source_hooks_path}/my\\ script.py")
    );
    assert_eq!(
        rewrite_hook_command_cla(
            &format!("python3 .{SOURCE_EXTERNAL_AGENT_NAME}\\hooks\\check.py"),
            Some(Path::new("/repo/.codex")),
        ),
        format!("python3 .{}\\hooks\\check.py", SOURCE_EXTERNAL_AGENT_NAME)
    );
    assert_eq!(
        rewrite_hook_command_cla(
            &format!(
                "python3 \"%{}%\\{}\\hooks\\check.py\"",
                project_dir_env_var,
                external_agent_config_dir()
            ),
            Some(Path::new("/repo/.codex")),
        ),
        format!(
            "python3 \"%{}%\\{}\\hooks\\check.py\"",
            project_dir_env_var,
            external_agent_config_dir()
        )
    );
    assert_eq!(
        rewrite_hook_command_cla(
            &format!("python3 '${{{project_dir_env_var}}}/{source_hooks_path}/my script.py'"),
            Some(Path::new("/repo/.codex")),
        ),
        migrated_quoted_hook_command("my script.py")
    );
    assert_eq!(
        rewrite_hook_command_cla(
            &format!("/repo/{source_hooks_path}/check.py 2>/dev/null || true"),
            Some(Path::new("/repo/.codex")),
        ),
        format!(
            "{} 2>/dev/null || true",
            shell_single_quote(
                Path::new("/repo/.codex")
                    .join(EXTERNAL_AGENT_MIGRATED_HOOKS_SUBDIR)
                    .join("check.py")
                    .to_string_lossy()
                    .as_ref()
            )
        )
    );
    let plugin_script_command = format!("${{{plugin_root_env_var}}}/scripts/format.sh");
    assert_eq!(
        rewrite_hook_command_cla(&plugin_script_command, Some(Path::new("/repo/.codex")),),
        plugin_script_command
    );
}

#[test]
fn hook_script_copy_keeps_existing_target_scripts() {
    let root = tempfile::TempDir::new().expect("tempdir");
    let source_external_agent_dir = root.path().join(external_agent_config_dir());
    let source_hooks = source_external_agent_dir.join(EXTERNAL_AGENT_HOOKS_SUBDIR);
    let target_config_dir = root.path().join(".codex");
    let target_hooks = target_config_dir.join(EXTERNAL_AGENT_MIGRATED_HOOKS_SUBDIR);
    fs::create_dir_all(&source_hooks).expect("create source hooks");
    fs::create_dir_all(&target_hooks).expect("create target hooks");
    fs::write(source_hooks.join("check.py"), "new script").expect("write source hook");
    fs::write(target_hooks.join("check.py"), "existing script").expect("write target hook");

    copy_hook_scripts(&source_external_agent_dir, &target_config_dir).expect("copy hooks");

    assert_eq!(
        fs::read_to_string(target_hooks.join("check.py")).expect("read target hook"),
        "existing script"
    );
}

#[test]
fn hook_migration_drops_negative_timeouts() {
    let settings = serde_json::json!({
        "hooks": {
            "SessionStart": [{
                "matcher": "startup",
                "hooks": [{
                    "type": "command",
                    "command": "echo setup",
                    "timeout": -1
                }]
            }]
        }
    });
    let mut migration = serde_json::Map::new();
    append_convertible_hook_groups_cla(
        &settings,
        &mut migration,
        /*target_config_dir*/ None,
        TEST_REWRITE_PROFILE,
    );

    assert_eq!(
        migration,
        serde_json::json!({
            "SessionStart": [{
                "matcher": "startup",
                "hooks": [{
                    "type": "command",
                    "command": "echo setup"
                }]
            }]
        })
        .as_object()
        .cloned()
        .expect("object")
    );
}
