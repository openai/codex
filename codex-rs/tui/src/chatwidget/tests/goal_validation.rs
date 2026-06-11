use super::*;
use codex_protocol::protocol::MAX_THREAD_GOAL_OBJECTIVE_CHARS;
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

fn next_goal_objective(
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
    expected_thread_id: ThreadId,
) -> String {
    loop {
        let event = rx.try_recv().expect("expected goal objective event");
        if let AppEvent::SetThreadGoalObjective {
            thread_id,
            objective,
            ..
        } = event
        {
            assert_eq!(thread_id, expected_thread_id);
            return objective;
        }
    }
}

#[tokio::test]
async fn goal_slash_command_accepts_objective_at_limit() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Goals, /*enabled*/ true);
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    let objective = "x".repeat(MAX_THREAD_GOAL_OBJECTIVE_CHARS);

    submit_composer_text(&mut chat, &format!("/goal {objective}"));

    assert_eq!(next_goal_objective(&mut rx, thread_id), objective);
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

    assert_eq!(next_goal_objective(&mut rx, thread_id), objective);
    assert_no_submit_op(&mut op_rx);
}

#[tokio::test]
async fn goal_slash_command_emits_oversized_objective() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Goals, /*enabled*/ true);
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    let objective = "x".repeat(MAX_THREAD_GOAL_OBJECTIVE_CHARS + 1);

    submit_composer_text(&mut chat, &format!("/goal {objective}"));

    assert_eq!(next_goal_objective(&mut rx, thread_id), objective);
    assert_no_submit_op(&mut op_rx);
}

#[tokio::test]
async fn goal_slash_command_expands_large_pasted_objective() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Goals, /*enabled*/ true);
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    let objective = "x".repeat(MAX_THREAD_GOAL_OBJECTIVE_CHARS + 1);
    chat.bottom_pane
        .set_composer_text("/goal ".to_string(), Vec::new(), Vec::new());
    chat.handle_paste(objective.clone());

    assert!(
        chat.bottom_pane.composer_text().contains("[Pasted Content"),
        "expected large paste placeholder in composer"
    );
    submit_current_composer(&mut chat);

    assert_eq!(next_goal_objective(&mut rx, thread_id), objective);
    assert_no_submit_op(&mut op_rx);
}

#[tokio::test]
async fn queued_goal_slash_command_emits_oversized_objective_and_stops_queue() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Goals, /*enabled*/ true);
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    handle_turn_started(&mut chat, "turn-1");
    let objective = "x".repeat(MAX_THREAD_GOAL_OBJECTIVE_CHARS + 1);

    queue_composer_text_with_tab(&mut chat, &format!("/goal {objective}"));
    queue_composer_text_with_tab(&mut chat, "continue");
    assert_eq!(chat.input_queue.queued_user_messages.len(), 2);

    complete_turn_with_message(&mut chat, "turn-1", Some("done"));

    assert_eq!(next_goal_objective(&mut rx, thread_id), objective);
    assert_eq!(chat.input_queue.queued_user_messages.len(), 1);
    assert_no_submit_op(&mut op_rx);
}
