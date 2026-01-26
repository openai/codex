use async_trait::async_trait;

use crate::function_tool::FunctionCallError;
use crate::protocol::SessionSource;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use codex_protocol::ask_user_question::AskUserQuestionArgs;

pub struct AskUserQuestionHandler;

#[async_trait]
impl ToolHandler for AskUserQuestionHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            call_id,
            payload,
            ..
        } = invocation;

        let session_source = turn.client.get_session_source();
        if !matches!(session_source, SessionSource::Cli | SessionSource::VSCode) {
            return Err(FunctionCallError::RespondToModel(format!(
                "ask_user_question is unavailable in non-interactive mode (session source: {session_source})"
            )));
        }

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "ask_user_question handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: AskUserQuestionArgs = parse_arguments(&arguments)?;
        let response = session
            .ask_user_question(turn.as_ref(), call_id, args)
            .await
            .ok_or_else(|| {
                FunctionCallError::RespondToModel(
                    "ask_user_question was cancelled before receiving a response".to_string(),
                )
            })?;

        let content = serde_json::to_string(&response).map_err(|err| {
            FunctionCallError::Fatal(format!(
                "failed to serialize ask_user_question response: {err}"
            ))
        })?;

        Ok(ToolOutput::Function {
            content,
            content_items: None,
            success: Some(true),
        })
    }
}
