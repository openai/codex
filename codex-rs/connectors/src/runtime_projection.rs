//! Connector-owned projection of raw runtime tools into installed app state.

use std::collections::BTreeMap;

use codex_config::ConfigLayerStack;

use crate::AppToolPolicyEvaluator;
use crate::AppToolPolicyInput;

/// Connector-relevant fields from one runtime tool.
///
/// MCP owns the raw tool type and computes generic visibility/filter decisions. Connector
/// consumers adapt those fields into this view so connector policy stays out of MCP modules.
#[derive(Debug, Clone, Copy)]
pub struct ConnectorRuntimeTool<'a> {
    pub connector_id: Option<&'a str>,
    pub connector_name: Option<&'a str>,
    pub tool_name: &'a str,
    pub tool_title: Option<&'a str>,
    pub destructive_hint: Option<bool>,
    pub open_world_hint: Option<bool>,
    pub synthetic: bool,
    pub model_visible: bool,
}

/// Installed state derived from one committed connector runtime snapshot.
///
/// `enabled` and `callable` include local and managed app/tool configuration. Global feature and
/// workspace policy remain host concerns and are applied by the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledConnectorRuntime {
    pub id: String,
    pub runtime_name: Option<String>,
    pub enabled: bool,
    pub callable: bool,
}

/// Projects raw runtime tools into one row per installed connector.
pub fn installed_connector_runtime<'a>(
    config_layer_stack: &ConfigLayerStack,
    tools: impl IntoIterator<Item = ConnectorRuntimeTool<'a>>,
) -> Vec<InstalledConnectorRuntime> {
    let policy = AppToolPolicyEvaluator::new(config_layer_stack);
    let mut apps = BTreeMap::<String, (Option<String>, bool)>::new();

    for tool in tools {
        if tool.synthetic {
            continue;
        }
        let Some(connector_id) = tool.connector_id.map(str::trim) else {
            continue;
        };
        if connector_id.is_empty() {
            continue;
        }

        let runtime_name = tool
            .connector_name
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_string);
        let entry = apps
            .entry(connector_id.to_string())
            .or_insert((None, false));
        if entry.0.is_none() {
            entry.0 = runtime_name;
        }

        let policy_allows_tool = policy
            .policy(AppToolPolicyInput {
                connector_id: Some(connector_id),
                tool_name: tool.tool_name,
                tool_title: tool.tool_title,
                destructive_hint: tool.destructive_hint,
                open_world_hint: tool.open_world_hint,
            })
            .enabled;
        entry.1 |= tool.model_visible && policy_allows_tool;
    }

    apps.into_iter()
        .map(|(id, (runtime_name, callable))| InstalledConnectorRuntime {
            enabled: policy.app_enabled(&id),
            id,
            runtime_name,
            callable,
        })
        .collect()
}

/// Returns whether connector metadata marks a runtime tool as a synthetic link helper.
pub fn connector_tool_is_synthetic(connector_meta: Option<&serde_json::Value>) -> bool {
    connector_meta
        .and_then(serde_json::Value::as_object)
        .and_then(|meta| meta.get("synthetic_link"))
        .and_then(serde_json::Value::as_bool)
        == Some(true)
}

#[cfg(test)]
#[path = "runtime_projection_tests.rs"]
mod tests;
