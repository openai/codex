//! Built-in model tool handlers for persisted thread goals.
//!
//! The public tool contract mirrors the app-server `thread/goal/set` overload:
//! providing an objective replaces the goal and resets accounting, while
//! omitting the objective updates the existing goal and preserves usage.

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::function_tool::FunctionCallError;
use crate::goals::GoalAccountingBoundary;
use crate::goals::SetGoalRequest;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use codex_protocol::protocol::ThreadGoal;
use codex_protocol::protocol::ThreadGoalStatus;
use codex_tools::GET_GOAL_TOOL_NAME;
use codex_tools::SET_GOAL_TOOL_NAME;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use std::fmt::Write as _;
use std::sync::Arc;

pub struct GoalHandler;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct SetGoalArgs {
    objective: Option<String>,
    status: Option<ToolGoalStatus>,
    #[serde(default, deserialize_with = "deserialize_double_option")]
    token_budget: Option<Option<i64>>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
enum ToolGoalStatus {
    Active,
    Paused,
    BudgetLimited,
    Complete,
}

impl From<ToolGoalStatus> for ThreadGoalStatus {
    fn from(value: ToolGoalStatus) -> Self {
        match value {
            ToolGoalStatus::Active => Self::Active,
            ToolGoalStatus::Paused => Self::Paused,
            ToolGoalStatus::BudgetLimited => Self::BudgetLimited,
            ToolGoalStatus::Complete => Self::Complete,
        }
    }
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
            SET_GOAL_TOOL_NAME => handle_set_goal(&session, turn.as_ref(), &arguments).await,
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

async fn handle_set_goal(
    session: &Arc<Session>,
    turn_context: &TurnContext,
    arguments: &str,
) -> Result<FunctionToolOutput, FunctionCallError> {
    let args: SetGoalArgs = parse_arguments(arguments)?;
    if args.objective.is_none()
        && matches!(
            args.status,
            Some(ToolGoalStatus::Paused | ToolGoalStatus::BudgetLimited | ToolGoalStatus::Complete)
        )
    {
        session
            .account_thread_goal_progress(turn_context, GoalAccountingBoundary::Tool)
            .await
            .map_err(|err| FunctionCallError::RespondToModel(format_goal_error(err)))?;
    }
    let goal = session
        .set_thread_goal(
            turn_context,
            SetGoalRequest {
                objective: args.objective,
                status: args.status.map(Into::into),
                token_budget: args.token_budget,
            },
        )
        .await
        .map_err(|err| FunctionCallError::RespondToModel(format_goal_error(err)))?;
    goal_response(Some(goal))
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
    if parts.is_empty() {
        None
    } else {
        Some(format!(
            "Goal achieved. Report final budget usage to the user: {}.",
            parts.join("; ")
        ))
    }
}

fn deserialize_double_option<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
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
                    "Goal achieved. Report final budget usage to the user: tokens used: 3250 of 10000."
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
