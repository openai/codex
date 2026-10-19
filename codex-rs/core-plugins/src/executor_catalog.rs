//! Converts V2 discovery contents to inert plugin catalogs without additional I/O.

use crate::PluginCatalog;
use crate::PluginCatalogEntry;
use crate::PluginIdentity;
use crate::PluginSourceLocation;
use codex_connectors::parse_plugin_app_config;
use codex_exec_server::ExecutorCapabilityDiscoveryV2Snapshot;
use codex_exec_server::ExecutorPlugin;
use codex_mcp::parse_plugin_mcp_declarations;

/// Projects V2 inventories into a plugin catalog without additional filesystem access.
pub fn catalog_from_snapshots(
    snapshots: &[ExecutorCapabilityDiscoveryV2Snapshot],
) -> PluginCatalog {
    let mut catalog = PluginCatalog::default();
    for snapshot in snapshots {
        let inventory = match &snapshot.result {
            Ok(inventory) => inventory,
            Err(error) => {
                catalog.warnings.push(format!(
                    "could not discover plugins in environment `{}`; skipping its plugin inventory: {error}",
                    snapshot.environment_id
                ));
                continue;
            }
        };
        catalog.warnings.extend(inventory.warnings.iter().cloned());
        for plugin in &inventory.plugins {
            let (entry, warnings) = from_discovered_plugin(&snapshot.environment_id, plugin);
            catalog.entries.push(entry);
            catalog.warnings.extend(warnings);
        }
    }
    catalog
}

fn from_discovered_plugin(
    environment_id: &str,
    plugin: &ExecutorPlugin,
) -> (PluginCatalogEntry, Vec<String>) {
    let plugin_id = &plugin.id;
    let mut warnings = Vec::new();
    let mcp_servers = match plugin
        .files
        .mcp_config
        .as_ref()
        .map(|file| parse_plugin_mcp_declarations(&file.contents))
    {
        Some(Ok(servers)) => servers,
        Some(Err(error)) => {
            warnings.push(format!("plugin `{plugin_id}` MCP configuration: {error}"));
            Default::default()
        }
        None => Default::default(),
    };
    let connector_ids = match plugin
        .files
        .apps_config
        .as_ref()
        .map(|file| parse_plugin_app_config(&file.contents))
    {
        Some(Ok(apps)) => apps.into_iter().map(|app| app.connector_id.0).collect(),
        Some(Err(error)) => {
            warnings.push(format!("plugin `{plugin_id}` app configuration: {error}"));
            Vec::new()
        }
        None => Vec::new(),
    };
    (
        PluginCatalogEntry {
            enabled: true,
            id: match &plugin.remote_plugin_id {
                Some(remote_plugin_id) => PluginIdentity::Remote {
                    remote_plugin_id: remote_plugin_id.clone(),
                },
                None => PluginIdentity::Local {
                    plugin_id: plugin.id.clone(),
                },
            },
            display_name: plugin.display_name.clone(),
            version: plugin.version.clone(),
            mcp_servers,
            connector_ids,
            locations: vec![PluginSourceLocation::Executor {
                environment_id: environment_id.to_string(),
                plugin_id: plugin_id.to_string(),
                root: plugin.root.clone(),
            }],
        },
        warnings,
    )
}

#[cfg(test)]
#[path = "executor_catalog_tests.rs"]
mod tests;
