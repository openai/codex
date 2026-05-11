use crate::ResponsesApiTool;
use crate::parse_tool_input_schema;
use codex_protocol::dynamic_tools::DynamicToolSpec;

pub fn parse_dynamic_tool(tool: &DynamicToolSpec) -> Result<ResponsesApiTool, serde_json::Error> {
    Ok(ResponsesApiTool {
        name: tool.name.clone(),
        description: tool.description.clone(),
        strict: false,
        defer_loading: tool.defer_loading.then_some(true),
        parameters: parse_tool_input_schema(&tool.input_schema)?,
        output_schema: None,
    })
}

#[cfg(test)]
#[path = "dynamic_tool_tests.rs"]
mod tests;
