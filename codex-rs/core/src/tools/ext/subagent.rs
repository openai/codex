//! Subagent Tool Specifications
//!
//! Task tool for spawning subagents and TaskOutput for retrieving background results.

use crate::client_common::tools::ResponsesApiTool;
use crate::client_common::tools::ToolSpec;
use crate::subagent::AgentDefinition;
use crate::subagent::ToolAccess;
use crate::tools::spec::JsonSchema;
use std::collections::BTreeMap;

/// Create Task tool specification
///
/// Task launches specialized subagents for complex, multi-step tasks.
/// Supports Explore, Plan, and custom agent types.
#[allow(dead_code)] // Public API for tool spec creation
pub fn create_task_tool() -> ToolSpec {
    let mut properties = BTreeMap::new();

    properties.insert(
        "subagent_type".to_string(),
        JsonSchema::String {
            description: Some(
                "The type of subagent to spawn (e.g., 'Explore', 'Plan')".to_string(),
            ),
        },
    );

    properties.insert(
        "prompt".to_string(),
        JsonSchema::String {
            description: Some("The task/prompt for the subagent to execute".to_string()),
        },
    );

    properties.insert(
        "description".to_string(),
        JsonSchema::String {
            description: Some("A short (3-5 word) description of the task".to_string()),
        },
    );

    properties.insert(
        "model".to_string(),
        JsonSchema::String {
            description: Some(
                "Optional model override: 'sonnet', 'opus', 'haiku', or 'inherit'".to_string(),
            ),
        },
    );

    properties.insert(
        "run_in_background".to_string(),
        JsonSchema::Boolean {
            description: Some("Set to true to run this agent in the background".to_string()),
        },
    );

    properties.insert(
        "resume".to_string(),
        JsonSchema::String {
            description: Some("Optional agent ID to resume from previous execution".to_string()),
        },
    );

    ToolSpec::Function(ResponsesApiTool {
        name: "Task".to_string(),
        description: r#"Launch a specialized subagent to handle complex, multi-step tasks autonomously.

Available agent types:
- Explore: Fast codebase exploration (read-only). Use for finding files, searching code, or answering questions about the codebase.
- Plan: Implementation planning (read-only). Use for designing implementation plans and architectural decisions.

Usage notes:
- Launch multiple agents concurrently when possible for efficiency
- Use run_in_background: true for long-running tasks, then retrieve results with TaskOutput
- Agents can be resumed using the 'resume' parameter with a previous agent ID
- Provide clear, detailed prompts for best results"#
            .to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec![
                "subagent_type".to_string(),
                "prompt".to_string(),
                "description".to_string(),
            ]),
            additional_properties: Some(false.into()),
        },
    })
}

/// Generate Task tool description from agent definitions.
///
/// Creates a dynamic description that includes all available agents,
/// their `when_to_use` descriptions, and available tools.
pub fn generate_task_description(agents: &[&AgentDefinition]) -> String {
    let mut desc = String::from(
        "Launch a new agent to handle complex, multi-step tasks autonomously.\n\n\
         Available agent types and the tools they have access to:\n",
    );

    for agent in agents {
        let agent_type = &agent.agent_type;
        let when_to_use = agent
            .when_to_use
            .as_deref()
            .unwrap_or("General purpose agent");
        let tools_desc = match &agent.tools {
            ToolAccess::All => "(Tools: *)".to_string(),
            ToolAccess::List(tools) => format!("(Tools: {})", tools.join(", ")),
        };

        desc.push_str(&format!("- {agent_type}: {when_to_use} {tools_desc}\n"));
    }

    desc.push_str(
        "\nWhen NOT to use the Task tool:\n\
        - If you want to read a specific file path, use the Read or Glob tool instead\n\
        - If you are searching for a specific class definition, use the Glob tool instead\n\
        - If you are searching for code within a specific file, use the Read tool instead\n\n\
        Usage notes:\n\
        - Always include a short description (3-5 words) summarizing what the agent will do\n\
        - Launch multiple agents concurrently whenever possible, to maximize performance\n\
        - Use run_in_background: true for long-running tasks, then retrieve results with TaskOutput\n\
        - Agents can be resumed using the 'resume' parameter with a previous agent ID\n\
        - Provide clear, detailed prompts so the agent can work autonomously\n",
    );

    desc
}

/// Create Task tool specification with dynamic description.
///
/// Uses the provided description (typically generated from agent definitions).
pub fn create_task_tool_with_description(description: String) -> ToolSpec {
    let mut properties = BTreeMap::new();

    properties.insert(
        "subagent_type".to_string(),
        JsonSchema::String {
            description: Some(
                "The type of subagent to spawn (e.g., 'Explore', 'Plan')".to_string(),
            ),
        },
    );

    properties.insert(
        "prompt".to_string(),
        JsonSchema::String {
            description: Some("The task/prompt for the subagent to execute".to_string()),
        },
    );

    properties.insert(
        "description".to_string(),
        JsonSchema::String {
            description: Some("A short (3-5 word) description of the task".to_string()),
        },
    );

    properties.insert(
        "model".to_string(),
        JsonSchema::String {
            description: Some(
                "Optional model override: 'sonnet', 'opus', 'haiku', or 'inherit'".to_string(),
            ),
        },
    );

    properties.insert(
        "run_in_background".to_string(),
        JsonSchema::Boolean {
            description: Some("Set to true to run this agent in the background".to_string()),
        },
    );

    properties.insert(
        "resume".to_string(),
        JsonSchema::String {
            description: Some("Optional agent ID to resume from previous execution".to_string()),
        },
    );

    ToolSpec::Function(ResponsesApiTool {
        name: "Task".to_string(),
        description,
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec![
                "subagent_type".to_string(),
                "prompt".to_string(),
                "description".to_string(),
            ]),
            additional_properties: Some(false.into()),
        },
    })
}

/// Create TaskOutput tool specification
///
/// TaskOutput retrieves results from background subagent tasks.
pub fn create_task_output_tool() -> ToolSpec {
    let mut properties = BTreeMap::new();

    properties.insert(
        "agent_id".to_string(),
        JsonSchema::String {
            description: Some("The agent ID to retrieve results for".to_string()),
        },
    );

    properties.insert(
        "block".to_string(),
        JsonSchema::Boolean {
            description: Some("Whether to wait for completion (default: true)".to_string()),
        },
    );

    properties.insert(
        "timeout".to_string(),
        JsonSchema::Number {
            description: Some("Max wait time in seconds (default: 300)".to_string()),
        },
    );

    ToolSpec::Function(ResponsesApiTool {
        name: "TaskOutput".to_string(),
        description: r#"Retrieve output from a running or completed background task.

Usage:
- Use block=true (default) to wait for task completion
- Use block=false for non-blocking check of current status
- Task IDs are returned when launching agents with run_in_background: true"#
            .to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["agent_id".to_string()]),
            additional_properties: Some(false.into()),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_task_tool_spec() {
        let spec = create_task_tool();

        let ToolSpec::Function(tool) = spec else {
            panic!("Expected function tool spec");
        };

        assert_eq!(tool.name, "Task");
        assert!(!tool.strict);
        assert!(tool.description.contains("subagent"));

        let JsonSchema::Object {
            properties,
            required,
            ..
        } = tool.parameters
        else {
            panic!("Expected object parameters");
        };

        // Check required fields
        let required = required.expect("Should have required fields");
        assert!(required.contains(&"subagent_type".to_string()));
        assert!(required.contains(&"prompt".to_string()));
        assert!(required.contains(&"description".to_string()));

        // Check properties exist
        assert!(properties.contains_key("subagent_type"));
        assert!(properties.contains_key("prompt"));
        assert!(properties.contains_key("description"));
        assert!(properties.contains_key("model"));
        assert!(properties.contains_key("run_in_background"));
        assert!(properties.contains_key("resume"));
    }

    #[test]
    fn test_create_task_output_tool_spec() {
        let spec = create_task_output_tool();

        let ToolSpec::Function(tool) = spec else {
            panic!("Expected function tool spec");
        };

        assert_eq!(tool.name, "TaskOutput");
        assert!(tool.description.contains("background task"));

        let JsonSchema::Object {
            properties,
            required,
            ..
        } = tool.parameters
        else {
            panic!("Expected object parameters");
        };

        let required = required.expect("Should have required fields");
        assert!(required.contains(&"agent_id".to_string()));
        assert!(properties.contains_key("agent_id"));
        assert!(properties.contains_key("block"));
        assert!(properties.contains_key("timeout"));
    }

    #[test]
    fn test_generate_task_description() {
        use crate::subagent::get_builtin_agents;

        let agents = get_builtin_agents();
        let desc = generate_task_description(&agents);

        // Should contain built-in agent types
        assert!(desc.contains("Explore"), "Should contain Explore agent");
        assert!(desc.contains("Plan"), "Should contain Plan agent");

        // Should contain tool names from built-in agents
        assert!(desc.contains("read_file"), "Should contain read_file tool");
        assert!(
            desc.contains("glob_files"),
            "Should contain glob_files tool"
        );

        // Should contain usage guidance
        assert!(
            desc.contains("When NOT to use"),
            "Should contain guidance on when not to use"
        );
        assert!(
            desc.contains("run_in_background"),
            "Should contain background usage note"
        );
    }

    #[test]
    fn test_create_task_tool_with_description() {
        let custom_desc = "Custom description for testing".to_string();
        let spec = create_task_tool_with_description(custom_desc.clone());

        let ToolSpec::Function(tool) = spec else {
            panic!("Expected function tool spec");
        };

        assert_eq!(tool.name, "Task");
        assert_eq!(tool.description, custom_desc);
    }
}
