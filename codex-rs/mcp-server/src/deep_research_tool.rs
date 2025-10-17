//! MCP tool for Deep Research capabilities.

use mcp_types::Tool;
use mcp_types::ToolInputSchema;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;

/// Parameters for the deep research tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepResearchToolParam {
    /// The research query.
    pub query: String,

    /// Research strategy: "comprehensive", "focused", or "exploratory".
    #[serde(default)]
    pub strategy: Option<String>,

    /// Research depth level (1-5).
    #[serde(default)]
    pub depth: Option<u32>,

    /// Maximum number of sources to gather.
    #[serde(default)]
    pub max_sources: Option<u32>,

    /// Output format: "text" or "json".
    #[serde(default = "default_format")]
    pub format: String,
}

fn default_format() -> String {
    "text".to_string()
}

/// Create the MCP tool definition for deep research.
pub fn create_deep_research_tool() -> Tool {
    Tool {
        name: "codex-deep-research".to_string(),
        title: Some("Deep Research".to_string()),
        description: Some(
            "Conduct comprehensive research on a topic before making implementation decisions. \
             Gathers context from multiple sources, analyzes for quality and bias, generates \
             structured findings with citations.\n\n\
             Research Strategies:\n\
             - 'comprehensive': Deep, multi-level research (5+ sources, 3+ levels)\n\
             - 'focused': Targeted research for specific questions (3-5 sources)\n\
             - 'exploratory': Broad survey of a topic (10+ sources, shallow depth)\n\n\
             Use this when:\n\
             - Evaluating technology choices\n\
             - Learning new frameworks/patterns\n\
             - Architectural decision-making\n\
             - Best practices research\n\
             - Security pattern investigation\n\n\
             Example: 'Best practices for Rust async error handling in production'"
                .to_string(),
        ),
        input_schema: ToolInputSchema {
            r#type: "object".to_string(),
            properties: Some(json!({
                "query": {
                    "type": "string",
                    "description": "The research query. Be specific about what you want to learn."
                },
                "strategy": {
                    "type": "string",
                    "description": "Research strategy: 'comprehensive' (deep), 'focused' (targeted), 'exploratory' (broad)",
                    "enum": ["comprehensive", "focused", "exploratory"]
                },
                "depth": {
                    "type": "integer",
                    "description": "Research depth level (1-5). Higher = more thorough but slower.",
                    "minimum": 1,
                    "maximum": 5
                },
                "max_sources": {
                    "type": "integer",
                    "description": "Maximum number of sources to gather (3-20)",
                    "minimum": 3,
                    "maximum": 20
                },
                "format": {
                    "type": "string",
                    "description": "Output format: 'text' (markdown report) or 'json' (structured data)",
                    "enum": ["text", "json"],
                    "default": "text"
                }
            })),
            required: Some(vec!["query".to_string()]),
        },
        output_schema: None,
        annotations: None,
    }
}
