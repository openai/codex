use crate::function_tool::FunctionCallError;
use crate::goals::CreateGoalRequest;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::goal_spec::CREATE_GOAL_TOOL_NAME;
use crate::tools::handlers::goal_spec::create_create_goal_tool;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use codex_tools::ToolName;
use codex_tools::ToolSpec;

use super::CompletionBudgetReport;
use super::CreateGoalArgs;
use super::format_goal_error;
use super::goal_response;

pub struct CreateGoalHandler;

#[async_trait::async_trait]
impl ToolExecutor<ToolInvocation> for CreateGoalHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(CREATE_GOAL_TOOL_NAME)
    }

    fn spec(&self) -> Option<ToolSpec> {
        Some(create_create_goal_tool())
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
                    "goal handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: CreateGoalArgs = parse_arguments(&arguments)?;
        let goal = session
            .create_thread_goal(
                turn.as_ref(),
                CreateGoalRequest {
                    objective: args.objective,
                    token_budget: args.token_budget,
                },
            )
            .await
            .map_err(|err| {
                if err
                    .chain()
                    .any(|cause| cause.to_string().contains("already has a goal"))
                {
                    FunctionCallError::RespondToModel(
                        "cannot create a new goal because this thread already has a goal; use update_goal only when the existing goal is complete"
                            .to_string(),
                    )
                } else {
                    FunctionCallError::RespondToModel(format_goal_error(err))
                }
            })?;
        goal_response(Some(goal), CompletionBudgetReport::Omit).map(boxed_tool_output)
    }
}

impl CoreToolRuntime for CreateGoalHandler {}
