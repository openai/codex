use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use codex_tools::REFLECTIONS_GET_CONTEXT_REMAINING_TOOL_NAME;
use codex_tools::REFLECTIONS_NEW_CONTEXT_WINDOW_TOOL_NAME;
use serde_json::json;

pub struct ReflectionsNewContextWindowHandler;

impl ToolHandler for ReflectionsNewContextWindowHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn is_mutating(&self, _invocation: &ToolInvocation) -> bool {
        true
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        if !matches!(invocation.payload, ToolPayload::Function { .. }) {
            return Err(FunctionCallError::RespondToModel(format!(
                "{REFLECTIONS_NEW_CONTEXT_WINDOW_TOOL_NAME} handler received unsupported payload"
            )));
        }

        invocation
            .session
            .request_reflections_context_window_reset();
        Ok(FunctionToolOutput::from_text(
            "A fresh Reflections context window will start after this tool result is recorded."
                .to_string(),
            Some(true),
        ))
    }
}

pub struct ReflectionsGetContextRemainingHandler;

impl ToolHandler for ReflectionsGetContextRemainingHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        if !matches!(invocation.payload, ToolPayload::Function { .. }) {
            return Err(FunctionCallError::RespondToModel(format!(
                "{REFLECTIONS_GET_CONTEXT_REMAINING_TOOL_NAME} handler received unsupported payload"
            )));
        }

        let used_tokens = invocation.session.get_total_token_usage().await;
        let context_window_size = invocation.turn.model_context_window();
        let remaining_tokens =
            context_window_size.map(|size| size.saturating_sub(used_tokens).max(0));
        let content = json!({
            "context_window_size": context_window_size,
            "used_tokens": used_tokens,
            "remaining_tokens": remaining_tokens,
        })
        .to_string();

        Ok(FunctionToolOutput::from_text(content, Some(true)))
    }
}
