//! Verifies a trusted source for the exact MCP tool under review.
//! Bounded rendering and trusted delivery belong to the shared context section.

use std::path::Path;

use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_extension_api::McpToolInfo;
use codex_extension_api::McpToolSource;
use codex_features::Feature;
use codex_guardian_context::TrustedTool;
use codex_guardian_context::TrustedToolSource;

#[derive(Clone, Copy)]
enum PluginCapability {
    Connector,
    Mcp,
}

pub(crate) async fn trusted_tool_context(
    tool: &McpToolInfo,
    source: &McpToolSource,
    manager: &ThreadManager,
    config: &Config,
) -> Option<TrustedTool> {
    if source == &McpToolSource::Connector
        && tool
            .connector_id
            .as_deref()
            .is_some_and(|connector_id| !connector_id.trim().is_empty())
        && config
            .features
            .enabled(Feature::GuardianTrustOrchestratorConnectors)
    {
        return Some(TrustedTool {
            server: tool.server_name.clone(),
            connector_id: tool.connector_id.clone(),
            source: TrustedToolSource::PluginServiceOrchestrator,
        });
    }

    let codex_home = config.codex_home.as_path().canonicalize().ok()?;
    let plugins = match source {
        McpToolSource::Connector => Some(
            manager
                .plugins_manager()
                .plugins_for_config(&config.plugins_config_input())
                .await,
        ),
        McpToolSource::Config | McpToolSource::Plugin { .. } => None,
        McpToolSource::SelectedPlugin | McpToolSource::Other => return None,
    };

    let source = match source {
        McpToolSource::Connector => {
            let connector_id = tool.connector_id.as_deref()?;
            if let Some(plugin) = plugins.as_ref()?.plugins().iter().find(|plugin| {
                plugin.is_active()
                    && plugin
                        .apps
                        .iter()
                        .any(|app| app.connector_id.0 == connector_id)
                    && is_home_owned_plugin_capability(
                        plugin.root.as_path(),
                        &codex_home,
                        PluginCapability::Connector,
                    )
            }) {
                plugin.root.as_path().display().to_string()
            } else {
                trusted_user_config_source(config, "apps", connector_id, &codex_home)?
            }
        }
        McpToolSource::Plugin { root, .. } => {
            let plugin_root = root.to_abs_path().ok()?;
            if !is_home_owned_plugin_capability(
                plugin_root.as_path(),
                &codex_home,
                PluginCapability::Mcp,
            ) {
                return None;
            }
            plugin_root.as_path().display().to_string()
        }
        McpToolSource::Config => {
            trusted_user_config_source(config, "mcp_servers", &tool.server_name, &codex_home)?
        }
        McpToolSource::SelectedPlugin | McpToolSource::Other => return None,
    };

    Some(TrustedTool {
        server: tool.server_name.clone(),
        connector_id: tool.connector_id.clone(),
        source: TrustedToolSource::UserConfiguration(source),
    })
}

fn is_home_owned_plugin_capability(
    plugin_root: &Path,
    codex_home: &Path,
    capability: PluginCapability,
) -> bool {
    if !is_home_owned_path(plugin_root, codex_home) {
        return false;
    }

    let root_manifest = plugin_root.join("plugin.json");
    let manifest_path = [
        root_manifest.clone(),
        plugin_root.join(".codex-plugin").join("plugin.json"),
        plugin_root.join(".claude-plugin").join("plugin.json"),
        plugin_root.join(".cursor-plugin").join("plugin.json"),
    ]
    .into_iter()
    .find(|path| path.is_file());
    let Some(manifest_path) = manifest_path else {
        return false;
    };
    if !is_home_owned_path(&manifest_path, codex_home) {
        return false;
    }

    let Ok(manifest_contents) = std::fs::read_to_string(&manifest_path) else {
        return false;
    };
    let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&manifest_contents) else {
        return false;
    };
    let declaration_path = match capability {
        PluginCapability::Connector => manifest
            .get("apps")
            .and_then(serde_json::Value::as_str)
            .map_or_else(
                || plugin_root.join(".app.json"),
                |path| plugin_root.join(path),
            ),
        PluginCapability::Mcp => match manifest.get("mcpServers") {
            Some(serde_json::Value::Object(_)) => manifest_path,
            Some(serde_json::Value::String(path)) => plugin_root.join(path),
            Some(_) => return false,
            None if manifest_path == root_manifest => plugin_root.join("mcp.json"),
            None => plugin_root.join(".mcp.json"),
        },
    };

    is_home_owned_path(&declaration_path, codex_home)
}

fn trusted_user_config_source(
    config: &Config,
    section: &str,
    name: &str,
    codex_home: &Path,
) -> Option<String> {
    let user_config_file = config.config_layer_stack.get_user_config_file()?;
    if !is_home_owned_path(user_config_file.as_path(), codex_home) {
        return None;
    }

    let user_config = config.config_layer_stack.effective_user_config()?;
    let user_entry = user_config.get(section)?.get(name)?;
    let effective_config = config.config_layer_stack.effective_config();
    let effective_entry = effective_config.get(section)?.get(name)?;
    (user_entry == effective_entry).then(|| user_config_file.as_path().display().to_string())
}

fn is_home_owned_path(path: &Path, codex_home: &Path) -> bool {
    path.canonicalize()
        .is_ok_and(|canonical_path| canonical_path.starts_with(codex_home))
}

#[cfg(test)]
#[path = "trusted_tools_tests.rs"]
mod tests;
