//! Helpers for mapping thread-goal state into the compact status-line indicator.

use codex_app_server_protocol::ThreadGoal as AppThreadGoal;
use codex_app_server_protocol::ThreadGoalStatus as AppThreadGoalStatus;

use crate::bottom_pane::GoalStatusIndicator;
use crate::goal_display::format_goal_elapsed_seconds;
use crate::status::format_tokens_compact;

pub(super) fn goal_status_indicator_from_app_goal(
    goal: &AppThreadGoal,
) -> Option<GoalStatusIndicator> {
    match goal.status {
        AppThreadGoalStatus::Active => Some(GoalStatusIndicator::Active {
            usage: active_goal_usage(goal.token_budget, goal.tokens_used, goal.time_used_seconds),
        }),
        AppThreadGoalStatus::Paused => Some(GoalStatusIndicator::Paused),
        AppThreadGoalStatus::BudgetLimited => Some(GoalStatusIndicator::BudgetLimited {
            usage: stopped_goal_budget_usage(goal.token_budget, goal.tokens_used),
        }),
        AppThreadGoalStatus::Complete => Some(GoalStatusIndicator::Complete),
    }
}

fn active_goal_usage(
    token_budget: Option<i64>,
    tokens_used: i64,
    time_used_seconds: i64,
) -> Option<String> {
    if let Some(token_budget) = token_budget {
        return Some(format!(
            "{} / {}",
            format_tokens_compact(tokens_used),
            format_tokens_compact(token_budget)
        ));
    }

    Some(format_goal_elapsed_seconds(time_used_seconds))
}

fn stopped_goal_budget_usage(token_budget: Option<i64>, tokens_used: i64) -> Option<String> {
    token_budget.map(|token_budget| {
        format!(
            "{} / {} tokens",
            format_tokens_compact(tokens_used),
            format_tokens_compact(token_budget)
        )
    })
}
