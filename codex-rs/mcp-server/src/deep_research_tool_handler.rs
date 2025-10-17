//! Handler for deep research tool calls via MCP.

use crate::deep_research_tool::DeepResearchToolParam;
use codex_deep_research::DeepResearcher;
use codex_deep_research::ResearchStrategy;
use codex_deep_research::WebSearchProvider;
use codex_deep_research::types::DeepResearcherConfig;
use mcp_types::CallToolResult;
use mcp_types::ContentBlock;
use mcp_types::RequestId;
use mcp_types::TextContent;
use std::sync::Arc;

/// Handle a deep research tool call.
pub async fn handle_deep_research_tool_call(
    _id: RequestId,
    arguments: Option<serde_json::Value>,
) -> CallToolResult {
    let params = match arguments {
        Some(json_val) => match serde_json::from_value::<DeepResearchToolParam>(json_val) {
            Ok(p) => p,
            Err(e) => {
                return CallToolResult {
                    content: vec![ContentBlock::TextContent(TextContent {
                        r#type: "text".to_string(),
                        text: format!("Invalid deep-research parameters: {}", e),
                        annotations: None,
                    })],
                    is_error: Some(true),
                    structured_content: None,
                };
            }
        },
        None => {
            return CallToolResult {
                content: vec![ContentBlock::TextContent(TextContent {
                    r#type: "text".to_string(),
                    text: "Missing deep-research parameters".to_string(),
                    annotations: None,
                })],
                is_error: Some(true),
                structured_content: None,
            };
        }
    };

    // Execute deep research
    let result_text = match execute_deep_research(&params).await {
        Ok(output) => {
            if params.format == "json" {
                output
            } else {
                format!(
                    "# Deep Research Report\n\n\
                     **Query**: {}\n\n\
                     **Strategy**: {}\n\
                     **Depth**: {}\n\n\
                     {}",
                    params.query,
                    params
                        .strategy
                        .as_ref()
                        .unwrap_or(&"comprehensive".to_string()),
                    params.depth.unwrap_or(3),
                    output
                )
            }
        }
        Err(e) => {
            return CallToolResult {
                content: vec![ContentBlock::TextContent(TextContent {
                    r#type: "text".to_string(),
                    text: format!("Deep research failed: {}", e),
                    annotations: None,
                })],
                is_error: Some(true),
                structured_content: None,
            };
        }
    };

    CallToolResult {
        content: vec![ContentBlock::TextContent(TextContent {
            r#type: "text".to_string(),
            text: result_text,
            annotations: None,
        })],
        is_error: None,
        structured_content: None,
    }
}

/// Execute the deep research pipeline.
async fn execute_deep_research(params: &DeepResearchToolParam) -> anyhow::Result<String> {
    // Parse strategy from string
    let default_strategy = "comprehensive".to_string();
    let strategy_str = params
        .strategy
        .as_ref()
        .unwrap_or(&default_strategy)
        .as_str();

    let strategy = match strategy_str {
        "focused" => ResearchStrategy::Focused,
        "exploratory" => ResearchStrategy::Exploratory,
        _ => ResearchStrategy::Comprehensive,
    };

    let depth = params.depth.unwrap_or(3) as u8;
    let max_sources = params.max_sources.unwrap_or(10) as u8;

    // Create config and researcher
    let config = DeepResearcherConfig {
        max_depth: depth,
        max_sources,
        strategy: strategy.clone(),
    };

    // Use real WebSearchProvider instead of MockProvider
    let provider = Arc::new(WebSearchProvider::default());
    let researcher = DeepResearcher::new(config, provider);

    // Conduct research
    let report = researcher.research(&params.query).await?;

    if params.format == "json" {
        // Return JSON format
        Ok(serde_json::to_string_pretty(&report)?)
    } else {
        // Return markdown format
        let mut markdown = format!(
            "# Deep Research Report\n\n\
             **Query**: {}\n\n\
             **Strategy**: {:?}\n\
             **Depth Reached**: {}/{}\n\
             **Sources Found**: {}\n\n\
             ## Summary\n\n\
             {}\n\n",
            report.query,
            report.strategy,
            report.depth_reached,
            depth,
            report.sources.len(),
            report.summary
        );

        if !report.sources.is_empty() {
            markdown.push_str("## Sources\n\n");
            for (i, source) in report.sources.iter().enumerate() {
                markdown.push_str(&format!(
                    "{}. **{}** (relevance: {:.2})\n   - URL: {}\n   - {}\n\n",
                    i + 1,
                    source.title,
                    source.relevance_score,
                    source.url,
                    source.snippet
                ));
            }
        }

        if !report.findings.is_empty() {
            markdown.push_str("## Key Findings\n\n");
            for (i, finding) in report.findings.iter().enumerate() {
                markdown.push_str(&format!(
                    "{}. {} (confidence: {:.0}%)\n\n",
                    i + 1,
                    finding.content,
                    finding.confidence * 100.0
                ));
            }
        }

        Ok(markdown)
    }
}
