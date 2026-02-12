pub mod auth;
mod skill_dependencies;
pub(crate) use skill_dependencies::maybe_prompt_and_install_mcp_dependencies;

use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::time::Duration;

use async_channel::unbounded;
use codex_protocol::mcp::Resource;
use codex_protocol::mcp::ResourceTemplate;
use codex_protocol::mcp::Tool;
use codex_protocol::protocol::McpListToolsResponseEvent;
use codex_protocol::protocol::SandboxPolicy;
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use crate::AuthManager;
use crate::CodexAuth;
use crate::config::Config;
use crate::config::types::McpServerConfig;
use crate::config::types::McpServerTransportConfig;
use crate::features::Feature;
use crate::mcp::auth::compute_auth_statuses;
use crate::mcp_connection_manager::McpConnectionManager;
use crate::mcp_connection_manager::SandboxState;

const MCP_TOOL_NAME_PREFIX: &str = "mcp";
const MCP_TOOL_NAME_DELIMITER: &str = "__";
pub(crate) const CODEX_APPS_MCP_SERVER_NAME: &str = "codex_apps";
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
        required: false,
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
        sandbox_policy: SandboxPolicy::new_read_only_policy(),
        codex_linux_sandbox_exe: config.codex_linux_sandbox_exe.clone(),
        sandbox_cwd: env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
        use_linux_sandbox_bwrap: config.features.enabled(Feature::UseLinuxSandboxBwrap),
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

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn make_tool(name: &str) -> Tool {
        Tool {
            name: name.to_string(),
            title: None,
            description: None,
            input_schema: serde_json::json!({"type": "object", "properties": {}}),
            output_schema: None,
            annotations: None,
            icons: None,
            meta: None,
        }
    }

    #[test]
    fn split_qualified_tool_name_returns_server_and_tool() {
        assert_eq!(
            split_qualified_tool_name("mcp__alpha__do_thing"),
            Some(("alpha".to_string(), "do_thing".to_string()))
        );
    }

    #[test]
    fn split_qualified_tool_name_rejects_invalid_names() {
        assert_eq!(split_qualified_tool_name("other__alpha__do_thing"), None);
        assert_eq!(split_qualified_tool_name("mcp__alpha__"), None);
    }

    #[test]
    fn group_tools_by_server_strips_prefix_and_groups() {
        let mut tools = HashMap::new();
        tools.insert("mcp__alpha__do_thing".to_string(), make_tool("do_thing"));
        tools.insert(
            "mcp__alpha__nested__op".to_string(),
            make_tool("nested__op"),
        );
        tools.insert("mcp__beta__do_other".to_string(), make_tool("do_other"));

        let mut expected_alpha = HashMap::new();
        expected_alpha.insert("do_thing".to_string(), make_tool("do_thing"));
        expected_alpha.insert("nested__op".to_string(), make_tool("nested__op"));

        let mut expected_beta = HashMap::new();
        expected_beta.insert("do_other".to_string(), make_tool("do_other"));

        let mut expected = HashMap::new();
        expected.insert("alpha".to_string(), expected_alpha);
        expected.insert("beta".to_string(), expected_beta);

        assert_eq!(group_tools_by_server(&tools), expected);
    }
}
