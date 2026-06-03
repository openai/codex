use std::collections::HashMap;

#[derive(Debug)]
pub(super) struct GoalTurnParticipationTracker {
    participants: HashMap<String, GoalTurnParticipant>,
}

#[derive(Debug)]
pub(super) struct GoalTurnParticipant {
    pub(super) goal_id: String,
    pub(super) goal_status: codex_state::ThreadGoalStatus,
    pub(super) has_token_budget: bool,
}

impl GoalTurnParticipationTracker {
    pub(super) fn new() -> Self {
        Self {
            participants: HashMap::new(),
        }
    }

    pub(super) fn mark_active_goal(&mut self, goal: &codex_state::ThreadGoal) {
        self.participants
            .entry(goal.goal_id.clone())
            .and_modify(|participant| participant.update_goal(goal))
            .or_insert_with(|| GoalTurnParticipant::new(goal));
    }

    pub(super) fn update_goal(&mut self, goal: &codex_state::ThreadGoal) {
        if let Some(participant) = self.participants.get_mut(goal.goal_id.as_str()) {
            participant.update_goal(goal);
        }
    }

    pub(super) fn into_participants(self) -> Vec<GoalTurnParticipant> {
        self.participants.into_values().collect()
    }
}

impl GoalTurnParticipant {
    fn new(goal: &codex_state::ThreadGoal) -> Self {
        Self {
            goal_id: goal.goal_id.clone(),
            goal_status: goal.status,
            has_token_budget: goal.token_budget.is_some(),
        }
    }

    fn update_goal(&mut self, goal: &codex_state::ThreadGoal) {
        self.goal_status = goal.status;
        self.has_token_budget = goal.token_budget.is_some();
    }
}

#[cfg(test)]
#[path = "turn_participation_tests.rs"]
mod tests;
