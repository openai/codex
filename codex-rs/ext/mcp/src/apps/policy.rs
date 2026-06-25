use std::collections::HashMap;
use std::sync::Arc;

use codex_apps::CodexAppsSnapshot;
use codex_config::McpServerToolConfig;
use codex_connectors::AppToolPolicyEvaluator;
use codex_connectors::AppToolPolicyInput;
use codex_connectors::ConnectorSnapshot;
use codex_connectors::apps_config_from_layer_stack;
use codex_core::config::Config;
use codex_core::config::edit::ConfigEdit;
use codex_core::config::edit::ConfigEditsBuilder;
use codex_mcp::EffectiveMcpServer;
use codex_mcp::McpToolApprovalPersistence;
use toml_edit::value;

pub(super) fn apply_apps_server_policy(
    config: &Config,
    snapshot: &CodexAppsSnapshot,
    plugin_connectors: &ConnectorSnapshot,
    servers: Vec<(String, EffectiveMcpServer)>,
) -> Vec<(String, EffectiveMcpServer)> {
    let evaluator = AppToolPolicyEvaluator::new(&config.config_layer_stack);
    let apps_config = apps_config_from_layer_stack(&config.config_layer_stack);
    let approval_config = Arc::new(config.clone());
    let mut tools_by_server = HashMap::<_, Vec<_>>::new();
    for (server_name, tool_name, metadata) in snapshot.tools() {
        tools_by_server
            .entry(server_name)
            .or_default()
            .push((tool_name, metadata));
    }
    servers
        .into_iter()
        .map(|(server_name, server)| {
            let tools = tools_by_server
                .remove(server_name.as_str())
                .unwrap_or_default();
            let connector_id = tools
                .first()
                .map(|(_, metadata)| metadata.connector_id().to_string());
            let app_reviewer = apps_config.as_ref().and_then(|apps_config| {
                connector_id
                    .as_deref()
                    .and_then(|connector_id| apps_config.apps.get(connector_id))
                    .and_then(|app| app.approvals_reviewer)
                    .or_else(|| {
                        apps_config
                            .default
                            .as_ref()
                            .and_then(|defaults| defaults.approvals_reviewer)
                    })
            });
            let app_reviewer = app_reviewer.filter(|reviewer| {
                config
                    .config_layer_stack
                    .requirements()
                    .approvals_reviewer
                    .can_set(reviewer)
                    .is_ok()
            });
            let plugin_display_names = connector_id
                .as_deref()
                .map(|connector_id| {
                    plugin_connectors
                        .plugin_display_names_for_connector_id(connector_id)
                        .to_vec()
                })
                .unwrap_or_default();
            let mut runtime_metadata = server
                .runtime_metadata()
                .clone()
                .with_plugin_display_names(plugin_display_names);
            if let Some(reviewer) = app_reviewer {
                runtime_metadata = runtime_metadata.with_approvals_reviewer(reviewer);
            }
            let mut enabled_tools = Vec::new();
            let mut tool_configs = HashMap::new();
            for (tool_name, metadata) in tools {
                let policy = evaluator.policy(AppToolPolicyInput {
                    connector_id: Some(metadata.connector_id()),
                    tool_name: metadata.upstream_tool_name(),
                    tool_title: metadata.tool_title(),
                    destructive_hint: metadata.destructive_hint(),
                    open_world_hint: metadata.open_world_hint(),
                });
                if !policy.enabled {
                    continue;
                }
                if let Some(runtime_tool) = runtime_metadata.tool(tool_name).cloned() {
                    runtime_metadata = runtime_metadata.with_tool(
                        tool_name,
                        runtime_tool.with_approval_persistence(apps_approval_persistence(
                            Arc::clone(&approval_config),
                            metadata.connector_id().to_string(),
                            metadata.upstream_tool_name().to_string(),
                        )),
                    );
                }
                enabled_tools.push(tool_name.to_string());
                tool_configs.insert(
                    tool_name.to_string(),
                    McpServerToolConfig {
                        approval_mode: Some(policy.approval),
                    },
                );
            }
            enabled_tools.sort();
            let server = server
                .with_runtime_metadata(runtime_metadata)
                .with_tool_policy(enabled_tools, tool_configs);
            (server_name, server)
        })
        .collect()
}

fn apps_approval_persistence(
    config: Arc<Config>,
    connector_id: String,
    tool_name: String,
) -> McpToolApprovalPersistence {
    McpToolApprovalPersistence::new(move || {
        let config = Arc::clone(&config);
        let connector_id = connector_id.clone();
        let tool_name = tool_name.clone();
        async move {
            ConfigEditsBuilder::for_config(config.as_ref())
                .with_edits([ConfigEdit::SetPath {
                    segments: vec![
                        "apps".to_string(),
                        connector_id,
                        "tools".to_string(),
                        tool_name,
                        "approval_mode".to_string(),
                    ],
                    value: value("approve"),
                }])
                .apply()
                .await
        }
    })
}

#[cfg(test)]
#[path = "policy_tests.rs"]
mod tests;
