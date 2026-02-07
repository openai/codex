//! MCPSearch tool for discovering MCP tools by keyword.
//!
//! When the full MCP tool list exceeds the context budget,
//! this tool is registered instead to allow on-demand discovery.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::Mutex;

use crate::context::ToolContext;
use crate::error::ToolError;
use crate::registry::McpToolInfo;
use crate::tool::Tool;
use cocode_protocol::ConcurrencySafety;
use cocode_protocol::ToolOutput;

/// MCPSearch tool for discovering MCP tools by keyword.
///
/// When the full MCP tool list exceeds the context budget,
/// this tool is registered instead to allow on-demand discovery.
/// The LLM can call this tool to search MCP tool names and descriptions
/// by keyword, returning matching tool schemas for use.
pub struct McpSearchTool {
    /// Shared reference to available MCP tool metadata.
    mcp_tools: Arc<Mutex<Vec<McpToolInfo>>>,
}

impl McpSearchTool {
    /// Create a new MCPSearch tool with a shared reference to MCP tool metadata.
    pub fn new(mcp_tools: Arc<Mutex<Vec<McpToolInfo>>>) -> Self {
        Self { mcp_tools }
    }
}

#[async_trait]
impl Tool for McpSearchTool {
    fn name(&self) -> &str {
        "MCPSearch"
    }

    fn description(&self) -> &str {
        "Search for MCP tools by keyword when the full tool list exceeds context budget. \
         Returns matching tool names, descriptions, and input schemas."
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query to match against tool names and descriptions"
                },
                "server": {
                    "type": "string",
                    "description": "Optional server name to filter results"
                }
            },
            "required": ["query"]
        })
    }

    fn concurrency_safety(&self) -> ConcurrencySafety {
        ConcurrencySafety::Safe
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn execute(&self, input: Value, _ctx: &mut ToolContext) -> Result<ToolOutput, ToolError> {
        let query = input
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase();

        let server_filter = input
            .get("server")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let tools = self.mcp_tools.lock().await;

        let mut matches: Vec<&McpToolInfo> = tools
            .iter()
            .filter(|t| {
                // Filter by server if specified
                if let Some(ref server) = server_filter {
                    if &t.server != server {
                        return false;
                    }
                }

                // Match against name and description
                let name_match = t.name.to_lowercase().contains(&query)
                    || t.qualified_name().to_lowercase().contains(&query);
                let desc_match = t
                    .description
                    .as_deref()
                    .map(|d| d.to_lowercase().contains(&query))
                    .unwrap_or(false);

                name_match || desc_match
            })
            .collect();

        // Sort by relevance: name matches first, then description matches
        matches.sort_by(|a, b| {
            let a_name_match = a.name.to_lowercase().contains(&query);
            let b_name_match = b.name.to_lowercase().contains(&query);
            b_name_match.cmp(&a_name_match)
        });

        if matches.is_empty() {
            return Ok(ToolOutput::text(format!(
                "No MCP tools found matching query: \"{query}\". Try a different search term.",
            )));
        }

        let mut output = format!(
            "Found {} MCP tool(s) matching \"{query}\":\n\n",
            matches.len()
        );
        for tool in &matches {
            output.push_str(&format!("## {}\n", tool.qualified_name()));
            output.push_str(&format!("Server: {}\n", tool.server));
            if let Some(desc) = &tool.description {
                output.push_str(&format!("Description: {desc}\n"));
            }
            output.push_str(&format!(
                "Schema: {}\n\n",
                serde_json::to_string_pretty(&tool.input_schema).unwrap_or_default()
            ));
        }

        Ok(ToolOutput::text(output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tool_info(server: &str, name: &str, desc: &str) -> McpToolInfo {
        McpToolInfo {
            server: server.to_string(),
            name: name.to_string(),
            description: Some(desc.to_string()),
            input_schema: serde_json::json!({"type": "object"}),
        }
    }

    /// Helper to extract text content from a ToolOutput.
    fn extract_text(output: &ToolOutput) -> &str {
        match &output.content {
            cocode_protocol::ToolResultContent::Text(t) => t,
            _ => panic!("Expected text content"),
        }
    }

    #[tokio::test]
    async fn test_search_by_name() {
        let tools = Arc::new(Mutex::new(vec![
            make_tool_info("github", "list_repos", "List GitHub repositories"),
            make_tool_info("github", "create_issue", "Create a GitHub issue"),
            make_tool_info("slack", "send_message", "Send a Slack message"),
        ]));
        let tool = McpSearchTool::new(tools);
        let mut ctx = ToolContext::new("test", "session", std::path::PathBuf::from("."));

        let result = tool
            .execute(serde_json::json!({"query": "repo"}), &mut ctx)
            .await
            .unwrap();
        let text = extract_text(&result);
        assert!(text.contains("list_repos"));
        assert!(!text.contains("send_message"));
    }

    #[tokio::test]
    async fn test_search_by_description() {
        let tools = Arc::new(Mutex::new(vec![
            make_tool_info("github", "list_repos", "List GitHub repositories"),
            make_tool_info("slack", "send_message", "Send a Slack message"),
        ]));
        let tool = McpSearchTool::new(tools);
        let mut ctx = ToolContext::new("test", "session", std::path::PathBuf::from("."));

        let result = tool
            .execute(serde_json::json!({"query": "slack"}), &mut ctx)
            .await
            .unwrap();
        let text = extract_text(&result);
        assert!(text.contains("send_message"));
    }

    #[tokio::test]
    async fn test_search_with_server_filter() {
        let tools = Arc::new(Mutex::new(vec![
            make_tool_info("github", "list_repos", "List repos"),
            make_tool_info("gitlab", "list_repos", "List repos"),
        ]));
        let tool = McpSearchTool::new(tools);
        let mut ctx = ToolContext::new("test", "session", std::path::PathBuf::from("."));

        let result = tool
            .execute(
                serde_json::json!({"query": "repo", "server": "github"}),
                &mut ctx,
            )
            .await
            .unwrap();
        let text = extract_text(&result);
        assert!(text.contains("github"));
        assert!(!text.contains("gitlab"));
    }

    #[tokio::test]
    async fn test_search_no_results() {
        let tools = Arc::new(Mutex::new(vec![make_tool_info(
            "github",
            "list_repos",
            "List repos",
        )]));
        let tool = McpSearchTool::new(tools);
        let mut ctx = ToolContext::new("test", "session", std::path::PathBuf::from("."));

        let result = tool
            .execute(serde_json::json!({"query": "nonexistent"}), &mut ctx)
            .await
            .unwrap();
        let text = extract_text(&result);
        assert!(text.contains("No MCP tools found"));
    }

    #[tokio::test]
    async fn test_search_empty_query() {
        let tools = Arc::new(Mutex::new(vec![
            make_tool_info("github", "list_repos", "List repos"),
            make_tool_info("slack", "send_message", "Send a message"),
        ]));
        let tool = McpSearchTool::new(tools);
        let mut ctx = ToolContext::new("test", "session", std::path::PathBuf::from("."));

        // Empty query matches all tools
        let result = tool
            .execute(serde_json::json!({"query": ""}), &mut ctx)
            .await
            .unwrap();
        let text = extract_text(&result);
        assert!(text.contains("list_repos"));
        assert!(text.contains("send_message"));
    }

    #[tokio::test]
    async fn test_search_case_insensitive() {
        let tools = Arc::new(Mutex::new(vec![make_tool_info(
            "github",
            "ListRepos",
            "List GitHub Repositories",
        )]));
        let tool = McpSearchTool::new(tools);
        let mut ctx = ToolContext::new("test", "session", std::path::PathBuf::from("."));

        let result = tool
            .execute(serde_json::json!({"query": "listrepos"}), &mut ctx)
            .await
            .unwrap();
        let text = extract_text(&result);
        assert!(text.contains("ListRepos"));
    }

    #[test]
    fn test_tool_metadata() {
        let tools = Arc::new(Mutex::new(Vec::new()));
        let tool = McpSearchTool::new(tools);
        assert_eq!(tool.name(), "MCPSearch");
        assert!(tool.is_read_only());
        assert!(tool.is_concurrent_safe());
    }
}
