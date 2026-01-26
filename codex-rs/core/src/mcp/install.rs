use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_rmcp_client::supports_oauth_login;

use crate::config::edit::ConfigEditsBuilder;
use crate::config::load_global_mcp_servers;
use crate::config::types::McpServerConfig;
use crate::config::types::McpServerTransportConfig;

#[derive(Debug, Clone)]
pub enum McpServerAuthFlow {
    NotRequired,
    OAuth {
        url: String,
        http_headers: Option<HashMap<String, String>>,
        env_http_headers: Option<HashMap<String, String>>,
    },
    Unknown,
}

#[derive(Debug, Clone)]
pub struct McpServerInstallResult {
    pub name: String,
    pub replaced: bool,
    pub auth_flow: McpServerAuthFlow,
    pub servers: BTreeMap<String, McpServerConfig>,
}

pub async fn install_mcp_server(
    codex_home: &Path,
    name: String,
    transport: McpServerTransportConfig,
) -> Result<McpServerInstallResult> {
    validate_mcp_server_name(&name)?;

    let mut servers = load_global_mcp_servers(codex_home)
        .await
        .with_context(|| format!("failed to load MCP servers from {}", codex_home.display()))?;

    let replaced = servers.contains_key(&name);

    let new_entry = McpServerConfig {
        transport: transport.clone(),
        enabled: true,
        disabled_reason: None,
        startup_timeout_sec: None,
        tool_timeout_sec: None,
        enabled_tools: None,
        disabled_tools: None,
    };

    servers.insert(name.clone(), new_entry);

    ConfigEditsBuilder::new(codex_home)
        .replace_mcp_servers(&servers)
        .apply()
        .await
        .with_context(|| format!("failed to write MCP servers to {}", codex_home.display()))?;

    let auth_flow = match transport {
        McpServerTransportConfig::StreamableHttp {
            url,
            bearer_token_env_var,
            http_headers,
            env_http_headers,
        } if bearer_token_env_var.is_none() => match supports_oauth_login(&url).await {
            Ok(true) => McpServerAuthFlow::OAuth {
                url,
                http_headers,
                env_http_headers,
            },
            Ok(false) => McpServerAuthFlow::NotRequired,
            Err(_) => McpServerAuthFlow::Unknown,
        },
        _ => McpServerAuthFlow::NotRequired,
    };

    Ok(McpServerInstallResult {
        name,
        replaced,
        auth_flow,
        servers,
    })
}

fn validate_mcp_server_name(name: &str) -> Result<()> {
    let is_valid = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');

    if is_valid {
        Ok(())
    } else {
        bail!("invalid server name '{name}' (use letters, numbers, '-', '_')")
    }
}
