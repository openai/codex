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

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
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
use tokio::task::JoinSet;
use tracing::info;
use tracing::warn;

use crate::config_types::McpServerConfig;
use crate::config_types::McpServerTransportConfig;

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

fn qualify_tools(tools: Vec<ToolInfo>) -> HashMap<String, ToolInfo> {
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
        qualified_tools.insert(qualified_name, tool);
    }

    qualified_tools
}

struct ToolInfo {
    server_name: String,
    tool_name: String,
    tool: Tool,
}

struct ManagedClient {
    client: Arc<RmcpClient>,
    startup_timeout: Duration,
    tool_timeout: Option<Duration>,
}

/// A thin wrapper around a set of running [`RmcpClient`] instances.
#[derive(Default)]
pub(crate) struct McpConnectionManager {
    /// Server-name -> client instance.
    ///
    /// The server name originates from the keys of the `mcp_servers` map in
    /// the user configuration.
    clients: HashMap<String, ManagedClient>,

    /// Fully qualified tool name -> tool instance.
    tools: HashMap<String, ToolInfo>,

    /// Server-name -> configured tool filters.
    tool_filters: HashMap<String, ToolFilter>,
}

impl McpConnectionManager {
    /// Spawn a [`RmcpClient`] for each configured server.
    ///
    /// * `mcp_servers` – Map loaded from the user configuration where *keys*
    ///   are human-readable server identifiers and *values* are the spawn
    ///   instructions.
    ///
    /// Servers that fail to start are reported in `ClientStartErrors`: the
    /// user should be informed about these errors.
    pub async fn new(
        mcp_servers: HashMap<String, McpServerConfig>,
        store_mode: OAuthCredentialsStoreMode,
    ) -> Result<(Self, ClientStartErrors)> {
        // Early exit if no servers are configured.
        if mcp_servers.is_empty() {
            return Ok((Self::default(), ClientStartErrors::default()));
        }

        // Launch all configured servers concurrently.
        let mut join_set = JoinSet::new();
        let mut errors = ClientStartErrors::new();
        let mut tool_filters: HashMap<String, ToolFilter> = HashMap::new();

        for (server_name, cfg) in mcp_servers {
            // Validate server name before spawning
            if !is_valid_mcp_server_name(&server_name) {
                let error = anyhow::anyhow!(
                    "invalid server name '{server_name}': must match pattern ^[a-zA-Z0-9_-]+$"
                );
                errors.insert(server_name, error);
                continue;
            }

            if !cfg.enabled {
                tool_filters.insert(server_name, ToolFilter::from_config(&cfg));
                continue;
            }

            let startup_timeout = cfg.startup_timeout_sec.unwrap_or(DEFAULT_STARTUP_TIMEOUT);
            let tool_timeout = cfg.tool_timeout_sec.unwrap_or(DEFAULT_TOOL_TIMEOUT);
            tool_filters.insert(server_name.clone(), ToolFilter::from_config(&cfg));

            let resolved_bearer_token = match &cfg.transport {
                McpServerTransportConfig::StreamableHttp {
                    bearer_token_env_var,
                    ..
                } => resolve_bearer_token(&server_name, bearer_token_env_var.as_deref()),
                _ => Ok(None),
            };

            join_set.spawn(async move {
                let McpServerConfig { transport, .. } = cfg;
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

                let resolved_bearer_token = resolved_bearer_token.unwrap_or_default();
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
                        match RmcpClient::new_stdio_client(command_os, args_os, env, &env_vars, cwd)
                            .await
                        {
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
                        ..
                    } => {
                        match RmcpClient::new_streamable_http_client(
                            &server_name,
                            &url,
                            resolved_bearer_token.clone(),
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

                (
                    (server_name, tool_timeout),
                    client_result.map(|client| (client, startup_timeout)),
                )
            });
        }

        let mut clients: HashMap<String, ManagedClient> = HashMap::with_capacity(join_set.len());

        while let Some(res) = join_set.join_next().await {
            let ((server_name, tool_timeout), client_res) = match res {
                Ok(result) => result,
                Err(e) => {
                    warn!("Task panic when starting MCP server: {e:#}");
                    continue;
                }
            };

            match client_res {
                Ok((client, startup_timeout)) => {
                    clients.insert(
                        server_name,
                        ManagedClient {
                            client,
                            startup_timeout,
                            tool_timeout: Some(tool_timeout),
                        },
                    );
                }
                Err(e) => {
                    errors.insert(server_name, e);
                }
            }
        }

        let all_tools = match list_all_tools(&clients).await {
            Ok(tools) => tools,
            Err(e) => {
                warn!("Failed to list tools from some MCP servers: {e:#}");
                Vec::new()
            }
        };

        let filtered_tools = filter_tools(all_tools, &tool_filters);
        let tools = qualify_tools(filtered_tools);

        Ok((
            Self {
                clients,
                tools,
                tool_filters,
            },
            errors,
        ))
    }

    /// Returns a single map that contains all tools. Each key is the
    /// fully-qualified name for the tool.
    pub fn list_all_tools(&self) -> HashMap<String, Tool> {
        self.tools
            .iter()
            .map(|(name, tool)| (name.clone(), tool.tool.clone()))
            .collect()
    }

    /// Returns a single map that contains all resources. Each key is the
    /// server name and the value is a vector of resources.
    pub async fn list_all_resources(&self) -> HashMap<String, Vec<Resource>> {
        let mut join_set = JoinSet::new();

        for (server_name, managed_client) in &self.clients {
            let server_name_cloned = server_name.clone();
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
                        Err(err) => return (server_name_cloned, Err(err)),
                    };

                    collected.extend(response.resources);

                    match response.next_cursor {
                        Some(next) => {
                            if cursor.as_ref() == Some(&next) {
                                return (
                                    server_name_cloned,
                                    Err(anyhow!("resources/list returned duplicate cursor")),
                                );
                            }
                            cursor = Some(next);
                        }
                        None => return (server_name_cloned, Ok(collected)),
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

        for (server_name, managed_client) in &self.clients {
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
        if let Some(filter) = self.tool_filters.get(server)
            && !filter.allows(tool)
        {
            return Err(anyhow!(
                "tool '{tool}' is disabled for MCP server '{server}'"
            ));
        }
        let managed = self
            .clients
            .get(server)
            .ok_or_else(|| anyhow!("unknown MCP server '{server}'"))?;
        let client = &managed.client;
        let timeout = managed.tool_timeout;

        client
            .call_tool(tool.to_string(), arguments, timeout)
            .await
            .with_context(|| format!("tool call failed for `{server}/{tool}`"))
    }

    /// List resources from the specified server.
    pub async fn list_resources(
        &self,
        server: &str,
        params: Option<ListResourcesRequestParams>,
    ) -> Result<ListResourcesResult> {
        let managed = self
            .clients
            .get(server)
            .ok_or_else(|| anyhow!("unknown MCP server '{server}'"))?;
        let client = managed.client.clone();
        let timeout = managed.tool_timeout;

        client
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
        let managed = self
            .clients
            .get(server)
            .ok_or_else(|| anyhow!("unknown MCP server '{server}'"))?;
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
        let managed = self
            .clients
            .get(server)
            .ok_or_else(|| anyhow!("unknown MCP server '{server}'"))?;
        let client = managed.client.clone();
        let timeout = managed.tool_timeout;
        let uri = params.uri.clone();

        client
            .read_resource(params, timeout)
            .await
            .with_context(|| format!("resources/read failed for `{server}` ({uri})"))
    }

    pub fn parse_tool_name(&self, tool_name: &str) -> Option<(String, String)> {
        self.tools
            .get(tool_name)
            .map(|tool| (tool.server_name.clone(), tool.tool_name.clone()))
    }
}

/// A tool is allowed to be used if both are true:
/// 1. enabled is None (no allowlist is set) or the tool is explicitly enabled.
/// 2. The tool is not explicitly disabled.
#[derive(Default, Clone)]
struct ToolFilter {
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

fn filter_tools(tools: Vec<ToolInfo>, filters: &HashMap<String, ToolFilter>) -> Vec<ToolInfo> {
    tools
        .into_iter()
        .filter(|tool| {
            filters
                .get(&tool.server_name)
                .is_none_or(|filter| filter.allows(&tool.tool_name))
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

/// Query every server for its available tools and return a single map that
/// contains all tools. Each key is the fully-qualified name for the tool.
async fn list_all_tools(clients: &HashMap<String, ManagedClient>) -> Result<Vec<ToolInfo>> {
    let mut join_set = JoinSet::new();

    // Spawn one task per server so we can query them concurrently. This
    // keeps the overall latency roughly at the slowest server instead of
    // the cumulative latency.
    for (server_name, managed_client) in clients {
        let server_name_cloned = server_name.clone();
        let client_clone = managed_client.client.clone();
        let startup_timeout = managed_client.startup_timeout;
        join_set.spawn(async move {
            let res = client_clone.list_tools(None, Some(startup_timeout)).await;
            (server_name_cloned, res)
        });
    }

    let mut aggregated: Vec<ToolInfo> = Vec::with_capacity(join_set.len());

    while let Some(join_res) = join_set.join_next().await {
        let (server_name, list_result) = if let Ok(result) = join_res {
            result
        } else {
            warn!("Task panic when listing tools for MCP server: {join_res:#?}");
            continue;
        };

        let list_result = if let Ok(result) = list_result {
            result
        } else {
            warn!("Failed to list tools for MCP server '{server_name}': {list_result:#?}");
            continue;
        };

        for tool in list_result.tools {
            let tool_info = ToolInfo {
                server_name: server_name.clone(),
                tool_name: tool.name.clone(),
                tool,
            };
            aggregated.push(tool_info);
        }
    }

    info!(
        "aggregated {} tools from {} servers",
        aggregated.len(),
        clients.len()
    );

    Ok(aggregated)
}

fn is_valid_mcp_server_name(server_name: &str) -> bool {
    !server_name.is_empty()
        && server_name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;
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

        let qualified_tools = qualify_tools(tools);

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

        let qualified_tools = qualify_tools(tools);

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

        let qualified_tools = qualify_tools(tools);

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

        let filtered = filter_tools(tools, &filters);

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].server_name, "server1");
        assert_eq!(filtered[0].tool_name, "tool_a");
    }
}
