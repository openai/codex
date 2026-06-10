use codex_analytics::AnalyticsEventsClient;
use codex_analytics::CodexGoalEvent;
use codex_analytics::GoalEventKind;

#[derive(Clone)]
pub(crate) struct GoalAnalytics {
    client: AnalyticsEventsClient,
}

pub(crate) enum GoalEventAttribution<'a> {
    Turn(&'a str),
    NoTurn,
}

impl GoalAnalytics {
    pub(crate) fn new(client: AnalyticsEventsClient) -> Self {
        Self { client }
    }

    pub(crate) fn created(
        &self,
        goal: &codex_state::ThreadGoal,
        attribution: GoalEventAttribution<'_>,
    ) {
        self.track(goal, attribution, GoalEventKind::Created);
    }

    pub(crate) fn usage_accounted(
        &self,
        goal: &codex_state::ThreadGoal,
        attribution: GoalEventAttribution<'_>,
    ) {
        self.track(goal, attribution, GoalEventKind::UsageAccounted);
    }

    pub(crate) fn status_changed(
        &self,
        goal: &codex_state::ThreadGoal,
        previous_status: Option<codex_state::ThreadGoalStatus>,
        attribution: GoalEventAttribution<'_>,
    ) {
        if previous_status.is_some_and(|status| status != goal.status) {
            self.track(goal, attribution, GoalEventKind::StatusChanged);
        }
    }

    pub(crate) fn cleared(&self, goal: &codex_state::ThreadGoal) {
        self.track(goal, GoalEventAttribution::NoTurn, GoalEventKind::Cleared);
    }

    fn track(
        &self,
        goal: &codex_state::ThreadGoal,
        attribution: GoalEventAttribution<'_>,
        event_kind: GoalEventKind,
    ) {
        let (cumulative_tokens_accounted, cumulative_time_accounted_seconds) = match event_kind {
            GoalEventKind::UsageAccounted => (Some(goal.tokens_used), Some(goal.time_used_seconds)),
            GoalEventKind::Created | GoalEventKind::StatusChanged | GoalEventKind::Cleared => {
                (None, None)
            }
        };
        self.client.track_goal_event(CodexGoalEvent {
            thread_id: goal.thread_id.to_string(),
            turn_id: match attribution {
                GoalEventAttribution::Turn(turn_id) => Some(turn_id.to_string()),
                GoalEventAttribution::NoTurn => None,
            },
            goal_id: goal.goal_id.clone(),
            event_kind,
            goal_status: goal.status,
            has_token_budget: goal.token_budget.is_some(),
            cumulative_tokens_accounted,
            cumulative_time_accounted_seconds,
        });
    }
}
