use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

use async_channel::unbounded;
use codex_protocol::mcp::Resource;
use codex_protocol::mcp::ResourceTemplate;
use codex_protocol::mcp::Tool;
use codex_protocol::protocol::McpListToolsResponseEvent;
use codex_protocol::protocol::SandboxPolicy;
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use crate::AuthManager;
use crate::config::Config;
use crate::mcp::auth::compute_auth_statuses;
use crate::mcp_connection_manager::McpConnectionManager;
use crate::mcp_connection_manager::SandboxState;

use super::effective_mcp_servers;

pub async fn collect_mcp_snapshot(config: &Config) -> McpListToolsResponseEvent {
    let auth_manager = AuthManager::shared(
        config.codex_home.clone(),
        false,
        config.cli_auth_credentials_store_mode,
    );
    let auth = auth_manager.auth().await;
    let mcp_servers = effective_mcp_servers(config, auth.as_ref());
    if mcp_servers.is_empty() {
        return McpListToolsResponseEvent {
            tools: HashMap::new(),
            resources: HashMap::new(),
            resource_templates: HashMap::new(),
            auth_statuses: HashMap::new(),
        };
    }

    let auth_status_entries =
        compute_auth_statuses(mcp_servers.iter(), config.mcp_oauth_credentials_store_mode).await;

    let mut mcp_connection_manager = McpConnectionManager::default();
    let (tx_event, rx_event) = unbounded();
    drop(rx_event);
    let cancel_token = CancellationToken::new();

    // Use ReadOnly sandbox policy for MCP snapshot collection (safest default)
    let sandbox_state = SandboxState {
        sandbox_policy: SandboxPolicy::ReadOnly,
        codex_linux_sandbox_exe: config.codex_linux_sandbox_exe.clone(),
        sandbox_cwd: env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
    };

    mcp_connection_manager
        .initialize(
            &mcp_servers,
            config.mcp_oauth_credentials_store_mode,
            auth_status_entries.clone(),
            tx_event,
            cancel_token.clone(),
            sandbox_state,
        )
        .await;

    let snapshot =
        collect_mcp_snapshot_from_manager(&mcp_connection_manager, auth_status_entries).await;

    cancel_token.cancel();

    snapshot
}

pub(crate) async fn collect_mcp_snapshot_from_manager(
    mcp_connection_manager: &McpConnectionManager,
    auth_status_entries: HashMap<String, crate::mcp::auth::McpAuthStatusEntry>,
) -> McpListToolsResponseEvent {
    let (tools, resources, resource_templates) = tokio::join!(
        mcp_connection_manager.list_all_tools(),
        mcp_connection_manager.list_all_resources(),
        mcp_connection_manager.list_all_resource_templates(),
    );

    let auth_statuses = auth_status_entries
        .iter()
        .map(|(name, entry)| (name.clone(), entry.auth_status))
        .collect();

    let tools = tools
        .into_iter()
        .filter_map(|(name, tool)| match serde_json::to_value(tool.tool) {
            Ok(value) => match Tool::from_mcp_value(value) {
                Ok(tool) => Some((name, tool)),
                Err(err) => {
                    tracing::warn!("Failed to convert MCP tool '{name}': {err}");
                    None
                }
            },
            Err(err) => {
                tracing::warn!("Failed to serialize MCP tool '{name}': {err}");
                None
            }
        })
        .collect();

    let resources = resources
        .into_iter()
        .map(|(name, resources)| {
            let resources = resources
                .into_iter()
                .filter_map(|resource| match serde_json::to_value(resource) {
                    Ok(value) => match Resource::from_mcp_value(value.clone()) {
                        Ok(resource) => Some(resource),
                        Err(err) => {
                            let (uri, resource_name) = match value {
                                Value::Object(obj) => (
                                    obj.get("uri")
                                        .and_then(|v| v.as_str().map(ToString::to_string)),
                                    obj.get("name")
                                        .and_then(|v| v.as_str().map(ToString::to_string)),
                                ),
                                _ => (None, None),
                            };

                            tracing::warn!(
                                "Failed to convert MCP resource (uri={uri:?}, name={resource_name:?}): {err}"
                            );
                            None
                        }
                    },
                    Err(err) => {
                        tracing::warn!("Failed to serialize MCP resource: {err}");
                        None
                    }
                })
                .collect::<Vec<_>>();
            (name, resources)
        })
        .collect();

    let resource_templates = resource_templates
        .into_iter()
        .map(|(name, templates)| {
            let templates = templates
                .into_iter()
                .filter_map(|template| match serde_json::to_value(template) {
                    Ok(value) => match ResourceTemplate::from_mcp_value(value.clone()) {
                        Ok(template) => Some(template),
                        Err(err) => {
                            let (uri_template, template_name) = match value {
                                Value::Object(obj) => (
                                    obj.get("uriTemplate")
                                        .or_else(|| obj.get("uri_template"))
                                        .and_then(|v| v.as_str().map(ToString::to_string)),
                                    obj.get("name")
                                        .and_then(|v| v.as_str().map(ToString::to_string)),
                                ),
                                _ => (None, None),
                            };

                            tracing::warn!(
                                "Failed to convert MCP resource template (uri_template={uri_template:?}, name={template_name:?}): {err}"
                            );
                            None
                        }
                    },
                    Err(err) => {
                        tracing::warn!("Failed to serialize MCP resource template: {err}");
                        None
                    }
                })
                .collect::<Vec<_>>();
            (name, templates)
        })
        .collect();

    McpListToolsResponseEvent {
        tools,
        resources,
        resource_templates,
        auth_statuses,
    }
}
