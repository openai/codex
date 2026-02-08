//! Tool registry for managing available tools.
//!
//! This module provides [`ToolRegistry`] which manages the collection of
//! available tools, including both built-in tools and MCP tools.
//!
//! ## MCP Tool Naming Convention
//!
//! MCP tools are registered with the naming convention `mcp__<server>__<tool>` to avoid
//! name collisions between different MCP servers and built-in tools.

use crate::mcp_tool::McpToolWrapper;
use crate::tool::Tool;
use cocode_mcp_types::Tool as McpToolDef;
use cocode_rmcp_client::RmcpClient;
use hyper_sdk::ToolDefinition;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::debug;

/// Information about an MCP tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolInfo {
    /// Server name.
    pub server: String,
    /// Tool name.
    pub name: String,
    /// Tool description.
    pub description: Option<String>,
    /// Input schema.
    pub input_schema: serde_json::Value,
}

impl McpToolInfo {
    /// Get the fully qualified name (mcp__server__tool).
    pub fn qualified_name(&self) -> String {
        format!("mcp__{}_{}", self.server, self.name)
    }
}

/// Registry of available tools.
///
/// The registry manages both built-in tools (implementing the [`Tool`] trait)
/// and MCP tools (remote tools from MCP servers).
#[derive(Default)]
pub struct ToolRegistry {
    /// Built-in tools.
    tools: HashMap<String, Arc<dyn Tool>>,
    /// MCP tools, keyed by qualified name.
    mcp_tools: HashMap<String, McpToolInfo>,
    /// Tool aliases (alternative names).
    aliases: HashMap<String, String>,
}

impl ToolRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a built-in tool.
    pub fn register(&mut self, tool: impl Tool + 'static) {
        let name = tool.name().to_string();
        self.tools.insert(name, Arc::new(tool));
    }

    /// Register a tool with an alias.
    pub fn register_with_alias(&mut self, tool: impl Tool + 'static, alias: impl Into<String>) {
        let name = tool.name().to_string();
        let alias = alias.into();
        self.tools.insert(name.clone(), Arc::new(tool));
        self.aliases.insert(alias, name);
    }

    /// Register an MCP server's tools (info only, not executable).
    ///
    /// This registers the tool metadata but doesn't make them executable.
    /// For executable MCP tools, use [`Self::register_mcp_tools_executable`].
    pub fn register_mcp_server(&mut self, server_name: &str, tools: Vec<McpToolInfo>) {
        for mut tool in tools {
            tool.server = server_name.to_string();
            let qualified = tool.qualified_name();
            self.mcp_tools.insert(qualified, tool);
        }
    }

    /// Register MCP tools as executable tools using the Tool trait.
    ///
    /// This registers MCP tools with the `mcp__<server>__<tool>` naming convention
    /// and makes them executable through the standard tool execution pipeline.
    ///
    /// # Arguments
    ///
    /// * `server_name` - Name of the MCP server
    /// * `tools` - Tool definitions from the MCP server
    /// * `client` - Shared MCP client for executing tool calls (uses `Arc<RmcpClient>`
    ///   not `Arc<Mutex<...>>` because RmcpClient has internal synchronization)
    /// * `timeout` - Timeout for tool calls
    ///
    /// # Example
    ///
    /// ```ignore
    /// let client = Arc::new(rmcp_client);
    /// registry.register_mcp_tools_executable(
    ///     "filesystem",
    ///     mcp_tools,
    ///     client,
    ///     Duration::from_secs(30),
    /// );
    /// ```
    pub fn register_mcp_tools_executable(
        &mut self,
        server_name: &str,
        tools: Vec<McpToolDef>,
        client: Arc<RmcpClient>,
        timeout: Duration,
    ) {
        for tool_def in tools {
            let tool_name = tool_def.name.clone();
            let wrapper =
                McpToolWrapper::new(server_name.to_string(), tool_def, client.clone(), timeout);
            let qualified_name = wrapper.qualified_name();

            debug!(
                server = %server_name,
                tool = %tool_name,
                qualified = %qualified_name,
                "Registering MCP tool"
            );

            // Register as executable tool
            self.tools.insert(qualified_name.clone(), Arc::new(wrapper));

            // Also keep info in mcp_tools for metadata queries
            self.mcp_tools.insert(
                qualified_name,
                McpToolInfo {
                    server: server_name.to_string(),
                    name: tool_name,
                    description: None, // Could be added from tool_def if needed
                    input_schema: serde_json::json!({}), // Simplified
                },
            );
        }
    }

    /// Unregister an MCP server's tools.
    pub fn unregister_mcp_server(&mut self, server_name: &str) {
        let prefix = format!("mcp__{server_name}_");
        self.mcp_tools.retain(|name, _| !name.starts_with(&prefix));
        self.tools.retain(|name, _| !name.starts_with(&prefix));
    }

    /// Get a tool by name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        // Check direct name first
        if let Some(tool) = self.tools.get(name) {
            return Some(tool.clone());
        }

        // Check aliases
        if let Some(real_name) = self.aliases.get(name) {
            return self.tools.get(real_name).cloned();
        }

        None
    }

    /// Get an MCP tool by name.
    pub fn get_mcp(&self, name: &str) -> Option<&McpToolInfo> {
        self.mcp_tools.get(name)
    }

    /// Check if a tool exists.
    pub fn has(&self, name: &str) -> bool {
        self.tools.contains_key(name)
            || self.aliases.contains_key(name)
            || self.mcp_tools.contains_key(name)
    }

    /// Check if a tool is an MCP tool.
    pub fn is_mcp_tool(&self, name: &str) -> bool {
        self.mcp_tools.contains_key(name)
    }

    /// Get tool definitions filtered by feature flags.
    ///
    /// Tools whose `feature_gate()` returns a disabled Feature are excluded.
    /// MCP tools (no feature gate) are always included.
    pub fn definitions_filtered(
        &self,
        features: &cocode_protocol::Features,
    ) -> Vec<ToolDefinition> {
        let mut definitions = Vec::new();

        // Built-in tools — skip those gated on a disabled feature
        for tool in self.tools.values() {
            if let Some(feature) = tool.feature_gate() {
                if !features.enabled(feature) {
                    continue;
                }
            }
            definitions.push(tool.to_definition());
        }

        // MCP tools — always included (no feature gate)
        for mcp_tool in self.mcp_tools.values() {
            definitions.push(ToolDefinition::full(
                mcp_tool.qualified_name(),
                mcp_tool
                    .description
                    .clone()
                    .unwrap_or_else(|| format!("MCP tool from {}", mcp_tool.server)),
                mcp_tool.input_schema.clone(),
            ));
        }

        definitions
    }

    /// Get all tool definitions for API requests.
    pub fn all_definitions(&self) -> Vec<ToolDefinition> {
        let mut definitions = Vec::new();

        // Built-in tools
        for tool in self.tools.values() {
            definitions.push(tool.to_definition());
        }

        // MCP tools
        for mcp_tool in self.mcp_tools.values() {
            definitions.push(ToolDefinition::full(
                mcp_tool.qualified_name(),
                mcp_tool
                    .description
                    .clone()
                    .unwrap_or_else(|| format!("MCP tool from {}", mcp_tool.server)),
                mcp_tool.input_schema.clone(),
            ));
        }

        definitions
    }

    /// Get definitions for specific tools.
    pub fn definitions_for(&self, names: &[&str]) -> Vec<ToolDefinition> {
        names
            .iter()
            .filter_map(|name| {
                if let Some(tool) = self.get(name) {
                    Some(tool.to_definition())
                } else if let Some(mcp) = self.get_mcp(name) {
                    Some(ToolDefinition::full(
                        mcp.qualified_name(),
                        mcp.description.clone().unwrap_or_default(),
                        mcp.input_schema.clone(),
                    ))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get the number of registered tools.
    pub fn len(&self) -> usize {
        self.tools.len() + self.mcp_tools.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty() && self.mcp_tools.is_empty()
    }

    /// Get all tool names.
    pub fn tool_names(&self) -> Vec<String> {
        let mut names: Vec<_> = self.tools.keys().cloned().collect();
        names.extend(self.mcp_tools.keys().cloned());
        names.sort();
        names
    }

    /// Get names of built-in tools only.
    pub fn builtin_names(&self) -> Vec<String> {
        let mut names: Vec<_> = self.tools.keys().cloned().collect();
        names.sort();
        names
    }

    /// Get names of MCP tools only.
    pub fn mcp_names(&self) -> Vec<String> {
        let mut names: Vec<_> = self.mcp_tools.keys().cloned().collect();
        names.sort();
        names
    }

    /// Clear all tools.
    pub fn clear(&mut self) {
        self.tools.clear();
        self.mcp_tools.clear();
        self.aliases.clear();
    }

    /// Calculate total chars of MCP tool descriptions.
    ///
    /// This accounts for the qualified name, description, and serialized input schema
    /// of each MCP tool. Used to determine if auto-search mode should be enabled.
    pub fn mcp_description_chars(&self) -> i32 {
        self.mcp_tools
            .values()
            .map(|t| {
                let name_len = t.qualified_name().len();
                let desc_len = t.description.as_deref().map(|d| d.len()).unwrap_or(0);
                let schema_len = serde_json::to_string(&t.input_schema)
                    .map(|s| s.len())
                    .unwrap_or(0);
                (name_len + desc_len + schema_len) as i32
            })
            .sum()
    }

    /// Check if MCP tool definitions should be deferred to auto-search mode.
    ///
    /// Returns `true` if the total MCP tool description size exceeds the threshold
    /// configured in `McpAutoSearchConfig` for the given context window.
    pub fn should_enable_auto_search(
        &self,
        context_window: i32,
        config: &cocode_protocol::McpAutoSearchConfig,
    ) -> bool {
        config.should_use_auto_search(
            context_window,
            self.mcp_description_chars(),
            true, // has_tool_calling
        )
    }

    /// Get a snapshot of all MCP tool metadata for use by MCPSearch.
    ///
    /// Returns a cloned vector of all MCP tool info entries. This snapshot
    /// is passed to the `McpSearchTool` for keyword-based discovery.
    pub fn mcp_tool_snapshot(&self) -> Vec<McpToolInfo> {
        self.mcp_tools.values().cloned().collect()
    }

    /// Remove MCP tool definitions from the active set (keep metadata for search).
    ///
    /// When auto-search mode is enabled, MCP tools are removed from the
    /// executable tools map so they are not sent as tool definitions in API
    /// requests. The metadata is kept in `mcp_tools` for search.
    ///
    /// Returns the qualified names of the removed tools.
    pub fn defer_mcp_tool_definitions(&mut self) -> Vec<String> {
        let mcp_tool_names: Vec<String> = self.mcp_tools.keys().cloned().collect();

        // Remove from executable tools map (keep in mcp_tools for metadata)
        for name in &mcp_tool_names {
            self.tools.remove(name);
        }

        mcp_tool_names
    }
}

impl std::fmt::Debug for ToolRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolRegistry")
            .field("builtin_tools", &self.tools.keys().collect::<Vec<_>>())
            .field("mcp_tools", &self.mcp_tools.keys().collect::<Vec<_>>())
            .field("aliases", &self.aliases)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::ToolContext;
    use crate::error::Result;
    use async_trait::async_trait;
    use cocode_protocol::ToolOutput;

    struct TestTool {
        name: String,
    }

    #[async_trait]
    impl Tool for TestTool {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            "Test tool"
        }

        fn input_schema(&self) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "properties": {}
            })
        }

        async fn execute(
            &self,
            _input: serde_json::Value,
            _ctx: &mut ToolContext,
        ) -> Result<ToolOutput> {
            Ok(ToolOutput {
                content: cocode_protocol::ToolResultContent::Text("ok".to_string()),
                is_error: false,
                modifiers: Vec::new(),
            })
        }
    }

    struct GatedTool {
        name: String,
        gate: cocode_protocol::Feature,
    }

    #[async_trait]
    impl Tool for GatedTool {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            "Gated tool"
        }

        fn input_schema(&self) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "properties": {}
            })
        }

        fn feature_gate(&self) -> Option<cocode_protocol::Feature> {
            Some(self.gate)
        }

        async fn execute(
            &self,
            _input: serde_json::Value,
            _ctx: &mut ToolContext,
        ) -> Result<ToolOutput> {
            Ok(ToolOutput {
                content: cocode_protocol::ToolResultContent::Text("ok".to_string()),
                is_error: false,
                modifiers: Vec::new(),
            })
        }
    }

    #[test]
    fn test_register_and_get() {
        let mut registry = ToolRegistry::new();
        registry.register(TestTool {
            name: "test".to_string(),
        });

        assert!(registry.has("test"));
        assert!(registry.get("test").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_alias() {
        let mut registry = ToolRegistry::new();
        registry.register_with_alias(
            TestTool {
                name: "read_file".to_string(),
            },
            "Read",
        );

        assert!(registry.has("read_file"));
        assert!(registry.has("Read"));
        assert!(registry.get("Read").is_some());
    }

    #[test]
    fn test_mcp_tools() {
        let mut registry = ToolRegistry::new();

        let tools = vec![
            McpToolInfo {
                server: "".to_string(),
                name: "tool1".to_string(),
                description: Some("Tool 1".to_string()),
                input_schema: serde_json::json!({}),
            },
            McpToolInfo {
                server: "".to_string(),
                name: "tool2".to_string(),
                description: None,
                input_schema: serde_json::json!({}),
            },
        ];

        registry.register_mcp_server("myserver", tools);

        assert!(registry.is_mcp_tool("mcp__myserver_tool1"));
        assert!(registry.is_mcp_tool("mcp__myserver_tool2"));
        assert!(!registry.is_mcp_tool("tool1"));

        // Unregister
        registry.unregister_mcp_server("myserver");
        assert!(!registry.is_mcp_tool("mcp__myserver_tool1"));
    }

    #[test]
    fn test_all_definitions() {
        let mut registry = ToolRegistry::new();
        registry.register(TestTool {
            name: "tool1".to_string(),
        });
        registry.register(TestTool {
            name: "tool2".to_string(),
        });

        let defs = registry.all_definitions();
        assert_eq!(defs.len(), 2);
    }

    #[test]
    fn test_tool_names() {
        let mut registry = ToolRegistry::new();
        registry.register(TestTool {
            name: "beta".to_string(),
        });
        registry.register(TestTool {
            name: "alpha".to_string(),
        });

        let names = registry.tool_names();
        assert_eq!(names, vec!["alpha", "beta"]);
    }

    #[test]
    fn test_mcp_description_chars() {
        let mut registry = ToolRegistry::new();

        // Empty registry should return 0
        assert_eq!(registry.mcp_description_chars(), 0);

        let tools = vec![McpToolInfo {
            server: "".to_string(),
            name: "tool1".to_string(),
            description: Some("A test tool".to_string()),
            input_schema: serde_json::json!({"type": "object"}),
        }];
        registry.register_mcp_server("srv", tools);

        let chars = registry.mcp_description_chars();
        assert!(chars > 0);
    }

    #[test]
    fn test_should_enable_auto_search() {
        let mut registry = ToolRegistry::new();
        let config = cocode_protocol::McpAutoSearchConfig::default();

        // No MCP tools => should not enable
        assert!(!registry.should_enable_auto_search(200_000, &config));

        // Add many MCP tools with large descriptions to exceed threshold
        // Threshold for 200k context: 0.1 * 200000 * 2.5 = 50000 chars
        let large_desc = "x".repeat(5000);
        let tools: Vec<McpToolInfo> = (0..15)
            .map(|i| McpToolInfo {
                server: "".to_string(),
                name: format!("tool_{i}"),
                description: Some(large_desc.clone()),
                input_schema: serde_json::json!({"type": "object", "properties": {}}),
            })
            .collect();
        registry.register_mcp_server("big_server", tools);

        // Should exceed 50k chars threshold
        assert!(registry.mcp_description_chars() >= 50000);
        assert!(registry.should_enable_auto_search(200_000, &config));
    }

    #[test]
    fn test_mcp_tool_snapshot() {
        let mut registry = ToolRegistry::new();

        let tools = vec![
            McpToolInfo {
                server: "".to_string(),
                name: "tool_a".to_string(),
                description: Some("Tool A".to_string()),
                input_schema: serde_json::json!({}),
            },
            McpToolInfo {
                server: "".to_string(),
                name: "tool_b".to_string(),
                description: Some("Tool B".to_string()),
                input_schema: serde_json::json!({}),
            },
        ];
        registry.register_mcp_server("srv", tools);

        let snapshot = registry.mcp_tool_snapshot();
        assert_eq!(snapshot.len(), 2);
        // All entries should have server set to "srv"
        for info in &snapshot {
            assert_eq!(info.server, "srv");
        }
    }

    #[test]
    fn test_defer_mcp_tool_definitions() {
        let mut registry = ToolRegistry::new();

        // Register a builtin tool
        registry.register(TestTool {
            name: "builtin".to_string(),
        });

        // Register MCP tools (info only)
        let tools = vec![McpToolInfo {
            server: "".to_string(),
            name: "mcp_tool".to_string(),
            description: Some("An MCP tool".to_string()),
            input_schema: serde_json::json!({}),
        }];
        registry.register_mcp_server("srv", tools);

        // Also put a matching entry in the tools map to simulate executable registration
        registry.register(TestTool {
            name: "mcp__srv_mcp_tool".to_string(),
        });

        assert!(registry.get("mcp__srv_mcp_tool").is_some());

        let deferred = registry.defer_mcp_tool_definitions();
        assert!(deferred.contains(&"mcp__srv_mcp_tool".to_string()));

        // Tool should be removed from executable set
        assert!(registry.get("mcp__srv_mcp_tool").is_none());

        // But metadata should still be available
        assert!(registry.is_mcp_tool("mcp__srv_mcp_tool"));

        // Builtin tool should not be affected
        assert!(registry.get("builtin").is_some());
    }

    #[test]
    fn test_definitions_filtered_excludes_disabled_gate() {
        let mut registry = ToolRegistry::new();
        registry.register(TestTool {
            name: "always_on".to_string(),
        });
        registry.register(GatedTool {
            name: "ls_tool".to_string(),
            gate: cocode_protocol::Feature::Ls,
        });

        // Ls disabled → gated tool excluded
        let mut features = cocode_protocol::Features::with_defaults();
        features.disable(cocode_protocol::Feature::Ls);
        let defs = registry.definitions_filtered(&features);
        assert!(defs.iter().any(|d| d.name == "always_on"));
        assert!(defs.iter().all(|d| d.name != "ls_tool"));
    }

    #[test]
    fn test_definitions_filtered_includes_enabled_gate() {
        let mut registry = ToolRegistry::new();
        registry.register(TestTool {
            name: "always_on".to_string(),
        });
        registry.register(GatedTool {
            name: "ls_tool".to_string(),
            gate: cocode_protocol::Feature::Ls,
        });

        // Ls enabled → gated tool included
        let features = cocode_protocol::Features::with_defaults(); // Ls is default enabled
        let defs = registry.definitions_filtered(&features);
        assert!(defs.iter().any(|d| d.name == "always_on"));
        assert!(defs.iter().any(|d| d.name == "ls_tool"));
    }
}
