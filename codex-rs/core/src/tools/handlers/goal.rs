//! Built-in model tool handlers for persisted thread goals.
//!
//! The public tool contract intentionally splits goal creation from completion:
//! `create_goal` starts an active objective, while `update_goal` can only mark
//! the existing goal complete.

use crate::function_tool::FunctionCallError;
use crate::goals::BudgetLimitSteering;
use crate::goals::CreateGoalRequest;
use crate::goals::SetGoalRequest;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use codex_protocol::protocol::ThreadGoal;
use codex_protocol::protocol::ThreadGoalStatus;
use codex_tools::CREATE_GOAL_TOOL_NAME;
use codex_tools::GET_GOAL_TOOL_NAME;
use codex_tools::UPDATE_GOAL_TOOL_NAME;
use serde::Deserialize;
use serde::Serialize;
use std::fmt::Write as _;

pub struct GoalHandler;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct CreateGoalArgs {
    objective: String,
    token_budget: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct UpdateGoalArgs {
    status: ThreadGoalStatus,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct GoalToolResponse {
    goal: Option<ThreadGoal>,
    remaining_tokens: Option<i64>,
    completion_budget_report: Option<String>,
}

impl GoalToolResponse {
    fn new(goal: Option<ThreadGoal>) -> Self {
        let remaining_tokens = goal.as_ref().and_then(|goal| {
            goal.token_budget
                .map(|budget| (budget - goal.tokens_used).max(0))
        });
        let completion_budget_report = goal
            .as_ref()
            .filter(|goal| goal.status == ThreadGoalStatus::Complete)
            .and_then(completion_budget_report);
        Self {
            goal,
            remaining_tokens,
            completion_budget_report,
        }
    }
}

impl ToolHandler for GoalHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            payload,
            tool_name,
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

        match tool_name.name.as_str() {
            GET_GOAL_TOOL_NAME => handle_get_goal(session.as_ref()).await,
            CREATE_GOAL_TOOL_NAME => {
                handle_create_goal(session.as_ref(), turn.as_ref(), &arguments).await
            }
            UPDATE_GOAL_TOOL_NAME => {
                handle_update_goal(session.as_ref(), turn.as_ref(), &arguments).await
            }
            other => Err(FunctionCallError::Fatal(format!(
                "goal handler received unsupported tool: {other}"
            ))),
        }
    }
}

async fn handle_get_goal(session: &Session) -> Result<FunctionToolOutput, FunctionCallError> {
    let goal = session
        .get_thread_goal()
        .await
        .map_err(|err| FunctionCallError::RespondToModel(format_goal_error(err)))?;
    goal_response(goal)
}

async fn handle_create_goal(
    session: &Session,
    turn_context: &TurnContext,
    arguments: &str,
) -> Result<FunctionToolOutput, FunctionCallError> {
    let args: CreateGoalArgs = parse_arguments(arguments)?;
    let goal = session
        .create_thread_goal(
            turn_context,
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
    goal_response(Some(goal))
}

async fn handle_update_goal(
    session: &Session,
    turn_context: &TurnContext,
    arguments: &str,
) -> Result<FunctionToolOutput, FunctionCallError> {
    let args: UpdateGoalArgs = parse_arguments(arguments)?;
    if args.status != ThreadGoalStatus::Complete {
        return Err(FunctionCallError::RespondToModel(
            "update_goal can only mark the existing goal complete; pause, resume, and budget-limited status changes are controlled by the user or system"
                .to_string(),
        ));
    }
    session
        .account_thread_goal_progress(turn_context, BudgetLimitSteering::Suppressed)
        .await
        .map_err(|err| FunctionCallError::RespondToModel(format_goal_error(err)))?;
    session
        .set_thread_goal(
            turn_context,
            SetGoalRequest {
                objective: None,
                status: Some(ThreadGoalStatus::Complete),
                token_budget: None,
            },
        )
        .await
        .map_err(|err| FunctionCallError::RespondToModel(format_goal_error(err)))?;
    let goal = session
        .get_thread_goal()
        .await
        .map_err(|err| FunctionCallError::RespondToModel(format_goal_error(err)))?;
    goal_response(goal)
}

fn format_goal_error(err: anyhow::Error) -> String {
    let mut message = err.to_string();
    for cause in err.chain().skip(1) {
        let _ = write!(message, ": {cause}");
    }
    message
}

fn goal_response(goal: Option<ThreadGoal>) -> Result<FunctionToolOutput, FunctionCallError> {
    let response = serde_json::to_string_pretty(&GoalToolResponse::new(goal))
        .map_err(|err| FunctionCallError::Fatal(err.to_string()))?;
    Ok(FunctionToolOutput::from_text(response, Some(true)))
}

fn completion_budget_report(goal: &ThreadGoal) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(budget) = goal.token_budget {
        parts.push(format!("tokens used: {} of {budget}", goal.tokens_used));
    }
    if goal.time_used_seconds > 0 {
        parts.push(format!("time used: {} seconds", goal.time_used_seconds));
    }
    if parts.is_empty() {
        None
    } else {
        Some(format!(
            "Goal achieved. Report final budget usage to the user: {}.",
            parts.join("; ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::ThreadId;
    use pretty_assertions::assert_eq;

    #[test]
    fn completed_budgeted_goal_response_reports_final_usage() {
        let goal = ThreadGoal {
            thread_id: ThreadId::new(),
            objective: "Keep optimizing".to_string(),
            status: ThreadGoalStatus::Complete,
            token_budget: Some(10_000),
            tokens_used: 3_250,
            time_used_seconds: 75,
            created_at: 1,
            updated_at: 2,
        };

        let response = GoalToolResponse::new(Some(goal.clone()));

        assert_eq!(
            response,
            GoalToolResponse {
                goal: Some(goal),
                remaining_tokens: Some(6_750),
                completion_budget_report: Some(
                    "Goal achieved. Report final budget usage to the user: tokens used: 3250 of 10000; time used: 75 seconds."
                        .to_string()
                ),
            }
        );
    }

    #[test]
    fn completed_unbudgeted_goal_response_omits_budget_report() {
        let goal = ThreadGoal {
            thread_id: ThreadId::new(),
            objective: "Write a poem".to_string(),
            status: ThreadGoalStatus::Complete,
            token_budget: None,
            tokens_used: 120,
            time_used_seconds: 0,
            created_at: 1,
            updated_at: 2,
        };

        let response = GoalToolResponse::new(Some(goal.clone()));

        assert_eq!(
            response,
            GoalToolResponse {
                goal: Some(goal),
                remaining_tokens: None,
                completion_budget_report: None,
            }
        );
    }
}
