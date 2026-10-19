//! Tests conversion of DiscoveryV2 payloads into inert plugin catalogs.

use super::catalog_from_snapshots;
use crate::PluginCatalog;
use crate::PluginCatalogEntry;
use crate::PluginIdentity;
use crate::PluginSourceLocation;
use codex_exec_server::CapabilityTextFile;
use codex_exec_server::DiscoverV2CapabilitiesResponse;
use codex_exec_server::DiscoveredPluginFiles;
use codex_exec_server::ExecutorCapabilityDiscoveryV2Snapshot;
use codex_exec_server::ExecutorPlugin;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::sync::Arc;

#[test]
fn discovered_mcp_contents_populate_catalog() -> anyhow::Result<()> {
    let root = PathUri::parse("file:///plugins/example/1.0.0")?;
    let manifest_path = root.join(".codex-plugin/plugin.json")?;
    let declarations = serde_json::json!({"example": {"command": "node"}});
    let snapshot = ExecutorCapabilityDiscoveryV2Snapshot {
        environment_id: "executor-a".to_string(),
        result: Ok(Arc::new(DiscoverV2CapabilitiesResponse {
            plugins: vec![ExecutorPlugin {
                id: "example@marketplace".to_string(),
                display_name: "Example".to_string(),
                remote_plugin_id: Some("plugin_a".to_string()),
                version: "1.0.0".to_string(),
                files: DiscoveredPluginFiles {
                    manifest: CapabilityTextFile {
                        path: manifest_path.clone(),
                        contents: r#"{"name":"example"}"#.to_string(),
                    },
                    mcp_config: Some(CapabilityTextFile {
                        path: manifest_path,
                        contents: declarations.to_string(),
                    }),
                    apps_config: None,
                },
                root: root.clone(),
                skills: Vec::new(),
            }],
            ..Default::default()
        })),
    };
    assert_eq!(
        catalog_from_snapshots(&[snapshot]),
        PluginCatalog {
            entries: vec![PluginCatalogEntry {
                enabled: true,
                id: PluginIdentity::Remote {
                    remote_plugin_id: "plugin_a".to_string(),
                },
                display_name: "Example".to_string(),
                version: "1.0.0".to_string(),
                mcp_servers: BTreeMap::from([(
                    "example".to_string(),
                    serde_json::json!({"command": "node"}),
                )]),
                connector_ids: Vec::new(),
                locations: vec![PluginSourceLocation::Executor {
                    environment_id: "executor-a".to_string(),
                    plugin_id: "example@marketplace".to_string(),
                    root,
                }],
            }],
            ..Default::default()
        },
    );
    Ok(())
}
