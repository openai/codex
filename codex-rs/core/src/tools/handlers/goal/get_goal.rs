use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::goal_spec::GET_GOAL_TOOL_NAME;
use crate::tools::handlers::goal_spec::create_get_goal_tool;
use crate::tools::registry::ToolExecutor;
use crate::tools::registry::ToolHandler;
use codex_tools::ToolName;
use codex_tools::ToolSpec;

use super::CompletionBudgetReport;
use super::format_goal_error;
use super::goal_response;

pub struct GetGoalHandler;

#[async_trait::async_trait]
impl ToolExecutor<ToolInvocation> for GetGoalHandler {
    type Output = FunctionToolOutput;

    fn tool_name(&self) -> ToolName {
        ToolName::plain(GET_GOAL_TOOL_NAME)
    }

    fn spec(&self) -> Option<ToolSpec> {
        Some(create_get_goal_tool())
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation {
            session, payload, ..
        } = invocation;

        match payload {
            ToolPayload::Function { .. } => {
                let goal = session
                    .get_thread_goal()
                    .await
                    .map_err(|err| FunctionCallError::RespondToModel(format_goal_error(err)))?;
                goal_response(goal, CompletionBudgetReport::Omit)
            }
            _ => Err(FunctionCallError::RespondToModel(
                "get_goal handler received unsupported payload".to_string(),
            )),
        }
    }
}

impl ToolHandler for GetGoalHandler {}
