use crate::FunctionCallError;
use crate::ToolName;
use crate::ToolPayload;
use codex_protocol::models::ResponseItem;
use codex_utils_output_truncation::TruncationPolicy;
use std::sync::Arc;

/// Raw response history snapshot available when an extension tool is invoked.
#[derive(Clone, Debug, Default)]
pub struct ConversationHistory {
    items: Arc<[ResponseItem]>,
}

impl ConversationHistory {
    pub fn new(items: Vec<ResponseItem>) -> Self {
        Self {
            items: items.into(),
        }
    }

    pub fn items(&self) -> &[ResponseItem] {
        &self.items
    }
}

// TODO: this is temporary and will disappear in the next PR (as we make codex-extension-api generic on Invocation.
#[derive(Clone, Debug)]
pub struct ToolCall {
    pub turn_id: String,
    pub call_id: String,
    pub tool_name: ToolName,
    pub truncation_policy: TruncationPolicy,
    pub conversation_history: ConversationHistory,
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
