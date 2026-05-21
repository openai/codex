use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use codex_protocol::setup_codex_context_picker::SetupCodexContextPickerArgs;
use codex_tools::SETUP_CODEX_CONTEXT_PICKER_TOOL_NAME;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use codex_tools::create_setup_codex_context_picker_tool;

pub struct SetupCodexContextPickerHandler;

#[async_trait::async_trait]
impl ToolExecutor<ToolInvocation> for SetupCodexContextPickerHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(SETUP_CODEX_CONTEXT_PICKER_TOOL_NAME)
    }

    fn spec(&self) -> ToolSpec {
        create_setup_codex_context_picker_tool()
    }

    async fn handle(
        &self,
        invocation: ToolInvocation,
    ) -> Result<Box<dyn crate::tools::context::ToolOutput>, FunctionCallError> {
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

        if turn.session_source.is_non_root_agent() {
            return Err(FunctionCallError::RespondToModel(
                "setup_codex_context_picker can only be used by the root thread".to_string(),
            ));
        }

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

        Ok(boxed_tool_output(FunctionToolOutput::from_text(
            content,
            Some(true),
        )))
    }
}

impl CoreToolRuntime for SetupCodexContextPickerHandler {}
