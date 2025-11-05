//! MCP tool for automatic task orchestration.
//!
//! Automatically analyzes task complexity and orchestrates sub-agents if needed.

use mcp_types::Tool;
use mcp_types::ToolInputSchema;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;

/// Parameters for the auto-orchestrator tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoOrchestratorToolParam {
    /// The goal to analyze and potentially orchestrate
    pub goal: String,

    /// Complexity threshold (0.0-1.0) for triggering orchestration
    #[serde(default = "default_threshold")]
    pub auto_threshold: f64,

    /// Execution strategy: "sequential", "parallel", or "hybrid"
    #[serde(default = "default_strategy")]
    pub strategy: String,

    /// Output format: "text" or "json"
    #[serde(default = "default_format")]
    pub format: String,
}

fn default_threshold() -> f64 {
    0.7
}

fn default_strategy() -> String {
    "hybrid".to_string()
}

fn default_format() -> String {
    "text".to_string()
}

/// Create the MCP tool definition for auto-orchestrator.
pub fn create_auto_orchestrator_tool() -> Tool {
    Tool {
        name: "codex-auto-orchestrate".to_string(),
        title: Some("Automatic Sub-Agent Orchestration".to_string()),
        description: Some(
            "Automatically analyze task complexity and orchestrate sub-agents if needed. \
             Uses TaskAnalyzer to determine if parallel agent execution would benefit, \
             then coordinates via Supervisor and executes agents in parallel.\n\n\
             Use this when:\n\
             - Task appears complex with multiple domains\n\
             - Multiple specialized agents would benefit the task\n\
             - User wants ClaudeCode-style transparent orchestration\n\n\
             Example: 'Implement user authentication with JWT, write tests, and security review'"
                .to_string(),
        ),
        input_schema: ToolInputSchema {
            r#type: "object".to_string(),
            properties: Some(json!({
                "goal": {
                    "type": "string",
                    "description": "The task goal to analyze and potentially orchestrate"
                },
                "auto_threshold": {
                    "type": "number",
                    "description": "Complexity threshold (0.0-1.0) for triggering orchestration. Tasks above this threshold will be orchestrated.",
                    "default": 0.7,
                    "minimum": 0.0,
                    "maximum": 1.0
                },
                "strategy": {
                    "type": "string",
                    "description": "Execution strategy: 'sequential' (one by one), 'parallel' (simultaneously), 'hybrid' (adaptive)",
                    "enum": ["sequential", "parallel", "hybrid"],
                    "default": "hybrid"
                },
                "format": {
                    "type": "string",
                    "description": "Output format: 'text' (human-readable) or 'json' (structured)",
                    "enum": ["text", "json"],
                    "default": "text"
                }
            })),
            required: Some(vec!["goal".to_string()]),
        },
        output_schema: None,
        annotations: None,
    }
}
