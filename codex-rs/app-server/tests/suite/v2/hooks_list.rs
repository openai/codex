use std::time::Duration;

use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::HookEventName;
use codex_app_server_protocol::HookHandlerType;
use codex_app_server_protocol::HookMetadata;
use codex_app_server_protocol::HookSource;
use codex_app_server_protocol::HooksListEntry;
use codex_app_server_protocol::HooksListParams;
use codex_app_server_protocol::HooksListResponse;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use codex_core::config::set_project_trust_level;
use codex_protocol::config_types::TrustLevel;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

fn write_user_hook_config(codex_home: &std::path::Path) -> Result<()> {
    std::fs::write(
        codex_home.join("config.toml"),
        r#"[hooks]

[[hooks.PreToolUse]]
matcher = "Bash"

[[hooks.PreToolUse.hooks]]
type = "command"
command = "python3 /tmp/listed-hook.py"
timeout = 5
statusMessage = "running listed hook"
"#,
    )?;
    Ok(())
}

fn write_plugin_hook_config(codex_home: &std::path::Path, hooks_json: &str) -> Result<()> {
    let plugin_root = codex_home.join("plugins/cache/test/demo/local");
    std::fs::create_dir_all(plugin_root.join(".codex-plugin"))?;
    std::fs::create_dir_all(plugin_root.join("hooks"))?;
    std::fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        r#"{"name":"demo"}"#,
    )?;
    std::fs::write(plugin_root.join("hooks/hooks.json"), hooks_json)?;
    std::fs::write(
        codex_home.join("config.toml"),
        r#"[features]
plugins = true
plugin_hooks = true
codex_hooks = true

[plugins."demo@test"]
enabled = true
"#,
    )?;
    Ok(())
}

#[tokio::test]
async fn hooks_list_shows_discovered_hook() -> Result<()> {
    let codex_home = TempDir::new()?;
    let cwd = TempDir::new()?;
    write_user_hook_config(codex_home.path())?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_hooks_list_request(HooksListParams {
            cwds: vec![cwd.path().to_path_buf()],
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let HooksListResponse { data } = to_response(response)?;
    assert_eq!(
        data,
        vec![HooksListEntry {
            cwd: cwd.path().to_path_buf(),
            hooks: vec![HookMetadata {
                event_name: HookEventName::PreToolUse,
                handler_type: HookHandlerType::Command,
                matcher: Some("Bash".to_string()),
                command: Some("python3 /tmp/listed-hook.py".to_string()),
                timeout_sec: 5,
                status_message: Some("running listed hook".to_string()),
                source_path: AbsolutePathBuf::from_absolute_path(std::fs::canonicalize(
                    codex_home.path().join("config.toml")
                )?,)?,
                source: HookSource::User,
                plugin_id: None,
                display_order: 0,
            }],
            warnings: Vec::new(),
            errors: Vec::new(),
        }]
    );
    Ok(())
}

#[tokio::test]
async fn hooks_list_shows_discovered_plugin_hook() -> Result<()> {
    let codex_home = TempDir::new()?;
    let cwd = TempDir::new()?;
    write_plugin_hook_config(
        codex_home.path(),
        r#"{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "echo plugin hook",
            "timeout": 7,
            "statusMessage": "running plugin hook"
          }
        ]
      }
    ]
  }
}"#,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_hooks_list_request(HooksListParams {
            cwds: vec![cwd.path().to_path_buf()],
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let HooksListResponse { data } = to_response(response)?;
    assert_eq!(
        data,
        vec![HooksListEntry {
            cwd: cwd.path().to_path_buf(),
            hooks: vec![HookMetadata {
                event_name: HookEventName::PreToolUse,
                handler_type: HookHandlerType::Command,
                matcher: Some("Bash".to_string()),
                command: Some("echo plugin hook".to_string()),
                timeout_sec: 7,
                status_message: Some("running plugin hook".to_string()),
                source_path: AbsolutePathBuf::from_absolute_path(std::fs::canonicalize(
                    codex_home
                        .path()
                        .join("plugins/cache/test/demo/local/hooks/hooks.json"),
                )?,)?,
                source: HookSource::Plugin,
                plugin_id: Some("demo@test".to_string()),
                display_order: 0,
            }],
            warnings: Vec::new(),
            errors: Vec::new(),
        }]
    );
    Ok(())
}

#[tokio::test]
async fn hooks_list_shows_plugin_hook_load_warnings() -> Result<()> {
    let codex_home = TempDir::new()?;
    let cwd = TempDir::new()?;
    write_plugin_hook_config(codex_home.path(), "{ not-json")?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_hooks_list_request(HooksListParams {
            cwds: vec![cwd.path().to_path_buf()],
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let HooksListResponse { data } = to_response(response)?;

    assert_eq!(data.len(), 1);
    assert_eq!(data[0].hooks, Vec::new());
    assert_eq!(data[0].warnings.len(), 1);
    assert!(
        data[0].warnings[0].contains("failed to parse plugin hooks config"),
        "unexpected warnings: {:?}",
        data[0].warnings
    );
    Ok(())
}

#[tokio::test]
async fn hooks_list_uses_each_cwds_effective_feature_enablement() -> Result<()> {
    let codex_home = TempDir::new()?;
    let workspace = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        r#"[features]
codex_hooks = false
"#,
    )?;
    std::fs::create_dir_all(workspace.path().join(".git"))?;
    std::fs::create_dir_all(workspace.path().join(".codex"))?;
    std::fs::write(
        workspace.path().join(".codex/config.toml"),
        r#"[features]
codex_hooks = true

[hooks]

[[hooks.PreToolUse]]
matcher = "Bash"

[[hooks.PreToolUse.hooks]]
type = "command"
command = "echo project hook"
timeout = 5
"#,
    )?;
    set_project_trust_level(codex_home.path(), workspace.path(), TrustLevel::Trusted)?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_hooks_list_request(HooksListParams {
            cwds: vec![
                codex_home.path().to_path_buf(),
                workspace.path().to_path_buf(),
            ],
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let HooksListResponse { data } = to_response(response)?;
    assert_eq!(
        data,
        vec![
            HooksListEntry {
                cwd: codex_home.path().to_path_buf(),
                hooks: Vec::new(),
                warnings: Vec::new(),
                errors: Vec::new(),
            },
            HooksListEntry {
                cwd: workspace.path().to_path_buf(),
                hooks: vec![HookMetadata {
                    event_name: HookEventName::PreToolUse,
                    handler_type: HookHandlerType::Command,
                    matcher: Some("Bash".to_string()),
                    command: Some("echo project hook".to_string()),
                    timeout_sec: 5,
                    status_message: None,
                    source_path: AbsolutePathBuf::try_from(
                        workspace.path().join(".codex/config.toml"),
                    )?,
                    source: HookSource::Project,
                    plugin_id: None,
                    display_order: 0,
                }],
                warnings: Vec::new(),
                errors: Vec::new(),
            },
        ]
    );
    Ok(())
}
