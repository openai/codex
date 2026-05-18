use std::sync::Arc;

use async_trait::async_trait;
use codex_extension_api::FunctionCallError;
use codex_extension_api::JsonToolOutput;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolExecutor;
use codex_extension_api::ToolName;
use codex_extension_api::ToolOutput;
use codex_extension_api::ToolSpec;
use codex_protocol::ThreadId;
use codex_protocol::protocol::ThreadGoal;
use codex_protocol::protocol::ThreadGoalStatus;
use codex_protocol::protocol::validate_thread_goal_objective;
use serde::Deserialize;
use serde::Serialize;

use crate::extension::GoalToolBackend;
use crate::spec::CREATE_GOAL_TOOL_NAME;
use crate::spec::GET_GOAL_TOOL_NAME;
use crate::spec::UPDATE_GOAL_TOOL_NAME;
use crate::spec::create_create_goal_tool;
use crate::spec::create_get_goal_tool;
use crate::spec::create_update_goal_tool;

#[derive(Clone)]
pub(crate) struct GoalToolExecutor {
    kind: GoalToolKind,
    thread_id: ThreadId,
    backend: Arc<dyn GoalToolBackend>,
}

#[derive(Clone, Copy)]
enum GoalToolKind {
    Get,
    Create,
    Update,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CreateGoalRequest {
    pub objective: String,
    pub token_budget: Option<i64>,
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

#[derive(Clone, Copy)]
enum CompletionBudgetReport {
    Include,
    Omit,
}

impl GoalToolExecutor {
    pub(crate) fn get(thread_id: ThreadId, backend: Arc<dyn GoalToolBackend>) -> Self {
        Self {
            kind: GoalToolKind::Get,
            thread_id,
            backend,
        }
    }

    pub(crate) fn create(thread_id: ThreadId, backend: Arc<dyn GoalToolBackend>) -> Self {
        Self {
            kind: GoalToolKind::Create,
            thread_id,
            backend,
        }
    }

    pub(crate) fn update(thread_id: ThreadId, backend: Arc<dyn GoalToolBackend>) -> Self {
        Self {
            kind: GoalToolKind::Update,
            thread_id,
            backend,
        }
    }
}

#[async_trait]
impl ToolExecutor<ToolCall> for GoalToolExecutor {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(match self.kind {
            GoalToolKind::Get => GET_GOAL_TOOL_NAME,
            GoalToolKind::Create => CREATE_GOAL_TOOL_NAME,
            GoalToolKind::Update => UPDATE_GOAL_TOOL_NAME,
        })
    }

    fn spec(&self) -> Option<ToolSpec> {
        Some(match self.kind {
            GoalToolKind::Get => create_get_goal_tool(),
            GoalToolKind::Create => create_create_goal_tool(),
            GoalToolKind::Update => create_update_goal_tool(),
        })
    }

    async fn handle(&self, invocation: ToolCall) -> Result<Box<dyn ToolOutput>, FunctionCallError> {
        match self.kind {
            GoalToolKind::Get => self.handle_get(invocation).await,
            GoalToolKind::Create => self.handle_create(invocation).await,
            GoalToolKind::Update => self.handle_update(invocation).await,
        }
    }
}

impl GoalToolExecutor {
    async fn handle_get(
        &self,
        invocation: ToolCall,
    ) -> Result<Box<dyn ToolOutput>, FunctionCallError> {
        let _ = invocation.function_arguments()?;
        let goal = self
            .backend
            .get_goal(self.thread_id)
            .await
            .map_err(FunctionCallError::RespondToModel)?;
        goal_response(goal, CompletionBudgetReport::Omit)
    }

    async fn handle_create(
        &self,
        invocation: ToolCall,
    ) -> Result<Box<dyn ToolOutput>, FunctionCallError> {
        let mut request: CreateGoalRequest = parse_arguments(invocation.function_arguments()?)?;
        request.objective = request.objective.trim().to_string();
        validate_thread_goal_objective(&request.objective)
            .map_err(FunctionCallError::RespondToModel)?;
        validate_goal_budget(request.token_budget).map_err(FunctionCallError::RespondToModel)?;

        let goal = self
            .backend
            .create_goal(self.thread_id, request)
            .await
            .map_err(FunctionCallError::RespondToModel)?;
        goal_response(Some(goal), CompletionBudgetReport::Omit)
    }

    async fn handle_update(
        &self,
        invocation: ToolCall,
    ) -> Result<Box<dyn ToolOutput>, FunctionCallError> {
        let args: UpdateGoalArgs = parse_arguments(invocation.function_arguments()?)?;
        if args.status != ThreadGoalStatus::Complete {
            return Err(FunctionCallError::RespondToModel(
                "update_goal can only mark the existing goal complete; pause, resume, and budget-limited status changes are controlled by the user or system"
                    .to_string(),
            ));
        }

        // TODO: update_goal needs a host callback before completion to flush
        // final active-turn accounting with budget steering suppressed.
        let goal = self
            .backend
            .complete_goal(self.thread_id)
            .await
            .map_err(FunctionCallError::RespondToModel)?;
        goal_response(Some(goal), CompletionBudgetReport::Include)
    }
}

fn parse_arguments<T>(arguments: &str) -> Result<T, FunctionCallError>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_str(arguments)
        .map_err(|err| FunctionCallError::RespondToModel(err.to_string()))
}

fn validate_goal_budget(value: Option<i64>) -> Result<(), String> {
    if let Some(value) = value
        && value <= 0
    {
        return Err("goal budgets must be positive when provided".to_string());
    }
    Ok(())
}

fn goal_response(
    goal: Option<ThreadGoal>,
    completion_budget_report: CompletionBudgetReport,
) -> Result<Box<dyn ToolOutput>, FunctionCallError> {
    let value = serde_json::to_value(GoalToolResponse::new(goal, completion_budget_report))
        .map_err(|err| FunctionCallError::Fatal(err.to_string()))?;
    Ok(Box::new(JsonToolOutput::new(value)))
}

impl GoalToolResponse {
    fn new(goal: Option<ThreadGoal>, report_mode: CompletionBudgetReport) -> Self {
        let remaining_tokens = goal.as_ref().and_then(|goal| {
            goal.token_budget
                .map(|budget| (budget - goal.tokens_used).max(0))
        });
        let completion_budget_report = match report_mode {
            CompletionBudgetReport::Include => goal
                .as_ref()
                .filter(|goal| goal.status == ThreadGoalStatus::Complete)
                .and_then(completion_budget_report),
            CompletionBudgetReport::Omit => None,
        };
        Self {
            goal,
            remaining_tokens,
            completion_budget_report,
        }
    }
}

fn completion_budget_report(goal: &ThreadGoal) -> Option<String> {
    if goal.token_budget.is_none() && goal.time_used_seconds <= 0 {
        None
    } else {
        Some(
            "Goal achieved. Report final usage from this tool result's structured goal fields. If `goal.tokenBudget` is present, include token usage from `goal.tokensUsed` and `goal.tokenBudget`. If `goal.timeUsedSeconds` is greater than 0, summarize elapsed time in a concise, human-friendly form appropriate to the response language."
                .to_string(),
        )
    }
}
