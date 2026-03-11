use std::collections::HashMap;

use serde::Serialize;
use serde_json::Value as JsonValue;

pub type ToolCallHandler =
    dyn FnMut(String, Option<JsonValue>) -> Result<JsonValue, String> + Send + 'static;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
    Function,
    Freeform,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EnabledTool {
    pub tool_name: String,
    #[serde(rename = "module")]
    pub module_path: String,
    pub namespace: Vec<String>,
    pub name: String,
    pub description: String,
    pub kind: ToolKind,
}

#[derive(Debug)]
pub struct ExecutionResult {
    pub content_items: Vec<JsonValue>,
    pub stored_values: HashMap<String, JsonValue>,
    pub max_output_tokens_per_exec_call: usize,
    pub success: bool,
    pub error_text: Option<String>,
}
