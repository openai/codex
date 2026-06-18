use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::new_context_window_spec::NEW_CONTEXT_WINDOW_TOOL_NAME;
use crate::tools::handlers::new_context_window_spec::create_new_context_window_tool;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use codex_tools::ToolName;
use codex_tools::ToolSpec;

pub(crate) const NEW_CONTEXT_WINDOW_MESSAGE: &str =
    "A new context window will start without summarizing conversation history.";

pub struct NewContextWindowHandler;

impl ToolExecutor<ToolInvocation> for NewContextWindowHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(NEW_CONTEXT_WINDOW_TOOL_NAME)
    }

    fn spec(&self) -> ToolSpec {
        create_new_context_window_tool()
    }

    fn handle(&self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'_> {
        Box::pin(async move {
            if !matches!(invocation.payload, ToolPayload::Function { .. }) {
                return Err(FunctionCallError::RespondToModel(
                    "new_context handler received unsupported payload".to_string(),
                ));
            }

            invocation.session.request_new_context_window().await;

            Ok(boxed_tool_output(FunctionToolOutput::from_text(
                NEW_CONTEXT_WINDOW_MESSAGE.to_string(),
                Some(true),
            )))
        })
    }
}

impl CoreToolRuntime for NewContextWindowHandler {}
