use codex_protocol::models::ResponseItem;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Default, Serialize)]
pub struct Prompt {
    #[serde(skip_serializing)]
    pub input: Vec<ResponseItem>,
    #[serde(skip_serializing)]
    pub tools: Vec<Value>,
    #[serde(skip_serializing)]
    pub parallel_tool_calls: bool,
    #[serde(skip_serializing)]
    pub output_schema: Option<Value>,
}

impl Prompt {
    pub fn new(
        input: Vec<ResponseItem>,
        tools: Vec<Value>,
        parallel_tool_calls: bool,
        output_schema: Option<Value>,
    ) -> Self {
        Self {
            input,
            tools,
            parallel_tool_calls,
            output_schema,
        }
    }
}
