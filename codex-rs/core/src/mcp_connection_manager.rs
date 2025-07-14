//! Connection manager for Model Context Protocol (MCP) servers.
//!
//! The [`McpConnectionManager`] owns one [`codex_mcp_client::McpClient`] per
//! configured server (keyed by the *server name*). It offers convenience
//! helpers to query the available tools across *all* servers and returns them
//! in a single aggregated map using the fully-qualified tool name
//! `"<server><MCP_TOOL_NAME_DELIMITER><tool>"` as the key.

use std::collections::HashMap;
use std::collections::HashSet;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use codex_mcp_client::McpClient;
use mcp_types::ClientCapabilities;
use mcp_types::Implementation;
use mcp_types::Tool;
use rand::distr::Alphanumeric;
use rand::distr::SampleString;
use tokio::task::JoinSet;
use tracing::info;

use crate::config_types::McpServerConfig;

/// Delimiter used to separate the server name from the tool name in a fully
/// qualified tool name.
///
/// OpenAI requires tool names to conform to `^[a-zA-Z0-9_-]+$`, so we must
/// choose a delimiter from this character set.
const MCP_TOOL_NAME_DELIMITER: &str = "__";
const MAX_TOOL_NAME_LENGTH: usize = 64;
const RANDOM_SUFFIX_LENGTH: usize = 10;

/// Timeout for the `tools/list` request.
const LIST_TOOLS_TIMEOUT: Duration = Duration::from_secs(10);

/// Map that holds a startup error for every MCP server that could **not** be
/// spawned successfully.
pub type ClientStartErrors = HashMap<String, anyhow::Error>;

fn qualify_tools(tools: Vec<ToolInfo>) -> HashMap<String, ToolInfo> {
    let mut used_names = HashSet::new();
    let mut qualified_tools = HashMap::new();
    for tool in tools {
        let mut base_name = format!(
            "{}{}{}",
            tool.server_name, MCP_TOOL_NAME_DELIMITER, tool.tool_name
        );
        if base_name.len() > MAX_TOOL_NAME_LENGTH {
            base_name = base_name[..MAX_TOOL_NAME_LENGTH].to_string();
        }

        let mut deduplicated_name = base_name.clone();
        while used_names.contains(&deduplicated_name) {
            let random_suffix = Alphanumeric.sample_string(&mut rand::rng(), RANDOM_SUFFIX_LENGTH);
            let trim_length =
                std::cmp::min(MAX_TOOL_NAME_LENGTH - RANDOM_SUFFIX_LENGTH, base_name.len());
            deduplicated_name = format!(
                "{}{}",
                &base_name[..trim_length],
                random_suffix.to_lowercase()
            );
        }

        used_names.insert(deduplicated_name.clone());
        qualified_tools.insert(deduplicated_name, tool);
    }

    qualified_tools
}

struct ToolInfo {
    server_name: String,
    tool_name: String,
    tool: Tool,
}

/// A thin wrapper around a set of running [`McpClient`] instances.
#[derive(Default)]
pub(crate) struct McpConnectionManager {
    /// Server-name -> client instance.
    ///
    /// The server name originates from the keys of the `mcp_servers` map in
    /// the user configuration.
    clients: HashMap<String, std::sync::Arc<McpClient>>,

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
    ) -> Result<(Self, ClientStartErrors)> {
        // Early exit if no servers are configured.
        if mcp_servers.is_empty() {
            return Ok((Self::default(), ClientStartErrors::default()));
        }

        // Launch all configured servers concurrently.
        let mut join_set = JoinSet::new();

        for (server_name, cfg) in mcp_servers {
            // TODO: Verify server name: require `^[a-zA-Z0-9_-]+$`?
            join_set.spawn(async move {
                let McpServerConfig { command, args, env } = cfg;
                let client_res = McpClient::new_stdio_client(command, args, env).await;
                match client_res {
                    Ok(client) => {
                        // Initialize the client.
                        let params = mcp_types::InitializeRequestParams {
                            capabilities: ClientCapabilities {
                                experimental: None,
                                roots: None,
                                sampling: None,
                            },
                            client_info: Implementation {
                                name: "codex-mcp-client".to_owned(),
                                version: env!("CARGO_PKG_VERSION").to_owned(),
                            },
                            protocol_version: mcp_types::MCP_SCHEMA_VERSION.to_owned(),
                        };
                        let initialize_notification_params = None;
                        let timeout = Some(Duration::from_secs(10));
                        match client
                            .initialize(params, initialize_notification_params, timeout)
                            .await
                        {
                            Ok(_response) => (server_name, Ok(client)),
                            Err(e) => (server_name, Err(e)),
                        }
                    }
                    Err(e) => (server_name, Err(e.into())),
                }
            });
        }

        let mut clients: HashMap<String, std::sync::Arc<McpClient>> =
            HashMap::with_capacity(join_set.len());
        let mut errors = ClientStartErrors::new();

        while let Some(res) = join_set.join_next().await {
            let (server_name, client_res) = res?; // JoinError propagation

            match client_res {
                Ok(client) => {
                    clients.insert(server_name, std::sync::Arc::new(client));
                }
                Err(e) => {
                    errors.insert(server_name, e);
                }
            }
        }

        let all_tools = list_all_tools(&clients).await?;

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
        timeout: Option<Duration>,
    ) -> Result<mcp_types::CallToolResult> {
        let client = self
            .clients
            .get(server)
            .ok_or_else(|| anyhow!("unknown MCP server '{server}'"))?
            .clone();

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
async fn list_all_tools(
    clients: &HashMap<String, std::sync::Arc<McpClient>>,
) -> Result<Vec<ToolInfo>> {
    let mut join_set = JoinSet::new();

    // Spawn one task per server so we can query them concurrently. This
    // keeps the overall latency roughly at the slowest server instead of
    // the cumulative latency.
    for (server_name, client) in clients {
        let server_name_cloned = server_name.clone();
        let client_clone = client.clone();
        join_set.spawn(async move {
            let res = client_clone
                .list_tools(None, Some(LIST_TOOLS_TIMEOUT))
                .await;
            (server_name_cloned, res)
        });
    }

    let mut aggregated: Vec<ToolInfo> = Vec::with_capacity(join_set.len());

    while let Some(join_res) = join_set.join_next().await {
        let (server_name, list_result) = join_res?;
        let list_result = list_result?;

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
                description: Some(format!("Test tool: {}", tool_name)),
                input_schema: ToolInputSchema {
                    properties: None,
                    required: None,
                    r#type: "object".to_string(),
                },
                name: tool_name.to_string(),
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
    fn test_qualify_tools_long_names_trimmed() {
        let tools = vec![create_test_tool(
            "very_long_server_name_that_exceeds_limits",
            "very_long_tool_name_that_also_exceeds_limits",
        )];

        let qualified_tools = qualify_tools(tools);

        assert_eq!(qualified_tools.len(), 1);
        let keys: Vec<_> = qualified_tools.keys().cloned().collect();

        assert_eq!(keys[0].len(), 64);
        assert_eq!(
            keys[0],
            "very_long_server_name_that_exceeds_limits__very_long_tool_name_t"
        );
    }

    #[test]
    fn test_qualify_tools_duplicated_names_with_suffix() {
        let tools = vec![
            create_test_tool("server1", "duplicate_tool"),
            create_test_tool("server1", "duplicate_tool"),
        ];

        let qualified_tools = qualify_tools(tools);

        assert_eq!(qualified_tools.len(), 2);

        let mut keys: Vec<_> = qualified_tools.keys().cloned().collect();
        keys.sort();

        assert_ne!(keys[0], keys[1]);

        assert_eq!(keys[0], "server1__duplicate_tool");

        assert!(keys[1].starts_with("server1__duplicate_tool"));

        let suffix = &keys[1][23..];
        assert_eq!(suffix.len(), 10);
        assert!(
            suffix
                .chars()
                .all(|c| c.is_alphanumeric() && !c.is_uppercase())
        );
    }

    #[test]
    fn test_qualify_tools_long_duplicated_names_conflict() {
        let tool_name =
            "very_long_tool_name_that_definitely_exceeds_the_maximum_length_of_64_characters";
        let server_name = "server";

        let tools = vec![
            create_test_tool(server_name, tool_name),
            create_test_tool(server_name, tool_name),
        ];

        let qualified_tools = qualify_tools(tools);

        assert_eq!(qualified_tools.len(), 2);

        let mut keys: Vec<_> = qualified_tools.keys().cloned().collect();
        keys.sort();

        assert_ne!(keys[0], keys[1]);

        assert_eq!(
            keys[0],
            "server__very_long_tool_name_that_definitely_exceeds_the_maximum_"
        );
        assert!(keys[1].starts_with("server__very_long_tool_name_that_definitely_exceeds_th"));

        let suffix = &keys[1][54..];
        assert_eq!(suffix.len(), 10);
        assert!(
            suffix
                .chars()
                .all(|c| c.is_alphanumeric() && !c.is_uppercase())
        );
    }
}
