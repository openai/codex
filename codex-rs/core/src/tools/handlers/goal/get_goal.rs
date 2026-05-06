use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use codex_tools::GET_GOAL_TOOL_NAME;
use codex_tools::ToolName;

use super::CompletionBudgetReport;
use super::format_goal_error;
use super::goal_response;

pub struct GetGoalHandler;

impl ToolHandler for GetGoalHandler {
    type Output = FunctionToolOutput;

    fn tool_name(&self) -> ToolName {
        ToolName::plain(GET_GOAL_TOOL_NAME)
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Function
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
