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
use std::sync::RwLock;
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
use tokio::sync::OnceCell;
use tokio::sync::watch;
use tokio::task::JoinSet;
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
/// Timeout for MCP initialize handshake.
const INIT_TIMEOUT: Duration = Duration::from_secs(10);

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

/// A thin wrapper around a set of running [`McpClient`] instances.
pub(crate) struct McpConnectionManager {
    /// Server-name -> client instance.
    ///
    /// The server name originates from the keys of the `mcp_servers` map in
    /// the user configuration.
    clients: HashMap<String, std::sync::Arc<McpClient>>,

    /// Fully qualified tool name -> tool instance. Populated asynchronously
    /// after clients are initialized to avoid blocking shell startup.
    tools: std::sync::Arc<RwLock<HashMap<String, ToolInfo>>>,

    /// Lazy server initialization status (initialize is sent once per server on first use)
    init_cells: HashMap<String, std::sync::Arc<OnceCell<()>>>,

    /// Broadcasts current MCP tool count so callers can await initial load.
    tool_count_tx: watch::Sender<usize>,
}

impl Default for McpConnectionManager {
    fn default() -> Self {
        let (tx, _rx) = watch::channel(0usize);
        Self {
            clients: HashMap::new(),
            tools: std::sync::Arc::new(RwLock::new(HashMap::new())),
            init_cells: HashMap::new(),
            tool_count_tx: tx,
        }
    }
}

impl McpConnectionManager {
    fn build_initialize_params() -> mcp_types::InitializeRequestParams {
        mcp_types::InitializeRequestParams {
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
        }
    }
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

            join_set.spawn(async move {
                let client_res = match cfg {
                    McpServerConfig::Stdio { command, args, env } => McpClient::new_stdio_client(
                        command.into(),
                        args.into_iter().map(OsString::from).collect(),
                        env,
                    )
                    .await
                    .map_err(anyhow::Error::from),
                    McpServerConfig::StreamableHttp { url, headers } => {
                        McpClient::new_streamable_http_client(url, headers).await
                    }
                };
                match client_res {
                    Ok(client) => (server_name, Ok(client)),
                    Err(e) => (server_name, Err(e)),
                }
            });
        }

        let mut clients: HashMap<String, std::sync::Arc<McpClient>> =
            HashMap::with_capacity(join_set.len());

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

        // Initialize with an empty tool registry. No network calls are made here
        // to keep shell startup and teardown snappy.
        let tools: std::sync::Arc<RwLock<HashMap<String, ToolInfo>>> =
            std::sync::Arc::new(RwLock::new(HashMap::new()));

        // Prepare lazy init cells for each server.
        let mut init_cells = HashMap::new();
        for server in clients.keys() {
            init_cells.insert(server.clone(), std::sync::Arc::new(OnceCell::new()));
        }

        let (tool_count_tx, _rx) = watch::channel(0usize);
        Ok((
            Self {
                clients,
                tools,
                init_cells,
                tool_count_tx,
            },
            errors,
        ))
    }

    /// Returns a single map that contains **all** tools. Each key is the
    /// fully-qualified name for the tool.
    pub fn list_all_tools(&self) -> HashMap<String, Tool> {
        match self.tools.read() {
            Ok(guard) => guard
                .iter()
                .map(|(name, tool)| (name.clone(), tool.tool.clone()))
                .collect(),
            Err(_) => HashMap::new(),
        }
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
        // Ensure the server is initialized before invoking tools
        self.ensure_initialized(server, &client).await?;
        client
            .call_tool(tool.to_string(), arguments, timeout)
            .await
            .with_context(|| format!("tool call failed for `{server}/{tool}`"))
    }

    pub fn parse_tool_name(&self, tool_name: &str) -> Option<(String, String)> {
        match self.tools.read() {
            Ok(guard) => guard
                .get(tool_name)
                .map(|tool| (tool.server_name.clone(), tool.tool_name.clone())),
            Err(_) => None,
        }
    }

    /// Refresh the internal tool registry synchronously on demand.
    pub async fn refresh_tools(&self) -> Result<()> {
        // Initialize each server on demand before listing tools
        for (server, client) in &self.clients {
            self.ensure_initialized(server, client).await?;
        }
        let clients_snapshot: HashMap<_, _> = self
            .clients
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let all_tools = list_all_tools(&clients_snapshot).await?;
        let qualified = qualify_tools(all_tools);
        if let Ok(mut guard) = self.tools.write() {
            *guard = qualified;
        }
        let _ = self
            .tool_count_tx
            .send(self.tools.read().map(|m| m.len()).unwrap_or(0));
        Ok(())
    }

    async fn ensure_initialized(
        &self,
        server_name: &str,
        client: &std::sync::Arc<McpClient>,
    ) -> Result<()> {
        let cell = self
            .init_cells
            .get(server_name)
            .ok_or_else(|| anyhow!(format!("missing init cell for server {server_name}")))?
            .clone();

        let initialize = async {
            let params = Self::build_initialize_params();
            let initialize_notification_params = None;
            let timeout = Some(INIT_TIMEOUT);
            client
                .initialize(params, initialize_notification_params, timeout)
                .await
                .map(|_| ())
        };

        cell.get_or_try_init(|| initialize).await.map(|_| ())
    }

    /// Spawn a background refresh that initializes servers and loads tools.
    /// This does not block the caller and is safe to call multiple times.
    pub fn refresh_tools_in_background(&self) {
        let tools_ref = self.tools.clone();
        let tx = self.tool_count_tx.clone();
        let clients_snapshot: HashMap<_, _> = self
            .clients
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let init_cells_snapshot: HashMap<_, _> = self
            .init_cells
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        tokio::spawn(async move {
            // Initialize each server once
            for (server, client) in &clients_snapshot {
                if let Some(cell) = init_cells_snapshot.get(server) {
                    let params = Self::build_initialize_params();
                    let initialize_notification_params = None;
                    let timeout = Some(INIT_TIMEOUT);
                    let client_clone = client.clone();
                    let _ = cell
                        .get_or_try_init(|| async move {
                            client_clone
                                .initialize(params, initialize_notification_params, timeout)
                                .await
                                .map(|_| ())
                        })
                        .await;
                }
            }

            // Load tools after init
            match list_all_tools(&clients_snapshot).await {
                Ok(all_tools) => {
                    let qualified = qualify_tools(all_tools);
                    if let Ok(mut guard) = tools_ref.write() {
                        *guard = qualified;
                    }
                    let _ = tx.send(tools_ref.read().map(|m| m.len()).unwrap_or(0));
                }
                Err(e) => warn!("failed to list MCP tools in background: {e:#}"),
            }
        });
    }

    /// Optionally await until tools are available, bounded by a timeout.
    pub async fn wait_for_tools_with_timeout(&self, timeout: Duration) {
        let mut rx = self.tool_count_tx.subscribe();
        if *rx.borrow() > 0 {
            return;
        }
        let _ = tokio::time::timeout(timeout, async {
            while rx.changed().await.is_ok() {
                if *rx.borrow() > 0 {
                    break;
                }
            }
        })
        .await;
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

    #[test]
    fn test_is_valid_server_name() {
        assert!(is_valid_mcp_server_name("valid-Server_01"));
        assert!(!is_valid_mcp_server_name("invalid server"));
        assert!(!is_valid_mcp_server_name("invalid/server"));
        assert!(!is_valid_mcp_server_name(""));
    }

    #[test]
    fn test_wait_for_tools_returns_immediately_when_ready() {
        // Arrange: manager with one tool already present and signal sent
        let mgr = McpConnectionManager::default();

        {
            let mut guard = mgr.tools.write().expect("lock tools for write");
            guard.insert("srv__tool".to_string(), create_test_tool("srv", "tool"));
        }
        let _ = mgr
            .tool_count_tx
            .send(mgr.tools.read().map(|m| m.len()).unwrap_or(0));

        // Act: should return without waiting
        let rt = tokio::runtime::Runtime::new().expect("create tokio runtime");
        rt.block_on(async {
            mgr.wait_for_tools_with_timeout(Duration::from_millis(1))
                .await;
        });
    }

    #[test]
    fn test_parse_tool_name_reads_from_cache() {
        let mgr = McpConnectionManager::default();
        {
            let mut guard = mgr.tools.write().expect("lock tools for write");
            guard.insert(
                "serverA__alpha".to_string(),
                create_test_tool("serverA", "alpha"),
            );
        }
        let parsed = mgr.parse_tool_name("serverA__alpha");
        assert_eq!(parsed, Some(("serverA".to_string(), "alpha".to_string())));
    }

    #[test]
    fn test_list_all_tools_snapshot() {
        let mgr = McpConnectionManager::default();
        {
            let mut guard = mgr.tools.write().expect("lock tools for write");
            guard.insert("s__a".to_string(), create_test_tool("s", "a"));
            guard.insert("s__b".to_string(), create_test_tool("s", "b"));
        }
        let tools = mgr.list_all_tools();
        assert_eq!(tools.len(), 2);
        assert!(tools.contains_key("s__a"));
        assert!(tools.contains_key("s__b"));
    }
}
