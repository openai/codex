use crate::ToolDefinition;
use crate::parse_tool_input_schema;
use codex_protocol::dynamic_tools::DynamicToolSpec;

pub fn parse_dynamic_tool(tool: &DynamicToolSpec) -> Result<ToolDefinition, serde_json::Error> {
    Ok(ToolDefinition {
        name: tool.name.clone(),
        description: tool.description.clone(),
        input_schema: parse_tool_input_schema(&tool.input_schema)?,
        output_schema: None,
        defer_loading: tool.defer_loading,
    })
}

#[cfg(test)]
#[path = "dynamic_tool_tests.rs"]
mod tests;
