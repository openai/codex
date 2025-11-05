//! Handler for auto-orchestrator tool calls via MCP.

use crate::auto_orchestrator_tool::AutoOrchestratorToolParam;
use mcp_types::CallToolResult;
use mcp_types::ContentBlock;
use mcp_types::RequestId;
use mcp_types::TextContent;
use serde_json::json;

/// Handle an auto-orchestrator tool call.
pub async fn handle_auto_orchestrator_tool_call(
    _id: RequestId,
    arguments: Option<serde_json::Value>,
) -> CallToolResult {
    let params = match arguments {
        Some(json_val) => match serde_json::from_value::<AutoOrchestratorToolParam>(json_val) {
            Ok(p) => p,
            Err(e) => {
                return CallToolResult {
                    content: vec![ContentBlock::TextContent(TextContent {
                        r#type: "text".to_string(),
                        text: format!("Invalid auto-orchestrator parameters: {e}"),
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
                    text: "Missing auto-orchestrator parameters".to_string(),
                    annotations: None,
                })],
                is_error: Some(true),
                structured_content: None,
            };
        }
    };

    // Execute auto-orchestration
    let result_text = match execute_auto_orchestration(&params).await {
        Ok(output) => {
            if params.format == "json" {
                output
            } else {
                format!(
                    "# Auto-Orchestration Result\n\n\
                     **Goal**: {}\n\n\
                     **Threshold**: {}\n\n\
                     **Strategy**: {}\n\n\
                     ## Analysis & Execution\n\n\
                     {}",
                    params.goal, params.auto_threshold, params.strategy, output
                )
            }
        }
        Err(e) => {
            return CallToolResult {
                content: vec![ContentBlock::TextContent(TextContent {
                    r#type: "text".to_string(),
                    text: format!("Auto-orchestration execution failed: {e}"),
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

/// Execute the auto-orchestration logic.
async fn execute_auto_orchestration(params: &AutoOrchestratorToolParam) -> anyhow::Result<String> {
    use codex_core::orchestration::TaskAnalyzer;

    // 1. Create TaskAnalyzer and analyze the goal
    let analyzer = TaskAnalyzer::new(params.auto_threshold);
    let analysis = analyzer.analyze(&params.goal);

    let complexity = analysis.complexity_score;

    // 2. Check if complexity > threshold
    if analysis.should_orchestrate(params.auto_threshold) {
        // 3. Execute orchestration
        // Note: Full orchestration requires AgentRuntime which needs Config, Auth, etc.
        // For MCP tool context, we return the analysis and recommended plan
        // The actual execution happens in codex.rs when this is called from main agent

        if params.format == "json" {
            Ok(json!({
                "was_orchestrated": true,
                "complexity_score": complexity,
                "threshold": params.auto_threshold,
                "recommended_agents": analysis.recommended_agents,
                "subtasks": analysis.subtasks,
                "detected_keywords": analysis.detected_keywords,
                "strategy": params.strategy,
                "execution_summary": format!(
                    "Task complexity ({:.2}) exceeds threshold ({:.2}). \
                     Recommending {} specialized agents using {} strategy.",
                    complexity,
                    params.auto_threshold,
                    analysis.recommended_agents.len(),
                    params.strategy
                ),
                "task_analysis": {
                    "complexity_score": complexity,
                    "detected_keywords": analysis.detected_keywords,
                    "recommended_agents": analysis.recommended_agents,
                    "subtasks": analysis.subtasks
                }
            })
            .to_string())
        } else {
            Ok(format!(
                "**Complexity Analysis**: {:.2} (threshold: {:.2}) ✅ **Will Orchestrate**\n\n\
                 **Recommended Agents**: {}\n\n\
                 **Execution Strategy**: {}\n\n\
                 **Detected Keywords**: {}\n\n\
                 **Subtasks**:\n{}\n\n\
                 **Summary**: Task complexity exceeds threshold. \
                 Recommending {} specialized agents to handle this task.",
                complexity,
                params.auto_threshold,
                analysis.recommended_agents.join(", "),
                params.strategy,
                analysis.detected_keywords.join(", "),
                analysis
                    .subtasks
                    .iter()
                    .enumerate()
                    .map(|(i, t)| format!("{}. {}", i + 1, t))
                    .collect::<Vec<_>>()
                    .join("\n"),
                analysis.recommended_agents.len()
            ))
        }
    } else {
        // Would not be orchestrated
        if params.format == "json" {
            Ok(json!({
                "was_orchestrated": false,
                "complexity_score": complexity,
                "threshold": params.auto_threshold,
                "detected_keywords": analysis.detected_keywords,
                "execution_summary": format!(
                    "Task complexity ({:.2}) below threshold ({:.2}). Using normal execution.",
                    complexity,
                    params.auto_threshold
                )
            })
            .to_string())
        } else {
            Ok(format!(
                "**Complexity Analysis**: {:.2} (threshold: {:.2}) ❌ **Normal Execution**\n\n\
                 **Detected Keywords**: {}\n\n\
                 **Summary**: Task complexity is below threshold. \
                 Will use standard single-agent execution.",
                complexity,
                params.auto_threshold,
                analysis.detected_keywords.join(", ")
            ))
        }
    }
}
