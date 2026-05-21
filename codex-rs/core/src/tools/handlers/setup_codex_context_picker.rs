use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use codex_protocol::setup_codex_context_picker::SetupCodexContextPickerArgs;
use codex_tools::SETUP_CODEX_CONTEXT_PICKER_TOOL_NAME;

pub struct SetupCodexContextPickerHandler;

impl ToolHandler for SetupCodexContextPickerHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            call_id,
            payload,
            ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(format!(
                    "{SETUP_CODEX_CONTEXT_PICKER_TOOL_NAME} handler received unsupported payload"
                )));
            }
        };

        let _: SetupCodexContextPickerArgs = parse_arguments(&arguments)?;
        let response = session
            .request_setup_codex_context_picker(turn.as_ref(), call_id)
            .await
            .ok_or_else(|| {
                FunctionCallError::RespondToModel(format!(
                    "{SETUP_CODEX_CONTEXT_PICKER_TOOL_NAME} was cancelled before receiving a response"
                ))
            })?;

        let content = serde_json::to_string(&response).map_err(|err| {
            FunctionCallError::Fatal(format!(
                "failed to serialize {SETUP_CODEX_CONTEXT_PICKER_TOOL_NAME} response: {err}"
            ))
        })?;

        Ok(FunctionToolOutput::from_text(content, Some(true)))
    }
}
