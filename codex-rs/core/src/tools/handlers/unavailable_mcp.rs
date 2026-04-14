use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

pub struct UnavailableMcpHandler;

impl ToolHandler for UnavailableMcpHandler {
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
                format!(
                    "MCP tool `{}` is currently unavailable. It appeared in earlier tool calls in this conversation, but the MCP server or tool is not available in the current request. Retry after the MCP server reconnects or ask the user to re-enable it.",
                    tool_name.display()
                ),
                Some(false),
            )),
            _ => Err(FunctionCallError::RespondToModel(
                "unavailable MCP handler received unsupported payload".to_string(),
            )),
        }
    }
}
