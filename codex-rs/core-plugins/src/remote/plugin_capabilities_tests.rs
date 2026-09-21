//! Covers local capability accumulation and removal hints from the shared bundle sync.

use super::*;
use crate::remote::RemoteInstalledPluginBundleSyncOutcome;
use crate::remote::RemotePluginChange;
use crate::remote::RemotePluginServiceConfig;
use crate::remote::sync_remote_installed_plugin_bundles_once;
use crate::test_support::write_file;
use codex_login::CodexAuth;
use codex_utils_plugins::AGENT_PLUGIN_SCHEMA_URI;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::query_param;

#[tokio::test]
async fn agent_plugin_capabilities_do_not_require_runtime_data_directory() {
    let codex_home = TempDir::new().expect("Codex home");
    let store = PluginStore::new(codex_home.path().to_path_buf());
    let plugin_id = PluginId::parse("example@test").expect("plugin id");
    let root = store.plugin_root(&plugin_id, "1.0.0");
    write_file(
        root.join("plugin.json").as_path(),
        &json!({
            "$schema": AGENT_PLUGIN_SCHEMA_URI,
            "name": "example",
            "extensions": {"com.openai": {"apps": "./connectors.json"}},
        })
        .to_string(),
    );
    write_file(
        root.join("mcp.json").as_path(),
        &json!({
            "$schema": "https://agent-plugins.org/schemas/1.0.0/mcp.schema.json",
            "mcpServers": {"example": {"type": "stdio", "command": "echo"}},
        })
        .to_string(),
    );
    write_file(
        root.join("connectors.json").as_path(),
        &json!({"apps": {"example": {"id": "connector_example"}}}).to_string(),
    );
    let data_root = store.mcp_data_root(&plugin_id, PluginManifestFormat::AgentPlugin);

    for block_data_root in [false, true] {
        if block_data_root {
            write_file(data_root.as_path(), "not a directory");
        }
        let mut capabilities = RemotePluginCapabilities::default();
        capabilities.include_active_bundle(&store, &plugin_id).await;
        assert_eq!(
            capabilities,
            RemotePluginCapabilities {
                has_mcps: true,
                has_apps: true,
                // Agent Plugins declare a conventional skills root even if it is absent.
                has_skills: true,
                ..Default::default()
            }
        );
        assert_eq!(data_root.as_path().exists(), block_data_root);
        assert!(!data_root.as_path().is_dir());
    }
}

#[tokio::test]
async fn capabilities_union_cached_versions_and_sync_reports_removal() -> anyhow::Result<()> {
    let codex_home = TempDir::new()?;
    let store = PluginStore::new(codex_home.path().to_path_buf());
    let plugin_id = PluginId::parse("example@openai-curated-remote")?;
    let old_root = store.plugin_root(&plugin_id, "1.0.0");
    for (path, contents) in [
        (".codex-plugin/plugin.json", r#"{"name":"example"}"#),
        (
            ".mcp.json",
            r#"{"mcpServers":{"example":{"command":"unused"}}}"#,
        ),
        (
            "hooks/hooks.json",
            r#"{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"echo example"}]}]}}"#,
        ),
        (
            "skills/example/SKILL.md",
            "---\nname: example\ndescription: Example skill\n---\nExample skill",
        ),
    ] {
        write_file(old_root.join(path).as_path(), contents);
    }
    let mut capabilities = RemotePluginCapabilities::default();
    capabilities.include_active_bundle(&store, &plugin_id).await;

    // Accumulate the old and new declarations without involving bundle transport.
    let new_root = store.plugin_root(&plugin_id, "2.0.0");
    write_file(
        new_root.join(".codex-plugin/plugin.json").as_path(),
        r#"{"name":"example"}"#,
    );
    write_file(
        new_root.join(".app.json").as_path(),
        r#"{"apps":{"example":{"id":"connector_example"}}}"#,
    );
    capabilities.include_active_bundle(&store, &plugin_id).await;
    assert_eq!(
        capabilities,
        RemotePluginCapabilities {
            has_mcps: true,
            has_apps: true,
            has_hooks: true,
            has_skills: true,
        }
    );

    let server = MockServer::start().await;
    let config = RemotePluginServiceConfig::new(
        format!("{}/backend-api", server.uri()),
        crate::test_support::test_http_client_factory(),
        /*product_sku*/ None,
    );
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/installed"))
        .and(query_param("includeDownloadUrls", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "plugins": [],
            "pagination": {"next_page_token": null},
        })))
        .expect(1)
        .mount(&server)
        .await;

    // Removal describes only the active version, not the union accumulated above.
    assert_eq!(
        sync_remote_installed_plugin_bundles_once(
            codex_home.path().to_path_buf(),
            &config,
            Some(&auth),
        )
        .await?,
        RemoteInstalledPluginBundleSyncOutcome {
            changed_plugins: vec![RemotePluginChange {
                plugin_id: plugin_id.as_key(),
                capabilities: RemotePluginCapabilities {
                    has_apps: true,
                    ..Default::default()
                },
            }],
            ..Default::default()
        }
    );
    assert!(!store.plugin_base_root(&plugin_id).as_path().exists());
    server.verify().await;
    Ok(())
}
