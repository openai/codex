use codex_tool_api::FunctionToolSpec;
use codex_tool_api::ToolDefinition;
use codex_tool_api::ToolName;
use serde_json::Value as JsonValue;
use serde_json::json;

pub fn mcp_tool_definition(tool_name: ToolName, tool: &rmcp::model::Tool) -> ToolDefinition<()> {
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

    let structured_content_schema = tool
        .output_schema
        .as_ref()
        .map(|output_schema| serde_json::Value::Object(output_schema.as_ref().clone()))
        .unwrap_or_else(|| JsonValue::Object(serde_json::Map::new()));

    ToolDefinition::new(
        tool_name.clone(),
        FunctionToolSpec {
            name: tool_name.name,
            description: tool.description.clone().map(Into::into).unwrap_or_default(),
            strict: false,
            parameters: serialized_input_schema,
        },
        (),
    )
    .with_output_schema(mcp_call_tool_result_output_schema(
        structured_content_schema,
    ))
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
