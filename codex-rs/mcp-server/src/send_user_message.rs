use mcp_types::Tool;
use mcp_types::ToolInputSchema;
use schemars::JsonSchema;
use schemars::r#gen::SchemaSettings;
use serde::Deserialize;
use serde::Serialize;

/// Parameters for the `send_user_message` tool call.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SendUserMessageParam {
    /// The message to send to the conversation.
    pub message: String,
    pub session_id: String,
}

/// Build a `Tool` definition for the `send_user_message` tool-call.
pub(crate) fn create_tool_for_send_user_message_param() -> Tool {
    let schema = SchemaSettings::draft2019_09()
        .with(|s| {
            s.inline_subschemas = true;
            s.option_add_null_type = false;
        })
        .into_generator()
        .into_root_schema_for::<SendUserMessageParam>();

    let schema_value = serde_json::to_value(&schema)
        .unwrap_or_else(|e| panic!("send_user_message tool schema should serialize to JSON: {e}"));

    let tool_input_schema = serde_json::from_value::<ToolInputSchema>(schema_value)
        .unwrap_or_else(|e| panic!("failed to create Tool from schema: {e}"));

    Tool {
        name: "send-user-message".to_string(),
        title: Some("Send User Message".to_string()),
        input_schema: tool_input_schema,
        // No output schema currently defined.
        output_schema: None,
        description: Some("Send a user message to a running conversation.".to_string()),
        annotations: None,
    }
}
