pub use auth::McpAuthStatusEntry;
pub use auth::McpOAuthLoginConfig;
pub use auth::McpOAuthLoginSupport;
pub use auth::McpOAuthScopesSource;
pub use auth::ResolvedMcpOAuthScopes;
pub use auth::compute_auth_statuses;
pub use auth::discover_supported_scopes;
pub use auth::oauth_login_support;
pub use auth::resolve_oauth_scopes;
pub use auth::should_retry_without_scopes;

pub(crate) mod auth;

use std::collections::HashMap;
use std::collections::HashSet;
use std::env;
use std::path::PathBuf;
use std::time::Duration;

use async_channel::unbounded;
use codex_config::Constrained;
use codex_config::McpServerConfig;
use codex_config::McpServerTransportConfig;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_login::CodexAuth;
use codex_plugin::PluginCapabilitySummary;
use codex_protocol::mcp::Resource;
use codex_protocol::mcp::ResourceTemplate;
use codex_protocol::mcp::Tool;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::McpAuthStatus;
use codex_protocol::protocol::McpListToolsResponseEvent;
use codex_protocol::protocol::SandboxPolicy;
use rmcp::model::ReadResourceRequestParams;
use rmcp::model::ReadResourceResult;
use serde_json::Value;

use crate::mcp_connection_manager::McpConnectionManager;
use crate::mcp_connection_manager::McpRuntimeEnvironment;
use crate::mcp_connection_manager::codex_apps_tools_cache_key;

pub const CODEX_APPS_MCP_SERVER_NAME: &str = "codex_apps";
const MCP_TOOL_NAME_PREFIX: &str = "mcp";
const MCP_TOOL_NAME_DELIMITER: &str = "__";
const CODEX_CONNECTORS_TOKEN_ENV_VAR: &str = "CODEX_CONNECTORS_TOKEN";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum McpSnapshotDetail {
    #[default]
    Full,
    ToolsAndAuthOnly,
}

impl McpSnapshotDetail {
    fn include_resources(self) -> bool {
        matches!(self, Self::Full)
    }
}

pub fn qualified_mcp_tool_name_prefix(server_name: &str) -> String {
    sanitize_responses_api_tool_name(&format!(
        "{MCP_TOOL_NAME_PREFIX}{MCP_TOOL_NAME_DELIMITER}{server_name}{MCP_TOOL_NAME_DELIMITER}"
    ))
}

/// Returns true when MCP permission prompts should resolve as approved instead
/// of being shown to the user.
pub fn mcp_permission_prompt_is_auto_approved(
    approval_policy: AskForApproval,
    sandbox_policy: &SandboxPolicy,
) -> bool {
    approval_policy == AskForApproval::Never
        && matches!(
            sandbox_policy,
            SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. }
        )
}

/// MCP runtime settings derived from `codex_core::config::Config`.
///
/// This struct should contain only long-lived configuration values that the
/// `codex-mcp` crate needs to construct server transports, enforce MCP
/// approval/sandbox policy, locate OAuth state, and merge plugin-provided MCP
/// servers. Request-scoped or auth-scoped state should not be stored here;
/// thread those values explicitly into runtime entry points such as
/// [`with_codex_apps_mcp`] and snapshot collection helpers so config objects do
/// not go stale when auth changes.
#[derive(Debug, Clone)]
pub struct McpConfig {
    /// Base URL for ChatGPT-hosted app MCP servers, copied from the root config.
    pub chatgpt_base_url: String,
    /// Codex home directory used for MCP OAuth state and app-tool cache files.
    pub codex_home: PathBuf,
    /// Preferred credential store for MCP OAuth tokens.
    pub mcp_oauth_credentials_store_mode: OAuthCredentialsStoreMode,
    /// Optional fixed localhost callback port for MCP OAuth login.
    pub mcp_oauth_callback_port: Option<u16>,
    /// Optional OAuth redirect URI override for MCP login.
    pub mcp_oauth_callback_url: Option<String>,
    /// Whether skill MCP dependency installation prompts are enabled.
    pub skill_mcp_dependency_install_enabled: bool,
    /// Approval policy used for MCP tool calls and MCP elicitation requests.
    pub approval_policy: Constrained<AskForApproval>,
    /// Optional path to `codex-linux-sandbox` for sandboxed MCP tool execution.
    pub codex_linux_sandbox_exe: Option<PathBuf>,
    /// Whether to use legacy Landlock behavior in the MCP sandbox state.
    pub use_legacy_landlock: bool,
    /// Whether the app MCP integration is enabled by config.
    ///
    /// ChatGPT auth is checked separately at runtime before the built-in apps
    /// MCP server is added.
    pub apps_enabled: bool,
    /// User-configured and plugin-provided MCP servers keyed by server name.
    pub configured_mcp_servers: HashMap<String, McpServerConfig>,
    /// Plugin metadata used to attribute MCP tools/connectors to plugin display names.
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

    pub fn connector_id_is_plugin_declared(&self, connector_id: &str) -> bool {
        !self
            .plugin_display_names_for_connector_id(connector_id)
            .is_empty()
    }

    pub fn plugin_declared_connector_ids(&self) -> HashSet<String> {
        self.plugin_display_names_by_connector_id
            .keys()
            .cloned()
            .collect()
    }

    pub fn plugin_display_names_for_mcp_server_name(&self, server_name: &str) -> &[String] {
        self.plugin_display_names_by_mcp_server_name
            .get(server_name)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub(crate) fn from_capability_summaries(
        capability_summaries: &[PluginCapabilitySummary],
    ) -> Self {
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

pub fn with_codex_apps_mcp(
    mut servers: HashMap<String, McpServerConfig>,
    auth: Option<&CodexAuth>,
    config: &McpConfig,
) -> HashMap<String, McpServerConfig> {
    if config.apps_enabled && auth.is_some_and(CodexAuth::uses_codex_backend) {
        servers.insert(
            CODEX_APPS_MCP_SERVER_NAME.to_string(),
            codex_apps_mcp_server_config(config),
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

pub async fn read_mcp_resource(
    config: &McpConfig,
    auth: Option<&CodexAuth>,
    runtime_environment: McpRuntimeEnvironment,
    server: &str,
    uri: &str,
) -> anyhow::Result<ReadResourceResult> {
    let mut mcp_servers = effective_mcp_servers(config, auth);
    mcp_servers.retain(|name, _| name == server);
    let auth_statuses = compute_auth_statuses(
        mcp_servers.iter(),
        config.mcp_oauth_credentials_store_mode,
        auth,
    )
    .await;
    let (tx_event, rx_event) = unbounded();
    drop(rx_event);
    let (manager, cancel_token) = McpConnectionManager::new(
        &mcp_servers,
        config.mcp_oauth_credentials_store_mode,
        auth_statuses,
        &config.approval_policy,
        String::new(),
        tx_event,
        SandboxPolicy::new_read_only_policy(),
        runtime_environment,
        config.codex_home.clone(),
        codex_apps_tools_cache_key(auth),
        tool_plugin_provenance(config),
        auth,
    )
    .await;

    let result = manager
        .read_resource(
            server,
            ReadResourceRequestParams {
                meta: None,
                uri: uri.to_string(),
            },
        )
        .await;
    cancel_token.cancel();
    result
}

#[derive(Debug, Clone)]
pub struct McpServerStatusSnapshot {
    pub tools_by_server: HashMap<String, HashMap<String, Tool>>,
    pub resources: HashMap<String, Vec<Resource>>,
    pub resource_templates: HashMap<String, Vec<ResourceTemplate>>,
    pub auth_statuses: HashMap<String, McpAuthStatus>,
}

pub async fn collect_mcp_server_status_snapshot_with_detail(
    config: &McpConfig,
    auth: Option<&CodexAuth>,
    submit_id: String,
    runtime_environment: McpRuntimeEnvironment,
    detail: McpSnapshotDetail,
) -> McpServerStatusSnapshot {
    let mcp_servers = effective_mcp_servers(config, auth);
    let tool_plugin_provenance = tool_plugin_provenance(config);
    if mcp_servers.is_empty() {
        return McpServerStatusSnapshot {
            tools_by_server: HashMap::new(),
            resources: HashMap::new(),
            resource_templates: HashMap::new(),
            auth_statuses: HashMap::new(),
        };
    }

    let auth_status_entries = compute_auth_statuses(
        mcp_servers.iter(),
        config.mcp_oauth_credentials_store_mode,
        auth,
    )
    .await;

    let (tx_event, rx_event) = unbounded();
    drop(rx_event);

    let (mcp_connection_manager, cancel_token) = McpConnectionManager::new(
        &mcp_servers,
        config.mcp_oauth_credentials_store_mode,
        auth_status_entries.clone(),
        &config.approval_policy,
        submit_id,
        tx_event,
        SandboxPolicy::new_read_only_policy(),
        runtime_environment,
        config.codex_home.clone(),
        codex_apps_tools_cache_key(auth),
        tool_plugin_provenance,
        auth,
    )
    .await;

    let snapshot = collect_mcp_server_status_snapshot_from_manager(
        &mcp_connection_manager,
        auth_status_entries,
        detail,
    )
    .await;

    cancel_token.cancel();

    snapshot
}

pub async fn collect_mcp_snapshot_from_manager(
    mcp_connection_manager: &McpConnectionManager,
    auth_status_entries: HashMap<String, McpAuthStatusEntry>,
) -> McpListToolsResponseEvent {
    collect_mcp_snapshot_from_manager_with_detail(
        mcp_connection_manager,
        auth_status_entries,
        McpSnapshotDetail::Full,
    )
    .await
}

pub(crate) fn codex_apps_mcp_url(config: &McpConfig) -> String {
    codex_apps_mcp_url_for_base_url(&config.chatgpt_base_url)
}

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

fn codex_apps_mcp_bearer_token_env_var() -> Option<String> {
    match env::var(CODEX_CONNECTORS_TOKEN_ENV_VAR) {
        Ok(value) if !value.trim().is_empty() => Some(CODEX_CONNECTORS_TOKEN_ENV_VAR.to_string()),
        Ok(_) => None,
        Err(env::VarError::NotPresent) => None,
        Err(env::VarError::NotUnicode(_)) => Some(CODEX_CONNECTORS_TOKEN_ENV_VAR.to_string()),
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

fn codex_apps_mcp_server_config(config: &McpConfig) -> McpServerConfig {
    let url = codex_apps_mcp_url(config);

    McpServerConfig {
        transport: McpServerTransportConfig::StreamableHttp {
            url,
            bearer_token_env_var: codex_apps_mcp_bearer_token_env_var(),
            http_headers: None,
            env_http_headers: None,
        },
        experimental_environment: None,
        enabled: true,
        required: false,
        supports_parallel_tool_calls: false,
        disabled_reason: None,
        startup_timeout_sec: Some(Duration::from_secs(30)),
        tool_timeout_sec: None,
        default_tools_approval_mode: None,
        enabled_tools: None,
        disabled_tools: None,
        scopes: None,
        oauth_resource: None,
        tools: HashMap::new(),
    }
}

fn protocol_tool_from_rmcp_tool(name: &str, tool: &rmcp::model::Tool) -> Option<Tool> {
    match serde_json::to_value(tool) {
        Ok(value) => match Tool::from_mcp_value(value) {
            Ok(tool) => Some(tool),
            Err(err) => {
                tracing::warn!("Failed to convert MCP tool '{name}': {err}");
                None
            }
        },
        Err(err) => {
            tracing::warn!("Failed to serialize MCP tool '{name}': {err}");
            None
        }
    }
}

fn auth_statuses_from_entries(
    auth_status_entries: &HashMap<String, crate::mcp::auth::McpAuthStatusEntry>,
) -> HashMap<String, McpAuthStatus> {
    auth_status_entries
        .iter()
        .map(|(name, entry)| (name.clone(), entry.auth_status))
        .collect::<HashMap<_, _>>()
}

fn convert_mcp_resources(
    resources: HashMap<String, Vec<rmcp::model::Resource>>,
) -> HashMap<String, Vec<Resource>> {
    resources
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
        .collect::<HashMap<_, _>>()
}

fn convert_mcp_resource_templates(
    resource_templates: HashMap<String, Vec<rmcp::model::ResourceTemplate>>,
) -> HashMap<String, Vec<ResourceTemplate>> {
    resource_templates
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
        .collect::<HashMap<_, _>>()
}

async fn collect_mcp_server_status_snapshot_from_manager(
    mcp_connection_manager: &McpConnectionManager,
    auth_status_entries: HashMap<String, crate::mcp::auth::McpAuthStatusEntry>,
    detail: McpSnapshotDetail,
) -> McpServerStatusSnapshot {
    let (tools, resources, resource_templates) = tokio::join!(
        mcp_connection_manager.list_all_tools(),
        async {
            if detail.include_resources() {
                mcp_connection_manager.list_all_resources().await
            } else {
                HashMap::new()
            }
        },
        async {
            if detail.include_resources() {
                mcp_connection_manager.list_all_resource_templates().await
            } else {
                HashMap::new()
            }
        },
    );

    let mut tools_by_server = HashMap::<String, HashMap<String, Tool>>::new();
    for (_qualified_name, tool_info) in tools {
        let raw_tool_name = tool_info.tool.name.to_string();
        let Some(tool) = protocol_tool_from_rmcp_tool(&raw_tool_name, &tool_info.tool) else {
            continue;
        };
        let tool_name = tool.name.clone();
        tools_by_server
            .entry(tool_info.server_name)
            .or_default()
            .insert(tool_name, tool);
    }

    McpServerStatusSnapshot {
        tools_by_server,
        resources: convert_mcp_resources(resources),
        resource_templates: convert_mcp_resource_templates(resource_templates),
        auth_statuses: auth_statuses_from_entries(&auth_status_entries),
    }
}

async fn collect_mcp_snapshot_from_manager_with_detail(
    mcp_connection_manager: &McpConnectionManager,
    auth_status_entries: HashMap<String, McpAuthStatusEntry>,
    detail: McpSnapshotDetail,
) -> McpListToolsResponseEvent {
    let (tools, resources, resource_templates) = tokio::join!(
        mcp_connection_manager.list_all_tools(),
        async {
            if detail.include_resources() {
                mcp_connection_manager.list_all_resources().await
            } else {
                HashMap::new()
            }
        },
        async {
            if detail.include_resources() {
                mcp_connection_manager.list_all_resource_templates().await
            } else {
                HashMap::new()
            }
        },
    );

    let tools = tools
        .into_iter()
        .filter_map(|(name, tool)| {
            protocol_tool_from_rmcp_tool(&name, &tool.tool).map(|tool| (name, tool))
        })
        .collect::<HashMap<_, _>>();

    McpListToolsResponseEvent {
        tools,
        resources: convert_mcp_resources(resources),
        resource_templates: convert_mcp_resource_templates(resource_templates),
        auth_statuses: auth_statuses_from_entries(&auth_status_entries),
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
