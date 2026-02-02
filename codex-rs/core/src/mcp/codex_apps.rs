use std::collections::HashMap;
use std::env;
use std::time::Duration;

use crate::CodexAuth;
use crate::config::Config;
use crate::config::types::McpServerConfig;
use crate::config::types::McpServerTransportConfig;
use crate::features::Feature;

use super::CODEX_APPS_MCP_SERVER_NAME;

const CODEX_CONNECTORS_TOKEN_ENV_VAR: &str = "CODEX_CONNECTORS_TOKEN";

fn codex_apps_mcp_bearer_token_env_var() -> Option<String> {
    match env::var(CODEX_CONNECTORS_TOKEN_ENV_VAR) {
        Ok(value) if !value.trim().is_empty() => Some(CODEX_CONNECTORS_TOKEN_ENV_VAR.to_string()),
        Ok(_) => None,
        Err(env::VarError::NotPresent) => None,
        Err(env::VarError::NotUnicode(_)) => Some(CODEX_CONNECTORS_TOKEN_ENV_VAR.to_string()),
    }
}

fn codex_apps_mcp_bearer_token(auth: Option<&CodexAuth>) -> Option<String> {
    let token = auth.and_then(|auth| auth.get_token().ok())?;
    let token = token.trim();
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

fn codex_apps_mcp_http_headers(auth: Option<&CodexAuth>) -> Option<HashMap<String, String>> {
    let mut headers = HashMap::new();
    if let Some(token) = codex_apps_mcp_bearer_token(auth) {
        headers.insert("Authorization".to_string(), format!("Bearer {token}"));
    }
    if let Some(account_id) = auth.and_then(CodexAuth::get_account_id) {
        headers.insert("ChatGPT-Account-ID".to_string(), account_id);
    }
    if headers.is_empty() {
        None
    } else {
        Some(headers)
    }
}

fn codex_apps_mcp_url(base_url: &str) -> String {
    let mut base_url = base_url.trim_end_matches('/').to_string();
    if (base_url.starts_with("https://chatgpt.com")
        || base_url.starts_with("https://chat.openai.com"))
        && !base_url.contains("/backend-api")
    {
        base_url = format!("{base_url}/backend-api");
    }
    if base_url.contains("/backend-api") {
        format!("{base_url}/wham/apps")
    } else if base_url.contains("/api/codex") {
        format!("{base_url}/apps")
    } else {
        format!("{base_url}/api/codex/apps")
    }
}

fn codex_apps_mcp_server_config(config: &Config, auth: Option<&CodexAuth>) -> McpServerConfig {
    let bearer_token_env_var = codex_apps_mcp_bearer_token_env_var();
    let http_headers = if bearer_token_env_var.is_some() {
        None
    } else {
        codex_apps_mcp_http_headers(auth)
    };
    let url = codex_apps_mcp_url(&config.chatgpt_base_url);

    McpServerConfig {
        transport: McpServerTransportConfig::StreamableHttp {
            url,
            bearer_token_env_var,
            http_headers,
            env_http_headers: None,
        },
        enabled: true,
        disabled_reason: None,
        startup_timeout_sec: Some(Duration::from_secs(30)),
        tool_timeout_sec: None,
        enabled_tools: None,
        disabled_tools: None,
        scopes: None,
    }
}

pub(crate) fn with_codex_apps_mcp(
    mut servers: HashMap<String, McpServerConfig>,
    connectors_enabled: bool,
    auth: Option<&CodexAuth>,
    config: &Config,
) -> HashMap<String, McpServerConfig> {
    if connectors_enabled {
        servers.insert(
            CODEX_APPS_MCP_SERVER_NAME.to_string(),
            codex_apps_mcp_server_config(config, auth),
        );
    } else {
        servers.remove(CODEX_APPS_MCP_SERVER_NAME);
    }
    servers
}

pub(crate) fn effective_mcp_servers(
    config: &Config,
    auth: Option<&CodexAuth>,
) -> HashMap<String, McpServerConfig> {
    with_codex_apps_mcp(
        config.mcp_servers.get().clone(),
        config.features.enabled(Feature::Apps),
        auth,
        config,
    )
}
