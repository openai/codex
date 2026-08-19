use std::time::Duration;

use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence_unchecked;
use codex_app_server_protocol::ConfigBatchWriteParams;
use codex_app_server_protocol::ConfigEdit;
use codex_app_server_protocol::HookEventName;
use codex_app_server_protocol::HookHandlerMetadata;
use codex_app_server_protocol::HookMetadata;
use codex_app_server_protocol::HookSource;
use codex_app_server_protocol::HookTrustStatus;
use codex_app_server_protocol::HooksListEntry;
use codex_app_server_protocol::HooksListParams;
use codex_app_server_protocol::HooksListResponse;
use codex_app_server_protocol::MergeStrategy;
use codex_app_server_protocol::PluginInstallParams;
use codex_app_server_protocol::PluginInstallResponse;
use codex_app_server_protocol::ThreadArchiveParams;
use codex_app_server_protocol::ThreadArchiveResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_core::config::set_project_trust_level;
use codex_features::Feature;
use codex_protocol::config_types::TrustLevel;
use codex_utils_absolute_path::AbsolutePathBuf;
use core_test_support::skip_if_host_windows;
use core_test_support::skip_if_remote;
use pretty_assertions::assert_eq;
use serde::Serialize;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Serialize)]
struct NormalizedHookIdentity {
    event_name: &'static str,
    #[serde(flatten)]
    group: codex_config::MatcherGroup,
}

fn command_hook_hash(
    event_name: &'static str,
    matcher: Option<&str>,
    command: &str,
    timeout_sec: u64,
    r#async: bool,
    status_message: Option<&str>,
    additional_context_limit: Option<usize>,
) -> String {
    let identity = NormalizedHookIdentity {
        event_name,
        group: codex_config::MatcherGroup {
            matcher: matcher.map(ToOwned::to_owned),
            hooks: vec![codex_config::HookHandlerConfig::Command {
                command: command.to_string(),
                command_windows: None,
                timeout_sec: Some(timeout_sec),
                r#async,
                status_message: status_message.map(ToOwned::to_owned),
                additional_context_limit,
            }],
        },
    };
    let Ok(value) = codex_config::TomlValue::try_from(identity) else {
        unreachable!("normalized hook identity should serialize to TOML");
    };
    codex_config::version_for_toml(&value)
}

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
async = true
statusMessage = "running listed hook"
additionalContextLimit = 4096
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
hooks = true

[plugins."demo@test"]
enabled = true
"#,
    )?;
    Ok(())
}

fn write_versioned_plugin_hook(
    plugin_root: &std::path::Path,
    version: &str,
    hook_log_path: &std::path::Path,
) -> Result<()> {
    std::fs::create_dir_all(plugin_root.join(".codex-plugin"))?;
    std::fs::create_dir_all(plugin_root.join("hooks"))?;
    std::fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        format!(r#"{{"name":"demo","version":"{version}"}}"#),
    )?;
    std::fs::write(
        plugin_root.join("hooks/hooks.json"),
        r#"{
  "hooks": {
    "UserPromptSubmit": [{
      "hooks": [{
        "type": "command",
        "command": "python3 ${PLUGIN_ROOT}/hooks/log_version.py"
      }]
    }],
    "SessionEnd": [{
      "hooks": [{
        "type": "command",
        "command": "python3 ${PLUGIN_ROOT}/hooks/log_version.py"
      }]
    }]
  }
}"#,
    )?;
    std::fs::write(
        plugin_root.join("hooks/log_version.py"),
        format!(
            r#"from pathlib import Path
import os

with Path(r"{hook_log_path}").open("a", encoding="utf-8") as handle:
    handle.write(Path(os.environ["PLUGIN_ROOT"]).name + "\n")
"#,
            hook_log_path = hook_log_path.display(),
        ),
    )?;
    Ok(())
}

fn write_project_hook_config(dot_codex_folder: &std::path::Path, command: &str) -> Result<()> {
    std::fs::create_dir_all(dot_codex_folder)?;
    std::fs::write(
        dot_codex_folder.join("config.toml"),
        format!(
            r#"[features]
hooks = true

[hooks]

[[hooks.PreToolUse]]
matcher = "Bash"

[[hooks.PreToolUse.hooks]]
type = "command"
command = "{command}"
timeout = 5
"#
        ),
    )?;
    Ok(())
}

#[tokio::test]
async fn hooks_list_shows_discovered_hook() -> Result<()> {
    let codex_home = TempDir::new()?;
    let cwd = TempDir::new()?;
    write_user_hook_config(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_hooks_list_request(HooksListParams {
            cwds: vec![cwd.path().to_path_buf()],
        })
        .await?;
    let HooksListResponse { data } =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
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
                handler: HookHandlerMetadata::Command {
                    command: "python3 /tmp/listed-hook.py".to_string(),
                    r#async: true,
                },
                matcher: Some("Bash".to_string()),
                timeout_sec: 5,
                status_message: Some("running listed hook".to_string()),
                additional_context_limit: Some(4_096),
                source_path: config_path,
                source: HookSource::User,
                plugin_id: None,
                display_order: 0,
                enabled: true,
                is_managed: false,
                current_hash: command_hook_hash(
                    "pre_tool_use",
                    Some("Bash"),
                    "python3 /tmp/listed-hook.py",
                    /*timeout_sec*/ 5,
                    /*async*/ true,
                    Some("running listed hook"),
                    /*additional_context_limit*/ Some(4_096),
                ),
                trust_status: HookTrustStatus::Untrusted,
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

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_hooks_list_request(HooksListParams {
            cwds: vec![cwd.path().to_path_buf()],
        })
        .await?;
    let HooksListResponse { data } =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
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
                handler: HookHandlerMetadata::Command {
                    command: "echo plugin hook".to_string(),
                    r#async: false,
                },
                matcher: Some("Bash".to_string()),
                timeout_sec: 7,
                status_message: Some("running plugin hook".to_string()),
                additional_context_limit: None,
                source_path: plugin_hooks_path,
                source: HookSource::Plugin,
                plugin_id: Some("demo@test".to_string()),
                display_order: 0,
                enabled: true,
                is_managed: false,
                current_hash: command_hook_hash(
                    "pre_tool_use",
                    Some("Bash"),
                    "echo plugin hook",
                    /*timeout_sec*/ 7,
                    /*async*/ false,
                    Some("running plugin hook"),
                    /*additional_context_limit*/ None,
                ),
                trust_status: HookTrustStatus::Untrusted,
            }],
            warnings: Vec::new(),
            errors: Vec::new(),
        }]
    );
    Ok(())
}

#[tokio::test]
async fn plugin_upgrade_refreshes_hook_runtime_for_loaded_session() -> Result<()> {
    skip_if_host_windows!(Ok(()));
    skip_if_remote!(Ok(()), "command hooks use host-local script and log paths");

    let responses = vec![
        create_final_assistant_message_sse_response("Warmup")?,
        create_final_assistant_message_sse_response("Version 1")?,
        create_final_assistant_message_sse_response("Version 2")?,
    ];
    let server = create_mock_responses_server_sequence_unchecked(responses).await;
    let codex_home = TempDir::new()?;
    let marketplace = TempDir::new()?;
    let plugin_source = marketplace.path().join("demo");
    let hook_log_path = codex_home.path().join("plugin-hook-versions.log");

    std::fs::create_dir_all(marketplace.path().join(".git"))?;
    std::fs::create_dir_all(marketplace.path().join(".agents/plugins"))?;
    std::fs::write(
        marketplace.path().join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "test",
  "plugins": [{
    "name": "demo",
    "source": { "source": "local", "path": "./demo" }
  }]
}"#,
    )?;
    write_versioned_plugin_hook(&plugin_source, "1", &hook_log_path)?;
    MockResponsesConfig::new(&server.uri())
        .enable_feature(Feature::Plugins)
        .enable_feature(Feature::CodexHooks)
        .write(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;
    let marketplace_path =
        AbsolutePathBuf::try_from(marketplace.path().join(".agents/plugins/marketplace.json"))?;
    let install_id = mcp
        .send_plugin_install_request(PluginInstallParams {
            marketplace_path: Some(marketplace_path.clone()),
            remote_marketplace_name: None,
            install_attempt_id: None,
            plugin_name: "demo".to_string(),
        })
        .await?;
    let _: PluginInstallResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(install_id)).await??;

    let hooks_list_id = mcp
        .send_hooks_list_request(HooksListParams {
            cwds: vec![codex_home.path().to_path_buf()],
        })
        .await?;
    let HooksListResponse { data } =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(hooks_list_id)).await??;
    let hook = data[0]
        .hooks
        .iter()
        .find(|hook| hook.event_name == HookEventName::UserPromptSubmit)
        .expect("plugin should register a user-prompt hook");
    let trusted_hooks = data[0]
        .hooks
        .iter()
        .map(|hook| {
            (
                hook.key.clone(),
                serde_json::json!({ "trusted_hash": hook.current_hash }),
            )
        })
        .collect::<serde_json::Map<String, serde_json::Value>>();
    let trust_id = mcp
        .send_config_batch_write_request(ConfigBatchWriteParams {
            edits: vec![ConfigEdit {
                key_path: "hooks.state".to_string(),
                value: serde_json::Value::Object(trusted_hooks),
                merge_strategy: MergeStrategy::Upsert,
            }],
            file_path: None,
            expected_version: None,
            reload_user_config: true,
        })
        .await?;
    let _: codex_app_server_protocol::ConfigWriteResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(trust_id)).await??;

    let thread_start_id = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(thread_start_id)).await??;
    let first_turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
            input: vec![V2UserInput::Text {
                text: "run version 1".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _: TurnStartResponse = timeout(DEFAULT_TIMEOUT, mcp.read_response(first_turn_id)).await??;
    timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    assert_eq!(std::fs::read_to_string(&hook_log_path)?, "1\n");

    let staged_write_id = mcp
        .send_config_batch_write_request(ConfigBatchWriteParams {
            edits: vec![ConfigEdit {
                key_path: "hooks.state".to_string(),
                value: serde_json::json!({
                    hook.key.clone(): {
                        "enabled": false,
                    },
                }),
                merge_strategy: MergeStrategy::Upsert,
            }],
            file_path: None,
            expected_version: None,
            reload_user_config: false,
        })
        .await?;
    let _: codex_app_server_protocol::ConfigWriteResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(staged_write_id)).await??;

    std::fs::write(
        plugin_source.join(".codex-plugin/plugin.json"),
        r#"{"name":"demo","version":"2"}"#,
    )?;
    let install_id = mcp
        .send_plugin_install_request(PluginInstallParams {
            marketplace_path: Some(marketplace_path.clone()),
            remote_marketplace_name: None,
            install_attempt_id: None,
            plugin_name: "demo".to_string(),
        })
        .await?;
    let _: PluginInstallResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(install_id)).await??;
    assert!(!codex_home.path().join("plugins/cache/test/demo/1").exists());

    let second_turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
            input: vec![V2UserInput::Text {
                text: "run version 2".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _: TurnStartResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(second_turn_id)).await??;
    timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    assert_eq!(std::fs::read_to_string(&hook_log_path)?, "1\n2\n");

    let hooks_list_id = mcp
        .send_hooks_list_request(HooksListParams {
            cwds: vec![codex_home.path().to_path_buf()],
        })
        .await?;
    let HooksListResponse { data } =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(hooks_list_id)).await??;
    let expected_hook_path = codex_home
        .path()
        .join("plugins/cache/test/demo/2/hooks/log_version.py")
        .canonicalize()?;
    let hook = data[0]
        .hooks
        .iter()
        .find(|hook| hook.event_name == HookEventName::UserPromptSubmit)
        .expect("plugin should register a user-prompt hook");
    assert_eq!(
        hook.handler,
        HookHandlerMetadata::Command {
            command: format!("python3 {}", expected_hook_path.display()),
            r#async: false,
        }
    );
    assert!(!hook.enabled);

    std::fs::write(
        plugin_source.join(".codex-plugin/plugin.json"),
        r#"{"name":"demo","version":"3"}"#,
    )?;
    let install_id = mcp
        .send_plugin_install_request(PluginInstallParams {
            marketplace_path: Some(marketplace_path),
            remote_marketplace_name: None,
            install_attempt_id: None,
            plugin_name: "demo".to_string(),
        })
        .await?;
    let _: PluginInstallResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(install_id)).await??;
    assert!(!codex_home.path().join("plugins/cache/test/demo/2").exists());

    let archive_id = mcp
        .send_thread_archive_request(ThreadArchiveParams {
            thread_id: thread.id,
        })
        .await?;
    let _: ThreadArchiveResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(archive_id)).await??;
    assert_eq!(std::fs::read_to_string(&hook_log_path)?, "1\n2\n3\n");

    Ok(())
}

#[tokio::test]
async fn automatic_marketplace_upgrade_refreshes_hook_runtime_for_loaded_session() -> Result<()> {
    skip_if_host_windows!(Ok(()));
    skip_if_remote!(Ok(()), "command hooks use host-local script and log paths");

    let server = create_mock_responses_server_sequence_unchecked(vec![
        create_final_assistant_message_sse_response("Version 2")?,
    ])
    .await;
    let codex_home = TempDir::new()?;
    let marketplace = TempDir::new()?;
    let git_wrapper = TempDir::new()?;
    let hook_log_path = codex_home.path().join("plugin-hook-versions.log");
    let old_plugin_root = codex_home.path().join("plugins/cache/test/demo/1");
    let new_plugin_root = codex_home.path().join("plugins/cache/test/demo/2");
    let upgrade_gate = git_wrapper.path().join("allow-marketplace-upgrade");

    std::fs::create_dir_all(marketplace.path().join(".agents/plugins"))?;
    std::fs::write(
        marketplace.path().join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "test",
  "plugins": [{
    "name": "demo",
    "source": { "source": "local", "path": "./demo" }
  }]
}"#,
    )?;
    write_versioned_plugin_hook(&marketplace.path().join("demo"), "2", &hook_log_path)?;
    write_versioned_plugin_hook(&old_plugin_root, "1", &hook_log_path)?;

    for args in [
        &["init"][..],
        &["config", "user.email", "codex@example.com"],
        &["config", "user.name", "Codex Tests"],
        &["add", "."],
        &["commit", "-m", "install marketplace plugin version 2"],
    ] {
        let output = std::process::Command::new("git")
            .current_dir(marketplace.path())
            .args(args)
            .output()?;
        anyhow::ensure!(
            output.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let original_path =
        std::env::var_os("PATH").ok_or_else(|| anyhow::anyhow!("PATH is required for git"))?;
    let real_git = std::env::split_paths(&original_path)
        .map(|directory| directory.join("git"))
        .find(|path| path.is_file())
        .ok_or_else(|| anyhow::anyhow!("git was not found on PATH"))?;
    let wrapper_path = git_wrapper.path().join("git");
    std::fs::write(
        &wrapper_path,
        format!(
            r#"#!/bin/sh
if [ "$3" = "ls-remote" ] && [ "$4" = "{marketplace}" ]; then
    while [ ! -e "{upgrade_gate}" ]; do
        sleep 0.01
    done
fi
exec "{real_git}" "$@"
"#,
            marketplace = marketplace.path().display(),
            upgrade_gate = upgrade_gate.display(),
            real_git = real_git.display(),
        ),
    )?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = std::fs::metadata(&wrapper_path)?.permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&wrapper_path, permissions)?;
    }
    let path_with_wrapper = std::env::join_paths(
        std::iter::once(git_wrapper.path().to_path_buf())
            .chain(std::env::split_paths(&original_path)),
    )?;
    let path_with_wrapper = path_with_wrapper.to_string_lossy().into_owned();

    MockResponsesConfig::new(&server.uri())
        .enable_feature(Feature::Plugins)
        .enable_feature(Feature::CodexHooks)
        .with_root_config(&format!(
            "chatgpt_base_url = \"{}/backend-api/\"",
            server.uri()
        ))
        .with_extra_config(&format!(
            r#"[plugins."demo@test"]
enabled = true

[marketplaces.test]
source_type = "git"
source = "{}"
"#,
            marketplace.path().display(),
        ))
        .write(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_plugin_startup_tasks()
        .with_env_overrides(&[("PATH", Some(path_with_wrapper.as_str()))])
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;
    let thread_start_id = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("mock-model".to_string()),
            config: Some(std::collections::HashMap::from([(
                "bypass_hook_trust".to_string(),
                serde_json::json!(true),
            )])),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(thread_start_id)).await??;
    assert!(old_plugin_root.exists());
    assert!(!new_plugin_root.exists());

    std::fs::write(&upgrade_gate, "ready")?;
    timeout(DEFAULT_TIMEOUT, async {
        while !new_plugin_root.exists() || old_plugin_root.exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;

    let hooks_list_id = mcp
        .send_hooks_list_request(HooksListParams {
            cwds: vec![codex_home.path().to_path_buf()],
        })
        .await?;
    let HooksListResponse { data } =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(hooks_list_id)).await??;
    let expected_hook_path = new_plugin_root
        .join("hooks/log_version.py")
        .canonicalize()?;
    assert_eq!(
        data[0].hooks[0].handler,
        HookHandlerMetadata::Command {
            command: format!("python3 {}", expected_hook_path.display()),
            r#async: false,
        }
    );

    let turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id,
            client_user_message_id: None,
            input: vec![V2UserInput::Text {
                text: "run version 2".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _: TurnStartResponse = timeout(DEFAULT_TIMEOUT, mcp.read_response(turn_id)).await??;
    timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    assert_eq!(std::fs::read_to_string(&hook_log_path)?, "2\n");

    Ok(())
}

#[tokio::test]
async fn hooks_list_shows_discovered_plugin_mcp_tool_hook() -> Result<()> {
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
            "type": "mcp_tool",
            "server": "security",
            "tool": "inspect",
            "input": {"path": "${tool_input.path}"},
            "timeout": 9,
            "statusMessage": "checking security policy"
          }
        ]
      }
    ]
  }
}"#,
    )?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_hooks_list_request(HooksListParams {
            cwds: vec![cwd.path().to_path_buf()],
        })
        .await?;
    let HooksListResponse { data } =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    let source_path = AbsolutePathBuf::from_absolute_path(std::fs::canonicalize(
        codex_home
            .path()
            .join("plugins/cache/test/demo/local/hooks/hooks.json"),
    )?)?;
    let identity = NormalizedHookIdentity {
        event_name: "pre_tool_use",
        group: codex_config::MatcherGroup {
            matcher: Some("Bash".to_string()),
            hooks: vec![codex_config::HookHandlerConfig::McpTool {
                server: "security".to_string(),
                tool: "inspect".to_string(),
                input: serde_json::from_value(serde_json::json!({
                    "path": "${tool_input.path}",
                }))?,
                timeout_sec: Some(9),
                status_message: Some("checking security policy".to_string()),
            }],
        },
    };
    let identity = codex_config::TomlValue::try_from(identity)?;

    assert_eq!(
        data,
        vec![HooksListEntry {
            cwd: cwd.path().to_path_buf(),
            hooks: vec![HookMetadata {
                key: "demo@test:hooks/hooks.json:pre_tool_use:0:0".to_string(),
                event_name: HookEventName::PreToolUse,
                handler: HookHandlerMetadata::McpTool {
                    server: "security".to_string(),
                    tool: "inspect".to_string(),
                },
                matcher: Some("Bash".to_string()),
                timeout_sec: 9,
                status_message: Some("checking security policy".to_string()),
                additional_context_limit: None,
                source_path,
                source: HookSource::Plugin,
                plugin_id: Some("demo@test".to_string()),
                display_order: 0,
                enabled: true,
                is_managed: false,
                current_hash: codex_config::version_for_toml(&identity),
                trust_status: HookTrustStatus::Untrusted,
            }],
            warnings: Vec::new(),
            errors: Vec::new(),
        }]
    );

    Ok(())
}

#[tokio::test]
async fn hooks_list_warms_plugin_capabilities_for_thread_start() -> Result<()> {
    let codex_home = TempDir::new()?;
    let cwd = TempDir::new()?;
    write_plugin_hook_config(
        codex_home.path(),
        r#"{
  "hooks": {
    "PreToolUse": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "echo plugin hook"
          }
        ]
      }
    ]
  }
}"#,
    )?;
    let plugin_mcp_path = codex_home
        .path()
        .join("plugins/cache/test/demo/local/.mcp.json");
    std::fs::write(
        &plugin_mcp_path,
        r#"{
  "mcpServers": {
    "plugin-server": {
      "url": "http://127.0.0.1:1/mcp"
    }
  }
}"#,
    )?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let hooks_list_id = mcp
        .send_hooks_list_request(HooksListParams {
            cwds: vec![cwd.path().to_path_buf()],
        })
        .await?;
    let _: HooksListResponse = timeout(DEFAULT_TIMEOUT, mcp.read_response(hooks_list_id)).await??;

    std::fs::remove_file(plugin_mcp_path)?;

    let thread_start_id = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams::default())
        .await?;
    let _: ThreadStartResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(thread_start_id)).await??;
    timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_matching_notification("plugin MCP server starting", |notification| {
            notification.method == "mcpServer/startupStatus/updated"
                && notification
                    .params
                    .as_ref()
                    .and_then(|params| params.get("name"))
                    .and_then(serde_json::Value::as_str)
                    == Some("plugin-server")
        }),
    )
    .await??;

    Ok(())
}

#[tokio::test]
async fn hooks_list_shows_plugin_hook_load_warnings() -> Result<()> {
    let codex_home = TempDir::new()?;
    let cwd = TempDir::new()?;
    write_plugin_hook_config(codex_home.path(), "{ not-json")?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_hooks_list_request(HooksListParams {
            cwds: vec![cwd.path().to_path_buf()],
        })
        .await?;
    let HooksListResponse { data } =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;

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

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_hooks_list_request(HooksListParams {
            cwds: vec![
                codex_home.path().to_path_buf(),
                workspace.path().to_path_buf(),
            ],
        })
        .await?;
    let HooksListResponse { data } =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
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
                    handler: HookHandlerMetadata::Command {
                        command: "echo project hook".to_string(),
                        r#async: false,
                    },
                    matcher: Some("Bash".to_string()),
                    timeout_sec: 5,
                    status_message: None,
                    additional_context_limit: None,
                    source_path: project_config_path,
                    source: HookSource::Project,
                    plugin_id: None,
                    display_order: 0,
                    enabled: true,
                    is_managed: false,
                    current_hash: command_hook_hash(
                        "pre_tool_use",
                        Some("Bash"),
                        "echo project hook",
                        /*timeout_sec*/ 5,
                        /*async*/ false,
                        /*status_message*/ None,
                        /*additional_context_limit*/ None,
                    ),
                    trust_status: HookTrustStatus::Untrusted,
                }],
                warnings: Vec::new(),
                errors: Vec::new(),
            },
        ]
    );
    Ok(())
}

#[tokio::test]
async fn hooks_list_uses_root_repo_hooks_for_linked_worktrees() -> Result<()> {
    let codex_home = TempDir::new()?;
    let workspace = TempDir::new()?;
    let repo_root = workspace.path().join("repo");
    let worktree_root = workspace.path().join("worktree");
    let worktree_git_dir = repo_root.join(".git/worktrees/feature-x");

    std::fs::create_dir_all(&worktree_git_dir)?;
    std::fs::create_dir_all(&worktree_root)?;
    std::fs::write(
        worktree_root.join(".git"),
        format!("gitdir: {}\n", worktree_git_dir.display()),
    )?;
    std::fs::write(
        worktree_git_dir.join("gitdir"),
        format!("{}\n", worktree_root.join(".git").display()),
    )?;
    std::fs::write(worktree_git_dir.join("commondir"), "../..\n")?;
    write_project_hook_config(&repo_root.join(".codex"), "echo root hook")?;
    write_project_hook_config(&worktree_root.join(".codex"), "echo worktree hook")?;
    set_project_trust_level(codex_home.path(), &repo_root, TrustLevel::Trusted)?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let list_id = mcp
        .send_hooks_list_request(HooksListParams {
            cwds: vec![repo_root.clone(), worktree_root.clone()],
        })
        .await?;
    let HooksListResponse { data } = timeout(DEFAULT_TIMEOUT, mcp.read_response(list_id)).await??;
    let repo_hook = data[0].hooks[0].clone();
    let worktree_hook = data[1].hooks[0].clone();
    let repo_config_path =
        AbsolutePathBuf::from_absolute_path(repo_root.join(".codex/config.toml"))?;

    assert_eq!(
        repo_hook.handler,
        HookHandlerMetadata::Command {
            command: "echo root hook".to_string(),
            r#async: false,
        }
    );
    assert_eq!(
        worktree_hook.handler,
        HookHandlerMetadata::Command {
            command: "echo root hook".to_string(),
            r#async: false,
        }
    );
    assert_eq!(repo_hook.key, worktree_hook.key);
    assert_eq!(repo_hook.source_path, repo_config_path);
    assert_eq!(worktree_hook.source_path, repo_config_path);

    let write_id = mcp
        .send_config_batch_write_request(ConfigBatchWriteParams {
            edits: vec![ConfigEdit {
                key_path: "hooks.state".to_string(),
                value: serde_json::json!({
                    repo_hook.key.clone(): {
                        "trusted_hash": repo_hook.current_hash.clone()
                    }
                }),
                merge_strategy: MergeStrategy::Upsert,
            }],
            file_path: None,
            expected_version: None,
            reload_user_config: true,
        })
        .await?;
    let _: codex_app_server_protocol::ConfigWriteResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(write_id)).await??;

    let list_id = mcp
        .send_hooks_list_request(HooksListParams {
            cwds: vec![worktree_root],
        })
        .await?;
    let HooksListResponse { data } = timeout(DEFAULT_TIMEOUT, mcp.read_response(list_id)).await??;
    assert_eq!(data[0].hooks[0].trust_status, HookTrustStatus::Trusted);

    Ok(())
}

#[tokio::test]
async fn config_batch_write_toggles_user_hook() -> Result<()> {
    let codex_home = TempDir::new()?;
    let cwd = TempDir::new()?;
    write_user_hook_config(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_hooks_list_request(HooksListParams {
            cwds: vec![cwd.path().to_path_buf()],
        })
        .await?;
    let HooksListResponse { data } =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
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
    let _: codex_app_server_protocol::ConfigWriteResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(write_id)).await??;

    let request_id = mcp
        .send_hooks_list_request(HooksListParams {
            cwds: vec![cwd.path().to_path_buf()],
        })
        .await?;
    let HooksListResponse { data } =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
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
    let _: codex_app_server_protocol::ConfigWriteResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(write_id)).await??;

    let request_id = mcp
        .send_hooks_list_request(HooksListParams {
            cwds: vec![cwd.path().to_path_buf()],
        })
        .await?;
    let HooksListResponse { data } =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;
    assert_eq!(data[0].hooks[0].enabled, true);
    Ok(())
}

#[tokio::test]
async fn config_batch_write_updates_hook_trust_for_loaded_session() -> Result<()> {
    skip_if_host_windows!(Ok(()));
    // TODO(anp): Teach command-hook fixtures to run in selected remote environments.
    skip_if_remote!(Ok(()), "command hooks use host-local script and log paths");

    let responses = vec![
        create_final_assistant_message_sse_response("Warmup")?,
        create_final_assistant_message_sse_response("Untrusted turn")?,
        create_final_assistant_message_sse_response("Trusted turn")?,
        create_final_assistant_message_sse_response("Modified turn")?,
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
    MockResponsesConfig::new(&server.uri())
        .with_extra_config(&format!(
            r#"[hooks]

[[hooks.UserPromptSubmit]]

[[hooks.UserPromptSubmit.hooks]]
type = "command"
command = "python3 {}"
"#,
            hook_script_path.display()
        ))
        .write(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let hook_list_id = mcp
        .send_hooks_list_request(HooksListParams {
            cwds: vec![codex_home.path().to_path_buf()],
        })
        .await?;
    let HooksListResponse { data } =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(hook_list_id)).await??;
    let hook = data[0].hooks[0].clone();
    assert_eq!(hook.trust_status, HookTrustStatus::Untrusted);

    let thread_start_id = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(thread_start_id)).await??;

    let first_turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
            input: vec![V2UserInput::Text {
                text: "first turn".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _: TurnStartResponse = timeout(DEFAULT_TIMEOUT, mcp.read_response(first_turn_id)).await??;
    timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    assert!(!std::fs::exists(&hook_log_path)?);

    let write_id = mcp
        .send_config_batch_write_request(ConfigBatchWriteParams {
            edits: vec![ConfigEdit {
                key_path: "hooks.state".to_string(),
                value: serde_json::json!({
                    hook.key.clone(): {
                        "trusted_hash": hook.current_hash.clone()
                    }
                }),
                merge_strategy: MergeStrategy::Upsert,
            }],
            file_path: None,
            expected_version: None,
            reload_user_config: true,
        })
        .await?;
    let _: codex_app_server_protocol::ConfigWriteResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(write_id)).await??;

    let hook_list_id = mcp
        .send_hooks_list_request(HooksListParams {
            cwds: vec![codex_home.path().to_path_buf()],
        })
        .await?;
    let HooksListResponse { data } =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(hook_list_id)).await??;
    let trusted_hook = &data[0].hooks[0];
    assert_eq!(trusted_hook.key, hook.key);
    assert_eq!(trusted_hook.current_hash, hook.current_hash);
    assert_eq!(trusted_hook.trust_status, HookTrustStatus::Trusted);

    let second_turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
            input: vec![V2UserInput::Text {
                text: "second turn".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _: TurnStartResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(second_turn_id)).await??;
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
                key_path: "hooks.UserPromptSubmit".to_string(),
                value: serde_json::json!([{
                    "hooks": [{
                        "type": "command",
                        "command": format!("python3 {}", hook_script_path.display()),
                        "statusMessage": "modified hook",
                    }],
                }]),
                merge_strategy: MergeStrategy::Replace,
            }],
            file_path: None,
            expected_version: None,
            reload_user_config: true,
        })
        .await?;
    let _: codex_app_server_protocol::ConfigWriteResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(write_id)).await??;

    let hook_list_id = mcp
        .send_hooks_list_request(HooksListParams {
            cwds: vec![codex_home.path().to_path_buf()],
        })
        .await?;
    let HooksListResponse { data } =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(hook_list_id)).await??;
    let modified_hook = &data[0].hooks[0];
    assert_eq!(modified_hook.key, hook.key);
    assert_ne!(modified_hook.current_hash, hook.current_hash);
    assert_eq!(modified_hook.trust_status, HookTrustStatus::Modified);

    let third_turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id,
            client_user_message_id: None,
            input: vec![V2UserInput::Text {
                text: "third turn".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _: TurnStartResponse = timeout(DEFAULT_TIMEOUT, mcp.read_response(third_turn_id)).await??;
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

#[tokio::test]
async fn config_batch_write_disables_hook_for_loaded_session() -> Result<()> {
    skip_if_host_windows!(Ok(()));
    // TODO(anp): Teach command-hook fixtures to run in selected remote environments.
    skip_if_remote!(Ok(()), "command hooks use host-local script and log paths");

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
    MockResponsesConfig::new(&server.uri())
        .with_extra_config(&format!(
            r#"[hooks]

[[hooks.UserPromptSubmit]]

[[hooks.UserPromptSubmit.hooks]]
type = "command"
command = "python3 {}"
"#,
            hook_script_path.display()
        ))
        .write(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let hook_list_id = mcp
        .send_hooks_list_request(HooksListParams {
            cwds: vec![codex_home.path().to_path_buf()],
        })
        .await?;
    let HooksListResponse { data } =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(hook_list_id)).await??;
    let hook = &data[0].hooks[0];
    assert_eq!(hook.enabled, true);

    let write_id = mcp
        .send_config_batch_write_request(ConfigBatchWriteParams {
            edits: vec![ConfigEdit {
                key_path: "hooks.state".to_string(),
                value: serde_json::json!({
                    hook.key.clone(): {
                        "trusted_hash": hook.current_hash.clone()
                    }
                }),
                merge_strategy: MergeStrategy::Upsert,
            }],
            file_path: None,
            expected_version: None,
            reload_user_config: true,
        })
        .await?;
    let _: codex_app_server_protocol::ConfigWriteResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(write_id)).await??;

    let thread_start_id = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(thread_start_id)).await??;

    let first_turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id.clone(),
            client_user_message_id: None,
            input: vec![V2UserInput::Text {
                text: "first turn".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _: TurnStartResponse = timeout(DEFAULT_TIMEOUT, mcp.read_response(first_turn_id)).await??;
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
    let _: codex_app_server_protocol::ConfigWriteResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(write_id)).await??;

    let second_turn_id = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id,
            client_user_message_id: None,
            input: vec![V2UserInput::Text {
                text: "second turn".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _: TurnStartResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(second_turn_id)).await??;
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
