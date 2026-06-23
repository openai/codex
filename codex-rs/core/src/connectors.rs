use std::collections::HashSet;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex as StdMutex;
use std::time::Duration;
use std::time::Instant;

use async_channel::unbounded;
pub use codex_connectors::AppBranding;
pub use codex_connectors::AppInfo;
pub use codex_connectors::AppMetadata;
use codex_connectors::ConnectorDirectoryCacheContext;
use codex_connectors::ConnectorDirectoryCacheKey;
use codex_connectors::app_is_enabled;
use codex_connectors::apps_config_from_layer_stack;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecServerRuntimePaths;
use codex_protocol::models::PermissionProfile;
use codex_tools::DiscoverableTool;
use tokio_util::sync::CancellationToken;
use tracing::instrument;
use tracing::warn;

use crate::config::Config;
use crate::mcp::McpManager;
use crate::plugins::list_tool_suggest_discoverable_plugins;
use crate::session::INITIAL_SUBMIT_ID;
use codex_config::types::ApprovalsReviewer;
use codex_config::types::ToolSuggestDiscoverableType;
use codex_core_plugins::PluginsManager;
use codex_features::Feature;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::MCP_TOOL_CODEX_APPS_META_KEY;
use codex_mcp::McpConnectionManager;
use codex_mcp::McpRuntimeContext;
use codex_mcp::ToolInfo;
use codex_mcp::ToolPluginProvenance;
use codex_mcp::codex_apps_tools_cache_key;
use codex_mcp::compute_auth_statuses;
use codex_mcp::effective_mcp_servers;
use codex_mcp::tool_plugin_provenance;

const CONNECTORS_READY_TIMEOUT_ON_EMPTY_TOOLS: Duration = Duration::from_secs(30);

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

#[derive(Debug, Clone)]
pub struct AccessibleConnectorsStatus {
    pub connectors: Vec<AppInfo>,
    pub codex_apps_ready: bool,
}

pub async fn list_accessible_connectors_from_mcp_tools(
    config: &Config,
) -> anyhow::Result<Vec<AppInfo>> {
    Ok(
        list_accessible_connectors_from_mcp_tools_with_options_and_status(
            config, /*force_refetch*/ false,
        )
        .await?
        .connectors,
    )
}

pub(crate) async fn list_accessible_and_enabled_connectors_from_manager(
    mcp_connection_manager: &McpConnectionManager,
    config: &Config,
) -> Vec<AppInfo> {
    with_app_enabled_state(
        accessible_connectors_from_mcp_tools(&mcp_connection_manager.list_all_tools().await),
        config,
    )
    .into_iter()
    .filter(|connector| connector.is_accessible && connector.is_enabled)
    .collect()
}

#[instrument(level = "trace", skip_all)]
pub(crate) async fn list_tool_suggest_discoverable_tools_with_auth(
    config: &Config,
    plugins_manager: &PluginsManager,
    auth: Option<&CodexAuth>,
    accessible_connectors: &[AppInfo],
    loaded_plugin_app_connector_ids: &[String],
) -> anyhow::Result<Vec<DiscoverableTool>> {
    let connector_ids = tool_suggest_connector_ids(config, loaded_plugin_app_connector_ids);
    let directory_connectors = codex_connectors::merge::merge_plugin_connectors(
        cached_directory_connectors_for_tool_suggest_with_auth(config, auth).await,
        connector_ids.iter().cloned(),
    );
    let discoverable_connectors =
        codex_connectors::filter::filter_tool_suggest_discoverable_connectors(
            directory_connectors,
            accessible_connectors,
            &connector_ids,
        )
        .into_iter()
        .map(DiscoverableTool::from);
    let discoverable_plugins = list_tool_suggest_discoverable_plugins(
        config,
        plugins_manager,
        auth,
        loaded_plugin_app_connector_ids,
    )
    .await?
    .into_iter()
    .map(DiscoverableTool::from);
    Ok(discoverable_connectors
        .chain(discoverable_plugins)
        .collect())
}

pub async fn list_cached_accessible_connectors_from_mcp_tools(
    config: &Config,
) -> Option<Vec<AppInfo>> {
    let auth_manager =
        AuthManager::shared_from_config(config, /*enable_codex_api_key_env*/ false).await;
    let auth = auth_manager.auth().await;
    if !config
        .features
        .apps_enabled_for_auth(auth.as_ref().is_some_and(CodexAuth::uses_codex_backend))
    {
        return Some(Vec::new());
    }
    let cache_key = accessible_connectors_cache_key(config, auth.as_ref());
    read_cached_accessible_connectors(&cache_key)
}

pub(crate) fn refresh_accessible_connectors_cache_from_mcp_tools(
    config: &Config,
    auth: Option<&CodexAuth>,
    mcp_tools: &[ToolInfo],
) {
    if !config.features.enabled(Feature::Apps) {
        return;
    }

    let cache_key = accessible_connectors_cache_key(config, auth);
    let accessible_connectors = accessible_connectors_for_app_list_from_mcp_tools(mcp_tools);
    write_cached_accessible_connectors(cache_key, &accessible_connectors);
}

pub async fn list_accessible_connectors_from_mcp_tools_with_options(
    config: &Config,
    force_refetch: bool,
) -> anyhow::Result<Vec<AppInfo>> {
    Ok(
        list_accessible_connectors_from_mcp_tools_with_options_and_status(config, force_refetch)
            .await?
            .connectors,
    )
}

pub async fn list_accessible_connectors_from_mcp_tools_with_options_and_status(
    config: &Config,
    force_refetch: bool,
) -> anyhow::Result<AccessibleConnectorsStatus> {
    // TODO: Wire callers that already own an EnvironmentManager into
    // list_accessible_connectors_from_mcp_tools_with_environment_manager instead
    // of constructing a temporary manager here.
    let local_runtime_paths = ExecServerRuntimePaths::from_optional_paths(
        config.codex_self_exe.clone(),
        config.codex_linux_sandbox_exe.clone(),
    )?;
    let environment_manager =
        EnvironmentManager::from_codex_home(config.codex_home.clone(), Some(local_runtime_paths))
            .await?;
    list_accessible_connectors_from_mcp_tools_with_environment_manager(
        config,
        force_refetch,
        Arc::new(environment_manager),
    )
    .await
}

pub async fn list_accessible_connectors_from_mcp_tools_with_environment_manager(
    config: &Config,
    force_refetch: bool,
    environment_manager: Arc<EnvironmentManager>,
) -> anyhow::Result<AccessibleConnectorsStatus> {
    let plugins_manager = Arc::new(PluginsManager::new(config.codex_home.to_path_buf()));
    let mcp_manager = Arc::new(McpManager::new(plugins_manager));
    list_accessible_connectors_from_mcp_tools_with_mcp_manager(
        config,
        force_refetch,
        environment_manager,
        mcp_manager,
    )
    .await
}

pub async fn list_accessible_connectors_from_mcp_tools_with_mcp_manager(
    config: &Config,
    force_refetch: bool,
    environment_manager: Arc<EnvironmentManager>,
    mcp_manager: Arc<McpManager>,
) -> anyhow::Result<AccessibleConnectorsStatus> {
    let auth_manager =
        AuthManager::shared_from_config(config, /*enable_codex_api_key_env*/ false).await;
    let auth = auth_manager.auth().await;
    if !config
        .features
        .apps_enabled_for_auth(auth.as_ref().is_some_and(CodexAuth::uses_codex_backend))
    {
        return Ok(AccessibleConnectorsStatus {
            connectors: Vec::new(),
            codex_apps_ready: true,
        });
    }
    let cache_key = accessible_connectors_cache_key(config, auth.as_ref());
    let mcp_config = mcp_manager.runtime_config(config).await;
    let tool_plugin_provenance = tool_plugin_provenance(&mcp_config);
    if !force_refetch && let Some(cached_connectors) = read_cached_accessible_connectors(&cache_key)
    {
        let cached_connectors = with_app_plugin_sources(cached_connectors, &tool_plugin_provenance);
        return Ok(AccessibleConnectorsStatus {
            connectors: cached_connectors,
            codex_apps_ready: true,
        });
    }

    let mut mcp_servers = effective_mcp_servers(&mcp_config, auth.as_ref());
    mcp_servers.retain(|name, _| name == CODEX_APPS_MCP_SERVER_NAME);
    if mcp_servers.is_empty() {
        return Ok(AccessibleConnectorsStatus {
            connectors: Vec::new(),
            codex_apps_ready: true,
        });
    }

    let runtime_context =
        McpRuntimeContext::new(Arc::clone(&environment_manager), config.cwd.to_path_buf());
    let auth_status_entries = compute_auth_statuses(
        mcp_servers.iter(),
        config.mcp_oauth_credentials_store_mode,
        config.auth_keyring_backend_kind(),
        auth.as_ref(),
        &runtime_context,
    )
    .await;

    let (tx_event, rx_event) = unbounded();
    drop(rx_event);

    let cancel_token = CancellationToken::new();
    let mcp_connection_manager = McpConnectionManager::new(
        &mcp_servers,
        config.mcp_oauth_credentials_store_mode,
        config.auth_keyring_backend_kind(),
        auth_status_entries,
        &config.permissions.approval_policy,
        INITIAL_SUBMIT_ID.to_owned(),
        tx_event,
        cancel_token.clone(),
        PermissionProfile::default(),
        // Connector discovery is threadless. Use an actually configured env if
        // one exists, but do not reintroduce the old hidden-local fallback.
        runtime_context,
        config.codex_home.to_path_buf(),
        codex_apps_tools_cache_key(auth.as_ref()),
        mcp_config.prefix_mcp_tool_names,
        mcp_config.client_elicitation_capability,
        /*supports_openai_form_elicitation*/ false,
        ToolPluginProvenance::default(),
        auth.as_ref(),
        /*elicitation_reviewer*/ None,
    )
    .await;

    let refreshed_tools = if force_refetch {
        match mcp_connection_manager
            .hard_refresh_codex_apps_tools_cache()
            .await
        {
            Ok(tools) => Some(tools),
            Err(err) => {
                warn!(
                    "failed to force-refresh tools for MCP server '{CODEX_APPS_MCP_SERVER_NAME}', using cached/startup tools: {err:#}"
                );
                None
            }
        }
    } else {
        None
    };
    let refreshed_tools_succeeded = refreshed_tools.is_some();

    let mut tools = if let Some(tools) = refreshed_tools {
        tools
    } else {
        mcp_connection_manager.list_all_tools().await
    };
    let mut should_reload_tools = false;
    let codex_apps_ready = if refreshed_tools_succeeded {
        true
    } else if let Some(cfg) = mcp_servers.get(CODEX_APPS_MCP_SERVER_NAME) {
        let immediate_ready = mcp_connection_manager
            .wait_for_server_ready(CODEX_APPS_MCP_SERVER_NAME, Duration::ZERO)
            .await;
        if immediate_ready {
            true
        } else if tools.is_empty() {
            let timeout = cfg
                .configured_config()
                .and_then(|config| config.startup_timeout_sec)
                .unwrap_or(CONNECTORS_READY_TIMEOUT_ON_EMPTY_TOOLS);
            let ready = mcp_connection_manager
                .wait_for_server_ready(CODEX_APPS_MCP_SERVER_NAME, timeout)
                .await;
            should_reload_tools = ready;
            ready
        } else {
            false
        }
    } else {
        false
    };
    if should_reload_tools {
        tools = mcp_connection_manager.list_all_tools().await;
    }
    if codex_apps_ready {
        cancel_token.cancel();
    }

    let accessible_connectors = accessible_connectors_for_app_list_from_mcp_tools(&tools);
    if codex_apps_ready || !accessible_connectors.is_empty() {
        write_cached_accessible_connectors(cache_key, &accessible_connectors);
    }
    let accessible_connectors =
        with_app_plugin_sources(accessible_connectors, &tool_plugin_provenance);
    mcp_connection_manager.shutdown().await;
    Ok(AccessibleConnectorsStatus {
        connectors: accessible_connectors,
        codex_apps_ready,
    })
}

fn accessible_connectors_cache_key(
    config: &Config,
    auth: Option<&CodexAuth>,
) -> AccessibleConnectorsCacheKey {
    let account_id = auth.and_then(CodexAuth::get_account_id);
    let chatgpt_user_id = auth.and_then(CodexAuth::get_chatgpt_user_id);
    let is_workspace_account = auth.is_some_and(CodexAuth::is_workspace_account);
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
        expires_at: Instant::now() + codex_connectors::CONNECTORS_CACHE_TTL,
        connectors: connectors.to_vec(),
    });
}

fn tool_suggest_connector_ids(
    config: &Config,
    loaded_plugin_app_connector_ids: &[String],
) -> HashSet<String> {
    let mut connector_ids = loaded_plugin_app_connector_ids
        .iter()
        .cloned()
        .collect::<HashSet<_>>();
    connector_ids.extend(
        config
            .tool_suggest
            .discoverables
            .iter()
            .filter(|discoverable| discoverable.kind == ToolSuggestDiscoverableType::Connector)
            .map(|discoverable| discoverable.id.clone()),
    );
    let disabled_connector_ids = config
        .tool_suggest
        .disabled_tools
        .iter()
        .filter(|disabled_tool| disabled_tool.kind == ToolSuggestDiscoverableType::Connector)
        .map(|disabled_tool| disabled_tool.id.as_str())
        .collect::<HashSet<_>>();
    connector_ids.retain(|connector_id| !disabled_connector_ids.contains(connector_id.as_str()));
    connector_ids
}

#[instrument(level = "trace", skip_all)]
async fn cached_directory_connectors_for_tool_suggest_with_auth(
    config: &Config,
    auth: Option<&CodexAuth>,
) -> Vec<AppInfo> {
    if !config.features.enabled(Feature::Apps) {
        return Vec::new();
    }

    let loaded_auth;
    let auth = if let Some(auth) = auth {
        Some(auth)
    } else {
        let auth_manager =
            AuthManager::shared_from_config(config, /*enable_codex_api_key_env*/ false).await;
        loaded_auth = auth_manager.auth().await;
        loaded_auth.as_ref()
    };
    let Some(auth) = auth.filter(|auth| auth.uses_codex_backend()) else {
        return Vec::new();
    };

    let account_id = match auth.get_account_id() {
        Some(account_id) if !account_id.is_empty() => account_id,
        _ => return Vec::new(),
    };
    let is_workspace_account = auth.is_workspace_account();
    let cache_context = ConnectorDirectoryCacheContext::new(
        config.codex_home.to_path_buf(),
        ConnectorDirectoryCacheKey::new(
            config.chatgpt_base_url.clone(),
            Some(account_id),
            auth.get_chatgpt_user_id(),
            is_workspace_account,
        ),
    );

    codex_connectors::cached_directory_connectors(&cache_context).unwrap_or_default()
}

pub(crate) fn accessible_connectors_from_mcp_tools(mcp_tools: &[ToolInfo]) -> Vec<AppInfo> {
    collect_accessible_connectors_from_mcp_tools(mcp_tools.iter())
}

fn collect_accessible_connectors_from_mcp_tools<'a>(
    mcp_tools: impl Iterator<Item = &'a ToolInfo>,
) -> Vec<AppInfo> {
    // ToolInfo already carries plugin provenance, so app-level plugin sources
    // can be derived here instead of requiring a separate enrichment pass.
    let tools = mcp_tools.filter_map(|tool| {
        if tool.server_name != CODEX_APPS_MCP_SERVER_NAME {
            return None;
        }
        let connector_id = tool.connector_id.as_deref()?;
        Some(codex_connectors::accessible::AccessibleConnectorTool {
            connector_id: connector_id.to_string(),
            connector_name: tool.connector_name.clone(),
            connector_description: tool.namespace_description.clone(),
            plugin_display_names: tool.plugin_display_names.clone(),
        })
    });
    codex_connectors::accessible::collect_accessible_connectors(tools)
}

fn accessible_connectors_for_app_list_from_mcp_tools(mcp_tools: &[ToolInfo]) -> Vec<AppInfo> {
    let non_synthetic_tools = mcp_tools.iter().filter(|tool| {
        tool.tool
            .meta
            .as_deref()
            .and_then(|meta| meta.get(MCP_TOOL_CODEX_APPS_META_KEY))
            .and_then(serde_json::Value::as_object)
            .and_then(|meta| meta.get("synthetic_link"))
            .and_then(serde_json::Value::as_bool)
            != Some(true)
    });
    collect_accessible_connectors_from_mcp_tools(non_synthetic_tools)
}

pub fn with_app_enabled_state(mut connectors: Vec<AppInfo>, config: &Config) -> Vec<AppInfo> {
    let user_apps_config = apps_config_from_layer_stack(&config.config_layer_stack);
    let requirements_apps_config = config.config_layer_stack.requirements_toml().apps.as_ref();
    if user_apps_config.is_none() && requirements_apps_config.is_none() {
        return connectors;
    }

    for connector in &mut connectors {
        if let Some(apps_config) = user_apps_config.as_ref()
            && (apps_config.default.is_some()
                || apps_config.apps.contains_key(connector.id.as_str()))
        {
            connector.is_enabled = app_is_enabled(apps_config, Some(connector.id.as_str()));
        }

        if requirements_apps_config
            .and_then(|apps| apps.apps.get(connector.id.as_str()))
            .is_some_and(|app| app.enabled == Some(false))
        {
            connector.is_enabled = false;
        }
    }

    connectors
}

pub fn with_app_plugin_sources(
    mut connectors: Vec<AppInfo>,
    tool_plugin_provenance: &ToolPluginProvenance,
) -> Vec<AppInfo> {
    for connector in &mut connectors {
        connector.plugin_display_names = tool_plugin_provenance
            .plugin_display_names_for_connector_id(connector.id.as_str())
            .to_vec();
    }
    connectors
}

pub(crate) fn mcp_approvals_reviewer(
    config: &Config,
    server_name: &str,
    connector_id: Option<&str>,
) -> ApprovalsReviewer {
    let app_reviewer = if server_name == CODEX_APPS_MCP_SERVER_NAME {
        apps_config_from_layer_stack(&config.config_layer_stack).and_then(|apps_config| {
            connector_id
                .and_then(|connector_id| apps_config.apps.get(connector_id))
                .and_then(|app| app.approvals_reviewer)
                .or_else(|| {
                    apps_config
                        .default
                        .and_then(|defaults| defaults.approvals_reviewer)
                })
        })
    } else {
        None
    };

    if let Some(reviewer) = app_reviewer
        && config
            .config_layer_stack
            .requirements()
            .approvals_reviewer
            .can_set(&reviewer)
            .is_ok()
    {
        return reviewer;
    }

    config.approvals_reviewer
}

#[cfg(test)]
#[path = "connectors_tests.rs"]
mod tests;
