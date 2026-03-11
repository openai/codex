use std::collections::HashMap;

use serde_json::Value as JsonValue;

mod api;
pub use api::EnabledTool;
pub use api::ExecutionResult;
pub use api::ToolCallHandler;
pub use api::ToolKind;

const BAZEL_UNSUPPORTED_REASON: &str = "code_mode is unavailable in Bazel builds";

pub const fn is_supported() -> bool {
    false
}

pub fn unsupported_reason() -> Option<&'static str> {
    Some(BAZEL_UNSUPPORTED_REASON)
}

pub fn execute(
    code: String,
    enabled_tools: Vec<EnabledTool>,
    stored_values: HashMap<String, JsonValue>,
    on_tool_call: Box<ToolCallHandler>,
) -> Result<ExecutionResult, String> {
    let _ = code;
    let _ = enabled_tools;
    let _ = stored_values;
    let _ = on_tool_call;
    Err(BAZEL_UNSUPPORTED_REASON.to_string())
}
