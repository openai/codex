//! Helpers for mapping thread-goal state into the compact status-line indicator.

use codex_app_server_protocol::ThreadGoal as AppThreadGoal;
use codex_app_server_protocol::ThreadGoalStatus as AppThreadGoalStatus;
use std::time::Instant;

use crate::bottom_pane::GoalStatusIndicator;
use crate::goal_display::format_goal_elapsed_seconds;
use crate::status::format_tokens_compact;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct GoalStatusState {
    goal: AppThreadGoal,
    observed_at: Instant,
}

impl GoalStatusState {
    pub(super) fn new(goal: AppThreadGoal, observed_at: Instant) -> Self {
        Self { goal, observed_at }
    }

    pub(super) fn indicator(
        &self,
        now: Instant,
        active_turn_started_at: Option<Instant>,
    ) -> Option<GoalStatusIndicator> {
        let mut goal = self.goal.clone();
        if goal.status == AppThreadGoalStatus::Active
            && let Some(active_turn_started_at) = active_turn_started_at
        {
            let baseline = self.observed_at.max(active_turn_started_at);
            let active_seconds = now.saturating_duration_since(baseline).as_secs();
            goal.time_used_seconds = goal
                .time_used_seconds
                .saturating_add(i64::try_from(active_seconds).unwrap_or(i64::MAX));
        }
        goal_status_indicator_from_app_goal(&goal)
    }
}

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

#[cfg(test)]
mod tests {
    use super::GoalStatusState;
    use super::active_goal_usage;
    use super::stopped_goal_budget_usage;
    use crate::bottom_pane::GoalStatusIndicator;
    use codex_app_server_protocol::ThreadGoal as AppThreadGoal;
    use codex_app_server_protocol::ThreadGoalStatus as AppThreadGoalStatus;
    use std::time::Duration;
    use std::time::Instant;

    #[test]
    fn active_goal_usage_prefers_token_budget() {
        assert_eq!(
            active_goal_usage(
                Some(50_000),
                /*tokens_used*/ 12_500,
                /*time_used_seconds*/ 90
            ),
            Some("12.5K / 50K".to_string())
        );
    }

    #[test]
    fn active_goal_usage_reports_time_without_budget() {
        assert_eq!(
            active_goal_usage(None, /*tokens_used*/ 12_500, 120),
            Some("2m".to_string())
        );
    }

    #[test]
    fn stopped_goal_budget_usage_reports_budgeted_tokens() {
        assert_eq!(
            stopped_goal_budget_usage(Some(50_000), /*tokens_used*/ 63_876),
            Some("63.9K / 50K tokens".to_string())
        );
    }

    #[test]
    fn stopped_goal_budget_usage_omits_unbudgeted_usage() {
        assert_eq!(
            stopped_goal_budget_usage(None, /*tokens_used*/ 12_500),
            None
        );
    }

    #[test]
    fn active_goal_status_includes_current_turn_elapsed_time() {
        let observed_at = Instant::now();
        let state = active_goal_state(observed_at, /*time_used_seconds*/ 60);

        assert_eq!(
            state.indicator(
                observed_at + Duration::from_secs(60),
                Some(observed_at - Duration::from_secs(120)),
            ),
            Some(GoalStatusIndicator::Active {
                usage: Some("2m".to_string())
            })
        );
    }

    #[test]
    fn active_goal_status_does_not_count_idle_time_before_turn_start() {
        let observed_at = Instant::now();
        let active_turn_started_at = observed_at + Duration::from_secs(120);
        let state = active_goal_state(observed_at, /*time_used_seconds*/ 60);

        assert_eq!(
            state.indicator(
                active_turn_started_at + Duration::from_secs(60),
                Some(active_turn_started_at),
            ),
            Some(GoalStatusIndicator::Active {
                usage: Some("2m".to_string())
            })
        );
    }

    fn active_goal_state(observed_at: Instant, time_used_seconds: i64) -> GoalStatusState {
        GoalStatusState::new(
            AppThreadGoal {
                thread_id: "thread".to_string(),
                objective: "do the thing".to_string(),
                status: AppThreadGoalStatus::Active,
                token_budget: None,
                tokens_used: 0,
                time_used_seconds,
                created_at: 1,
                updated_at: 1,
            },
            observed_at,
        )
    }
}
