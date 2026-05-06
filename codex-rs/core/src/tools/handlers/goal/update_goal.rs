use crate::function_tool::FunctionCallError;
use crate::goals::GoalRuntimeEvent;
use crate::goals::SetGoalRequest;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use codex_protocol::protocol::ThreadGoalStatus;
use codex_tools::ToolName;
use codex_tools::UPDATE_GOAL_TOOL_NAME;

use super::CompletionBudgetReport;
use super::UpdateGoalArgs;
use super::format_goal_error;
use super::goal_response;

pub struct UpdateGoalHandler;

impl ToolHandler for UpdateGoalHandler {
    type Output = FunctionToolOutput;

    fn tool_name(&self) -> ToolName {
        ToolName::plain(UPDATE_GOAL_TOOL_NAME)
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            payload,
            ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "update_goal handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: UpdateGoalArgs = parse_arguments(&arguments)?;
        if args.status != ThreadGoalStatus::Complete {
            return Err(FunctionCallError::RespondToModel(
                "update_goal can only mark the existing goal complete; pause, resume, and budget-limited status changes are controlled by the user or system"
                    .to_string(),
            ));
        }
        session
            .goal_runtime_apply(GoalRuntimeEvent::ToolCompletedGoal {
                turn_context: turn.as_ref(),
            })
            .await
            .map_err(|err| FunctionCallError::RespondToModel(format_goal_error(err)))?;
        let goal = session
            .set_thread_goal(
                turn.as_ref(),
                SetGoalRequest {
                    objective: None,
                    status: Some(ThreadGoalStatus::Complete),
                    token_budget: None,
                },
            )
            .await
            .map_err(|err| FunctionCallError::RespondToModel(format_goal_error(err)))?;
        goal_response(Some(goal), CompletionBudgetReport::Include)
    }
}
