//! Exercise manifest and overlay invalidation across RPCs in one running app-server.

use anyhow::Result;
use app_test_support::TestAppServer;
use codex_app_server_protocol::PluginListMarketplaceKind;
use codex_app_server_protocol::PluginListParams;
use codex_app_server_protocol::PluginListResponse;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

#[tokio::test]
async fn plugin_list_observes_manifest_and_overlay_revisions_in_one_server() -> Result<()> {
    let codex_home = TempDir::new()?;
    let repository = TempDir::new()?;
    let root = repository.path();
    for directory in [".git", ".agents/plugins", "sample/.codex-plugin"] {
        std::fs::create_dir_all(root.join(directory))?;
    }
    std::fs::write(
        codex_home.path().join("config.toml"),
        "[features]\nplugins = true\nremote_plugin = false\n",
    )?;
    std::fs::write(
        root.join(".agents/plugins/marketplace.json"),
        serde_json::to_vec(&json!({
            "name": "cache-test", "plugins": [{"name": "sample", "source": {"source": "local", "path": "./sample"}}]
        }))?,
    )?;
    let mut server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized_with_timeout(Duration::from_secs(30))
        .await?;
    let overlay_path = root.join("sample/.codex-plugin/plugin.json");
    for (version, overlay_name, expected_name) in [
        ("1.0.0", Some("First"), "First"),
        ("2.0.0", Some("First"), "First"),
        ("2.0.0", Some("Changed"), "Changed"),
        ("2.0.0", None, "sample"),
    ] {
        std::fs::write(
            root.join("sample/plugin.json"),
            serde_json::to_vec(&json!({
                "$schema": "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json",
                "name": "sample", "version": version
            }))?,
        )?;
        if let Some(display_name) = overlay_name {
            std::fs::write(
                &overlay_path,
                serde_json::to_vec(&json!({"interface": {"displayName": display_name}}))?,
            )?;
        } else {
            std::fs::remove_file(&overlay_path)?;
        }
        let mut previous = None;
        for _ in 0..2 {
            let id = server
                .send_plugin_list_request(PluginListParams {
                    cwds: Some(vec![AbsolutePathBuf::try_from(root)?]),
                    marketplace_kinds: Some(vec![PluginListMarketplaceKind::Local]),
                    force_refetch: false,
                })
                .await?;
            let response: PluginListResponse =
                timeout(Duration::from_secs(30), server.read_response(id)).await??;
            assert_eq!(response.marketplace_load_errors, Vec::new());
            let marketplace = response
                .marketplaces
                .iter()
                .find(|entry| entry.name == "cache-test")
                .expect("local marketplace");
            let plugin = &marketplace.plugins[0];
            assert_eq!(
                (
                    plugin.id.as_str(),
                    plugin.local_version.as_deref(),
                    plugin
                        .interface
                        .as_ref()
                        .and_then(|value| value.display_name.as_deref())
                ),
                ("sample@cache-test", Some(version), Some(expected_name))
            );
            if let Some(previous) = previous {
                assert_eq!(marketplace, &previous);
            }
            previous = Some(marketplace.clone());
        }
    }
    Ok(())
}
