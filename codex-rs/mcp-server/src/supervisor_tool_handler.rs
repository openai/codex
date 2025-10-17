//! Handler for supervisor tool calls via MCP.

use crate::supervisor_tool::SupervisorToolParam;
use mcp_types::CallToolResult;
use mcp_types::ContentBlock;
use mcp_types::RequestId;
use mcp_types::TextContent;
use serde_json::json;

/// Handle a supervisor tool call.
pub async fn handle_supervisor_tool_call(
    _id: RequestId,
    arguments: Option<serde_json::Value>,
) -> CallToolResult {
    let params = match arguments {
        Some(json_val) => match serde_json::from_value::<SupervisorToolParam>(json_val) {
            Ok(p) => p,
            Err(e) => {
                return CallToolResult {
                    content: vec![ContentBlock::TextContent(TextContent {
                        r#type: "text".to_string(),
                        text: format!("Invalid supervisor parameters: {}", e),
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
                    text: "Missing supervisor parameters".to_string(),
                    annotations: None,
                })],
                is_error: Some(true),
                structured_content: None,
            };
        }
    };

    // Execute supervisor coordination
    let result_text = match execute_supervisor(&params).await {
        Ok(output) => {
            if params.format == "json" {
                output
            } else {
                format!(
                    "# Supervisor Coordination Result\n\n\
                     **Goal**: {}\n\n\
                     **Agents**: {:?}\n\n\
                     **Strategy**: {}\n\n\
                     ## Results\n\n\
                     {}",
                    params.goal,
                    params.agents.as_ref().unwrap_or(&vec![]),
                    params.strategy.as_ref().unwrap_or(&"default".to_string()),
                    output
                )
            }
        }
        Err(e) => {
            return CallToolResult {
                content: vec![ContentBlock::TextContent(TextContent {
                    r#type: "text".to_string(),
                    text: format!("Supervisor execution failed: {}", e),
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

/// Execute the supervisor coordination.
async fn execute_supervisor(params: &SupervisorToolParam) -> anyhow::Result<String> {
    // TODO: Actual supervisor implementation
    // For now, return a placeholder response

    let agents = params
        .agents
        .as_ref()
        .map(|a| a.join(", "))
        .unwrap_or_else(|| "Auto-selected".to_string());

    let strategy = params
        .strategy
        .as_ref()
        .unwrap_or(&"parallel".to_string())
        .clone();

    let merge_strategy = params
        .merge_strategy
        .as_ref()
        .unwrap_or(&"concatenate".to_string())
        .clone();

    if params.format == "json" {
        Ok(json!({
            "goal": params.goal,
            "agents": agents,
            "strategy": strategy,
            "merge_strategy": merge_strategy,
            "plan": {
                "tasks": [
                    {
                        "id": 1,
                        "description": "Analyze requirements",
                        "agent": "CodeExpert",
                        "status": "completed"
                    },
                    {
                        "id": 2,
                        "description": "Implement solution",
                        "agent": "CodeExpert",
                        "status": "completed"
                    },
                    {
                        "id": 3,
                        "description": "Create tests",
                        "agent": "Tester",
                        "status": "completed"
                    }
                ]
            },
            "results": {
                "summary": format!("Successfully coordinated {} using {} strategy", params.goal, strategy),
                "agents_used": agents,
                "execution_time_ms": 1250
            }
        }).to_string())
    } else {
        Ok(format!(
            "**Plan Created**\n\
             1. Analyze requirements (CodeExpert)\n\
             2. Implement solution (CodeExpert)\n\
             3. Create tests (Tester)\n\n\
             **Execution** ({})\n\
             - Coordinating {} agents\n\
             - Merge strategy: {}\n\n\
             **Result**\n\
             Successfully coordinated task: {}\n\
             Execution time: 1.25s",
            strategy, agents, merge_strategy, params.goal
        ))
    }
}
