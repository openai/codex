use crate::context::ContextualUserFragment;
use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::get_context_remaining_spec::GET_CONTEXT_REMAINING_TOOL_NAME;
use crate::tools::handlers::get_context_remaining_spec::create_get_context_remaining_tool;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use codex_tools::ToolName;
use codex_tools::ToolSpec;

pub struct GetContextRemainingHandler;

impl ToolExecutor<ToolInvocation> for GetContextRemainingHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(GET_CONTEXT_REMAINING_TOOL_NAME)
    }

    fn spec(&self) -> ToolSpec {
        create_get_context_remaining_tool()
    }

    fn handle(&self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'_> {
        Box::pin(async move {
            if !matches!(invocation.payload, ToolPayload::Function { .. }) {
                return Err(FunctionCallError::RespondToModel(
                    "get_context_remaining handler received unsupported payload".to_string(),
                ));
            }

            let Some(model_context_window) = invocation.turn.model_context_window() else {
                let fragment = crate::context::TokenBudgetRemainingContext::unknown().render();
                return Ok(boxed_tool_output(FunctionToolOutput::from_text(
                    fragment,
                    Some(true),
                )));
            };
            let active_context_tokens = invocation.session.get_total_token_usage().await.max(0);
            let tokens_left = model_context_window
                .saturating_sub(active_context_tokens)
                .max(0);
            let fragment = crate::context::TokenBudgetRemainingContext::new(tokens_left).render();

            Ok(boxed_tool_output(FunctionToolOutput::from_text(
                fragment,
                Some(true),
            )))
        })
    }
}

impl CoreToolRuntime for GetContextRemainingHandler {}
