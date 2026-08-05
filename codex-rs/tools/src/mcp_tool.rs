use crate::ToolDefinition;
use crate::parse_tool_input_schema;
use codex_utils_string::take_bytes_at_char_boundary;
use serde_json::Value as JsonValue;
use serde_json::json;

const MAX_MCP_TOOL_DESCRIPTION_BYTES: usize = 1_000;

pub fn parse_mcp_tool(tool: &rmcp::model::Tool) -> Result<ToolDefinition, serde_json::Error> {
    parse_mcp_tool_with_description_limit(tool, /*description_limit*/ None)
}

pub fn parse_agent_plugin_mcp_tool(
    tool: &rmcp::model::Tool,
) -> Result<ToolDefinition, serde_json::Error> {
    parse_mcp_tool_with_description_limit(tool, Some(MAX_MCP_TOOL_DESCRIPTION_BYTES))
}

fn parse_mcp_tool_with_description_limit(
    tool: &rmcp::model::Tool,
    description_limit: Option<usize>,
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
        name: tool.name.to_string(),
        description: tool
            .description
            .as_deref()
            .map(|description| match description_limit {
                Some(limit) => take_bytes_at_char_boundary(description, limit).to_string(),
                None => description.to_string(),
            })
            .unwrap_or_default(),
        input_schema,
        output_schema: Some(mcp_call_tool_result_output_schema(
            structured_content_schema,
        )),
        defer_loading: false,
    })
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
