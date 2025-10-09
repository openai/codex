use std::collections::HashMap;

use anyhow::Result;
use codex_protocol::protocol::McpAuthStatus;
use codex_rmcp_client::OAuthCredentialsStoreMode;
use codex_rmcp_client::determine_streamable_http_auth_status;
use futures::future::join_all;
use tracing::warn;

use crate::config_types::McpServerConfig;
use crate::config_types::McpServerTransportConfig;

pub async fn compute_auth_statuses<'a, I>(
    servers: I,
    store_mode: OAuthCredentialsStoreMode,
) -> HashMap<String, McpAuthStatus>
where
    I: IntoIterator<Item = (&'a String, &'a McpServerConfig)>,
{
    let futures = servers.into_iter().map(|(name, config)| {
        let name = name.clone();
        let config = config.clone();
        async move {
            let status = match compute_auth_status(&name, &config, store_mode).await {
                Ok(status) => status,
                Err(error) => {
                    warn!("failed to determine auth status for MCP server `{name}`: {error:?}");
                    McpAuthStatus::Unsupported
                }
            };
            (name, status)
        }
    });

    join_all(futures).await.into_iter().collect()
}

async fn compute_auth_status(
    server_name: &str,
    config: &McpServerConfig,
    store_mode: OAuthCredentialsStoreMode,
) -> Result<McpAuthStatus> {
    match &config.transport {
        McpServerTransportConfig::Stdio { .. } => Ok(McpAuthStatus::Unsupported),
        McpServerTransportConfig::StreamableHttp {
            url,
            bearer_token_env_var,
        } => {
            determine_streamable_http_auth_status(
                server_name,
                url,
                bearer_token_env_var.as_deref(),
                store_mode,
            )
            .await
        }
    }
}
