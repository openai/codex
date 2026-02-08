//! WebFetch tool for fetching and processing web content.

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

/// Maximum result size for web content (characters).
const MAX_RESULT_CHARS: i32 = 100_000;

/// Tool for fetching content from a URL.
///
/// Fetches the URL, converts HTML to markdown, and processes
/// the content with an optional prompt.
pub struct WebFetchTool;

impl WebFetchTool {
    /// Create a new WebFetch tool.
    pub fn new() -> Self {
        Self
    }
}

impl Default for WebFetchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "WebFetch"
    }

    fn description(&self) -> &str {
        prompts::WEB_FETCH_DESCRIPTION
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "format": "uri",
                    "description": "The URL to fetch content from"
                },
                "prompt": {
                    "type": "string",
                    "description": "The prompt to run on the fetched content"
                }
            },
            "required": ["url", "prompt"]
        })
    }

    fn concurrency_safety(&self) -> ConcurrencySafety {
        ConcurrencySafety::Safe
    }

    fn is_read_only(&self) -> bool {
        false // Network access requires approval
    }

    fn max_result_size_chars(&self) -> i32 {
        MAX_RESULT_CHARS
    }

    async fn check_permission(&self, input: &Value, _ctx: &ToolContext) -> PermissionResult {
        let url = match input.get("url").and_then(|v| v.as_str()) {
            Some(u) => u,
            None => return PermissionResult::Passthrough,
        };

        // Extract hostname from URL
        let hostname = url
            .strip_prefix("https://")
            .or_else(|| url.strip_prefix("http://"))
            .and_then(|rest| rest.split('/').next())
            .unwrap_or(url);

        // Preapproved hosts that don't need permission
        const PREAPPROVED_HOSTS: &[&str] = &[
            "docs.rs",
            "crates.io",
            "doc.rust-lang.org",
            "docs.python.org",
            "developer.mozilla.org",
            "en.wikipedia.org",
            "stackoverflow.com",
            "github.com",
            "raw.githubusercontent.com",
        ];

        if PREAPPROVED_HOSTS
            .iter()
            .any(|h| hostname == *h || hostname.ends_with(&format!(".{h}")))
        {
            return PermissionResult::Allowed;
        }

        // All other domains → NeedsApproval
        PermissionResult::NeedsApproval {
            request: ApprovalRequest {
                request_id: format!("webfetch-{hostname}"),
                tool_name: self.name().to_string(),
                description: format!("Fetch URL: {url}"),
                risks: vec![],
                allow_remember: true,
                proposed_prefix_pattern: None,
            },
        }
    }

    async fn execute(&self, input: Value, ctx: &mut ToolContext) -> Result<ToolOutput> {
        let url = input["url"].as_str().ok_or_else(|| {
            crate::error::tool_error::InvalidInputSnafu {
                message: "url must be a string",
            }
            .build()
        })?;
        let prompt = input["prompt"].as_str().ok_or_else(|| {
            crate::error::tool_error::InvalidInputSnafu {
                message: "prompt must be a string",
            }
            .build()
        })?;

        // Validate URL
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(crate::error::tool_error::InvalidInputSnafu {
                message: "url must start with http:// or https://",
            }
            .build());
        }

        ctx.emit_progress(format!("Fetching {url}")).await;

        // Stub implementation — actual HTTP fetch + HTML→markdown conversion
        // will be connected when reqwest/htmd dependencies are wired up.
        Ok(ToolOutput::text(format!(
            "WebFetch for URL: {url}\nPrompt: {prompt}\n\n\
             [HTTP client not yet connected — this is a stub response.\n\
             To enable, wire up reqwest for HTTP and a markdown converter for HTML.]"
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
    async fn test_web_fetch() {
        let tool = WebFetchTool::new();
        let mut ctx = make_context();

        let input = serde_json::json!({
            "url": "https://example.com",
            "prompt": "Extract the title"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_web_fetch_invalid_url() {
        let tool = WebFetchTool::new();
        let mut ctx = make_context();

        let input = serde_json::json!({
            "url": "not-a-url",
            "prompt": "Extract the title"
        });

        let result = tool.execute(input, &mut ctx).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_tool_properties() {
        let tool = WebFetchTool::new();
        assert_eq!(tool.name(), "WebFetch");
        assert!(tool.is_concurrent_safe());
        assert!(!tool.is_read_only()); // Network access requires approval
        assert_eq!(tool.max_result_size_chars(), 100_000);
    }
}
