mod api;
pub use api::EnabledTool;
pub use api::ExecutionResult;
pub use api::ToolCallHandler;
pub use api::ToolKind;

const MUSL_UNSUPPORTED_REASON: &str = "code_mode is unavailable on musl Linux";

pub const fn is_supported() -> bool {
    !cfg!(all(target_os = "linux", target_env = "musl"))
}

pub fn unsupported_reason() -> Option<&'static str> {
    if is_supported() {
        None
    } else {
        Some(MUSL_UNSUPPORTED_REASON)
    }
}

#[cfg(not(all(target_os = "linux", target_env = "musl")))]
mod imp;

#[cfg(not(all(target_os = "linux", target_env = "musl")))]
pub use imp::execute;

#[cfg(all(target_os = "linux", target_env = "musl"))]
pub fn execute(
    code: String,
    enabled_tools: Vec<EnabledTool>,
    stored_values: std::collections::HashMap<String, serde_json::Value>,
    on_tool_call: Box<ToolCallHandler>,
) -> Result<ExecutionResult, String> {
    let _ = code;
    let _ = enabled_tools;
    let _ = stored_values;
    let _ = on_tool_call;
    Err(MUSL_UNSUPPORTED_REASON.to_string())
}
