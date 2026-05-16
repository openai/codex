use crate::function_tool::FunctionCallError;
use crate::goals::GoalRuntimeEvent;
use crate::goals::SetGoalRequest;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::goal_spec::UPDATE_GOAL_TOOL_NAME;
use crate::tools::handlers::goal_spec::create_update_goal_tool;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use codex_protocol::protocol::ThreadGoalStatus;
use codex_tools::ToolName;
use codex_tools::ToolSpec;

use super::CompletionBudgetReport;
use super::UpdateGoalArgs;
use super::format_goal_error;
use super::goal_response;

pub struct UpdateGoalHandler;

#[async_trait::async_trait]
impl ToolExecutor<ToolInvocation> for UpdateGoalHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(UPDATE_GOAL_TOOL_NAME)
    }

    fn spec(&self) -> Option<ToolSpec> {
        Some(create_update_goal_tool())
    }

    async fn handle(
        &self,
        invocation: ToolInvocation,
    ) -> Result<Box<dyn crate::tools::context::ToolOutput>, FunctionCallError> {
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
        goal_response(Some(goal), CompletionBudgetReport::Include).map(boxed_tool_output)
    }
}

impl CoreToolRuntime for UpdateGoalHandler {}
