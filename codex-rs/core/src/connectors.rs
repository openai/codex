use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

use async_channel::unbounded;
use codex_protocol::protocol::SandboxPolicy;
use serde::Deserialize;
use serde::Serialize;
use tokio_util::sync::CancellationToken;

use crate::AuthManager;
use crate::SandboxState;
use crate::config::Config;
use crate::features::Feature;
use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;
use crate::mcp::auth::compute_auth_statuses;
use crate::mcp::with_codex_apps_mcp;
use crate::mcp_connection_manager::McpConnectionManager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorInfo {
    #[serde(rename = "id")]
    pub connector_id: String,
    #[serde(rename = "name")]
    pub connector_name: String,
    #[serde(default, rename = "description")]
    pub connector_description: Option<String>,
    #[serde(default, rename = "logo_url")]
    pub logo_url: Option<String>,
    #[serde(default, rename = "install_url")]
    pub install_url: Option<String>,
    #[serde(default)]
    pub is_accessible: bool,
}

pub async fn list_accessible_connectors_from_mcp_tools(
    config: &Config,
) -> anyhow::Result<Vec<ConnectorInfo>> {
    if !config.features.enabled(Feature::Connectors) {
        return Ok(Vec::new());
    }

    let auth_manager = auth_manager_from_config(config);
    let auth = auth_manager.auth().await;
    let mcp_servers = with_codex_apps_mcp(HashMap::new(), true, auth.as_ref(), config);
    if mcp_servers.is_empty() {
        return Ok(Vec::new());
    }

    let auth_status_entries =
        compute_auth_statuses(mcp_servers.iter(), config.mcp_oauth_credentials_store_mode).await;

    let mut mcp_connection_manager = McpConnectionManager::default();
    let (tx_event, rx_event) = unbounded();
    drop(rx_event);
    let cancel_token = CancellationToken::new();

    let sandbox_state = SandboxState {
        sandbox_policy: SandboxPolicy::ReadOnly,
        codex_linux_sandbox_exe: config.codex_linux_sandbox_exe.clone(),
        sandbox_cwd: env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
    };

    mcp_connection_manager
        .initialize(
            &mcp_servers,
            config.mcp_oauth_credentials_store_mode,
            auth_status_entries,
            tx_event,
            cancel_token.clone(),
            sandbox_state,
        )
        .await;

    let tools = mcp_connection_manager.list_all_tools().await;
    cancel_token.cancel();

    Ok(accessible_connectors_from_mcp_tools(&tools))
}

fn auth_manager_from_config(config: &Config) -> std::sync::Arc<AuthManager> {
    AuthManager::shared(
        config.codex_home.clone(),
        false,
        config.cli_auth_credentials_store_mode,
    )
}

pub fn connector_display_label(connector: &ConnectorInfo) -> String {
    format_connector_label(&connector.connector_name, &connector.connector_id)
}

pub(crate) fn accessible_connectors_from_mcp_tools(
    mcp_tools: &HashMap<String, crate::mcp_connection_manager::ToolInfo>,
) -> Vec<ConnectorInfo> {
    let tools = mcp_tools.values().filter_map(|tool| {
        if tool.server_name != CODEX_APPS_MCP_SERVER_NAME {
            return None;
        }
        let connector_id = tool.connector_id.as_deref()?;
        let connector_name = normalize_connector_value(tool.connector_name.as_deref());
        Some((connector_id.to_string(), connector_name))
    });
    collect_accessible_connectors(tools)
}

pub fn merge_connectors(
    connectors: Vec<ConnectorInfo>,
    accessible_connectors: Vec<ConnectorInfo>,
) -> Vec<ConnectorInfo> {
    let mut merged: HashMap<String, ConnectorInfo> = connectors
        .into_iter()
        .map(|mut connector| {
            connector.is_accessible = false;
            (connector.connector_id.clone(), connector)
        })
        .collect();

    for mut connector in accessible_connectors {
        connector.is_accessible = true;
        let connector_id = connector.connector_id.clone();
        if let Some(existing) = merged.get_mut(&connector_id) {
            existing.is_accessible = true;
            if existing.connector_name == existing.connector_id
                && connector.connector_name != connector.connector_id
            {
                existing.connector_name = connector.connector_name;
            }
            if existing.connector_description.is_none() && connector.connector_description.is_some()
            {
                existing.connector_description = connector.connector_description;
            }
            if existing.logo_url.is_none() && connector.logo_url.is_some() {
                existing.logo_url = connector.logo_url;
            }
        } else {
            merged.insert(connector_id, connector);
        }
    }

    let mut merged = merged.into_values().collect::<Vec<_>>();
    for connector in &mut merged {
        if connector.install_url.is_none() {
            connector.install_url = Some(connector_install_url(
                &connector.connector_name,
                &connector.connector_id,
            ));
        }
    }
    merged.sort_by(|left, right| {
        right
            .is_accessible
            .cmp(&left.is_accessible)
            .then_with(|| left.connector_name.cmp(&right.connector_name))
            .then_with(|| left.connector_id.cmp(&right.connector_id))
    });
    merged
}

fn collect_accessible_connectors<I>(tools: I) -> Vec<ConnectorInfo>
where
    I: IntoIterator<Item = (String, Option<String>)>,
{
    let mut connectors: HashMap<String, String> = HashMap::new();
    for (connector_id, connector_name) in tools {
        let connector_name = connector_name.unwrap_or_else(|| connector_id.clone());
        if let Some(existing_name) = connectors.get_mut(&connector_id) {
            if existing_name == &connector_id && connector_name != connector_id {
                *existing_name = connector_name;
            }
        } else {
            connectors.insert(connector_id, connector_name);
        }
    }
    let mut accessible: Vec<ConnectorInfo> = connectors
        .into_iter()
        .map(|(connector_id, connector_name)| ConnectorInfo {
            install_url: Some(connector_install_url(&connector_name, &connector_id)),
            connector_id,
            connector_name,
            connector_description: None,
            logo_url: None,
            is_accessible: true,
        })
        .collect();
    accessible.sort_by(|left, right| {
        right
            .is_accessible
            .cmp(&left.is_accessible)
            .then_with(|| left.connector_name.cmp(&right.connector_name))
            .then_with(|| left.connector_id.cmp(&right.connector_id))
    });
    accessible
}

fn normalize_connector_value(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub fn connector_install_url(name: &str, connector_id: &str) -> String {
    let slug = connector_name_slug(name);
    format!("https://chatgpt.com/apps/{slug}/{connector_id}")
}

fn connector_name_slug(name: &str) -> String {
    let mut normalized = String::with_capacity(name.len());
    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            normalized.push(character.to_ascii_lowercase());
        } else {
            normalized.push('-');
        }
    }
    let normalized = normalized.trim_matches('-');
    if normalized.is_empty() {
        "app".to_string()
    } else {
        normalized.to_string()
    }
}

fn format_connector_label(name: &str, _id: &str) -> String {
    name.to_string()
}
