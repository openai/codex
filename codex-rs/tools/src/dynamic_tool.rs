use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_tool_api::FunctionToolSpec;
use codex_tool_api::ToolDefinition;
use codex_tool_api::ToolName;

pub fn parse_dynamic_tool(tool: &DynamicToolSpec) -> ToolDefinition<()> {
    let definition = ToolDefinition::new(
        ToolName::new(tool.namespace.clone(), tool.name.clone()),
        FunctionToolSpec {
            name: tool.name.clone(),
            description: tool.description.clone(),
            strict: false,
            parameters: tool.input_schema.clone(),
        },
        (),
    );

    if tool.defer_loading {
        definition.deferred()
    } else {
        definition
    }
}

#[cfg(test)]
#[path = "dynamic_tool_tests.rs"]
mod tests;
