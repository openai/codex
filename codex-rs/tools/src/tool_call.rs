use crate::FunctionCallError;
use crate::ToolName;
use crate::ToolPayload;
use codex_utils_output_truncation::TruncationPolicy;

// TODO: this is temporary and will disappear in the next PR (as we make codex-extension-api generic on Invocation.
#[derive(Clone, Debug)]
pub struct ToolCall {
    pub turn_id: String,
    pub call_id: String,
    pub tool_name: ToolName,
    pub truncation_policy: TruncationPolicy,
    pub payload: ToolPayload,
}

impl ToolCall {
    pub fn function_arguments(&self) -> Result<&str, FunctionCallError> {
        match &self.payload {
            ToolPayload::Function { arguments } => Ok(arguments),
            _ => Err(FunctionCallError::Fatal(format!(
                "tool {} invoked with incompatible payload",
                self.tool_name
            ))),
        }
    }
}
