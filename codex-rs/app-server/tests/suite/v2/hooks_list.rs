use std::time::Duration;

use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use app_test_support::to_response;
use codex_app_server_protocol::ConfigBatchWriteParams;
use codex_app_server_protocol::ConfigEdit;
use codex_app_server_protocol::HookEventName;
use codex_app_server_protocol::HookHandlerType;
use codex_app_server_protocol::HookMetadata;
use codex_app_server_protocol::HookSource;
use codex_app_server_protocol::HooksListEntry;
use codex_app_server_protocol::HooksListParams;
use codex_app_server_protocol::HooksListResponse;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::MergeStrategy;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_core::config::set_project_trust_level;
use codex_protocol::config_types::TrustLevel;
use codex_utils_absolute_path::AbsolutePathBuf;
use core_test_support::skip_if_windows;
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
hooks = true

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
    let config_path = AbsolutePathBuf::from_absolute_path(std::fs::canonicalize(
        codex_home.path().join("config.toml"),
    )?)?;
    assert_eq!(
        data,
        vec![HooksListEntry {
            cwd: cwd.path().to_path_buf(),
            hooks: vec![HookMetadata {
                key: format!("{}:pre_tool_use:0:0", config_path.as_path().display()),
                event_name: HookEventName::PreToolUse,
                handler_type: HookHandlerType::Command,
                matcher: Some("Bash".to_string()),
                command: Some("python3 /tmp/listed-hook.py".to_string()),
                timeout_sec: 5,
                status_message: Some("running listed hook".to_string()),
                source_path: config_path,
                source: HookSource::User,
                plugin_id: None,
                display_order: 0,
                enabled: true,
                is_managed: false,
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
    let plugin_hooks_path = AbsolutePathBuf::from_absolute_path(std::fs::canonicalize(
        codex_home
            .path()
            .join("plugins/cache/test/demo/local/hooks/hooks.json"),
    )?)?;
    assert_eq!(
        data,
        vec![HooksListEntry {
            cwd: cwd.path().to_path_buf(),
            hooks: vec![HookMetadata {
                key: "demo@test:hooks/hooks.json:pre_tool_use:0:0".to_string(),
                event_name: HookEventName::PreToolUse,
                handler_type: HookHandlerType::Command,
                matcher: Some("Bash".to_string()),
                command: Some("echo plugin hook".to_string()),
                timeout_sec: 7,
                status_message: Some("running plugin hook".to_string()),
                source_path: plugin_hooks_path,
                source: HookSource::Plugin,
                plugin_id: Some("demo@test".to_string()),
                display_order: 0,
                enabled: true,
                is_managed: false,
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
hooks = false
"#,
    )?;
    std::fs::create_dir_all(workspace.path().join(".git"))?;
    std::fs::create_dir_all(workspace.path().join(".codex"))?;
    std::fs::write(
        workspace.path().join(".codex/config.toml"),
        r#"[features]
hooks = true

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
    let project_config_path =
        AbsolutePathBuf::try_from(workspace.path().join(".codex/config.toml"))?;
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
                    key: format!(
                        "{}:pre_tool_use:0:0",
                        project_config_path.as_path().display()
                    ),
                    event_name: HookEventName::PreToolUse,
                    handler_type: HookHandlerType::Command,
                    matcher: Some("Bash".to_string()),
                    command: Some("echo project hook".to_string()),
                    timeout_sec: 5,
                    status_message: None,
                    source_path: project_config_path,
                    source: HookSource::Project,
                    plugin_id: None,
                    display_order: 0,
                    enabled: true,
                    is_managed: false,
                }],
                warnings: Vec::new(),
                errors: Vec::new(),
            },
        ]
    );
    Ok(())
}

#[tokio::test]
async fn config_batch_write_toggles_user_hook() -> Result<()> {
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
    let hook = &data[0].hooks[0];
    assert_eq!(hook.enabled, true);

    let write_id = mcp
        .send_config_batch_write_request(ConfigBatchWriteParams {
            edits: vec![ConfigEdit {
                key_path: "hooks.state".to_string(),
                value: serde_json::json!({
                    hook.key.clone(): {
                        "enabled": false
                    }
                }),
                merge_strategy: MergeStrategy::Upsert,
            }],
            file_path: None,
            expected_version: None,
            reload_user_config: true,
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(write_id)),
    )
    .await??;
    let _: codex_app_server_protocol::ConfigWriteResponse = to_response(response)?;

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
    assert_eq!(data[0].hooks.len(), 1);
    assert_eq!(data[0].hooks[0].key, hook.key);
    assert_eq!(data[0].hooks[0].enabled, false);

    let write_id = mcp
        .send_config_batch_write_request(ConfigBatchWriteParams {
            edits: vec![ConfigEdit {
                key_path: "hooks.state".to_string(),
                value: serde_json::json!({
                    hook.key.clone(): {
                        "enabled": true
                    }
                }),
                merge_strategy: MergeStrategy::Upsert,
            }],
            file_path: None,
            expected_version: None,
            reload_user_config: true,
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(write_id)),
    )
    .await??;
    let _: codex_app_server_protocol::ConfigWriteResponse = to_response(response)?;

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
    assert_eq!(data[0].hooks[0].enabled, true);
    Ok(())
}

#[tokio::test]
async fn config_batch_write_disables_hook_for_loaded_session() -> Result<()> {
    skip_if_windows!(Ok(()));

    let responses = vec![
        create_final_assistant_message_sse_response("Warmup")?,
        create_final_assistant_message_sse_response("First turn")?,
        create_final_assistant_message_sse_response("Second turn")?,
    ];
    let server = create_mock_responses_server_sequence_unchecked(responses).await;
    let codex_home = TempDir::new()?;
    let hook_script_path = codex_home.path().join("user_prompt_submit_hook.py");
    let hook_log_path = codex_home.path().join("user_prompt_submit_hook_log.jsonl");
    std::fs::write(
        &hook_script_path,
        format!(
            r#"import json
from pathlib import Path
import sys

payload = json.load(sys.stdin)
with Path(r"{hook_log_path}").open("a", encoding="utf-8") as handle:
    handle.write(json.dumps(payload) + "\n")
"#,
            hook_log_path = hook_log_path.display(),
        ),
    )?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "read-only"

model_provider = "mock_provider"

[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0

[hooks]

[[hooks.UserPromptSubmit]]

[[hooks.UserPromptSubmit.hooks]]
type = "command"
command = "python3 {hook_script_path}"
"#,
            server_uri = server.uri(),
            hook_script_path = hook_script_path.display(),
        ),
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let hook_list_id = mcp
        .send_hooks_list_request(HooksListParams {
            cwds: vec![codex_home.path().to_path_buf()],
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(hook_list_id)),
    )
    .await??;
    let HooksListResponse { data } = to_response(response)?;
    let hook = &data[0].hooks[0];
    assert_eq!(hook.enabled, true);

    let thread_start_id = mcp
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(thread_start_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response(response)?;

    let first_turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![V2UserInput::Text {
                text: "first turn".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(first_turn_id)),
    )
    .await??;
    timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    assert_eq!(
        std::fs::read_to_string(&hook_log_path)?
            .lines()
            .filter(|line| !line.is_empty())
            .count(),
        1
    );

    let write_id = mcp
        .send_config_batch_write_request(ConfigBatchWriteParams {
            edits: vec![ConfigEdit {
                key_path: "hooks.state".to_string(),
                value: serde_json::json!({
                    hook.key.clone(): {
                        "enabled": false
                    }
                }),
                merge_strategy: MergeStrategy::Upsert,
            }],
            file_path: None,
            expected_version: None,
            reload_user_config: true,
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(write_id)),
    )
    .await??;
    let _: codex_app_server_protocol::ConfigWriteResponse = to_response(response)?;

    let second_turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id,
            input: vec![V2UserInput::Text {
                text: "second turn".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(second_turn_id)),
    )
    .await??;
    timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    assert_eq!(
        std::fs::read_to_string(&hook_log_path)?
            .lines()
            .filter(|line| !line.is_empty())
            .count(),
        1
    );
    Ok(())
}
