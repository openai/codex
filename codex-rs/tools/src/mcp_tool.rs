use crate::ToolDefinition;
use crate::ToolExecution;
use crate::ToolLoadingPolicy;
use crate::ToolName;
use crate::parse_tool_input_schema;
use serde_json::Value as JsonValue;
use serde_json::json;

// TODO(tool-definition-unification): remove this incomplete raw MCP adapter
// once callers can use a full `ToolInfo -> ToolDefinition` adapter that also
// populates MCP presentation/search metadata.
pub fn mcp_tool_to_tool_definition(
    tool_name: &ToolName,
    tool: &rmcp::model::Tool,
) -> Result<ToolDefinition, serde_json::Error> {
    let mut serialized_input_schema = serde_json::Value::Object(tool.input_schema.as_ref().clone());

    // OpenAI models mandate the "properties" field in the schema. Some MCP
    // servers omit it (or set it to null), so we insert an empty object to
    // match the behavior of the Agents SDK.
    if let serde_json::Value::Object(obj) = &mut serialized_input_schema
        && obj.get("properties").is_none_or(serde_json::Value::is_null)
    {
        obj.insert(
            "properties".to_string(),
            serde_json::Value::Object(serde_json::Map::new()),
        );
    }

    let input_schema = parse_tool_input_schema(&serialized_input_schema)?;
    let structured_content_schema = tool
        .output_schema
        .as_ref()
        .map(|output_schema| serde_json::Value::Object(output_schema.as_ref().clone()))
        .unwrap_or_else(|| JsonValue::Object(serde_json::Map::new()));

    Ok(ToolDefinition {
        name: tool_name.clone(),
        description: tool.description.clone().map(Into::into).unwrap_or_default(),
        input_schema,
        output_schema: Some(mcp_call_tool_result_output_schema(
            structured_content_schema,
        )),
        loading: ToolLoadingPolicy::Eager,
        execution: ToolExecution::Mcp,
        presentation: None,
        search: None,
        supports_parallel_tool_calls: false,
    })
}

// TODO(tool-definition-unification): remove this compatibility wrapper once
// callers can use the full `ToolInfo -> ToolDefinition` adapter.
pub fn parse_mcp_tool(tool: &rmcp::model::Tool) -> Result<ToolDefinition, serde_json::Error> {
    mcp_tool_to_tool_definition(&ToolName::plain(tool.name.to_string()), tool)
}

pub fn mcp_call_tool_result_output_schema(structured_content_schema: JsonValue) -> JsonValue {
    json!({
        "type": "object",
        "properties": {
            "content": {
                "type": "array",
                "items": {
                    "type": "object"
                }
            },
            "structuredContent": structured_content_schema,
            "isError": {
                "type": "boolean"
            },
            "_meta": {
                "type": "object"
            }
        },
        "required": ["content"],
        "additionalProperties": false
    })
}

#[cfg(test)]
#[path = "mcp_tool_tests.rs"]
mod tests;
