//! Connection manager for Model Context Protocol (MCP) servers.
//!
//! The [`McpConnectionManager`] owns one [`codex_mcp_client::McpClient`] per
//! configured server (keyed by the *server name*). It offers convenience
//! helpers to query the available tools across *all* servers and returns them
//! in a single aggregated map using the fully-qualified tool name
//! `"<server><MCP_TOOL_NAME_DELIMITER><tool>"` as the key.

use std::collections::HashMap;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use codex_mcp_client::McpClient;
use mcp_types::ClientCapabilities;
use mcp_types::Implementation;
use mcp_types::Tool;
use tokio::task::JoinSet;
use tracing::info;

use crate::config_types::McpServerConfig;

/// Delimiter used to separate the server name from the tool name in a fully
/// qualified tool name.
///
/// OpenAI requires tool names to conform to `^[a-zA-Z0-9_-]+$`, so we must
/// choose a delimiter from this character set. We use a short delimiter to
/// maximize the remaining characters available for server and tool names
/// within OpenAI's 64-character limit.
const MCP_TOOL_NAME_DELIMITER: &str = "__";

/// Timeout for the `tools/list` request.
const LIST_TOOLS_TIMEOUT: Duration = Duration::from_secs(10);

/// Maximum length for OpenAI tool names.
const MAX_TOOL_NAME_LENGTH: usize = 64;

/// Map that holds a startup error for every MCP server that could **not** be
/// spawned successfully.
pub type ClientStartErrors = HashMap<String, anyhow::Error>;

fn fully_qualified_tool_name(server: &str, tool: &str) -> String {
    format!("{server}{MCP_TOOL_NAME_DELIMITER}{tool}")
}

pub(crate) fn try_parse_fully_qualified_tool_name(fq_name: &str) -> Option<(String, String)> {
    let (server, tool) = fq_name.split_once(MCP_TOOL_NAME_DELIMITER)?;
    if server.is_empty() || tool.is_empty() {
        return None;
    }
    Some((server.to_string(), tool.to_string()))
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
    tools: HashMap<String, Tool>,
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

        let tools = list_all_tools(&clients).await?;

        Ok((Self { clients, tools }, errors))
    }

    /// Returns a single map that contains **all** tools. Each key is the
    /// fully-qualified name for the tool.
    pub fn list_all_tools(&self) -> HashMap<String, Tool> {
        self.tools.clone()
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
}

/// Query every server for its available tools and return a single map that
/// contains **all** tools. Each key is the fully-qualified name for the tool.
pub async fn list_all_tools(
    clients: &HashMap<String, std::sync::Arc<McpClient>>,
) -> Result<HashMap<String, Tool>> {
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

    let mut aggregated: HashMap<String, Tool> = HashMap::with_capacity(join_set.len());

    while let Some(join_res) = join_set.join_next().await {
        let (server_name, list_result) = join_res?;
        let list_result = list_result?;

        for tool in list_result.tools {
            // TODO(mbolin): escape tool names that contain invalid characters.
            let mut fq_name = fully_qualified_tool_name(&server_name, &tool.name);

            // Ensure the fully qualified name doesn't exceed OpenAI's limit
            if fq_name.len() > MAX_TOOL_NAME_LENGTH {
                // Truncate the tool name part to fit within the limit
                let prefix_len = server_name.len() + MCP_TOOL_NAME_DELIMITER.len();
                let max_tool_len = MAX_TOOL_NAME_LENGTH.saturating_sub(prefix_len);

                if max_tool_len < 3 {
                    // Server name alone is too long
                    tracing::warn!(
                        "Skipping tool '{}' from server '{}': server name too long for OpenAI limit",
                        tool.name,
                        server_name
                    );
                    continue;
                }

                // Truncate tool name and add a hash suffix for uniqueness
                let truncated_tool = if tool.name.len() > max_tool_len {
                    // Simple hash based on string bytes
                    let hash: u32 = tool
                        .name
                        .bytes()
                        .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
                    let hash_suffix = format!("{:04x}", hash & 0xFFFF); // Use lower 16 bits as 4-char hex
                    let available_len = max_tool_len.saturating_sub(5); // 4 for hash + 1 for underscore
                    format!("{}_{}", &tool.name[..available_len], hash_suffix)
                } else {
                    tool.name.clone()
                };

                fq_name = fully_qualified_tool_name(&server_name, &truncated_tool);
                tracing::info!(
                    "Truncated tool name from '{}' to '{}' to fit OpenAI limit",
                    tool.name,
                    truncated_tool
                );
            }

            if aggregated.insert(fq_name.clone(), tool).is_some() {
                panic!("tool name collision for '{fq_name}': suspicious");
            }
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

    #[test]
    fn test_fully_qualified_tool_name_length() {
        // Test that delimiter is short
        assert_eq!(MCP_TOOL_NAME_DELIMITER.len(), 2);

        // Test normal case
        let fq_name = fully_qualified_tool_name("myserver", "mytool");
        assert_eq!(fq_name, "myserver__mytool");
        assert!(fq_name.len() <= MAX_TOOL_NAME_LENGTH);

        // Test parsing
        let parsed = try_parse_fully_qualified_tool_name("myserver__mytool");
        assert_eq!(parsed, Some(("myserver".to_string(), "mytool".to_string())));

        // Test invalid parsing
        assert_eq!(try_parse_fully_qualified_tool_name("no_delimiter"), None);
        assert_eq!(try_parse_fully_qualified_tool_name("__only_tool"), None);
        assert_eq!(try_parse_fully_qualified_tool_name("only_server__"), None);
    }

    #[test]
    fn test_long_tool_names() {
        // Test that very long server names would be handled
        let long_server = "a".repeat(50);
        let long_tool = "b".repeat(50);
        let fq_name = fully_qualified_tool_name(&long_server, &long_tool);

        // With delimiter of 2 chars, 50 + 2 + 50 = 102 chars, which exceeds 64
        assert!(fq_name.len() > MAX_TOOL_NAME_LENGTH);

        // The actual truncation logic is in list_all_tools, but we can verify
        // that our delimiter change helps maximize available space
        let available_for_names = MAX_TOOL_NAME_LENGTH - MCP_TOOL_NAME_DELIMITER.len();
        assert_eq!(available_for_names, 62); // Much better than 47 with old delimiter
    }
}
