use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn goal_menu_active_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();

    chat.open_goal_menu(
        thread_id,
        test_goal(
            thread_id,
            AppThreadGoalStatus::Active,
            /*token_budget*/ Some(80_000),
        ),
    );

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert_chatwidget_snapshot!("goal_menu_active", popup);
}

#[tokio::test]
async fn goal_menu_paused_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();

    chat.open_goal_menu(
        thread_id,
        test_goal(
            thread_id,
            AppThreadGoalStatus::Paused,
            /*token_budget*/ None,
        ),
    );

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert_chatwidget_snapshot!("goal_menu_paused", popup);
}

#[tokio::test]
async fn goal_menu_budget_limited_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();

    chat.open_goal_menu(
        thread_id,
        test_goal(
            thread_id,
            AppThreadGoalStatus::BudgetLimited,
            /*token_budget*/ Some(80_000),
        ),
    );

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert_chatwidget_snapshot!("goal_menu_budget_limited", popup);
}

#[tokio::test]
async fn goal_menu_active_enter_pauses_goal() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();
    chat.open_goal_menu(
        thread_id,
        test_goal(
            thread_id,
            AppThreadGoalStatus::Active,
            /*token_budget*/ Some(80_000),
        ),
    );

    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    let event = rx.try_recv().expect("expected goal status event");
    let AppEvent::SetThreadGoalStatus {
        thread_id: actual_thread_id,
        status,
    } = event
    else {
        panic!("expected SetThreadGoalStatus, got {event:?}");
    };
    assert_eq!(actual_thread_id, thread_id);
    assert_eq!(status, AppThreadGoalStatus::Paused);
}

#[tokio::test]
async fn goal_menu_paused_enter_unpauses_goal() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();
    chat.open_goal_menu(
        thread_id,
        test_goal(
            thread_id,
            AppThreadGoalStatus::Paused,
            /*token_budget*/ None,
        ),
    );

    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    let event = rx.try_recv().expect("expected goal status event");
    let AppEvent::SetThreadGoalStatus {
        thread_id: actual_thread_id,
        status,
    } = event
    else {
        panic!("expected SetThreadGoalStatus, got {event:?}");
    };
    assert_eq!(actual_thread_id, thread_id);
    assert_eq!(status, AppThreadGoalStatus::Active);
}

#[tokio::test]
async fn goal_menu_clear_row_emits_clear_goal() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();
    chat.open_goal_menu(
        thread_id,
        test_goal(
            thread_id,
            AppThreadGoalStatus::Active,
            /*token_budget*/ Some(80_000),
        ),
    );

    chat.handle_key_event(KeyEvent::from(KeyCode::Down));
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    let event = rx.try_recv().expect("expected clear goal event");
    let AppEvent::ClearThreadGoal {
        thread_id: actual_thread_id,
    } = event
    else {
        panic!("expected ClearThreadGoal, got {event:?}");
    };
    assert_eq!(actual_thread_id, thread_id);
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
