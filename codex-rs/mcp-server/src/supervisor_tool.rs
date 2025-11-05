//! MCP tool for Multi-Agent Supervisor coordination.

use mcp_types::Tool;
use mcp_types::ToolInputSchema;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;

/// Parameters for the supervisor tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupervisorToolParam {
    /// The goal to coordinate across multiple agents.
    pub goal: String,

    /// Optional list of agent types to use (e.g., ["CodeExpert", "Security"]).
    #[serde(default)]
    pub agents: Option<Vec<String>>,

    /// Coordination strategy: "sequential", "parallel", or "hybrid".
    #[serde(default)]
    pub strategy: Option<String>,

    /// Merge strategy: "concatenate", "voting", or "highest_score".
    #[serde(default)]
    pub merge_strategy: Option<String>,

    /// Output format: "text" or "json".
    #[serde(default = "default_format")]
    pub format: String,
}

fn default_format() -> String {
    "text".to_string()
}

/// Create the MCP tool definition for the supervisor.
pub fn create_supervisor_tool() -> Tool {
    Tool {
        name: "codex-supervisor".to_string(),
        title: Some("Multi-Agent Supervisor".to_string()),
        description: Some(
            "Coordinate multiple specialized AI agents to accomplish a complex goal. \
             The supervisor analyzes the goal, creates a plan, assigns tasks to appropriate \
             agents (CodeExpert, Researcher, Tester, Security, Backend, Frontend, Database, DevOps), \
             executes the plan (sequentially, in parallel, or hybrid), and aggregates results.\n\n\
             Use this when:\n\
             - Task requires multiple areas of expertise\n\
             - Parallel execution would speed up completion\n\
             - Need coordinated work across domains\n\
             - Complex architectural decisions\n\n\
             Example: 'Implement secure user authentication with tests and documentation'"
                .to_string(),
        ),
        input_schema: ToolInputSchema {
            r#type: "object".to_string(),
            properties: Some(json!({
                "goal": {
                    "type": "string",
                    "description": "The high-level goal to accomplish. Be specific and comprehensive."
                },
                "agents": {
                    "type": "array",
                    "description": "Optional: Specific agent types to use. Available: CodeExpert, Researcher, Tester, Security, Backend, Frontend, Database, DevOps",
                    "items": {
                        "type": "string"
                    }
                },
                "strategy": {
                    "type": "string",
                    "description": "Coordination strategy. Options: 'sequential' (tasks run one by one), 'parallel' (tasks run simultaneously), 'hybrid' (adaptive)",
                    "enum": ["sequential", "parallel", "hybrid"]
                },
                "merge_strategy": {
                    "type": "string",
                    "description": "How to combine results from multiple agents. Options: 'concatenate' (combine all), 'voting' (majority consensus), 'highest_score' (best result)",
                    "enum": ["concatenate", "voting", "highest_score"]
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
