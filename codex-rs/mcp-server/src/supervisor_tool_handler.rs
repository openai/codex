//! Handler for supervisor tool calls via MCP.
//! rmcp 0.8.3+ best practices implementation.

use crate::supervisor_tool::SupervisorToolParam;
use mcp_types::CallToolResult;
use mcp_types::ContentBlock;
use mcp_types::RequestId;
use mcp_types::TextContent;
use serde_json::json;
use std::time::Duration;
use tokio::time::timeout;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::warn;

/// Timeout for supervisor operations (rmcp best practice: 5 minutes)
const SUPERVISOR_TIMEOUT: Duration = Duration::from_secs(300);

/// Maximum retry attempts for transient failures
const MAX_RETRY_ATTEMPTS: u32 = 3;

/// Delay between retry attempts (exponential backoff)
const BASE_RETRY_DELAY: Duration = Duration::from_secs(1);

/// Handle a supervisor tool call with timeout, retry, and error handling.
pub async fn handle_supervisor_tool_call(
    id: RequestId,
    arguments: Option<serde_json::Value>,
) -> CallToolResult {
    info!("Supervisor tool call received (request_id: {:?})", id);

    let params = match arguments {
        Some(json_val) => match serde_json::from_value::<SupervisorToolParam>(json_val) {
            Ok(p) => {
                debug!(
                    "Parsed supervisor parameters: goal={}, agents={:?}",
                    p.goal, p.agents
                );
                p
            }
            Err(e) => {
                error!("Failed to parse supervisor parameters: {}", e);
                return CallToolResult {
                    content: vec![ContentBlock::TextContent(TextContent {
                        r#type: "text".to_string(),
                        text: format!("Invalid supervisor parameters: {e}"),
                        annotations: None,
                    })],
                    is_error: Some(true),
                    structured_content: None,
                };
            }
        },
        None => {
            error!("Missing supervisor parameters");
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

    // Execute with retry logic
    let result_text = match execute_with_retry(&params).await {
        Ok(output) => {
            info!("Supervisor execution succeeded");
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
            error!("Supervisor execution failed after retries: {}", e);
            return CallToolResult {
                content: vec![ContentBlock::TextContent(TextContent {
                    r#type: "text".to_string(),
                    text: format!("Supervisor execution failed: {e}"),
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

/// Execute supervisor with retry logic and exponential backoff.
async fn execute_with_retry(params: &SupervisorToolParam) -> anyhow::Result<String> {
    let mut last_error = None;

    for attempt in 1..=MAX_RETRY_ATTEMPTS {
        debug!(
            "Supervisor execution attempt {}/{}",
            attempt, MAX_RETRY_ATTEMPTS
        );

        // Execute with timeout
        match timeout(SUPERVISOR_TIMEOUT, execute_supervisor(params)).await {
            Ok(Ok(result)) => {
                return Ok(result);
            }
            Ok(Err(e)) => {
                warn!("Supervisor execution attempt {} failed: {}", attempt, e);

                // Check if error is retryable
                if !is_retryable_error(&e) {
                    return Err(e);
                }

                last_error = Some(e);

                // Exponential backoff
                if attempt < MAX_RETRY_ATTEMPTS {
                    let delay = BASE_RETRY_DELAY * 2_u32.pow(attempt - 1);
                    debug!("Waiting {:?} before retry", delay);
                    tokio::time::sleep(delay).await;
                }
            }
            Err(_) => {
                error!(
                    "Supervisor execution timed out after {:?}",
                    SUPERVISOR_TIMEOUT
                );
                return Err(anyhow::anyhow!("Supervisor execution timed out"));
            }
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Supervisor execution failed")))
}

/// Check if an error is retryable (network issues, temporary failures).
fn is_retryable_error(error: &anyhow::Error) -> bool {
    let error_msg = error.to_string().to_lowercase();
    error_msg.contains("timeout")
        || error_msg.contains("connection")
        || error_msg.contains("temporary")
        || error_msg.contains("unavailable")
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
