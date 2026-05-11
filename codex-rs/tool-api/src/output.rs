use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use serde::Serialize;
use serde_json::Value;

use crate::ToolError;
use crate::ToolInput;

/// Tool-owned output rendering for each host-facing boundary.
pub trait ToolOutput: Send {
    fn log_preview(&self) -> String;

    fn success_for_logging(&self) -> bool;

    fn to_response_item(&self, call_id: &str, input: &ToolInput) -> ResponseInputItem;

    fn code_mode_result(&self, input: &ToolInput) -> Value;
}

/// Convenience output for ordinary JSON-returning function tools.
#[derive(Clone, Debug)]
pub struct JsonToolOutput {
    value: Value,
}

impl JsonToolOutput {
    /// Creates a JSON output from a serializable value.
    pub fn from_serializable(value: impl Serialize) -> Result<Self, ToolError> {
        serde_json::to_value(value).map(Self::new).map_err(|err| {
            ToolError::respond_to_model(format!("failed to serialize output: {err}"))
        })
    }

    /// Creates a JSON output from an already materialized value.
    pub fn new(value: Value) -> Self {
        Self { value }
    }
}

impl ToolOutput for JsonToolOutput {
    fn log_preview(&self) -> String {
        self.value.to_string()
    }

    fn success_for_logging(&self) -> bool {
        true
    }

    fn to_response_item(&self, call_id: &str, _input: &ToolInput) -> ResponseInputItem {
        ResponseInputItem::FunctionCallOutput {
            call_id: call_id.to_string(),
            output: FunctionCallOutputPayload {
                body: FunctionCallOutputBody::Text(self.value.to_string()),
                success: Some(true),
            },
        }
    }

    fn code_mode_result(&self, _input: &ToolInput) -> Value {
        self.value.clone()
    }
}

#[cfg(test)]
mod tests {
    use codex_protocol::models::FunctionCallOutputBody;
    use codex_protocol::models::FunctionCallOutputPayload;
    use codex_protocol::models::ResponseInputItem;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::JsonToolOutput;
    use super::ToolOutput;
    use crate::ToolInput;

    #[test]
    fn json_tool_output_renders_function_output() {
        let input = ToolInput::Function {
            arguments: "{}".to_string(),
        };
        let output = JsonToolOutput::from_serializable(json!({ "ok": true }))
            .expect("serializable value should produce json output");

        assert_eq!(output.log_preview(), "{\"ok\":true}");
        assert!(output.success_for_logging());
        assert_eq!(
            output.to_response_item("call-1", &input),
            ResponseInputItem::FunctionCallOutput {
                call_id: "call-1".to_string(),
                output: FunctionCallOutputPayload {
                    body: FunctionCallOutputBody::Text("{\"ok\":true}".to_string()),
                    success: Some(true),
                },
            }
        );
        assert_eq!(output.code_mode_result(&input), json!({ "ok": true }));
    }
}
