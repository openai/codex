//! Connection manager for Model Context Protocol (MCP) servers.
//!
//! The [`McpConnectionManager`] owns one [`codex_mcp_client::McpClient`] per
//! configured server (keyed by the *server name*). It offers convenience
//! helpers to query the available tools across *all* servers and returns them
//! in a single aggregated map using the fully-qualified tool name
//! `"<server><MCP_TOOL_NAME_DELIMITER><tool>"` as the key.

use std::collections::HashMap;
use std::collections::HashSet;
use std::ffi::OsString;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use codex_mcp_client::McpClient;
use codex_rmcp_client::RmcpClient;
use mcp_types::ClientCapabilities;
use mcp_types::Implementation;
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
const DEFAULT_STARTUP_TIMEOUT: Duration = Duration::from_secs(10);

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
            "{}{}{}",
            tool.server_name, MCP_TOOL_NAME_DELIMITER, tool.tool_name
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
    client: McpClientAdapter,
    startup_timeout: Duration,
    tool_timeout: Option<Duration>,
}

#[derive(Clone)]
enum McpClientAdapter {
    Legacy(Arc<McpClient>),
    Rmcp(Arc<RmcpClient>),
}

impl McpClientAdapter {
    async fn new_stdio_client(
        use_rmcp_client: bool,
        program: OsString,
        args: Vec<OsString>,
        env: Option<HashMap<String, String>>,
        params: mcp_types::InitializeRequestParams,
        startup_timeout: Duration,
    ) -> Result<Self> {
        info!(
            "new_stdio_client use_rmcp_client: {use_rmcp_client} program: {program:?} args: {args:?} env: {env:?} params: {params:?} startup_timeout: {startup_timeout:?}"
        );
        if use_rmcp_client {
            let client = Arc::new(RmcpClient::new_stdio_client(program, args, env).await?);
            client.initialize(params, Some(startup_timeout)).await?;
            Ok(McpClientAdapter::Rmcp(client))
        } else {
            let client = Arc::new(McpClient::new_stdio_client(program, args, env).await?);
            client.initialize(params, Some(startup_timeout)).await?;
            Ok(McpClientAdapter::Legacy(client))
        }
    }

    async fn new_streamable_http_client(
        url: String,
        bearer_token: Option<String>,
        params: mcp_types::InitializeRequestParams,
        startup_timeout: Duration,
    ) -> Result<Self> {
        let client = Arc::new(RmcpClient::new_streamable_http_client(url, bearer_token)?);
        client.initialize(params, Some(startup_timeout)).await?;
        Ok(McpClientAdapter::Rmcp(client))
    }

    async fn list_tools(
        &self,
        params: Option<mcp_types::ListToolsRequestParams>,
        timeout: Option<Duration>,
    ) -> Result<mcp_types::ListToolsResult> {
        match self {
            McpClientAdapter::Legacy(client) => client.list_tools(params, timeout).await,
            McpClientAdapter::Rmcp(client) => client.list_tools(params, timeout).await,
        }
    }

    async fn call_tool(
        &self,
        name: String,
        arguments: Option<serde_json::Value>,
        timeout: Option<Duration>,
    ) -> Result<mcp_types::CallToolResult> {
        match self {
            McpClientAdapter::Legacy(client) => client.call_tool(name, arguments, timeout).await,
            McpClientAdapter::Rmcp(client) => client.call_tool(name, arguments, timeout).await,
        }
    }
}

/// A thin wrapper around a set of running [`McpClient`] instances.
#[derive(Default)]
pub(crate) struct McpConnectionManager {
    /// Server-name -> client instance.
    ///
    /// The server name originates from the keys of the `mcp_servers` map in
    /// the user configuration.
    clients: HashMap<String, ManagedClient>,

    /// Fully qualified tool name -> tool instance.
    tools: HashMap<String, ToolInfo>,
}

impl McpConnectionManager {
    /// Spawn a [`McpClient`] for each configured server.
    ///
    /// * `mcp_servers` – Map loaded from the user configuration where *keys*
    ///   are human-readable server identifiers and *values* are the spawn
    ///   instructions.
    ///
    /// Servers that fail to start are reported in `ClientStartErrors`: the
    /// user should be informed about these errors.
    pub async fn new(
        mcp_servers: HashMap<String, McpServerConfig>,
        use_rmcp_client: bool,
    ) -> Result<(Self, ClientStartErrors)> {
        // Early exit if no servers are configured.
        if mcp_servers.is_empty() {
            return Ok((Self::default(), ClientStartErrors::default()));
        }

        // Launch all configured servers concurrently.
        let mut join_set = JoinSet::new();
        let mut errors = ClientStartErrors::new();

        for (server_name, cfg) in mcp_servers {
            // Validate server name before spawning
            if !is_valid_mcp_server_name(&server_name) {
                let error = anyhow::anyhow!(
                    "invalid server name '{}': must match pattern ^[a-zA-Z0-9_-]+$",
                    server_name
                );
                errors.insert(server_name, error);
                continue;
            }

            if matches!(
                cfg.transport,
                McpServerTransportConfig::StreamableHttp { .. }
            ) && !use_rmcp_client
            {
                info!(
                    "skipping MCP server `{}` configured with url because rmcp client is disabled",
                    server_name
                );
                continue;
            }

            let startup_timeout = cfg.startup_timeout_sec.unwrap_or(DEFAULT_STARTUP_TIMEOUT);
            let tool_timeout = cfg.tool_timeout_sec.unwrap_or(DEFAULT_TOOL_TIMEOUT);

            let use_rmcp_client_flag = use_rmcp_client;
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

                let client = match transport {
                    McpServerTransportConfig::Stdio { command, args, env } => {
                        let command_os: OsString = command.into();
                        let args_os: Vec<OsString> = args.into_iter().map(Into::into).collect();
                        McpClientAdapter::new_stdio_client(
                            use_rmcp_client_flag,
                            command_os,
                            args_os,
                            env,
                            params.clone(),
                            startup_timeout,
                        )
                        .await
                    }
                    McpServerTransportConfig::StreamableHttp { url, bearer_token } => {
                        McpClientAdapter::new_streamable_http_client(
                            url,
                            bearer_token,
                            params,
                            startup_timeout,
                        )
                        .await
                    }
                }
                .map(|c| (c, startup_timeout));

                ((server_name, tool_timeout), client)
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

        let tools = qualify_tools(all_tools);

        Ok((Self { clients, tools }, errors))
    }

    /// Returns a single map that contains **all** tools. Each key is the
    /// fully-qualified name for the tool.
    pub fn list_all_tools(&self) -> HashMap<String, Tool> {
        self.tools
            .iter()
            .map(|(name, tool)| (name.clone(), tool.tool.clone()))
            .collect()
    }

    /// Invoke the tool indicated by the (server, tool) pair.
    pub async fn call_tool(
        &self,
        server: &str,
        tool: &str,
        arguments: Option<serde_json::Value>,
    ) -> Result<mcp_types::CallToolResult> {
        let managed = self
            .clients
            .get(server)
            .ok_or_else(|| anyhow!("unknown MCP server '{server}'"))?;
        let client = managed.client.clone();
        let timeout = managed.tool_timeout;

        client
            .call_tool(tool.to_string(), arguments, timeout)
            .await
            .with_context(|| format!("tool call failed for `{server}/{tool}`"))
    }

    pub fn parse_tool_name(&self, tool_name: &str) -> Option<(String, String)> {
        self.tools
            .get(tool_name)
            .map(|tool| (tool.server_name.clone(), tool.tool_name.clone()))
    }
}

/// Query every server for its available tools and return a single map that
/// contains **all** tools. Each key is the fully-qualified name for the tool.
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
        assert!(qualified_tools.contains_key("server1__tool1"));
        assert!(qualified_tools.contains_key("server1__tool2"));
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
        assert!(qualified_tools.contains_key("server1__duplicate_tool"));
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
            "my_server__extremely_lena02e507efc5a9de88637e436690364fd4219e4ef"
        );

        assert_eq!(keys[1].len(), 64);
        assert_eq!(
            keys[1],
            "my_server__yet_another_e1c3987bd9c50b826cbe1687966f79f0c602d19ca"
        );
    }
}
