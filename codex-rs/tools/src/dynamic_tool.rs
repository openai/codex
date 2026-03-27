use crate::ParsedToolDefinition;
use crate::parse_tool_input_schema;
use codex_protocol::dynamic_tools::DynamicToolSpec;

pub fn parse_dynamic_tool(
    tool: &DynamicToolSpec,
) -> Result<ParsedToolDefinition, serde_json::Error> {
    let DynamicToolSpec {
        name: _,
        description,
        input_schema,
        defer_loading,
    } = tool;
    Ok(ParsedToolDefinition {
        description: description.clone(),
        input_schema: parse_tool_input_schema(input_schema)?,
        output_schema: None,
        defer_loading: *defer_loading,
    })
}

#[cfg(test)]
#[path = "dynamic_tool_tests.rs"]
mod tests;
