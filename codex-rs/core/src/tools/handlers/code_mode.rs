use async_trait::async_trait;

use crate::features::Feature;
use crate::function_tool::FunctionCallError;
use crate::tools::code_mode;
use crate::tools::code_mode::PUBLIC_TOOL_NAME;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

pub struct CodeModeHandler;

#[async_trait]
impl ToolHandler for CodeModeHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Custom { .. })
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            tracker,
            payload,
            ..
        } = invocation;

        if !session.features().enabled(Feature::CodeMode) {
            return Err(FunctionCallError::RespondToModel(format!(
                "{PUBLIC_TOOL_NAME} is disabled by feature flag"
            )));
        }
        if let Some(reason) = codex_code_mode::unsupported_reason() {
            return Err(FunctionCallError::RespondToModel(reason.to_string()));
        }

        let code = match payload {
            ToolPayload::Custom { input } => input,
            _ => {
                return Err(FunctionCallError::RespondToModel(format!(
                    "{PUBLIC_TOOL_NAME} expects raw JavaScript source text"
                )));
            }
        };

        code_mode::execute(session, turn, tracker, code).await
    }
}
