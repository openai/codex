use crate::JsonSchema;
use crate::ToolDefinition;
use codex_protocol::dynamic_tools::DynamicToolSpec;

pub fn parse_dynamic_tool(tool: &DynamicToolSpec) -> Result<ToolDefinition, serde_json::Error> {
    Ok(ToolDefinition {
        name: tool.name.clone(),
        description: tool.description.clone(),
        input_schema: JsonSchema::from_raw_tool_input_schema(tool.input_schema.clone()),
        output_schema: None,
        defer_loading: tool.defer_loading,
    })
}

#[cfg(test)]
#[path = "dynamic_tool_tests.rs"]
mod tests;
