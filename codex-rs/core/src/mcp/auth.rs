use std::collections::HashMap;

use anyhow::Result;
use codex_protocol::protocol::McpAuthStatus;
use codex_rmcp_client::OAuthCredentialsStoreMode;
use codex_rmcp_client::determine_streamable_http_auth_status;
use codex_rmcp_client::supports_oauth_login;
use futures::future::join_all;
use tracing::warn;

use crate::config::types::McpServerConfig;
use crate::config::types::McpServerTransportConfig;

#[derive(Debug, Clone)]
pub struct McpOAuthLoginConfig {
    pub url: String,
    pub http_headers: Option<HashMap<String, String>>,
    pub env_http_headers: Option<HashMap<String, String>>,
}

#[derive(Debug)]
pub enum McpOAuthLoginSupport {
    Supported(McpOAuthLoginConfig),
    Unsupported,
    Unknown(anyhow::Error),
}

pub async fn oauth_login_support(transport: &McpServerTransportConfig) -> McpOAuthLoginSupport {
    let McpServerTransportConfig::StreamableHttp {
        url,
        bearer_token_env_var,
        http_headers,
        env_http_headers,
    } = transport
    else {
        return McpOAuthLoginSupport::Unsupported;
    };

    if bearer_token_env_var.is_some() {
        return McpOAuthLoginSupport::Unsupported;
    }

    match supports_oauth_login(url).await {
        Ok(true) => McpOAuthLoginSupport::Supported(McpOAuthLoginConfig {
            url: url.clone(),
            http_headers: http_headers.clone(),
            env_http_headers: env_http_headers.clone(),
        }),
        Ok(false) => McpOAuthLoginSupport::Unsupported,
        Err(err) => McpOAuthLoginSupport::Unknown(err),
    }
}

#[derive(Debug, Clone)]
pub struct McpAuthStatusEntry {
    pub config: McpServerConfig,
    pub auth_status: McpAuthStatus,
}

pub async fn compute_auth_statuses<'a, I>(
    servers: I,
    store_mode: OAuthCredentialsStoreMode,
) -> HashMap<String, McpAuthStatusEntry>
where
    I: IntoIterator<Item = (&'a String, &'a McpServerConfig)>,
{
    let futures = servers.into_iter().map(|(name, config)| {
        let name = name.clone();
        let config = config.clone();
        async move {
            let auth_status = match compute_auth_status(&name, &config, store_mode).await {
                Ok(status) => status,
                Err(error) => {
                    warn!("failed to determine auth status for MCP server `{name}`: {error:?}");
                    McpAuthStatus::Unsupported
                }
            };
            let entry = McpAuthStatusEntry {
                config,
                auth_status,
            };
            (name, entry)
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
            http_headers,
            env_http_headers,
        } => {
            determine_streamable_http_auth_status(
                server_name,
                url,
                bearer_token_env_var.as_deref(),
                http_headers.clone(),
                env_http_headers.clone(),
                store_mode,
            )
            .await
        }
    }
}
