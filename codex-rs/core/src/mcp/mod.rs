pub mod auth;
use std::collections::HashMap;

use async_channel::unbounded;
use codex_protocol::protocol::McpListToolsResponseEvent;
use tokio_util::sync::CancellationToken;

use crate::config::Config;
use crate::mcp::auth::compute_auth_statuses;
use crate::mcp_connection_manager::McpConnectionManager;

pub async fn collect_mcp_snapshot(config: &Config) -> McpListToolsResponseEvent {
    if config.mcp_servers.is_empty() {
        return McpListToolsResponseEvent {
            tools: HashMap::new(),
            resources: HashMap::new(),
            resource_templates: HashMap::new(),
            auth_statuses: HashMap::new(),
        };
    }

    let auth_status_entries = compute_auth_statuses(
        config.mcp_servers.iter(),
        config.mcp_oauth_credentials_store_mode,
    )
    .await;

    let mut mcp_connection_manager = McpConnectionManager::default();
    let (tx_event, rx_event) = unbounded();
    drop(rx_event);
    let cancel_token = CancellationToken::new();

    mcp_connection_manager
        .initialize(
            config.mcp_servers.clone(),
            config.mcp_oauth_credentials_store_mode,
            auth_status_entries.clone(),
            tx_event,
            cancel_token.clone(),
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

    McpListToolsResponseEvent {
        tools: tools
            .into_iter()
            .map(|(name, tool)| (name, tool.tool))
            .collect(),
        resources,
        resource_templates,
        auth_statuses,
    }
}
