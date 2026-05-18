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
        if !matches!(
            args.status,
            ThreadGoalStatus::Complete | ThreadGoalStatus::Blocked
        ) {
            return Err(FunctionCallError::RespondToModel(
                "update_goal can only mark the existing goal complete or blocked; pause, resume, budget-limited, and usage-limited status changes are controlled by the user or system"
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
                    status: Some(args.status),
                    token_budget: None,
                },
            )
            .await
            .map_err(|err| FunctionCallError::RespondToModel(format_goal_error(err)))?;
        let completion_budget_report = if args.status == ThreadGoalStatus::Complete {
            CompletionBudgetReport::Include
        } else {
            CompletionBudgetReport::Omit
        };
        goal_response(Some(goal), completion_budget_report).map(boxed_tool_output)
    }
}

impl CoreToolRuntime for UpdateGoalHandler {}
