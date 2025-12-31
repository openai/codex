//! Exit Plan Mode Tool Specification
//!
//! Tool to exit plan mode and request user approval for the plan.

use crate::client_common::tools::ResponsesApiTool;
use crate::client_common::tools::ToolSpec;
use crate::tools::names;
use crate::tools::spec::JsonSchema;
use std::collections::BTreeMap;

/// Create exit_plan_mode tool specification
///
/// This tool is called by the LLM when it has finished writing the plan
/// and is ready for user review.
pub fn create_exit_plan_mode_tool() -> ToolSpec {
    ToolSpec::Function(ResponsesApiTool {
        name: names::EXIT_PLAN_MODE.to_string(),
        description: r#"Use this tool when you are in plan mode and have finished writing your plan to the plan file and are ready for user approval.

## How This Tool Works
- You should have already written your plan to the plan file specified in the plan mode system message
- This tool does NOT take the plan content as a parameter - it will read the plan from the file you wrote
- This tool simply signals that you're done planning and ready for the user to review and approve
- The user will see the contents of your plan file when they review it

## When to Use This Tool
IMPORTANT: Only use this tool when the task requires planning the implementation steps of a task that requires writing code. For research tasks where you're gathering information, searching files, reading files or in general trying to understand the codebase - do NOT use this tool.

## Handling Ambiguity in Plans
Before using this tool, ensure your plan is clear and unambiguous. If there are multiple valid approaches or unclear requirements:
1. Use the AskUserQuestion tool to clarify with the user
2. Ask about specific implementation choices (e.g., architectural patterns, which library to use)
3. Clarify any assumptions that could affect the implementation
4. Edit your plan file to incorporate user feedback
5. Only proceed with ExitPlanMode after resolving ambiguities and updating the plan file

## Examples

1. Initial task: "Search for and understand the implementation of vim mode in the codebase" - Do not use the exit plan mode tool because you are not planning the implementation steps of a task.
2. Initial task: "Help me implement yank mode for vim" - Use the exit plan mode tool after you have finished planning the implementation steps of the task.
3. Initial task: "Add a new feature to handle user authentication" - If unsure about auth method (OAuth, JWT, etc.), use AskUserQuestion first, then use exit plan mode tool after clarifying the approach."#
            .to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties: BTreeMap::new(), // No parameters needed
            required: None,
            additional_properties: Some(false.into()),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_exit_plan_mode_tool_spec() {
        let spec = create_exit_plan_mode_tool();

        let ToolSpec::Function(tool) = spec else {
            panic!("Expected function tool spec");
        };

        assert_eq!(tool.name, names::EXIT_PLAN_MODE);
        assert!(!tool.strict);
        assert!(tool.description.contains("plan mode"));
        assert!(tool.description.contains("user approval"));
    }

    #[test]
    fn test_no_parameters() {
        let spec = create_exit_plan_mode_tool();

        let ToolSpec::Function(tool) = spec else {
            panic!("Expected function tool spec");
        };

        let JsonSchema::Object {
            properties,
            required,
            ..
        } = tool.parameters
        else {
            panic!("Expected object parameters");
        };

        // Should have no parameters
        assert!(properties.is_empty());
        assert!(required.is_none());
    }
}
