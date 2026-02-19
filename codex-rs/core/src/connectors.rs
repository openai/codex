use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::sync::Mutex as StdMutex;
use std::time::Duration;
use std::time::Instant;

use async_channel::unbounded;
pub use codex_app_server_protocol::AppBranding;
pub use codex_app_server_protocol::AppInfo;
pub use codex_app_server_protocol::AppMetadata;
use codex_protocol::protocol::SandboxPolicy;
use serde::Deserialize;
use tracing::warn;

use crate::AuthManager;
use crate::CodexAuth;
use crate::SandboxState;
use crate::config::Config;
use crate::config::types::AppsConfigToml;
use crate::features::Feature;
use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;
use crate::mcp::auth::compute_auth_statuses;
use crate::mcp::with_codex_apps_mcp;
use crate::mcp_connection_manager::DEFAULT_STARTUP_TIMEOUT;
use crate::mcp_connection_manager::McpConnectionManager;
use crate::token_data::TokenData;

pub const CONNECTORS_CACHE_TTL: Duration = Duration::from_secs(3600);

#[derive(Clone, PartialEq, Eq)]
struct AccessibleConnectorsCacheKey {
    chatgpt_base_url: String,
    account_id: Option<String>,
    chatgpt_user_id: Option<String>,
    is_workspace_account: bool,
}

#[derive(Clone)]
struct CachedAccessibleConnectors {
    key: AccessibleConnectorsCacheKey,
    expires_at: Instant,
    connectors: Vec<AppInfo>,
}

static ACCESSIBLE_CONNECTORS_CACHE: LazyLock<StdMutex<Option<CachedAccessibleConnectors>>> =
    LazyLock::new(|| StdMutex::new(None));

pub async fn list_accessible_connectors_from_mcp_tools(
    config: &Config,
) -> anyhow::Result<Vec<AppInfo>> {
    list_accessible_connectors_from_mcp_tools_with_options(config, false).await
}

pub async fn list_cached_accessible_connectors_from_mcp_tools(
    config: &Config,
) -> Option<Vec<AppInfo>> {
    if !config.features.enabled(Feature::Apps) {
        return Some(Vec::new());
    }

    let auth_manager = auth_manager_from_config(config);
    let auth = auth_manager.auth().await;
    let cache_key = accessible_connectors_cache_key(config, auth.as_ref());
    read_cached_accessible_connectors(&cache_key)
}

pub async fn list_accessible_connectors_from_mcp_tools_with_options(
    config: &Config,
    force_refetch: bool,
) -> anyhow::Result<Vec<AppInfo>> {
    if !config.features.enabled(Feature::Apps) {
        return Ok(Vec::new());
    }

    let auth_manager = auth_manager_from_config(config);
    let auth = auth_manager.auth().await;
    let cache_key = accessible_connectors_cache_key(config, auth.as_ref());
    if !force_refetch && let Some(cached_connectors) = read_cached_accessible_connectors(&cache_key)
    {
        return Ok(cached_connectors);
    }

    let mcp_servers = with_codex_apps_mcp(HashMap::new(), true, auth.as_ref(), config);
    if mcp_servers.is_empty() {
        return Ok(Vec::new());
    }

    let auth_status_entries =
        compute_auth_statuses(mcp_servers.iter(), config.mcp_oauth_credentials_store_mode).await;

    let (tx_event, rx_event) = unbounded();
    drop(rx_event);

    let sandbox_state = SandboxState {
        sandbox_policy: SandboxPolicy::new_read_only_policy(),
        codex_linux_sandbox_exe: config.codex_linux_sandbox_exe.clone(),
        sandbox_cwd: env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
        use_linux_sandbox_bwrap: config.features.enabled(Feature::UseLinuxSandboxBwrap),
    };

    let (mcp_connection_manager, cancel_token) = McpConnectionManager::new(
        &mcp_servers,
        config.mcp_oauth_credentials_store_mode,
        auth_status_entries,
        &config.permissions.approval_policy,
        tx_event,
        sandbox_state,
    )
    .await;

    if force_refetch
        && let Err(err) = mcp_connection_manager
            .hard_refresh_codex_apps_tools_cache()
            .await
    {
        warn!(
            "failed to force-refresh tools for MCP server '{CODEX_APPS_MCP_SERVER_NAME}', using cached/startup tools: {err:#}"
        );
    }

    let codex_apps_ready = if let Some(cfg) = mcp_servers.get(CODEX_APPS_MCP_SERVER_NAME) {
        let timeout = cfg.startup_timeout_sec.unwrap_or(DEFAULT_STARTUP_TIMEOUT);
        mcp_connection_manager
            .wait_for_server_ready(CODEX_APPS_MCP_SERVER_NAME, timeout)
            .await
    } else {
        false
    };

    let tools = mcp_connection_manager.list_all_tools().await;
    cancel_token.cancel();

    let accessible_connectors = accessible_connectors_from_mcp_tools(&tools);
    if codex_apps_ready || !accessible_connectors.is_empty() {
        write_cached_accessible_connectors(cache_key, &accessible_connectors);
    }
    Ok(accessible_connectors)
}

fn accessible_connectors_cache_key(
    config: &Config,
    auth: Option<&CodexAuth>,
) -> AccessibleConnectorsCacheKey {
    let token_data: Option<TokenData> = auth.and_then(|auth| auth.get_token_data().ok());
    let account_id = token_data
        .as_ref()
        .and_then(|token_data| token_data.account_id.clone());
    let chatgpt_user_id = token_data
        .as_ref()
        .and_then(|token_data| token_data.id_token.chatgpt_user_id.clone());
    let is_workspace_account = token_data
        .as_ref()
        .is_some_and(|token_data| token_data.id_token.is_workspace_account());
    AccessibleConnectorsCacheKey {
        chatgpt_base_url: config.chatgpt_base_url.clone(),
        account_id,
        chatgpt_user_id,
        is_workspace_account,
    }
}

fn read_cached_accessible_connectors(
    cache_key: &AccessibleConnectorsCacheKey,
) -> Option<Vec<AppInfo>> {
    let mut cache_guard = ACCESSIBLE_CONNECTORS_CACHE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let now = Instant::now();

    if let Some(cached) = cache_guard.as_ref() {
        if now < cached.expires_at && cached.key == *cache_key {
            return Some(cached.connectors.clone());
        }
        if now >= cached.expires_at {
            *cache_guard = None;
        }
    }

    None
}

fn write_cached_accessible_connectors(
    cache_key: AccessibleConnectorsCacheKey,
    connectors: &[AppInfo],
) {
    let mut cache_guard = ACCESSIBLE_CONNECTORS_CACHE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *cache_guard = Some(CachedAccessibleConnectors {
        key: cache_key,
        expires_at: Instant::now() + CONNECTORS_CACHE_TTL,
        connectors: connectors.to_vec(),
    });
}

fn auth_manager_from_config(config: &Config) -> std::sync::Arc<AuthManager> {
    AuthManager::shared(
        config.codex_home.clone(),
        false,
        config.cli_auth_credentials_store_mode,
    )
}

pub fn connector_display_label(connector: &AppInfo) -> String {
    format_connector_label(&connector.name, &connector.id)
}

pub fn connector_mention_slug(connector: &AppInfo) -> String {
    connector_name_slug(&connector_display_label(connector))
}

pub(crate) fn accessible_connectors_from_mcp_tools(
    mcp_tools: &HashMap<String, crate::mcp_connection_manager::ToolInfo>,
) -> Vec<AppInfo> {
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
    connectors: Vec<AppInfo>,
    accessible_connectors: Vec<AppInfo>,
) -> Vec<AppInfo> {
    let mut merged: HashMap<String, AppInfo> = connectors
        .into_iter()
        .map(|mut connector| {
            connector.is_accessible = false;
            (connector.id.clone(), connector)
        })
        .collect();

    for mut connector in accessible_connectors {
        connector.is_accessible = true;
        let connector_id = connector.id.clone();
        if let Some(existing) = merged.get_mut(&connector_id) {
            existing.is_accessible = true;
            if existing.name == existing.id && connector.name != connector.id {
                existing.name = connector.name;
            }
            if existing.description.is_none() && connector.description.is_some() {
                existing.description = connector.description;
            }
            if existing.logo_url.is_none() && connector.logo_url.is_some() {
                existing.logo_url = connector.logo_url;
            }
            if existing.logo_url_dark.is_none() && connector.logo_url_dark.is_some() {
                existing.logo_url_dark = connector.logo_url_dark;
            }
            if existing.distribution_channel.is_none() && connector.distribution_channel.is_some() {
                existing.distribution_channel = connector.distribution_channel;
            }
        } else {
            merged.insert(connector_id, connector);
        }
    }

    let mut merged = merged.into_values().collect::<Vec<_>>();
    for connector in &mut merged {
        if connector.install_url.is_none() {
            connector.install_url = Some(connector_install_url(&connector.name, &connector.id));
        }
    }
    merged.sort_by(|left, right| {
        right
            .is_accessible
            .cmp(&left.is_accessible)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.id.cmp(&right.id))
    });
    merged
}

pub fn with_app_enabled_state(mut connectors: Vec<AppInfo>, config: &Config) -> Vec<AppInfo> {
    let apps = read_apps_config(config).map(|apps_config| apps_config.apps);
    for connector in &mut connectors {
        if let Some(app) = apps.as_ref().and_then(|apps| apps.get(&connector.id)) {
            connector.is_enabled = app.enabled;
        }
    }
    connectors
}

fn read_apps_config(config: &Config) -> Option<AppsConfigToml> {
    let effective_config = config.config_layer_stack.effective_config();
    let apps_config = effective_config.as_table()?.get("apps")?.clone();
    AppsConfigToml::deserialize(apps_config).ok()
}

fn collect_accessible_connectors<I>(tools: I) -> Vec<AppInfo>
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
    let mut accessible: Vec<AppInfo> = connectors
        .into_iter()
        .map(|(connector_id, connector_name)| AppInfo {
            id: connector_id.clone(),
            name: connector_name.clone(),
            description: None,
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: Some(connector_install_url(&connector_name, &connector_id)),
            is_accessible: true,
            is_enabled: true,
        })
        .collect();
    accessible.sort_by(|left, right| {
        right
            .is_accessible
            .cmp(&left.is_accessible)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.id.cmp(&right.id))
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

pub fn connector_name_slug(name: &str) -> String {
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
