#![allow(dead_code)]

#[path = "../src/accounting.rs"]
mod accounting;

use accounting::BudgetLimitedGoalDisposition;
use accounting::GoalAccountingState;
use codex_extension_api::ToolCallOutcome;
use codex_extension_api::ToolName;
use codex_protocol::config_types::ModeKind;
use codex_protocol::items::AgentMessageContent;
use codex_protocol::items::AgentMessageItem;
use codex_protocol::items::TurnItem;
use codex_protocol::models::MessagePhase;
use codex_protocol::protocol::TokenUsage;
use codex_state::ThreadGoalStatus;
use pretty_assertions::assert_eq;

#[test]
fn goal_accounting_uses_turn_start_baseline_for_exact_deltas() {
    let state = GoalAccountingState::default();
    state.start_turn(
        "turn-1",
        ModeKind::Default,
        &token_usage(
            /*input_tokens*/ 100, /*cached_input_tokens*/ 10, /*output_tokens*/ 30,
            /*reasoning_output_tokens*/ 5, /*total_tokens*/ 135,
        ),
    );

    let recorded = state
        .record_token_usage(
            "turn-1",
            &token_usage(
                /*input_tokens*/ 120, /*cached_input_tokens*/ 14,
                /*output_tokens*/ 42, /*reasoning_output_tokens*/ 8,
                /*total_tokens*/ 162,
            ),
        )
        .expect("token delta should be recorded");

    assert_eq!(28, recorded.turn_delta);
    assert_eq!(28, recorded.thread_unflushed_delta);
}

#[test]
fn goal_accounting_ignores_plan_mode_turns() {
    let state = GoalAccountingState::default();
    state.start_turn("turn-1", ModeKind::Plan, &TokenUsage::default());

    let recorded = state.record_token_usage(
        "turn-1",
        &token_usage(
            /*input_tokens*/ 20, /*cached_input_tokens*/ 5, /*output_tokens*/ 8,
            /*reasoning_output_tokens*/ 2, /*total_tokens*/ 30,
        ),
    );

    assert_eq!(None, recorded);
}

#[test]
fn empty_continuations_require_three_turns_without_activity_or_goal_changes() {
    let empty_final = TurnItem::AgentMessage(AgentMessageItem {
        id: "empty".into(),
        content: vec![AgentMessageContent::Text { text: " \n".into() }],
        phase: Some(MessagePhase::FinalAnswer),
        memory_citation: None,
        delivery: None,
        questions: None,
    });
    for (interruption, blocking_turn) in [
        ("none", 3),
        ("user", 6),
        ("tool", 6),
        ("goal", 5),
        ("reset", 6),
        ("missing final", 6),
        ("partial answer", 6),
    ] {
        let state = GoalAccountingState::default();
        for turn in 1..=blocking_turn {
            let id = turn.to_string();
            let goal_id = if interruption == "goal" && turn >= 3 {
                "new"
            } else {
                "goal"
            };
            state.start_turn(&id, ModeKind::Default, &TokenUsage::default());
            state.mark_turn_goal_active(&id, goal_id);
            if turn == 3 && interruption == "partial answer" {
                let mut partial = empty_final.clone();
                if let TurnItem::AgentMessage(message) = &mut partial {
                    message.phase = Some(MessagePhase::PartialAnswer);
                }
                state.record_item(&id, &partial);
            } else if !(turn == 3 && interruption == "missing final") {
                state.record_item(&id, &empty_final);
            }
            // Admission can finish after output arrives, but before turn-stop evaluation.
            if !(turn == 3 && interruption == "user") {
                state.mark_goal_continuation(id.clone());
            }
            if turn == 3 && interruption == "tool" {
                state.record_tool_outcome(
                    &id,
                    &ToolName::plain("shell"),
                    ToolCallOutcome::Completed { success: false },
                );
            }
            if turn == 3 && interruption == "reset" {
                state.reset_empty_responses();
            }
            assert_eq!(
                (turn == blocking_turn).then(|| goal_id.to_string()),
                state.empty_response_goal(&id),
                "interruption: {interruption}, turn: {turn}",
            );
            state.finish_turn(&id);
        }
    }
}

#[test]
fn execution_failures_do_not_transfer_to_a_replacement_goal() {
    let state = GoalAccountingState::default();

    for (turn, goal_id, replacement_goal_id) in [
        (1, "first-goal", Some("second-goal")),
        (2, "second-goal", None),
        (3, "second-goal", None),
        (4, "second-goal", None),
    ] {
        let turn_id = format!("turn-{turn}");
        state.start_turn(&turn_id, ModeKind::Default, &TokenUsage::default());
        state.mark_turn_goal_active(&turn_id, goal_id);
        state.record_tool_outcome(
            &turn_id,
            &ToolName::plain("exec"),
            ToolCallOutcome::Failed {
                handler_executed: true,
            },
        );
        if let Some(replacement_goal_id) = replacement_goal_id {
            state.mark_current_turn_goal_active(replacement_goal_id);
        }

        assert_eq!(
            (turn == 4).then(|| "second-goal".to_string()),
            state.execution_failure_goal(&turn_id)
        );
        state.finish_turn(&turn_id);
        if turn == 3 {
            state.reset_idle_progress_baseline_and_clear_active_goal();
        }
    }
}

#[test]
fn script_errors_and_failures_before_execution_do_not_block_goals() {
    for outcome in [
        ToolCallOutcome::Completed { success: false },
        ToolCallOutcome::Failed {
            handler_executed: false,
        },
    ] {
        let state = GoalAccountingState::default();
        for turn in 1..=3 {
            let turn_id = format!("turn-{turn}");
            state.start_turn(&turn_id, ModeKind::Default, &TokenUsage::default());
            state.mark_turn_goal_active(&turn_id, "goal");
            state.record_tool_outcome(&turn_id, &ToolName::plain("exec"), outcome);

            assert_eq!(None, state.execution_failure_goal(&turn_id));
            state.finish_turn(&turn_id);
        }
    }
}

#[test]
fn successful_tool_resets_failures_before_an_interrupted_turn_ends() {
    let state = GoalAccountingState::default();

    for (turn, tool_name, outcome) in [
        (
            1,
            "exec",
            ToolCallOutcome::Failed {
                handler_executed: true,
            },
        ),
        (
            2,
            "exec",
            ToolCallOutcome::Failed {
                handler_executed: true,
            },
        ),
        (3, "shell", ToolCallOutcome::Completed { success: true }),
        (
            4,
            "exec",
            ToolCallOutcome::Failed {
                handler_executed: true,
            },
        ),
    ] {
        let turn_id = format!("turn-{turn}");
        state.start_turn(&turn_id, ModeKind::Default, &TokenUsage::default());
        state.mark_turn_goal_active(&turn_id, "goal");
        state.record_tool_outcome(&turn_id, &ToolName::plain(tool_name), outcome);
        if turn != 3 {
            assert_eq!(None, state.execution_failure_goal(&turn_id));
        }
        state.finish_turn(&turn_id);
    }
}

#[test]
fn goal_accounting_preserves_concurrent_descendant_usage_across_checkpoints() {
    let state = GoalAccountingState::default();
    state.start_turn("turn-1", ModeKind::Default, &TokenUsage::default());
    state.mark_current_turn_goal_active("goal-1");
    let first_usage = token_usage(
        /*input_tokens*/ 20, /*cached_input_tokens*/ 5, /*output_tokens*/ 8,
        /*reasoning_output_tokens*/ 0, /*total_tokens*/ 28,
    );
    let second_usage = token_usage(
        /*input_tokens*/ 8, /*cached_input_tokens*/ 2, /*output_tokens*/ 4,
        /*reasoning_output_tokens*/ 0, /*total_tokens*/ 12,
    );
    std::thread::scope(|scope| {
        scope.spawn(|| state.record_descendant_token_usage(&first_usage));
        scope.spawn(|| state.record_descendant_token_usage(&second_usage));
    });

    let first = state
        .progress_snapshot("turn-1")
        .expect("descendant usage should create a progress snapshot");
    assert_eq!(33, first.token_delta);

    state.record_descendant_token_usage(&token_usage(
        /*input_tokens*/ 6, /*cached_input_tokens*/ 1, /*output_tokens*/ 3,
        /*reasoning_output_tokens*/ 0, /*total_tokens*/ 9,
    ));
    state.mark_progress_accounted_for_status(
        "turn-1",
        &first,
        ThreadGoalStatus::Active,
        BudgetLimitedGoalDisposition::KeepActive,
    );

    let second = state
        .progress_snapshot("turn-1")
        .expect("usage received during accounting should remain pending");
    assert_eq!(8, second.token_delta);
}

fn token_usage(
    input_tokens: i64,
    cached_input_tokens: i64,
    output_tokens: i64,
    reasoning_output_tokens: i64,
    total_tokens: i64,
) -> TokenUsage {
    TokenUsage {
        input_tokens,
        cached_input_tokens,
        cache_write_input_tokens: 0,
        output_tokens,
        reasoning_output_tokens,
        total_tokens,
        codex_rollout_budget_units: None,
    }
}
