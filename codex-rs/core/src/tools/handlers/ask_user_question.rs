use async_trait::async_trait;
use codex_protocol::ask_user_question::AskUserQuestionRequest;
use codex_protocol::ask_user_question::AskUserQuestionResponse;

use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

pub struct AskUserQuestionHandler;

#[async_trait]
impl ToolHandler for AskUserQuestionHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    async fn is_mutating(&self, _invocation: &ToolInvocation) -> bool {
        false
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            call_id,
            tool_name,
            payload,
            ..
        } = invocation;

        let ToolPayload::Function { arguments } = payload else {
            return Err(FunctionCallError::RespondToModel(format!(
                "unsupported payload for {tool_name}"
            )));
        };

        let request: AskUserQuestionRequest = serde_json::from_str(&arguments).map_err(|e| {
            FunctionCallError::RespondToModel(format!("failed to parse function arguments: {e:?}"))
        })?;

        let response: AskUserQuestionResponse = session
            .request_ask_user_question(turn.as_ref(), call_id.clone(), request)
            .await;

        let content = serde_json::to_string(&response).map_err(|e| {
            FunctionCallError::RespondToModel(format!(
                "failed to serialize AskUserQuestion response: {e:?}"
            ))
        })?;

        Ok(ToolOutput::Function {
            content,
            content_items: None,
            success: Some(!response.cancelled),
        })
    }
}
