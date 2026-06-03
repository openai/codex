use super::GoalTurnParticipationTracker;
use chrono::Utc;
use codex_protocol::ThreadId;
use pretty_assertions::assert_eq;

fn test_goal(
    goal_id: &str,
    status: codex_state::ThreadGoalStatus,
    token_budget: Option<i64>,
) -> codex_state::ThreadGoal {
    let now = Utc::now();
    codex_state::ThreadGoal {
        thread_id: ThreadId::new(),
        goal_id: goal_id.to_string(),
        objective: "test objective".to_string(),
        status,
        token_budget,
        tokens_used: 0,
        time_used_seconds: 0,
        created_at: now,
        updated_at: now,
    }
}

#[test]
fn retains_participant_after_goal_becomes_terminal() {
    let mut goal = test_goal("goal-1", codex_state::ThreadGoalStatus::Active, None);
    let mut tracker = GoalTurnParticipationTracker::new();
    tracker.mark_active_goal(&goal);

    goal.status = codex_state::ThreadGoalStatus::Complete;
    tracker.update_goal(&goal);

    let [participant] = tracker
        .into_participants()
        .try_into()
        .expect("one goal should participate");
    assert_eq!("goal-1", participant.goal_id);
    assert_eq!(
        codex_state::ThreadGoalStatus::Complete,
        participant.goal_status
    );
}

#[test]
fn emits_one_participant_when_same_goal_is_marked_active_repeatedly() {
    let mut goal = test_goal("goal-1", codex_state::ThreadGoalStatus::Active, None);
    let mut tracker = GoalTurnParticipationTracker::new();
    tracker.mark_active_goal(&goal);

    goal.token_budget = Some(1_000);
    tracker.mark_active_goal(&goal);

    let [participant] = tracker
        .into_participants()
        .try_into()
        .expect("one goal should participate");
    assert_eq!("goal-1", participant.goal_id);
    assert!(participant.has_token_budget);
}

#[test]
fn retains_multiple_goal_participants() {
    let first_goal = test_goal("goal-1", codex_state::ThreadGoalStatus::Active, None);
    let second_goal = test_goal("goal-2", codex_state::ThreadGoalStatus::Active, Some(1_000));
    let mut tracker = GoalTurnParticipationTracker::new();

    tracker.mark_active_goal(&first_goal);
    tracker.mark_active_goal(&second_goal);

    let mut participants = tracker.into_participants();
    participants.sort_by(|left, right| left.goal_id.cmp(&right.goal_id));
    assert_eq!(2, participants.len());
    assert_eq!("goal-1", participants[0].goal_id);
    assert!(!participants[0].has_token_budget);
    assert_eq!("goal-2", participants[1].goal_id);
    assert!(participants[1].has_token_budget);
}

#[test]
fn ignores_updates_for_goals_that_did_not_participate() {
    let goal = test_goal("goal-1", codex_state::ThreadGoalStatus::Complete, None);
    let mut tracker = GoalTurnParticipationTracker::new();

    tracker.update_goal(&goal);

    assert!(tracker.into_participants().is_empty());
}
