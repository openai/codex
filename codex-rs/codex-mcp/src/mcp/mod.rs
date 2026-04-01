pub mod auth;
mod skill_dependencies;
pub use skill_dependencies::canonical_mcp_server_key;
pub use skill_dependencies::collect_missing_mcp_dependencies;

use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::time::Duration;

use async_channel::unbounded;
use codex_config::Constrained;
use codex_config::McpServerConfig;
use codex_config::McpServerTransportConfig;
use codex_login::AuthCredentialsStoreMode;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_plugin::PluginCapabilitySummary;
use codex_protocol::mcp::Resource;
use codex_protocol::mcp::ResourceTemplate;
use codex_protocol::mcp::Tool;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::McpListToolsResponseEvent;
use codex_protocol::protocol::SandboxPolicy;
use codex_rmcp_client::OAuthCredentialsStoreMode;
use serde_json::Value;

use crate::mcp::auth::compute_auth_statuses;
use crate::mcp_connection_manager::McpConnectionManager;
use crate::mcp_connection_manager::SandboxState;
use crate::mcp_connection_manager::codex_apps_tools_cache_key;
pub type McpManager = McpConnectionManager;

const MCP_TOOL_NAME_PREFIX: &str = "mcp";
const MCP_TOOL_NAME_DELIMITER: &str = "__";
pub const CODEX_APPS_MCP_SERVER_NAME: &str = "codex_apps";
const CODEX_CONNECTORS_TOKEN_ENV_VAR: &str = "CODEX_CONNECTORS_TOKEN";

/// The Responses API requires tool names to match `^[a-zA-Z0-9_-]+$`.
/// MCP server/tool names are user-controlled, so sanitize the fully-qualified
/// name we expose to the model by replacing any disallowed character with `_`.
pub(crate) fn sanitize_responses_api_tool_name(name: &str) -> String {
    let mut sanitized = String::with_capacity(name.len());
    for c in name.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            sanitized.push(c);
        } else {
            sanitized.push('_');
        }
    }

    if sanitized.is_empty() {
        "_".to_string()
    } else {
        sanitized
    }
}

pub fn qualified_mcp_tool_name_prefix(server_name: &str) -> String {
    sanitize_responses_api_tool_name(&format!(
        "{MCP_TOOL_NAME_PREFIX}{MCP_TOOL_NAME_DELIMITER}{server_name}{MCP_TOOL_NAME_DELIMITER}"
    ))
}

#[derive(Debug, Clone)]
pub struct McpConfig {
    pub chatgpt_base_url: String,
    pub codex_home: PathBuf,
    pub cli_auth_credentials_store_mode: AuthCredentialsStoreMode,
    pub mcp_oauth_credentials_store_mode: OAuthCredentialsStoreMode,
    pub mcp_oauth_callback_port: Option<u16>,
    pub mcp_oauth_callback_url: Option<String>,
    pub skill_mcp_dependency_install_enabled: bool,
    pub approval_policy: Constrained<AskForApproval>,
    pub codex_linux_sandbox_exe: Option<PathBuf>,
    pub use_legacy_landlock: bool,
    pub connectors_enabled: bool,
    pub configured_mcp_servers: HashMap<String, McpServerConfig>,
    pub plugin_capability_summaries: Vec<PluginCapabilitySummary>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolPluginProvenance {
    plugin_display_names_by_connector_id: HashMap<String, Vec<String>>,
    plugin_display_names_by_mcp_server_name: HashMap<String, Vec<String>>,
}

impl ToolPluginProvenance {
    pub fn plugin_display_names_for_connector_id(&self, connector_id: &str) -> &[String] {
        self.plugin_display_names_by_connector_id
            .get(connector_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn plugin_display_names_for_mcp_server_name(&self, server_name: &str) -> &[String] {
        self.plugin_display_names_by_mcp_server_name
            .get(server_name)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn from_capability_summaries(capability_summaries: &[PluginCapabilitySummary]) -> Self {
        let mut tool_plugin_provenance = Self::default();
        for plugin in capability_summaries {
            for connector_id in &plugin.app_connector_ids {
                tool_plugin_provenance
                    .plugin_display_names_by_connector_id
                    .entry(connector_id.0.clone())
                    .or_default()
                    .push(plugin.display_name.clone());
            }

            for server_name in &plugin.mcp_server_names {
                tool_plugin_provenance
                    .plugin_display_names_by_mcp_server_name
                    .entry(server_name.clone())
                    .or_default()
                    .push(plugin.display_name.clone());
            }
        }

        for plugin_names in tool_plugin_provenance
            .plugin_display_names_by_connector_id
            .values_mut()
            .chain(
                tool_plugin_provenance
                    .plugin_display_names_by_mcp_server_name
                    .values_mut(),
            )
        {
            plugin_names.sort_unstable();
            plugin_names.dedup();
        }

        tool_plugin_provenance
    }
}

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

fn normalize_codex_apps_base_url(base_url: &str) -> String {
    let mut base_url = base_url.trim_end_matches('/').to_string();
    if (base_url.starts_with("https://chatgpt.com")
        || base_url.starts_with("https://chat.openai.com"))
        && !base_url.contains("/backend-api")
    {
        base_url = format!("{base_url}/backend-api");
    }
    base_url
}

fn codex_apps_mcp_url_for_base_url(base_url: &str) -> String {
    let base_url = normalize_codex_apps_base_url(base_url);
    if base_url.contains("/backend-api") {
        format!("{base_url}/wham/apps")
    } else if base_url.contains("/api/codex") {
        format!("{base_url}/apps")
    } else {
        format!("{base_url}/api/codex/apps")
    }
}

pub(crate) fn codex_apps_mcp_url(config: &McpConfig) -> String {
    codex_apps_mcp_url_for_base_url(&config.chatgpt_base_url)
}

fn codex_apps_mcp_server_config(config: &McpConfig, auth: Option<&CodexAuth>) -> McpServerConfig {
    let bearer_token_env_var = codex_apps_mcp_bearer_token_env_var();
    let http_headers = if bearer_token_env_var.is_some() {
        None
    } else {
        codex_apps_mcp_http_headers(auth)
    };
    let url = codex_apps_mcp_url(config);

    McpServerConfig {
        transport: McpServerTransportConfig::StreamableHttp {
            url,
            bearer_token_env_var,
            http_headers,
            env_http_headers: None,
        },
        enabled: true,
        required: false,
        disabled_reason: None,
        startup_timeout_sec: Some(Duration::from_secs(30)),
        tool_timeout_sec: None,
        enabled_tools: None,
        disabled_tools: None,
        scopes: None,
        oauth_resource: None,
        tools: HashMap::new(),
    }
}

pub fn with_codex_apps_mcp(
    mut servers: HashMap<String, McpServerConfig>,
    auth: Option<&CodexAuth>,
    config: &McpConfig,
) -> HashMap<String, McpServerConfig> {
    if config.connectors_enabled {
        servers.insert(
            CODEX_APPS_MCP_SERVER_NAME.to_string(),
            codex_apps_mcp_server_config(config, auth),
        );
    } else {
        servers.remove(CODEX_APPS_MCP_SERVER_NAME);
    }
    servers
}

pub fn configured_mcp_servers(config: &McpConfig) -> HashMap<String, McpServerConfig> {
    config.configured_mcp_servers.clone()
}

pub fn effective_mcp_servers(
    config: &McpConfig,
    auth: Option<&CodexAuth>,
) -> HashMap<String, McpServerConfig> {
    let servers = configured_mcp_servers(config);
    with_codex_apps_mcp(servers, auth, config)
}

pub fn tool_plugin_provenance(config: &McpConfig) -> ToolPluginProvenance {
    ToolPluginProvenance::from_capability_summaries(&config.plugin_capability_summaries)
}

pub async fn collect_mcp_snapshot(
    config: &McpConfig,
    submit_id: String,
) -> McpListToolsResponseEvent {
    let auth_manager = AuthManager::shared(
        config.codex_home.clone(),
        /*enable_codex_api_key_env*/ false,
        config.cli_auth_credentials_store_mode,
    );
    let auth = auth_manager.auth().await;
    let mcp_servers = effective_mcp_servers(config, auth.as_ref());
    let tool_plugin_provenance = tool_plugin_provenance(config);
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

    let (tx_event, rx_event) = unbounded();
    drop(rx_event);

    // Use ReadOnly sandbox policy for MCP snapshot collection (safest default)
    let sandbox_state = SandboxState {
        sandbox_policy: SandboxPolicy::new_read_only_policy(),
        codex_linux_sandbox_exe: config.codex_linux_sandbox_exe.clone(),
        sandbox_cwd: env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
        use_legacy_landlock: config.use_legacy_landlock,
    };

    let (mcp_connection_manager, cancel_token) = McpConnectionManager::new(
        &mcp_servers,
        config.mcp_oauth_credentials_store_mode,
        auth_status_entries.clone(),
        &config.approval_policy,
        submit_id,
        tx_event,
        sandbox_state,
        config.codex_home.clone(),
        codex_apps_tools_cache_key(auth.as_ref()),
        tool_plugin_provenance,
    )
    .await;

    let snapshot =
        collect_mcp_snapshot_from_manager(&mcp_connection_manager, auth_status_entries).await;

    cancel_token.cancel();

    snapshot
}

pub fn split_qualified_tool_name(qualified_name: &str) -> Option<(String, String)> {
    let mut parts = qualified_name.split(MCP_TOOL_NAME_DELIMITER);
    let prefix = parts.next()?;
    if prefix != MCP_TOOL_NAME_PREFIX {
        return None;
    }
    let server_name = parts.next()?;
    let tool_name: String = parts.collect::<Vec<_>>().join(MCP_TOOL_NAME_DELIMITER);
    if tool_name.is_empty() {
        return None;
    }
    Some((server_name.to_string(), tool_name))
}

pub fn group_tools_by_server(
    tools: &HashMap<String, Tool>,
) -> HashMap<String, HashMap<String, Tool>> {
    let mut grouped = HashMap::new();
    for (qualified_name, tool) in tools {
        if let Some((server_name, tool_name)) = split_qualified_tool_name(qualified_name) {
            grouped
                .entry(server_name)
                .or_insert_with(HashMap::new)
                .insert(tool_name, tool.clone());
        }
    }
    grouped
}

pub async fn collect_mcp_snapshot_from_manager(
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
        .collect::<HashMap<_, _>>();

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
        .collect::<HashMap<_, _>>();

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
        .collect::<HashMap<_, _>>();

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
        .collect::<HashMap<_, _>>();

    McpListToolsResponseEvent {
        tools,
        resources,
        resource_templates,
        auth_statuses,
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
