//! WebSearch tool for searching the web.

use super::prompts;
use crate::context::ToolContext;
use crate::error::Result;
use crate::tool::Tool;
use async_trait::async_trait;
use cocode_protocol::ApprovalRequest;
use cocode_protocol::ConcurrencySafety;
use cocode_protocol::PermissionResult;
use cocode_protocol::ToolOutput;
use serde_json::Value;

/// Tool for performing web searches.
///
/// Stub implementation — designed for pluggable search backends
/// (DuckDuckGo, Tavily, Google, etc.).
pub struct WebSearchTool;

impl WebSearchTool {
    /// Create a new WebSearch tool.
    pub fn new() -> Self {
        Self
    }
}

impl Default for WebSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "WebSearch"
    }

    fn description(&self) -> &str {
        prompts::WEB_SEARCH_DESCRIPTION
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query to use",
                    "minLength": 2
                },
                "allowed_domains": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Only include search results from these domains"
                },
                "blocked_domains": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Never include search results from these domains"
                }
            },
            "required": ["query"]
        })
    }

    fn concurrency_safety(&self) -> ConcurrencySafety {
        ConcurrencySafety::Safe
    }

    fn is_read_only(&self) -> bool {
        false // Network access requires approval
    }

    async fn check_permission(&self, input: &Value, _ctx: &ToolContext) -> PermissionResult {
        let query = match input.get("query").and_then(|v| v.as_str()) {
            Some(q) => q,
            None => return PermissionResult::Passthrough,
        };

        PermissionResult::NeedsApproval {
            request: ApprovalRequest {
                request_id: format!("websearch-{}", query.len()),
                tool_name: self.name().to_string(),
                description: format!("Web search: {query}"),
                risks: vec![],
                allow_remember: true,
            },
        }
    }

    async fn execute(&self, input: Value, ctx: &mut ToolContext) -> Result<ToolOutput> {
        let query = input["query"].as_str().ok_or_else(|| {
            crate::error::tool_error::InvalidInputSnafu {
                message: "query must be a string",
            }
            .build()
        })?;

        if query.len() < 2 {
            return Err(crate::error::tool_error::InvalidInputSnafu {
                message: "query must be at least 2 characters",
            }
            .build());
        }

        ctx.emit_progress(format!("Searching: {query}")).await;

        // Stub implementation — search backend not yet configured
        Ok(ToolOutput::text(format!(
            "Web search for: {query}\n\n\
             [Search backend not yet configured — this is a stub response.\n\
             To enable, configure a search provider (DuckDuckGo, Tavily, etc.).]"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_context() -> ToolContext {
        ToolContext::new("call-1", "session-1", PathBuf::from("/tmp"))
    }

    #[tokio::test]
    async fn test_web_search() {
        let tool = WebSearchTool::new();
        let mut ctx = make_context();

        let input = serde_json::json!({
            "query": "rust programming language"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_web_search_too_short() {
        let tool = WebSearchTool::new();
        let mut ctx = make_context();

        let input = serde_json::json!({
            "query": "a"
        });

        let result = tool.execute(input, &mut ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_web_search_with_domains() {
        let tool = WebSearchTool::new();
        let mut ctx = make_context();

        let input = serde_json::json!({
            "query": "rust async",
            "allowed_domains": ["docs.rs", "doc.rust-lang.org"],
            "blocked_domains": ["stackoverflow.com"]
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);
    }

    #[test]
    fn test_tool_properties() {
        let tool = WebSearchTool::new();
        assert_eq!(tool.name(), "WebSearch");
        assert!(tool.is_concurrent_safe());
        assert!(!tool.is_read_only()); // Network access requires approval
    }
}
