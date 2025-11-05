//! Webhook MCP tool definition.

use mcp_types::ToolInfo;
use serde_json::json;

pub(crate) fn webhook_tool_info() -> ToolInfo {
    ToolInfo {
        name: "codex-webhook".to_string(),
        description: Some(
            "Execute webhook calls to external services (GitHub, Slack, or custom endpoints). \
             Enables integration with external APIs for automated workflows."
                .to_string(),
        ),
        input_schema: json!({
            "type": "object",
            "properties": {
                "service": {
                    "type": "string",
                    "enum": ["github", "slack", "custom"],
                    "description": "Service to call"
                },
                "action": {
                    "type": "string",
                    "description": "API endpoint or action"
                },
                "data": {
                    "type": "object",
                    "description": "Payload data to send"
                },
                "headers": {
                    "type": "object",
                    "description": "Optional custom headers",
                    "additionalProperties": {"type": "string"}
                }
            },
            "required": ["service", "action", "data"]
        }),
    }
}
