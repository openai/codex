//! Connection manager for Model Context Protocol (MCP) servers.
//!
//! The [`McpConnectionManager`] owns one [`codex_rmcp_client::RmcpClient`] per
//! configured server (keyed by the *server name*). It offers convenience
//! helpers to query the available tools across *all* servers and returns them
//! in a single aggregated map using the fully-qualified tool name
//! `"<server><MCP_TOOL_NAME_DELIMITER><tool>"` as the key.

use std::collections::HashMap;
use std::collections::HashSet;
use std::env;
use std::ffi::OsString;
use std::sync::Arc;
use std::time::Duration;

use crate::mcp::auth::McpAuthStatusEntry;
use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use async_channel::Sender;
use codex_async_utils::CancelErr;
use codex_async_utils::OrCancelExt;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::McpStartupCompleteEvent;
use codex_protocol::protocol::McpStartupFailure;
use codex_protocol::protocol::McpStartupStatus;
use codex_protocol::protocol::McpStartupUpdateEvent;
use codex_rmcp_client::OAuthCredentialsStoreMode;
use codex_rmcp_client::RmcpClient;
use mcp_types::ClientCapabilities;
use mcp_types::Implementation;
use mcp_types::ListResourceTemplatesRequestParams;
use mcp_types::ListResourceTemplatesResult;
use mcp_types::ListResourcesRequestParams;
use mcp_types::ListResourcesResult;
use mcp_types::ReadResourceRequestParams;
use mcp_types::ReadResourceResult;
use mcp_types::Resource;
use mcp_types::ResourceTemplate;
use mcp_types::Tool;

use serde_json::json;
use sha1::Digest;
use sha1::Sha1;
use tokio::sync::RwLock;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::codex::INITIAL_SUBMIT_ID;
use crate::config::types::McpServerConfig;
use crate::config::types::McpServerTransportConfig;

/// Delimiter used to separate the server name from the tool name in a fully
/// qualified tool name.
///
/// OpenAI requires tool names to conform to `^[a-zA-Z0-9_-]+$`, so we must
/// choose a delimiter from this character set.
const MCP_TOOL_NAME_DELIMITER: &str = "__";
const MAX_TOOL_NAME_LENGTH: usize = 64;

/// Default timeout for initializing MCP server & initially listing tools.
pub const DEFAULT_STARTUP_TIMEOUT: Duration = Duration::from_secs(10);

/// Default timeout for individual tool calls.
const DEFAULT_TOOL_TIMEOUT: Duration = Duration::from_secs(60);

/// Map that holds a startup error for every MCP server that could **not** be
/// spawned successfully.
pub type ClientStartErrors = HashMap<String, anyhow::Error>;

fn qualify_tools<'a>(tools: &[&'a ToolInfo]) -> HashMap<String, &'a ToolInfo> {
    let mut used_names = HashSet::new();
    let mut qualified_tools = HashMap::new();
    for tool in tools {
        let mut qualified_name = format!(
            "mcp{}{}{}{}",
            MCP_TOOL_NAME_DELIMITER, tool.server_name, MCP_TOOL_NAME_DELIMITER, tool.tool_name
        );
        if qualified_name.len() > MAX_TOOL_NAME_LENGTH {
            let mut hasher = Sha1::new();
            hasher.update(qualified_name.as_bytes());
            let sha1 = hasher.finalize();
            let sha1_str = format!("{sha1:x}");

            // Truncate to make room for the hash suffix
            let prefix_len = MAX_TOOL_NAME_LENGTH - sha1_str.len();

            qualified_name = format!("{}{}", &qualified_name[..prefix_len], sha1_str);
        }

        if used_names.contains(&qualified_name) {
            warn!("skipping duplicated tool {}", qualified_name);
            continue;
        }

        used_names.insert(qualified_name.clone());
        qualified_tools.insert(qualified_name, *tool);
    }

    qualified_tools
}

#[derive(Clone)]
pub(crate) struct ToolInfo {
    server_name: String,
    tool_name: String,
    tool: Tool,
}

#[derive(Clone)]
pub(crate) struct ManagedClient {
    client: Arc<RmcpClient>,
    tools: Vec<ToolInfo>,
    tool_timeout: Option<Duration>,
}

#[derive(Default)]
struct McpConnectionManagerInner {
    clients: HashMap<String, ManagedClient>,
    tools: HashMap<String, ToolInfo>,
    tool_filters: HashMap<String, ToolFilter>,
}

/// A thin wrapper around a set of running [`RmcpClient`] instances.
#[derive(Clone, Default)]
pub(crate) struct McpConnectionManager {
    inner: Arc<RwLock<McpConnectionManagerInner>>,
}

impl McpConnectionManager {
    async fn clients_snapshot(&self) -> HashMap<String, ManagedClient> {
        let inner = self.inner.read().await;
        inner.clients.clone()
    }

    async fn client_by_name(&self, name: &str) -> Result<ManagedClient> {
        let inner = self.inner.read().await;
        inner
            .clients
            .get(name)
            .cloned()
            .ok_or_else(|| anyhow!("unknown MCP server '{name}'"))
    }

    async fn tool_filters_snapshot(&self) -> HashMap<String, ToolFilter> {
        let inner = self.inner.read().await;
        inner.tool_filters.clone()
    }

    pub async fn add_client(&self, server: String, managed: ManagedClient) {
        let mut inner = self.inner.write().await;
        let filtered = filter_tools(&managed.tools, &inner.tool_filters);
        let qualified = qualify_tools(&filtered);
        inner.tools.extend(
            qualified
                .into_iter()
                .map(|(name, tool)| (name, tool.clone())),
        );
        inner.clients.insert(server, managed);
    }

    /// Returns a single map that contains all tools. Each key is the
    /// fully-qualified name for the tool.
    pub async fn list_all_tools(&self) -> HashMap<String, Tool> {
        let inner = self.inner.read().await;
        inner
            .tools
            .iter()
            .map(|(name, tool)| (name.clone(), tool.tool.clone()))
            .collect()
    }

    /// Returns a single map that contains all resources. Each key is the
    /// server name and the value is a vector of resources.
    pub async fn list_all_resources(&self) -> HashMap<String, Vec<Resource>> {
        let mut join_set = JoinSet::new();

        let clients_snapshot = self.clients_snapshot().await;

        for (server_name, managed_client) in &clients_snapshot {
            let server_name = server_name.clone();
            let client_clone = managed_client.client.clone();
            let timeout = managed_client.tool_timeout;

            join_set.spawn(async move {
                let mut collected: Vec<Resource> = Vec::new();
                let mut cursor: Option<String> = None;

                loop {
                    let params = cursor.as_ref().map(|next| ListResourcesRequestParams {
                        cursor: Some(next.clone()),
                    });
                    let response = match client_clone.list_resources(params, timeout).await {
                        Ok(result) => result,
                        Err(err) => return (server_name, Err(err)),
                    };

                    collected.extend(response.resources);

                    match response.next_cursor {
                        Some(next) => {
                            if cursor.as_ref() == Some(&next) {
                                return (
                                    server_name,
                                    Err(anyhow!("resources/list returned duplicate cursor")),
                                );
                            }
                            cursor = Some(next);
                        }
                        None => return (server_name, Ok(collected)),
                    }
                }
            });
        }

        let mut aggregated: HashMap<String, Vec<Resource>> = HashMap::new();

        while let Some(join_res) = join_set.join_next().await {
            match join_res {
                Ok((server_name, Ok(resources))) => {
                    aggregated.insert(server_name, resources);
                }
                Ok((server_name, Err(err))) => {
                    warn!("Failed to list resources for MCP server '{server_name}': {err:#}");
                }
                Err(err) => {
                    warn!("Task panic when listing resources for MCP server: {err:#}");
                }
            }
        }

        aggregated
    }

    /// Returns a single map that contains all resource templates. Each key is the
    /// server name and the value is a vector of resource templates.
    pub async fn list_all_resource_templates(&self) -> HashMap<String, Vec<ResourceTemplate>> {
        let mut join_set = JoinSet::new();

        let clients_snapshot = self.clients_snapshot().await;

        for (server_name, managed_client) in &clients_snapshot {
            let server_name_cloned = server_name.clone();
            let client_clone = managed_client.client.clone();
            let timeout = managed_client.tool_timeout;

            join_set.spawn(async move {
                let mut collected: Vec<ResourceTemplate> = Vec::new();
                let mut cursor: Option<String> = None;

                loop {
                    let params = cursor
                        .as_ref()
                        .map(|next| ListResourceTemplatesRequestParams {
                            cursor: Some(next.clone()),
                        });
                    let response = match client_clone.list_resource_templates(params, timeout).await
                    {
                        Ok(result) => result,
                        Err(err) => return (server_name_cloned, Err(err)),
                    };

                    collected.extend(response.resource_templates);

                    match response.next_cursor {
                        Some(next) => {
                            if cursor.as_ref() == Some(&next) {
                                return (
                                    server_name_cloned,
                                    Err(anyhow!(
                                        "resources/templates/list returned duplicate cursor"
                                    )),
                                );
                            }
                            cursor = Some(next);
                        }
                        None => return (server_name_cloned, Ok(collected)),
                    }
                }
            });
        }

        let mut aggregated: HashMap<String, Vec<ResourceTemplate>> = HashMap::new();

        while let Some(join_res) = join_set.join_next().await {
            match join_res {
                Ok((server_name, Ok(templates))) => {
                    aggregated.insert(server_name, templates);
                }
                Ok((server_name, Err(err))) => {
                    warn!(
                        "Failed to list resource templates for MCP server '{server_name}': {err:#}"
                    );
                }
                Err(err) => {
                    warn!("Task panic when listing resource templates for MCP server: {err:#}");
                }
            }
        }

        aggregated
    }

    /// Invoke the tool indicated by the (server, tool) pair.
    pub async fn call_tool(
        &self,
        server: &str,
        tool: &str,
        arguments: Option<serde_json::Value>,
    ) -> Result<mcp_types::CallToolResult> {
        let tool_filters = self.tool_filters_snapshot().await;
        if let Some(filter) = tool_filters.get(server)
            && !filter.allows(tool)
        {
            return Err(anyhow!(
                "tool '{tool}' is disabled for MCP server '{server}'"
            ));
        }

        let managed = self.client_by_name(server).await?;
        managed
            .client
            .call_tool(tool.to_string(), arguments, managed.tool_timeout)
            .await
            .with_context(|| format!("tool call failed for `{server}/{tool}`"))
    }

    /// List resources from the specified server.
    pub async fn list_resources(
        &self,
        server: &str,
        params: Option<ListResourcesRequestParams>,
    ) -> Result<ListResourcesResult> {
        let managed = self.client_by_name(server).await?;
        let timeout = managed.tool_timeout;

        managed
            .client
            .list_resources(params, timeout)
            .await
            .with_context(|| format!("resources/list failed for `{server}`"))
    }

    /// List resource templates from the specified server.
    pub async fn list_resource_templates(
        &self,
        server: &str,
        params: Option<ListResourceTemplatesRequestParams>,
    ) -> Result<ListResourceTemplatesResult> {
        let managed = self.client_by_name(server).await?;
        let client = managed.client.clone();
        let timeout = managed.tool_timeout;

        client
            .list_resource_templates(params, timeout)
            .await
            .with_context(|| format!("resources/templates/list failed for `{server}`"))
    }

    /// Read a resource from the specified server.
    pub async fn read_resource(
        &self,
        server: &str,
        params: ReadResourceRequestParams,
    ) -> Result<ReadResourceResult> {
        let managed = self.client_by_name(server).await?;
        let client = managed.client.clone();
        let timeout = managed.tool_timeout;
        let uri = params.uri.clone();

        client
            .read_resource(params, timeout)
            .await
            .with_context(|| format!("resources/read failed for `{server}` ({uri})"))
    }

    pub fn parse_tool_name(&self, tool_name: &str) -> Option<(String, String)> {
        if let Ok(inner) = self.inner.try_read() {
            inner
                .tools
                .get(tool_name)
                .map(|tool| (tool.server_name.clone(), tool.tool_name.clone()))
        } else {
            None
        }
    }
}

pub struct McpStartupJobHandle {
    cancel_token: CancellationToken,
    ready: watch::Receiver<bool>,
}

impl McpStartupJobHandle {
    pub fn cancel(&self) {
        self.cancel_token.cancel();
    }

    pub fn readiness(&self) -> watch::Receiver<bool> {
        self.ready.clone()
    }
}

pub fn spawn_startup_job(
    mcp_servers: HashMap<String, McpServerConfig>,
    store_mode: OAuthCredentialsStoreMode,
    manager: McpConnectionManager,
    tx_event: Sender<Event>,
    auth_entries: HashMap<String, McpAuthStatusEntry>,
) -> Result<McpStartupJobHandle> {
    let mut errors = ClientStartErrors::new();
    let mut valid_servers = HashMap::new();
    let mut tool_filters: HashMap<String, ToolFilter> = HashMap::new();
    for (server_name, cfg) in mcp_servers {
        if let Err(error) = validate_mcp_server_name(&server_name) {
            errors.insert(server_name, error);
            continue;
        }
        tool_filters.insert(server_name.clone(), ToolFilter::from_config(&cfg));
        valid_servers.insert(server_name, cfg);
    }

    // Emit failures for invalid server names immediately.
    for (server, error) in &errors {
        let _ = tx_event.try_send(Event {
            id: INITIAL_SUBMIT_ID.to_owned(),
            msg: EventMsg::McpStartupUpdate(McpStartupUpdateEvent {
                server: server.clone(),
                status: McpStartupStatus::Failed {
                    error: format!("{error:#}"),
                },
            }),
        });
    }

    let _ = manager
        .inner
        .try_write()
        .map(|mut inner| inner.tool_filters = tool_filters);

    let starting_servers = valid_servers.keys().cloned().collect::<Vec<_>>();

    // Emit starting status for enabled servers.
    for server in &starting_servers {
        let _ = tx_event.try_send(Event {
            id: INITIAL_SUBMIT_ID.to_owned(),
            msg: EventMsg::McpStartupUpdate(McpStartupUpdateEvent {
                server: server.clone(),
                status: McpStartupStatus::Starting,
            }),
        });
    }

    let cancel_token = CancellationToken::new();
    let mut join_set = build_startup_tasks(valid_servers, store_mode, &cancel_token);
    let (ready_tx, ready_rx) = watch::channel(false);
    tokio::spawn(async move {
        let mut summary = McpStartupCompleteEvent::default();
        summary.failed.extend(errors.drain().map(|(server, err)| {
            let formatted = mcp_init_error_display(&server, auth_entries.get(&server), &err);
            McpStartupFailure {
                server,
                error: formatted,
            }
        }));

        while let Some(res) = join_set.join_next().await {
            match res {
                Ok((server_name, StartupOutcome::Ready { managed })) => {
                    manager.add_client(server_name.clone(), managed).await;
                    summary.ready.push(server_name.clone());
                    if let Err(err) = emit_update(
                        &tx_event,
                        McpStartupUpdateEvent {
                            server: server_name,
                            status: McpStartupStatus::Ready,
                        },
                    )
                    .await
                    {
                        warn!("failed to emit MCP ready update: {err:#}");
                    }
                }
                Ok((server_name, StartupOutcome::Failed { error })) => {
                    let error_str = mcp_init_error_display(
                        &server_name,
                        auth_entries.get(&server_name),
                        &error,
                    );
                    summary.failed.push(McpStartupFailure {
                        server: server_name.clone(),
                        error: error_str.clone(),
                    });
                    if let Err(err) = emit_update(
                        &tx_event,
                        McpStartupUpdateEvent {
                            server: server_name,
                            status: McpStartupStatus::Failed { error: error_str },
                        },
                    )
                    .await
                    {
                        warn!("failed to emit MCP failure update: {err:#}");
                    }
                }
                Ok((server_name, StartupOutcome::Cancelled)) => {
                    summary.cancelled.push(server_name.clone());
                    if let Err(err) = emit_update(
                        &tx_event,
                        McpStartupUpdateEvent {
                            server: server_name,
                            status: McpStartupStatus::Cancelled,
                        },
                    )
                    .await
                    {
                        warn!("failed to emit MCP cancellation update: {err:#}");
                    }
                }
                Err(err) => {
                    warn!("Task panic when starting MCP server: {err:#}");
                }
            }
        }

        let _ = tx_event
            .send(Event {
                id: INITIAL_SUBMIT_ID.to_owned(),
                msg: EventMsg::McpStartupComplete(summary),
            })
            .await;
        let _ = ready_tx.send(true);
    });

    Ok(McpStartupJobHandle {
        cancel_token,
        ready: ready_rx,
    })
}

async fn emit_update(
    tx_event: &Sender<Event>,
    update: McpStartupUpdateEvent,
) -> Result<(), async_channel::SendError<Event>> {
    tx_event
        .send(Event {
            id: INITIAL_SUBMIT_ID.to_owned(),
            msg: EventMsg::McpStartupUpdate(update),
        })
        .await
}

/// A tool is allowed to be used if both are true:
/// 1. enabled is None (no allowlist is set) or the tool is explicitly enabled.
/// 2. The tool is not explicitly disabled.
#[derive(Default, Clone)]
pub(crate) struct ToolFilter {
    enabled: Option<HashSet<String>>,
    disabled: HashSet<String>,
}

impl ToolFilter {
    fn from_config(cfg: &McpServerConfig) -> Self {
        let enabled = cfg
            .enabled_tools
            .as_ref()
            .map(|tools| tools.iter().cloned().collect::<HashSet<_>>());
        let disabled = cfg
            .disabled_tools
            .as_ref()
            .map(|tools| tools.iter().cloned().collect::<HashSet<_>>())
            .unwrap_or_default();

        Self { enabled, disabled }
    }

    fn allows(&self, tool_name: &str) -> bool {
        if let Some(enabled) = &self.enabled
            && !enabled.contains(tool_name)
        {
            return false;
        }

        !self.disabled.contains(tool_name)
    }
}

fn filter_tools<'a>(
    tools: &'a [ToolInfo],
    filters: &HashMap<String, ToolFilter>,
) -> Vec<&'a ToolInfo> {
    tools
        .iter()
        .filter(|tool| match filters.get(&tool.server_name) {
            Some(filter) => filter.allows(&tool.tool_name),
            None => true,
        })
        .collect()
}

fn resolve_bearer_token(
    server_name: &str,
    bearer_token_env_var: Option<&str>,
) -> Result<Option<String>> {
    let Some(env_var) = bearer_token_env_var else {
        return Ok(None);
    };

    match env::var(env_var) {
        Ok(value) => {
            if value.is_empty() {
                Err(anyhow!(
                    "Environment variable {env_var} for MCP server '{server_name}' is empty"
                ))
            } else {
                Ok(Some(value))
            }
        }
        Err(env::VarError::NotPresent) => Err(anyhow!(
            "Environment variable {env_var} for MCP server '{server_name}' is not set"
        )),
        Err(env::VarError::NotUnicode(_)) => Err(anyhow!(
            "Environment variable {env_var} for MCP server '{server_name}' contains invalid Unicode"
        )),
    }
}

enum StartupOutcome {
    Ready { managed: ManagedClient },
    Failed { error: anyhow::Error },
    Cancelled,
}

fn build_startup_tasks(
    mcp_servers: HashMap<String, McpServerConfig>,
    store_mode: OAuthCredentialsStoreMode,
    cancel_token: &CancellationToken,
) -> JoinSet<(String, StartupOutcome)> {
    let mut join_set = JoinSet::new();

    for (server_name, cfg) in mcp_servers {
        if !cfg.enabled {
            continue;
        }

        let startup_timeout = cfg.startup_timeout_sec.unwrap_or(DEFAULT_STARTUP_TIMEOUT);
        let tool_timeout = cfg.tool_timeout_sec.unwrap_or(DEFAULT_TOOL_TIMEOUT);

        let startup_cancel = cancel_token.clone();

        join_set.spawn(async move {
            let outcome = start_server_task(
                server_name.clone(),
                cfg.transport,
                store_mode,
                startup_timeout,
                tool_timeout,
                startup_cancel,
            )
            .await;
            (server_name, outcome)
        });
    }

    join_set
}

async fn start_server_task(
    server_name: String,
    transport: McpServerTransportConfig,
    store_mode: OAuthCredentialsStoreMode,
    startup_timeout: Duration,
    tool_timeout: Duration,
    cancel_token: CancellationToken,
) -> StartupOutcome {
    if cancel_token.is_cancelled() {
        return StartupOutcome::Cancelled;
    }

    let work_fut = async {
        let params = mcp_types::InitializeRequestParams {
            capabilities: ClientCapabilities {
                experimental: None,
                roots: None,
                sampling: None,
                // https://modelcontextprotocol.io/specification/2025-06-18/client/elicitation#capabilities
                // indicates this should be an empty object.
                elicitation: Some(json!({})),
            },
            client_info: Implementation {
                name: "codex-mcp-client".to_owned(),
                version: env!("CARGO_PKG_VERSION").to_owned(),
                title: Some("Codex".into()),
                // This field is used by Codex when it is an MCP
                // server: it should not be used when Codex is
                // an MCP client.
                user_agent: None,
            },
            protocol_version: mcp_types::MCP_SCHEMA_VERSION.to_owned(),
        };

        let client_result = match transport {
            McpServerTransportConfig::Stdio {
                command,
                args,
                env,
                env_vars,
                cwd,
            } => {
                let command_os: OsString = command.into();
                let args_os: Vec<OsString> = args.into_iter().map(Into::into).collect();
                match RmcpClient::new_stdio_client(command_os, args_os, env, &env_vars, cwd).await {
                    Ok(client) => {
                        let client = Arc::new(client);
                        client
                            .initialize(params.clone(), Some(startup_timeout))
                            .await
                            .map(|_| client)
                    }
                    Err(err) => Err(err.into()),
                }
            }
            McpServerTransportConfig::StreamableHttp {
                url,
                http_headers,
                env_http_headers,
                bearer_token_env_var,
            } => {
                let resolved_bearer_token =
                    match resolve_bearer_token(&server_name, bearer_token_env_var.as_deref()) {
                        Ok(token) => token,
                        Err(error) => {
                            return StartupOutcome::Failed { error };
                        }
                    };
                match RmcpClient::new_streamable_http_client(
                    &server_name,
                    &url,
                    resolved_bearer_token,
                    http_headers,
                    env_http_headers,
                    store_mode,
                )
                .await
                {
                    Ok(client) => {
                        let client = Arc::new(client);
                        client
                            .initialize(params.clone(), Some(startup_timeout))
                            .await
                            .map(|_| client)
                    }
                    Err(err) => Err(err),
                }
            }
        };

        let client = match client_result {
            Ok(client) => client,
            Err(error) => {
                return StartupOutcome::Failed { error };
            }
        };

        let tools = match list_tools_for_client(&server_name, &client, startup_timeout).await {
            Ok(tools) => tools,
            Err(error) => {
                return StartupOutcome::Failed { error };
            }
        };

        let managed = ManagedClient {
            client: Arc::clone(&client),
            tools,
            tool_timeout: Some(tool_timeout),
        };

        StartupOutcome::Ready { managed }
    };

    match work_fut.or_cancel(&cancel_token).await {
        Ok(result) => result,
        Err(CancelErr::Cancelled) => StartupOutcome::Cancelled,
    }
}

async fn list_tools_for_client(
    server_name: &str,
    client: &Arc<RmcpClient>,
    timeout: Duration,
) -> Result<Vec<ToolInfo>> {
    let resp = client.list_tools(None, Some(timeout)).await?;
    Ok(resp
        .tools
        .into_iter()
        .map(|tool| ToolInfo {
            server_name: server_name.to_owned(),
            tool_name: tool.name.clone(),
            tool,
        })
        .collect())
}

fn validate_mcp_server_name(server_name: &str) -> Result<()> {
    let re = regex_lite::Regex::new(r"^[a-zA-Z0-9_-]+$")?;
    if !re.is_match(server_name) {
        return Err(anyhow!(
            "Invalid MCP server name '{server_name}': must match pattern {pattern}",
            pattern = re.as_str()
        ));
    }
    Ok(())
}

fn mcp_init_error_display(
    server_name: &str,
    entry: Option<&McpAuthStatusEntry>,
    err: &anyhow::Error,
) -> String {
    if let Some(McpServerTransportConfig::StreamableHttp {
        url,
        bearer_token_env_var,
        http_headers,
        ..
    }) = &entry.map(|entry| &entry.config.transport)
        && url == "https://api.githubcopilot.com/mcp/"
        && bearer_token_env_var.is_none()
        && http_headers.as_ref().map(HashMap::is_empty).unwrap_or(true)
    {
        format!(
            "GitHub MCP does not support OAuth. Log in by adding a personal access token (https://github.com/settings/personal-access-tokens) to your environment and config.toml:\n[mcp_servers.{server_name}]\nbearer_token_env_var = CODEX_GITHUB_PERSONAL_ACCESS_TOKEN"
        )
    } else if is_mcp_client_auth_required_error(err) {
        format!(
            "The {server_name} MCP server is not logged in. Run `codex mcp login {server_name}`."
        )
    } else if is_mcp_client_startup_timeout_error(err) {
        let startup_timeout_secs = match entry {
            Some(entry) => match entry.config.startup_timeout_sec {
                Some(timeout) => timeout,
                None => DEFAULT_STARTUP_TIMEOUT,
            },
            None => DEFAULT_STARTUP_TIMEOUT,
        }
        .as_secs();
        format!(
            "MCP client for `{server_name}` timed out after {startup_timeout_secs} seconds. Add or adjust `startup_timeout_sec` in your config.toml:\n[mcp_servers.{server_name}]\nstartup_timeout_sec = XX"
        )
    } else {
        format!("MCP client for `{server_name}` failed to start: {err:#}")
    }
}

fn is_mcp_client_auth_required_error(error: &anyhow::Error) -> bool {
    error.to_string().contains("Auth required")
}

fn is_mcp_client_startup_timeout_error(error: &anyhow::Error) -> bool {
    let error_message = error.to_string();
    error_message.contains("request timed out")
        || error_message.contains("timed out handshaking with MCP server")
}

#[cfg(test)]
mod mcp_init_error_display_tests {}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::protocol::McpAuthStatus;
    use mcp_types::ToolInputSchema;
    use std::collections::HashSet;

    fn create_test_tool(server_name: &str, tool_name: &str) -> ToolInfo {
        ToolInfo {
            server_name: server_name.to_string(),
            tool_name: tool_name.to_string(),
            tool: Tool {
                annotations: None,
                description: Some(format!("Test tool: {tool_name}")),
                input_schema: ToolInputSchema {
                    properties: None,
                    required: None,
                    r#type: "object".to_string(),
                },
                name: tool_name.to_string(),
                output_schema: None,
                title: None,
            },
        }
    }

    #[test]
    fn test_qualify_tools_short_non_duplicated_names() {
        let tools = vec![
            create_test_tool("server1", "tool1"),
            create_test_tool("server1", "tool2"),
        ];

        let qualified_tools = qualify_tools(tools.iter().collect::<Vec<_>>().as_slice());

        assert_eq!(qualified_tools.len(), 2);
        assert!(qualified_tools.contains_key("mcp__server1__tool1"));
        assert!(qualified_tools.contains_key("mcp__server1__tool2"));
    }

    #[test]
    fn test_qualify_tools_duplicated_names_skipped() {
        let tools = vec![
            create_test_tool("server1", "duplicate_tool"),
            create_test_tool("server1", "duplicate_tool"),
        ];

        let qualified_tools = qualify_tools(tools.iter().collect::<Vec<_>>().as_slice());

        // Only the first tool should remain, the second is skipped
        assert_eq!(qualified_tools.len(), 1);
        assert!(qualified_tools.contains_key("mcp__server1__duplicate_tool"));
    }

    #[test]
    fn test_qualify_tools_long_names_same_server() {
        let server_name = "my_server";

        let tools = vec![
            create_test_tool(
                server_name,
                "extremely_lengthy_function_name_that_absolutely_surpasses_all_reasonable_limits",
            ),
            create_test_tool(
                server_name,
                "yet_another_extremely_lengthy_function_name_that_absolutely_surpasses_all_reasonable_limits",
            ),
        ];

        let qualified_tools = qualify_tools(tools.iter().collect::<Vec<_>>().as_slice());

        assert_eq!(qualified_tools.len(), 2);

        let mut keys: Vec<_> = qualified_tools.keys().cloned().collect();
        keys.sort();

        assert_eq!(keys[0].len(), 64);
        assert_eq!(
            keys[0],
            "mcp__my_server__extremel119a2b97664e41363932dc84de21e2ff1b93b3e9"
        );

        assert_eq!(keys[1].len(), 64);
        assert_eq!(
            keys[1],
            "mcp__my_server__yet_anot419a82a89325c1b477274a41f8c65ea5f3a7f341"
        );
    }

    #[test]
    fn tool_filter_allows_by_default() {
        let filter = ToolFilter::default();

        assert!(filter.allows("any"));
    }

    #[test]
    fn tool_filter_applies_enabled_list() {
        let filter = ToolFilter {
            enabled: Some(HashSet::from(["allowed".to_string()])),
            disabled: HashSet::new(),
        };

        assert!(filter.allows("allowed"));
        assert!(!filter.allows("denied"));
    }

    #[test]
    fn tool_filter_applies_disabled_list() {
        let filter = ToolFilter {
            enabled: None,
            disabled: HashSet::from(["blocked".to_string()]),
        };

        assert!(!filter.allows("blocked"));
        assert!(filter.allows("open"));
    }

    #[test]
    fn tool_filter_applies_enabled_then_disabled() {
        let filter = ToolFilter {
            enabled: Some(HashSet::from(["keep".to_string(), "remove".to_string()])),
            disabled: HashSet::from(["remove".to_string()]),
        };

        assert!(filter.allows("keep"));
        assert!(!filter.allows("remove"));
        assert!(!filter.allows("unknown"));
    }

    #[test]
    fn filter_tools_applies_per_server_filters() {
        let tools = vec![
            create_test_tool("server1", "tool_a"),
            create_test_tool("server1", "tool_b"),
            create_test_tool("server2", "tool_a"),
        ];
        let mut filters = HashMap::new();
        filters.insert(
            "server1".to_string(),
            ToolFilter {
                enabled: Some(HashSet::from(["tool_a".to_string(), "tool_b".to_string()])),
                disabled: HashSet::from(["tool_b".to_string()]),
            },
        );
        filters.insert(
            "server2".to_string(),
            ToolFilter {
                enabled: None,
                disabled: HashSet::from(["tool_a".to_string()]),
            },
        );

        let filtered = filter_tools(&tools, &filters);

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].server_name, "server1");
        assert_eq!(filtered[0].tool_name, "tool_a");
    }

    #[test]
    fn mcp_init_error_display_prompts_for_github_pat() {
        let server_name = "github";
        let entry = McpAuthStatusEntry {
            config: McpServerConfig {
                transport: McpServerTransportConfig::StreamableHttp {
                    url: "https://api.githubcopilot.com/mcp/".to_string(),
                    bearer_token_env_var: None,
                    http_headers: None,
                    env_http_headers: None,
                },
                enabled: true,
                startup_timeout_sec: None,
                tool_timeout_sec: None,
                enabled_tools: None,
                disabled_tools: None,
            },
            auth_status: McpAuthStatus::Unsupported,
        };
        let err = anyhow::anyhow!("OAuth is unsupported");

        let display = mcp_init_error_display(server_name, Some(&entry), &err);

        let expected = format!(
            "GitHub MCP does not support OAuth. Log in by adding a personal access token (https://github.com/settings/personal-access-tokens) to your environment and config.toml:\n[mcp_servers.{server_name}]\nbearer_token_env_var = CODEX_GITHUB_PERSONAL_ACCESS_TOKEN"
        );

        assert_eq!(expected, display);
    }

    #[test]
    fn mcp_init_error_display_prompts_for_login_when_auth_required() {
        let server_name = "example";
        let err = anyhow::anyhow!("Auth required for server");

        let display = mcp_init_error_display(server_name, None, &err);

        let expected = format!(
            "The {server_name} MCP server is not logged in. Run `codex mcp login {server_name}`."
        );

        assert_eq!(expected, display);
    }

    #[test]
    fn mcp_init_error_display_reports_generic_errors() {
        let server_name = "custom";
        let entry = McpAuthStatusEntry {
            config: McpServerConfig {
                transport: McpServerTransportConfig::StreamableHttp {
                    url: "https://example.com".to_string(),
                    bearer_token_env_var: Some("TOKEN".to_string()),
                    http_headers: None,
                    env_http_headers: None,
                },
                enabled: true,
                startup_timeout_sec: None,
                tool_timeout_sec: None,
                enabled_tools: None,
                disabled_tools: None,
            },
            auth_status: McpAuthStatus::Unsupported,
        };
        let err = anyhow::anyhow!("boom");

        let display = mcp_init_error_display(server_name, Some(&entry), &err);

        let expected = format!("MCP client for `{server_name}` failed to start: {err:#}");

        assert_eq!(expected, display);
    }

    #[test]
    fn mcp_init_error_display_includes_startup_timeout_hint() {
        let server_name = "slow";
        let err = anyhow::anyhow!("request timed out");

        let display = mcp_init_error_display(server_name, None, &err);

        assert_eq!(
            "MCP client for `slow` timed out after 10 seconds. Add or adjust `startup_timeout_sec` in your config.toml:\n[mcp_servers.slow]\nstartup_timeout_sec = XX",
            display
        );
    }
}
