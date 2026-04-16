use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

pub struct UnavailableToolHandler;

pub(crate) fn unavailable_tool_message(
    tool_name: impl std::fmt::Display,
    next_step: &str,
) -> String {
    format!(
        "Tool `{tool_name}` is not currently available. It appeared in earlier tool calls in this conversation, but its implementation is not available in the current request. {next_step}"
    )
}

impl ToolHandler for UnavailableToolHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation {
            tool_name, payload, ..
        } = invocation;

        match payload {
            ToolPayload::Function { .. } => Ok(FunctionToolOutput::from_text(
                unavailable_tool_message(
                    tool_name.display(),
                    "Retry after the tool becomes available or ask the user to re-enable it.",
                ),
                Some(false),
            )),
            _ => Err(FunctionCallError::RespondToModel(
                "unavailable tool handler received unsupported payload".to_string(),
            )),
        }
    }
}
