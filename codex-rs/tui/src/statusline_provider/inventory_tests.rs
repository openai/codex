#![cfg(test)]
//! Verify global inventory against the connected app server and its configuration.

use super::super::test_support::reply;
use super::super::test_support::row;
use super::super::test_support::test_client;
use super::*;
use crate::AppServerTarget;
use crate::backend_id::BackendId;
use crate::legacy_core::config::ConfigBuilder;
use crate::legacy_core::config::ConfigOverrides;
use codex_config::LoaderOverrides;
use codex_exec_server::EnvironmentManager;
use futures::TryStreamExt;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn global_inventory_ignores_the_startup_projects_overrides() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let home = temp.path().join("home");
    let project = temp.path().join("project");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(project.join(".git"))?;
    std::fs::write(project.join(".git/HEAD"), "ref: refs/heads/main\n")?;
    std::fs::create_dir(project.join(".codex"))?;
    let package = home.join("plugins/cache/local/daily/local");
    std::fs::create_dir_all(package.join(".codex-plugin"))?;
    std::fs::write(
        package.join(".codex-plugin/plugin.json"),
        r#"{"name":"daily"}"#,
    )?;
    let missing = temp.path().join("missing-mcp-fixture");
    std::fs::write(
        package.join(".mcp.json"),
        json!({"mcpServers":{
            "implicit-calendar":{"command":missing}
        }})
        .to_string(),
    )?;
    let command = toml::Value::String(missing.to_string_lossy().into_owned());
    let replacement = toml::Value::String(
        temp.path()
            .join("missing-project-fixture")
            .to_string_lossy()
            .into_owned(),
    );
    let trusted = toml::Value::String(project.to_string_lossy().into_owned());
    std::fs::write(
        home.join("config.toml"),
        format!(
            r#"
[projects.{trusted}]
trust_level = "trusted"
[features]
plugins = true
remote_plugin = false
[plugins."daily@local"]
enabled = true
[mcp_servers.calendar]
command = {command}
"#
        ),
    )?;
    std::fs::write(
        project.join(".codex/config.toml"),
        format!(
            r#"
[mcp_servers.calendar]
command = {replacement}
[mcp_servers.project_only]
command = {replacement}
[mcp_servers.implicit-calendar]
command = {replacement}
"#
        ),
    )?;
    let config = ConfigBuilder::default()
        .codex_home(home.clone())
        .harness_overrides(ConfigOverrides {
            cwd: Some(project.clone()),
            ..Default::default()
        })
        .build()
        .await?;
    assert!(config.mcp_servers.get().contains_key("project_only"));
    let session = Box::pin(crate::start_app_server_for_picker(
        BackendId::new(),
        &config,
        &AppServerTarget::Embedded,
        vec![("mcp_servers.cli-weather.command".into(), command)],
        LoaderOverrides::without_managed_config_for_tests(),
        /*state_db*/ None,
        Arc::new(EnvironmentManager::default_for_tests()),
    ))
    .await?;
    let client = session.request_handle();
    let servers = global_pages(&client).try_concat().await?;
    let daily = PluginId::parse("daily@local")?;
    let identities = servers
        .iter()
        .map(|s| (s.name.as_str(), s.plugin_id.as_ref()))
        .collect::<Vec<_>>();
    assert_eq!(
        identities,
        [
            ("calendar", None),
            ("cli-weather", None),
            ("implicit-calendar", Some(&daily))
        ]
    );
    session.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn global_pages_preserve_catalogs_and_skip_hosted_or_unidentified_transports() -> Result<()> {
    let (client, mut commands) = test_client();
    let host = async {
        let tool = json!({"name":"forecast","inputSchema":{}});
        let direct = row("cli-weather", vec![tool.clone()]);
        let mut plugin = row("planner", vec![tool]);
        plugin["pluginId"] = json!("work@local");
        let mut hosted = row("hosted", vec![]);
        hosted["httpOrigin"] = json!("https://example.test");
        let mut older = row("missing-origin", vec![]);
        older.as_object_mut().unwrap().remove("httpOrigin");
        let mut failed = row("failed", vec![]);
        failed["runtimeStatus"] = Value::Null;
        failed["toolsError"] = json!("connection refused");
        for (cursor, rows, next) in [
            (None, vec![direct, hosted, older], Some("next")),
            (Some("next"), vec![plugin, failed], None),
        ] {
            reply(
                &mut commands,
                "mcpServerStatus/list",
                json!({"threadId":null,"serverName":null,"environmentId":null,"detail":"toolsAndAuthOnly","cursor":cursor,"limit":null}),
                json!({"data":rows,"nextCursor":next}),
            ).await;
        }
    };
    let (servers, ()) = tokio::join!(global_pages(&client).try_concat(), host);
    let catalogs = servers?
        .into_iter()
        .map(|s| {
            (
                s.name,
                s.plugin_id,
                s.tools.map(|tools| tools.into_keys().collect::<Vec<_>>()),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        catalogs,
        [
            ("cli-weather".into(), None, Ok(vec!["forecast".to_owned()])),
            (
                "planner".into(),
                Some(PluginId::parse("work@local")?),
                Ok(vec!["forecast".to_owned()])
            ),
            ("failed".into(), None, Err("connection refused".to_owned())),
        ]
    );
    Ok(())
}

#[tokio::test]
async fn targeted_inventory_rejects_another_server() {
    let (client, mut commands) = test_client();
    let name = "calendar".into();
    let other = row("other", vec![json!({"name":"footer","inputSchema":{}})]);
    let (result, ()) = tokio::join!(
        server(&client, &name, /*thread*/ None),
        reply(
            &mut commands,
            "mcpServerStatus/list",
            json!({"threadId":null,"serverName":"calendar","environmentId":null,"detail":"toolsAndAuthOnly","cursor":null,"limit":null}),
            json!({"data":[other],"nextCursor":null}),
        )
    );
    assert_eq!(
        result.unwrap_err().to_string(),
        "targeted MCP inventory did not identify the requested server"
    );
}
