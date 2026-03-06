use std::time::Duration;

use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::PluginListParams;
use codex_app_server_protocol::PluginListResponse;
use codex_app_server_protocol::RequestId;
use codex_core::config::set_project_trust_level;
use codex_protocol::config_types::TrustLevel;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn plugin_list_returns_invalid_request_for_invalid_marketplace_file() -> Result<()> {
    let codex_home = TempDir::new()?;
    let repo_root = TempDir::new()?;
    std::fs::create_dir_all(repo_root.path().join(".git"))?;
    std::fs::create_dir_all(repo_root.path().join(".agents/plugins"))?;
    std::fs::write(
        repo_root.path().join(".agents/plugins/marketplace.json"),
        "{not json",
    )?;

    let home = codex_home.path().to_string_lossy().into_owned();
    let mut mcp = McpProcess::new_with_env(
        codex_home.path(),
        &[
            ("HOME", Some(home.as_str())),
            ("USERPROFILE", Some(home.as_str())),
        ],
    )
    .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_list_request(PluginListParams {
            cwds: Some(vec![AbsolutePathBuf::try_from(repo_root.path())?]),
        })
        .await?;

    let err = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(err.error.code, -32600);
    assert!(err.error.message.contains("invalid marketplace file"));
    Ok(())
}

#[tokio::test]
async fn plugin_list_rejects_relative_cwds() -> Result<()> {
    let codex_home = TempDir::new()?;
    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_raw_request(
            "plugin/list",
            Some(serde_json::json!({
                "cwds": ["relative-root"],
            })),
        )
        .await?;

    let err = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(err.error.code, -32600);
    assert!(err.error.message.contains("Invalid request"));
    Ok(())
}

#[tokio::test]
async fn plugin_list_accepts_omitted_cwds() -> Result<()> {
    let codex_home = TempDir::new()?;
    std::fs::create_dir_all(codex_home.path().join(".agents/plugins"))?;
    std::fs::write(
        codex_home.path().join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "home-plugin",
      "source": {
        "source": "local",
        "path": "./home-plugin"
      }
    }
  ]
}"#,
    )?;
    let home = codex_home.path().to_string_lossy().into_owned();
    let mut mcp = McpProcess::new_with_env(
        codex_home.path(),
        &[
            ("HOME", Some(home.as_str())),
            ("USERPROFILE", Some(home.as_str())),
        ],
    )
    .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_list_request(PluginListParams { cwds: None })
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let _: PluginListResponse = to_response(response)?;
    Ok(())
}

#[tokio::test]
async fn plugin_list_includes_enabled_state_from_config() -> Result<()> {
    let codex_home = TempDir::new()?;
    let repo_root = TempDir::new()?;
    std::fs::create_dir_all(repo_root.path().join(".git"))?;
    std::fs::create_dir_all(repo_root.path().join(".agents/plugins"))?;
    std::fs::write(
        repo_root.path().join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "enabled-plugin",
      "source": {
        "source": "local",
        "path": "./enabled-plugin"
      }
    },
    {
      "name": "disabled-plugin",
      "source": {
        "source": "local",
        "path": "./disabled-plugin"
      }
    }
  ]
}"#,
    )?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        r#"[features]
plugins = true

[plugins."enabled-plugin@codex-curated"]
enabled = true

[plugins."disabled-plugin@codex-curated"]
enabled = false
"#,
    )?;

    let mut mcp = McpProcess::new(codex_home.path()).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_list_request(PluginListParams {
            cwds: Some(vec![AbsolutePathBuf::try_from(repo_root.path())?]),
        })
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: PluginListResponse = to_response(response)?;

    let marketplace = response
        .marketplaces
        .into_iter()
        .find(|marketplace| {
            marketplace.path == repo_root.path().join(".agents/plugins/marketplace.json")
        })
        .expect("expected repo marketplace entry");

    assert_eq!(marketplace.name, "codex-curated");
    assert_eq!(marketplace.plugins.len(), 2);
    assert_eq!(marketplace.plugins[0].name, "enabled-plugin");
    assert_eq!(marketplace.plugins[0].enabled, true);
    assert_eq!(marketplace.plugins[1].name, "disabled-plugin");
    assert_eq!(marketplace.plugins[1].enabled, false);
    Ok(())
}

#[tokio::test]
async fn plugin_list_uses_home_config_for_enabled_state() -> Result<()> {
    let codex_home = TempDir::new()?;
    std::fs::create_dir_all(codex_home.path().join(".agents/plugins"))?;
    std::fs::write(
        codex_home.path().join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "shared-plugin",
      "source": {
        "source": "local",
        "path": "./shared-plugin"
      }
    }
  ]
}"#,
    )?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        r#"[features]
plugins = true

[plugins."shared-plugin@codex-curated"]
enabled = true
"#,
    )?;

    let workspace_enabled = TempDir::new()?;
    std::fs::create_dir_all(workspace_enabled.path().join(".git"))?;
    std::fs::create_dir_all(workspace_enabled.path().join(".agents/plugins"))?;
    std::fs::write(
        workspace_enabled
            .path()
            .join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "codex-curated",
  "plugins": [
    {
      "name": "shared-plugin",
      "source": {
        "source": "local",
        "path": "./shared-plugin"
      }
    }
  ]
}"#,
    )?;
    std::fs::create_dir_all(workspace_enabled.path().join(".codex"))?;
    std::fs::write(
        workspace_enabled.path().join(".codex/config.toml"),
        r#"[plugins."shared-plugin@codex-curated"]
enabled = false
"#,
    )?;
    set_project_trust_level(
        codex_home.path(),
        workspace_enabled.path(),
        TrustLevel::Trusted,
    )?;

    let workspace_default = TempDir::new()?;
    let home = codex_home.path().to_string_lossy().into_owned();
    let mut mcp = McpProcess::new_with_env(
        codex_home.path(),
        &[
            ("HOME", Some(home.as_str())),
            ("USERPROFILE", Some(home.as_str())),
        ],
    )
    .await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_plugin_list_request(PluginListParams {
            cwds: Some(vec![
                AbsolutePathBuf::try_from(workspace_enabled.path())?,
                AbsolutePathBuf::try_from(workspace_default.path())?,
            ]),
        })
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: PluginListResponse = to_response(response)?;

    let shared_plugin = response
        .marketplaces
        .iter()
        .flat_map(|marketplace| marketplace.plugins.iter())
        .find(|plugin| plugin.name == "shared-plugin")
        .expect("expected shared-plugin entry");
    assert_eq!(shared_plugin.enabled, true);
    Ok(())
}
