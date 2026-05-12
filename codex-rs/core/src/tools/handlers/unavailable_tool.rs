use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use codex_tools::ToolName;
use codex_tools::ToolSpec;

pub struct UnavailableToolHandler {
    tool_name: ToolName,
    spec: Option<ToolSpec>,
}

impl UnavailableToolHandler {
    pub fn new(tool_name: ToolName, spec: ToolSpec) -> Self {
        Self {
            tool_name,
            spec: Some(spec),
        }
    }

    pub fn without_spec(tool_name: ToolName) -> Self {
        Self {
            tool_name,
            spec: None,
        }
    }
}

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

    fn tool_name(&self) -> ToolName {
        self.tool_name.clone()
    }

    fn spec(&self) -> Option<ToolSpec> {
        self.spec.clone()
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation { payload, .. } = invocation;

        match payload {
            ToolPayload::Function { .. } => Ok(FunctionToolOutput::from_text(
                unavailable_tool_message(
                    &self.tool_name,
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
