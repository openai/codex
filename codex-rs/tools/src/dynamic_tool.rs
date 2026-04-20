use crate::ToolDefinition;
use crate::ToolExecution;
use crate::ToolLoadingPolicy;
use crate::ToolName;
use crate::parse_tool_input_schema;
use codex_protocol::dynamic_tools::DynamicToolSpec;

pub fn dynamic_tool_to_tool_definition(
    tool: &DynamicToolSpec,
) -> Result<ToolDefinition, serde_json::Error> {
    let DynamicToolSpec {
        name,
        description,
        input_schema,
        defer_loading,
    } = tool;
    Ok(ToolDefinition {
        name: ToolName::plain(name.clone()),
        description: description.clone(),
        input_schema: parse_tool_input_schema(input_schema)?,
        output_schema: None,
        loading: if *defer_loading {
            ToolLoadingPolicy::Deferred
        } else {
            ToolLoadingPolicy::Eager
        },
        execution: ToolExecution::Dynamic,
        presentation: None,
        search: None,
        supports_parallel_tool_calls: false,
    })
}

// TODO(tool-definition-unification): migrate remaining callers to
// `dynamic_tool_to_tool_definition` and remove this compatibility wrapper.
pub fn parse_dynamic_tool(tool: &DynamicToolSpec) -> Result<ToolDefinition, serde_json::Error> {
    dynamic_tool_to_tool_definition(tool)
}

#[cfg(test)]
#[path = "dynamic_tool_tests.rs"]
mod tests;
