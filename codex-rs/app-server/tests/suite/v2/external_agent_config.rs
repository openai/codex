use std::time::Duration;

use anyhow::Result;
use app_test_support::McpProcess;
use app_test_support::to_response;
use codex_app_server_protocol::ExternalAgentConfigImportResponse;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::PluginListParams;
use codex_app_server_protocol::PluginListResponse;
use codex_app_server_protocol::RequestId;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::timeout;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

#[tokio::test]
async fn external_agent_config_import_returns_completed_for_local_plugins() -> Result<()> {
    let codex_home = TempDir::new()?;
    let marketplace_root = codex_home.path().join("marketplace");
    let plugin_root = marketplace_root.join("plugins").join("sample");
    std::fs::create_dir_all(marketplace_root.join(".agents/plugins"))?;
    std::fs::create_dir_all(plugin_root.join(".codex-plugin"))?;
    std::fs::write(
        marketplace_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "debug",
  "plugins": [
    {
      "name": "sample",
      "source": {
        "source": "local",
        "path": "./plugins/sample"
      }
    }
  ]
}"#,
    )?;
    std::fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        r#"{"name":"sample","version":"0.1.0"}"#,
    )?;
    std::fs::create_dir_all(codex_home.path().join(".claude"))?;
    std::fs::write(
        codex_home.path().join(".claude").join("settings.json"),
        format!(
            r#"{{
  "enabledPlugins": {{
    "sample@debug": true
  }},
  "extraKnownMarketplaces": {{
    "debug": {{
      "source": "local",
      "path": "{}"
    }}
  }}
}}"#,
            marketplace_root.display()
        ),
    )?;

    let home_dir = codex_home.path().display().to_string();
    let mut mcp =
        McpProcess::new_with_env(codex_home.path(), &[("HOME", Some(home_dir.as_str()))]).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/import",
            Some(serde_json::json!({
                "migrationItems": [{
                    "itemType": "PLUGINS",
                    "description": "Import plugins",
                    "cwd": null,
                    "details": {
                        "plugins": [{
                            "marketplaceName": "debug",
                            "pluginNames": ["sample"]
                        }]
                    }
                }]
            })),
        )
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: ExternalAgentConfigImportResponse = to_response(response)?;

    assert_eq!(response, ExternalAgentConfigImportResponse {});
    let request_id = mcp
        .send_plugin_list_request(PluginListParams {
            cwds: None,
            force_remote_sync: false,
        })
        .await?;
    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: PluginListResponse = to_response(response)?;
    let plugin = response
        .marketplaces
        .iter()
        .find(|marketplace| marketplace.name == "debug")
        .and_then(|marketplace| {
            marketplace
                .plugins
                .iter()
                .find(|plugin| plugin.name == "sample")
        })
        .expect("expected imported plugin to be listed");
    assert!(plugin.installed);
    assert!(plugin.enabled);
    Ok(())
}

#[tokio::test]
async fn external_agent_config_import_sends_completion_notification_for_pending_plugins()
-> Result<()> {
    let codex_home = TempDir::new()?;
    std::fs::create_dir_all(codex_home.path().join(".claude"))?;
    std::fs::write(
        codex_home.path().join(".claude").join("settings.json"),
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
    )?;

    let home_dir = codex_home.path().display().to_string();
    let mut mcp =
        McpProcess::new_with_env(codex_home.path(), &[("HOME", Some(home_dir.as_str()))]).await?;
    timeout(DEFAULT_TIMEOUT, mcp.initialize()).await??;

    let request_id = mcp
        .send_raw_request(
            "externalAgentConfig/import",
            Some(serde_json::json!({
                "migrationItems": [{
                    "itemType": "PLUGINS",
                    "description": "Import plugins",
                    "cwd": null,
                    "details": {
                        "plugins": [{
                            "marketplaceName": "acme-tools",
                            "pluginNames": ["formatter"]
                        }]
                    }
                }]
            })),
        )
        .await?;

    let response: JSONRPCResponse = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_response_message(RequestId::Integer(request_id)),
    )
    .await??;
    let response: ExternalAgentConfigImportResponse = to_response(response)?;
    assert_eq!(response, ExternalAgentConfigImportResponse {});

    Ok(())
}
