use super::*;
use codex_protocol::protocol::MAX_THREAD_GOAL_OBJECTIVE_CHARS;
use codex_protocol::user_input::MAX_USER_INPUT_TEXT_CHARS;
use pretty_assertions::assert_eq;

fn complete_turn_with_message(chat: &mut ChatWidget, turn_id: &str, message: Option<&str>) {
    if let Some(message) = message {
        complete_assistant_message(
            chat,
            &format!("{turn_id}-message"),
            message,
            Some(MessagePhase::FinalAnswer),
        );
    }
    handle_turn_completed(chat, turn_id, /*duration_ms*/ None);
}

fn submit_composer_text(chat: &mut ChatWidget, text: &str) {
    chat.bottom_pane
        .set_composer_text(text.to_string(), Vec::new(), Vec::new());
    submit_current_composer(chat);
}

fn submit_current_composer(chat: &mut ChatWidget) {
    chat.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
}

fn queue_composer_text_with_tab(chat: &mut ChatWidget, text: &str) {
    chat.bottom_pane
        .set_composer_text(text.to_string(), Vec::new(), Vec::new());
    chat.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
}

fn drain_app_events(rx: &mut tokio::sync::mpsc::UnboundedReceiver<AppEvent>) -> Vec<AppEvent> {
    std::iter::from_fn(|| rx.try_recv().ok()).collect()
}

fn rendered_insert_history(events: &[AppEvent]) -> String {
    events
        .iter()
        .filter_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => Some(
                cell.display_lines(/*width*/ 80)
                    .into_iter()
                    .map(|line| line.to_string())
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn goal_slash_command_accepts_objective_at_limit() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Goals, /*enabled*/ true);
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    let objective = "x".repeat(MAX_THREAD_GOAL_OBJECTIVE_CHARS);
    let command = format!("/goal {objective}");

    submit_composer_text(&mut chat, &command);

    let event = rx.try_recv().expect("expected goal objective event");
    let AppEvent::SetThreadGoalObjective {
        thread_id: actual_thread_id,
        objective: actual_objective,
        ..
    } = event
    else {
        panic!("expected SetThreadGoalObjective, got {event:?}");
    };
    assert_eq!(actual_thread_id, thread_id);
    assert_eq!(actual_objective, objective);
    assert_no_submit_op(&mut op_rx);
}

#[tokio::test]
async fn goal_slash_command_accepts_multiline_objective_after_blank_first_line() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Goals, /*enabled*/ true);
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    let objective = "follow these instructions\npreserve this detail";

    submit_composer_text(&mut chat, &format!("/goal \n\n{objective}"));

    let event = rx.try_recv().expect("expected goal objective event");
    let AppEvent::SetThreadGoalObjective {
        thread_id: actual_thread_id,
        objective: actual_objective,
        ..
    } = event
    else {
        panic!("expected SetThreadGoalObjective, got {event:?}");
    };
    assert_eq!(actual_thread_id, thread_id);
    assert_eq!(actual_objective, objective);
    assert_no_submit_op(&mut op_rx);
}

#[tokio::test]
async fn goal_slash_command_rejects_oversized_objective() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Goals, /*enabled*/ true);
    chat.thread_id = Some(ThreadId::new());
    let objective = "x".repeat(MAX_THREAD_GOAL_OBJECTIVE_CHARS + 1);

    submit_composer_text(&mut chat, &format!("/goal {objective}"));

    let events = drain_app_events(&mut rx);
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, AppEvent::SetThreadGoalObjective { .. })),
        "oversized goal should not emit a SetThreadGoalObjective event: {events:?}"
    );
    let rendered = rendered_insert_history(&events);
    assert!(rendered.contains("Goal objective is too long"));
    assert!(rendered.contains("Put longer instructions in a file"));
    assert!(
        !rendered.contains("Message exceeds the maximum length"),
        "expected goal-specific length error, got {rendered:?}"
    );
    assert_no_submit_op(&mut op_rx);
}

#[tokio::test]
async fn goal_slash_command_rejects_large_paste_using_expanded_length() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Goals, /*enabled*/ true);
    chat.thread_id = Some(ThreadId::new());
    chat.bottom_pane
        .set_composer_text("/goal ".to_string(), Vec::new(), Vec::new());
    let objective = "x".repeat(MAX_THREAD_GOAL_OBJECTIVE_CHARS + 1);
    chat.handle_paste(objective);

    assert!(
        chat.bottom_pane.composer_text().contains("[Pasted Content"),
        "expected large paste placeholder in composer"
    );
    submit_current_composer(&mut chat);

    let events = drain_app_events(&mut rx);
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, AppEvent::SetThreadGoalObjective { .. })),
        "oversized pasted goal should not emit a SetThreadGoalObjective event: {events:?}"
    );
    let rendered = rendered_insert_history(&events);
    assert!(rendered.contains("Goal objective is too long"));
    assert!(rendered.contains("Put longer instructions in a file"));
    assert!(
        !rendered.contains("Message exceeds the maximum length"),
        "expected goal-specific length error, got {rendered:?}"
    );
    assert_no_submit_op(&mut op_rx);
}

#[tokio::test]
async fn goal_slash_command_giant_paste_uses_goal_specific_error() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Goals, /*enabled*/ true);
    chat.thread_id = Some(ThreadId::new());
    chat.bottom_pane
        .set_composer_text("/goal ".to_string(), Vec::new(), Vec::new());
    chat.handle_paste("x".repeat(MAX_USER_INPUT_TEXT_CHARS + 1));

    submit_current_composer(&mut chat);

    let events = drain_app_events(&mut rx);
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, AppEvent::SetThreadGoalObjective { .. })),
        "giant pasted goal should not emit a SetThreadGoalObjective event: {events:?}"
    );
    let rendered = rendered_insert_history(&events);
    assert!(rendered.contains("Goal objective is too long"));
    assert!(rendered.contains("Put longer instructions in a file"));
    assert!(
        !rendered.contains("Message exceeds the maximum length"),
        "expected goal-specific length error, got {rendered:?}"
    );
    assert_no_submit_op(&mut op_rx);
}

#[tokio::test]
async fn queued_goal_slash_command_rejects_oversized_objective_and_drains_next_input() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Goals, /*enabled*/ true);
    chat.thread_id = Some(ThreadId::new());
    handle_turn_started(&mut chat, "turn-1");
    let objective = "x".repeat(MAX_THREAD_GOAL_OBJECTIVE_CHARS + 1);

    queue_composer_text_with_tab(&mut chat, &format!("/goal {objective}"));
    queue_composer_text_with_tab(&mut chat, "continue");
    assert_eq!(chat.input_queue.queued_user_messages.len(), 2);

    complete_turn_with_message(&mut chat, "turn-1", Some("done"));

    let events = drain_app_events(&mut rx);
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, AppEvent::SetThreadGoalObjective { .. })),
        "oversized queued goal should not emit a SetThreadGoalObjective event: {events:?}"
    );
    let rendered = rendered_insert_history(&events);
    assert!(rendered.contains("Goal objective is too long"));
    assert!(rendered.contains("Put longer instructions in a file"));
    match next_submit_op(&mut op_rx) {
        Op::UserTurn { items, .. } => assert_eq!(
            items,
            vec![UserInput::Text {
                text: "continue".to_string(),
                text_elements: Vec::new(),
            }]
        ),
        other => panic!("expected queued follow-up after oversized goal, got {other:?}"),
    }
    assert!(chat.input_queue.queued_user_messages.is_empty());
    assert_no_submit_op(&mut op_rx);
}
