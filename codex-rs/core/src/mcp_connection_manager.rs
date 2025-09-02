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
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use codex_mcp_client::McpClient;
use mcp_types::ClientCapabilities;
use mcp_types::Implementation;
use mcp_types::Tool;

use serde_json::json;
use sha1::Digest;
use sha1::Sha1;
use tokio::sync::Mutex as AsyncMutex;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tokio::time::sleep;
use tracing::info;
use tracing::warn;

use crate::config_types::McpServerConfig;

/// Delimiter used to separate the server name from the tool name in a fully
/// qualified tool name.
///
/// OpenAI requires tool names to conform to `^[a-zA-Z0-9_-]+$`, so we must
/// choose a delimiter from this character set.
const MCP_TOOL_NAME_DELIMITER: &str = "__";
const MAX_TOOL_NAME_LENGTH: usize = 64;

/// Timeout for the `tools/list` request.
const LIST_TOOLS_TIMEOUT: Duration = Duration::from_secs(10);

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

/// A thin wrapper around MCP client state with lazy spawning.
#[derive(Default, Clone)]
pub(crate) struct McpConnectionManager {
    inner: std::sync::Arc<std::sync::Mutex<Inner>>, // interior mutability for lazy spawn
}

impl McpConnectionManager {
    #[inline]
    fn lock_inner(&self) -> std::sync::MutexGuard<'_, Inner> {
        // Recover poisoned locks by taking the inner state; avoid unwrap()
        match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

#[derive(Default)]
struct Inner {
    /// Server-name -> client instance (spawned lazily on first use).
    clients: HashMap<String, std::sync::Arc<McpClient>>,
    /// Fully qualified tool name -> tool instance (populated after server initialize/list).
    tools: HashMap<String, ToolInfo>,
    /// Monotonic version incremented whenever tools set changes.
    tools_version: u64,
    /// Watch channel to notify listeners when tools_version changes.
    tools_tx: watch::Sender<u64>,
    /// Original server spawn configs kept for lazy creation.
    server_configs: HashMap<String, McpServerConfig>,
    /// Per-server async init locks to serialize initialization and avoid races.
    init_locks: HashMap<String, std::sync::Arc<AsyncMutex<()>>>,
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
        // Validate server names but do not spawn – we will spawn lazily.
        let mut errors = ClientStartErrors::new();
        let mut valid_cfgs = HashMap::new();
        for (server_name, cfg) in mcp_servers {
            if !is_valid_mcp_server_name(&server_name) {
                let error = anyhow::anyhow!(
                    "invalid server name '{}': must match pattern ^[a-zA-Z0-9_-]+$",
                    server_name
                );
                errors.insert(server_name, error);
                continue;
            }
            valid_cfgs.insert(server_name, cfg);
        }
        let (tools_tx, _tools_rx) = watch::channel(0u64);
        let mgr = McpConnectionManager {
            inner: std::sync::Arc::new(std::sync::Mutex::new(Inner {
                clients: HashMap::new(),
                tools: HashMap::new(),
                tools_version: 0,
                tools_tx,
                server_configs: valid_cfgs,
                init_locks: HashMap::new(),
            })),
        };
        Ok((mgr, errors))
    }

    /// Returns a single map that contains **all** tools. Each key is the
    /// fully-qualified name for the tool.
    pub fn list_all_tools(&self) -> HashMap<String, Tool> {
        // Return the snapshot of known tools (may be empty until servers are first used).
        let guard = self.lock_inner();
        guard
            .tools
            .iter()
            .map(|(name, tool)| (name.clone(), tool.tool.clone()))
            .collect()
    }

    /// Ensure all configured servers are spawned in the background (non-blocking),
    /// then return the current snapshot of the tool map.
    /// This supports on-demand discovery for UI listing without blocking the UI.
    pub async fn list_all_tools_on_demand(&self) -> HashMap<String, Tool> {
        // Collect server names to avoid holding the lock across awaits.
        // Also compute a per-server cache presence flag by checking if we have
        // any qualified tool names starting with "<server>__".
        let (server_names, has_clients, server_has_tools): (Vec<String>, Vec<bool>, Vec<bool>) = {
            let guard = self.lock_inner();
            let names: Vec<String> = guard.server_configs.keys().cloned().collect();
            let has_clients: Vec<bool> = names
                .iter()
                .map(|n| guard.clients.contains_key(n))
                .collect();
            let server_has_tools: Vec<bool> = names
                .iter()
                .map(|n| {
                    let prefix = format!("{n}{MCP_TOOL_NAME_DELIMITER}");
                    guard.tools.keys().any(|k| k.starts_with(&prefix))
                })
                .collect();
            (names, has_clients, server_has_tools)
        };

        // Kick off background initialization or refresh without awaiting.
        for (idx, server) in server_names.iter().enumerate() {
            let server = server.clone();
            let already_client = has_clients[idx];
            let has_tools_for_server = server_has_tools[idx];
            // Spawn a task per server; best effort.
            let this = self.clone();
            tokio::spawn(async move {
                if !already_client {
                    // Lazy spawn and initialization also attempts to prefill this server's tools.
                    let _ = this.ensure_client(&server).await;
                } else if !has_tools_for_server {
                    // Try to refresh tools only for this server if its cache is empty.
                    let client_opt = {
                        let guard = this.lock_inner();
                        guard.clients.get(&server).cloned()
                    };
                    if let Some(client) = client_opt {
                        let _ =
                            client
                                .list_tools(None, Some(LIST_TOOLS_TIMEOUT))
                                .await
                                .map(|list| {
                                    let mut guard = this.lock_inner();
                                    let mut added = false;
                                    for tool in list.tools {
                                        let info = ToolInfo {
                                            server_name: server.clone(),
                                            tool_name: tool.name.clone(),
                                            tool: tool.clone(),
                                        };
                                        let qualified = qualify_tools(vec![info]);
                                        if !qualified.is_empty() {
                                            added = true;
                                        }
                                        guard.tools.extend(qualified);
                                    }
                                    if added {
                                        guard.tools_version = guard.tools_version.saturating_add(1);
                                        let _ = guard.tools_tx.send(guard.tools_version);
                                    }
                                });
                    }
                }
            });
        }

        self.list_all_tools()
    }

    /// Invoke the tool indicated by the (server, tool) pair.
    pub async fn call_tool(
        &self,
        server: &str,
        tool: &str,
        arguments: Option<serde_json::Value>,
        timeout: Option<Duration>,
    ) -> Result<mcp_types::CallToolResult> {
        // Jeśli mamy timeout dla wywołania, wykorzystaj go także na inicjalizację serwera.
        if let Some(t) = timeout {
            match tokio::time::timeout(t, self.ensure_client(server)).await {
                Ok(Ok(client)) => {
                    return client
                        .call_tool(tool.to_string(), arguments, timeout)
                        .await
                        .with_context(|| format!("tool call failed for `{server}/{tool}`"));
                }
                Ok(Err(e)) => {
                    return Err(e)
                        .with_context(|| format!("failed to initialize MCP server '{server}'"));
                }
                Err(_) => {
                    return Err(anyhow!(format!(
                        "MCP server '{server}' is still initializing; try again later"
                    )));
                }
            }
        }

        let client = self.ensure_client(server).await?;

        client
            .call_tool(tool.to_string(), arguments, timeout)
            .await
            .with_context(|| format!("tool call failed for `{server}/{tool}`"))
    }

    /// Ensure a client exists for the given server, spawning and initializing lazily if needed.
    async fn ensure_client(&self, server: &str) -> Result<std::sync::Arc<McpClient>> {
        // Fast path: already spawned
        {
            let guard = self.lock_inner();
            if let Some(c) = guard.clients.get(server) {
                return Ok(c.clone());
            }
        }

        // Take config and initialize per-server async lock
        let (cfg, lock_arc) = {
            let mut guard = self.lock_inner();
            let cfg = guard
                .server_configs
                .get(server)
                .cloned()
                .ok_or_else(|| anyhow!(format!("unknown MCP server '{server}'")))?;
            let lock = guard
                .init_locks
                .entry(server.to_string())
                .or_insert_with(|| std::sync::Arc::new(AsyncMutex::new(())))
                .clone();
            (cfg, lock)
        };

        // Serialize initialization for this server
        let _init_guard = lock_arc.lock().await;
        // Check again in case another waiter already finished init.
        {
            let guard = self.lock_inner();
            if let Some(c) = guard.clients.get(server) {
                return Ok(c.clone());
            }
        }

        let McpServerConfig { command, args, env } = cfg;
        let client = McpClient::new_stdio_client(
            command.into(),
            args.into_iter().map(OsString::from).collect(),
            env,
        )
        .await?;

        // Initialize
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
            },
            protocol_version: mcp_types::MCP_SCHEMA_VERSION.to_owned(),
        };
        let initialize_notification_params = None;
        let timeout = Some(Duration::from_secs(10));
        client
            .initialize(params, initialize_notification_params, timeout)
            .await
            .with_context(|| format!("failed to initialize MCP server '{server}'"))?;

        let client = std::sync::Arc::new(client);

        // Record client in map
        {
            let mut guard = self.lock_inner();
            guard.clients.insert(server.to_string(), client.clone());
        }

        // Try to fetch and cache tools for this server with light retry; ignore failures.
        let delays = [
            Duration::from_millis(50),
            Duration::from_millis(200),
            Duration::from_secs(1),
        ];
        for (i, d) in delays.iter().enumerate() {
            match client.list_tools(None, Some(LIST_TOOLS_TIMEOUT)).await {
                Ok(list) => {
                    let mut guard = self.lock_inner();
                    let mut added = false;
                    for tool in list.tools {
                        let info = ToolInfo {
                            server_name: server.to_string(),
                            tool_name: tool.name.clone(),
                            tool: tool.clone(),
                        };
                        let qualified = qualify_tools(vec![info]);
                        if !qualified.is_empty() {
                            added = true;
                        }
                        guard.tools.extend(qualified);
                    }
                    if added {
                        guard.tools_version = guard.tools_version.saturating_add(1);
                        let _ = guard.tools_tx.send(guard.tools_version);
                    }
                    break;
                }
                Err(_) if i + 1 < delays.len() => sleep(*d).await,
                Err(_) => break,
            }
        }

        Ok(client)
    }

    pub fn parse_tool_name(&self, tool_name: &str) -> Option<(String, String)> {
        let guard = self.lock_inner();
        guard
            .tools
            .get(tool_name)
            .map(|tool| (tool.server_name.clone(), tool.tool_name.clone()))
    }

    /// Subskrybuj zmiany listy narzędzi; emituje rosnący `tools_version` przy aktualizacji.
    ///
    /// Uwaga: obecnie nieużywane przez ścieżkę renderowania `/mcp`, która
    /// zwraca natychmiastowy snapshot bez oczekiwania. Pozostawiamy API
    /// do ewentualnych przyszłych live‑update’ów w TUI.
    #[allow(dead_code)]
    pub fn subscribe_tools_changes(&self) -> watch::Receiver<u64> {
        let guard = self.lock_inner();
        guard.tools_tx.subscribe()
    }
}

/// Query every server for its available tools and return a single map that
/// contains **all** tools. Each key is the fully-qualified name for the tool.
#[allow(dead_code)]
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
