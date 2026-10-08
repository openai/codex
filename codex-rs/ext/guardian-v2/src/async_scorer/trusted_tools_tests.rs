use std::path::Path;

use anyhow::Result;
use codex_extension_api::McpToolInfo;
use codex_extension_api::McpToolSource;
use codex_features::Feature;
use codex_guardian_context::TrustedTool;
use codex_guardian_context::TrustedToolSource;
use codex_login::CodexAuth;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::json;

#[cfg(unix)]
use super::PluginCapability;
#[cfg(unix)]
use super::is_home_owned_path;
#[cfg(unix)]
use super::is_home_owned_plugin_capability;
use super::trusted_tool_context;

fn mcp_tool(server: &str, connector_id: Option<&str>) -> Result<McpToolInfo> {
    Ok(serde_json::from_value(json!({
        "server_name": server,
        "tool_name": "inspect",
        "tool_namespace": format!("mcp__{server}"),
        "namespace_description": "Remote namespace instructions",
        "tool": {
            "name": "inspect",
            "description": "Remote tool instructions",
            "inputSchema": {"type": "object", "properties": {}}
        },
        "connector_id": connector_id,
        "connector_name": connector_id.map(|_| "Remote connector"),
        "plugin_display_names": []
    }))?)
}

fn expected_context(tool: &McpToolInfo, source: &Path) -> TrustedTool {
    TrustedTool {
        server: tool.server_name.clone(),
        connector_id: tool.connector_id.clone(),
        source: TrustedToolSource::UserConfiguration(source.display().to_string()),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn trusts_only_tools_configured_in_codex_home() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let test = test_codex()
        .with_pre_build_hook(|home| {
            std::fs::write(
                home.join("config.toml"),
                "[mcp_servers.home_server]\nurl = \"http://127.0.0.1:9/mcp\"\n\n[apps.connector_home]\nenabled = true\n",
            )
            .expect("write user MCP and connector config");
        })
        .build_with_auto_env(&server)
        .await?;
    for (tool, unrelated, source) in [
        (
            mcp_tool("home_server", /*connector_id*/ None)?,
            mcp_tool("other_server", /*connector_id*/ None)?,
            McpToolSource::Config,
        ),
        (
            mcp_tool("codex_apps", Some("connector_home"))?,
            mcp_tool("codex_apps", Some("connector_other"))?,
            McpToolSource::Connector,
        ),
    ] {
        let context = trusted_tool_context(&tool, &source, &test.thread_manager, &test.config)
            .await
            .expect("home-configured tool should be trusted");
        assert_eq!(
            context,
            expected_context(&tool, &test.home.path().join("config.toml")),
        );
        assert_eq!(
            trusted_tool_context(&unrelated, &source, &test.thread_manager, &test.config).await,
            None,
        );
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rejects_blank_orchestrator_connector_ids() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let test = test_codex().build_with_auto_env(&server).await?;
    let mut config = test.config.clone();
    config
        .features
        .enable(Feature::GuardianTrustOrchestratorConnectors)?;
    for connector_id in [Some(""), Some(" \t\n "), None] {
        let tool = mcp_tool("codex_apps", connector_id)?;
        assert_eq!(
            trusted_tool_context(
                &tool,
                &McpToolSource::Connector,
                &test.thread_manager,
                &config,
            )
            .await,
            None,
        );
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn trusts_connector_declared_by_home_owned_plugin() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let test = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_pre_build_hook(|home| {
            let plugin_root = home.join("plugins/cache/test/trusted/local");
            std::fs::create_dir_all(plugin_root.join(".codex-plugin"))
                .expect("create plugin manifest directory");
            std::fs::write(
                plugin_root.join(".codex-plugin/plugin.json"),
                r#"{"name":"trusted","description":"Trusted plugin instructions"}"#,
            )
            .expect("write plugin manifest");
            std::fs::write(
                plugin_root.join(".app.json"),
                r#"{"apps":{"calendar":{"id":"connector_calendar"}}}"#,
            )
            .expect("write plugin connector declaration");
            std::fs::write(
                plugin_root.join(".mcp.json"),
                r#"{"mcpServers":{"trusted_server":{"url":"http://127.0.0.1:9/mcp"}}}"#,
            )
            .expect("write plugin MCP declaration");
            std::fs::write(
                home.join("config.toml"),
                "[features]\nplugins = true\n\n[plugins.\"trusted@test\"]\nenabled = true\n",
            )
            .expect("write plugin configuration");
        })
        .build_with_auto_env(&server)
        .await?;
    let plugin_root = test
        .home
        .path()
        .join("plugins")
        .join("cache")
        .join("test")
        .join("trusted")
        .join("local");
    let tool = mcp_tool("codex_apps", Some("connector_calendar"))?;
    let context = trusted_tool_context(
        &tool,
        &McpToolSource::Connector,
        &test.thread_manager,
        &test.config,
    )
    .await
    .expect("home-owned plugin connector should be trusted");
    assert_eq!(context, expected_context(&tool, &plugin_root));

    let mcp = mcp_tool("trusted_server", /*connector_id*/ None)?;
    let mcp_context = trusted_tool_context(
        &mcp,
        &McpToolSource::Plugin {
            id: "trusted@test".to_string(),
            root: test
                .config
                .codex_home
                .join("plugins/cache/test/trusted/local")
                .into(),
        },
        &test.thread_manager,
        &test.config,
    )
    .await
    .expect("home-owned plugin MCP server should be trusted");
    assert_eq!(mcp_context, expected_context(&mcp, &plugin_root));
    // A trusted cached plugin with the same ID must not replace the frozen outside-home root.
    assert_eq!(
        trusted_tool_context(
            &mcp,
            &McpToolSource::Plugin {
                id: "trusted@test".to_string(),
                root: test.config.cwd.clone().into(),
            },
            &test.thread_manager,
            &test.config,
        )
        .await,
        None,
    );
    assert_eq!(
        trusted_tool_context(
            &mcp,
            &McpToolSource::SelectedPlugin,
            &test.thread_manager,
            &test.config,
        )
        .await,
        None,
    );

    Ok(())
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rejects_paths_that_escape_codex_home_through_symlinks() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let test = test_codex().build_with_auto_env(&server).await?;
    let link = test.home.path().join("external-plugin");
    std::os::unix::fs::symlink(test.cwd.path(), &link)?;
    let canonical_home = test.home.path().canonicalize()?;

    assert!(!is_home_owned_path(&link, &canonical_home));

    let plugin_root = test.home.path().join("trusted-plugin");
    std::fs::create_dir_all(plugin_root.join(".codex-plugin"))?;
    std::fs::write(
        plugin_root.join(".codex-plugin").join("plugin.json"),
        r#"{"name":"trusted"}"#,
    )?;
    let outside_apps = test.cwd.path().join("outside-app.json");
    let outside_mcp = test.cwd.path().join("outside-mcp.json");
    std::fs::write(&outside_apps, r#"{"apps":{}}"#)?;
    std::fs::write(&outside_mcp, r#"{"mcpServers":{}}"#)?;
    std::os::unix::fs::symlink(&outside_apps, plugin_root.join(".app.json"))?;
    std::os::unix::fs::symlink(&outside_mcp, plugin_root.join(".mcp.json"))?;

    assert!(!is_home_owned_plugin_capability(
        &plugin_root,
        &canonical_home,
        PluginCapability::Connector,
    ));
    assert!(!is_home_owned_plugin_capability(
        &plugin_root,
        &canonical_home,
        PluginCapability::Mcp,
    ));

    Ok(())
}
