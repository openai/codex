use super::*;

#[tokio::test]
async fn goal_menu_active_snapshot() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();

    chat.show_goal_summary(test_goal(
        thread_id,
        AppThreadGoalStatus::Active,
        /*token_budget*/ Some(80_000),
    ));

    assert_chatwidget_snapshot!("goal_menu_active", rendered_goal_summary(&mut rx));
}

#[tokio::test]
async fn goal_menu_paused_snapshot() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();

    chat.show_goal_summary(test_goal(
        thread_id,
        AppThreadGoalStatus::Paused,
        /*token_budget*/ None,
    ));

    assert_chatwidget_snapshot!("goal_menu_paused", rendered_goal_summary(&mut rx));
}

#[tokio::test]
async fn goal_menu_budget_limited_snapshot() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();

    chat.show_goal_summary(test_goal(
        thread_id,
        AppThreadGoalStatus::BudgetLimited,
        /*token_budget*/ Some(80_000),
    ));

    assert_chatwidget_snapshot!("goal_menu_budget_limited", rendered_goal_summary(&mut rx));
}

fn test_goal(
    thread_id: ThreadId,
    status: AppThreadGoalStatus,
    token_budget: Option<i64>,
) -> AppThreadGoal {
    AppThreadGoal {
        thread_id: thread_id.to_string(),
        objective: "Keep improving the bare goal command until it feels calm and useful."
            .to_string(),
        status,
        token_budget,
        tokens_used: 12_500,
        time_used_seconds: 90,
        created_at: 1_776_272_400,
        updated_at: 1_776_272_460,
    }
}

fn rendered_goal_summary(
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<crate::app_event::AppEvent>,
) -> String {
    drain_insert_history(rx)
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<Vec<_>>()
        .join("\n")
}
