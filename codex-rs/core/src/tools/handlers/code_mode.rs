use async_trait::async_trait;

use crate::features::Feature;
use crate::function_tool::FunctionCallError;
use crate::tools::code_mode;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use codex_protocol::models::FunctionCallOutputBody;

pub struct CodeModeHandler;

#[async_trait]
impl ToolHandler for CodeModeHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Custom { .. })
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            tracker,
            payload,
            ..
        } = invocation;

        if !session.features().enabled(Feature::CodeMode) {
            return Err(FunctionCallError::RespondToModel(
                "code_mode is disabled by feature flag".to_string(),
            ));
        }

        let code = match payload {
            ToolPayload::Custom { input } => input,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "code_mode expects raw JavaScript source text".to_string(),
                ));
            }
        };

        let content_items = code_mode::execute(session, turn, tracker, code).await?;
        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::ContentItems(content_items),
            success: Some(true),
        })
    }
}
