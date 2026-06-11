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

#[tokio::test]
async fn goal_slash_command_emits_only_inserted_paste_text_element() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Goals, /*enabled*/ true);
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    let paste = "x".repeat(MAX_THREAD_GOAL_OBJECTIVE_CHARS + 1);
    let placeholder = format!("[Pasted Content {} chars]", paste.chars().count());
    chat.bottom_pane.set_composer_text(
        format!("/goal keep literal {placeholder} and "),
        Vec::new(),
        Vec::new(),
    );
    chat.handle_paste(paste.clone());

    submit_current_composer(&mut chat);

    let draft = next_goal_draft(&mut rx, thread_id);
    assert!(
        draft
            .objective
            .contains(&format!("keep literal {placeholder} and {placeholder}")),
        "expected literal placeholder and inserted paste placeholder, got {:?}",
        draft.objective
    );
    assert_eq!(draft.pending_pastes, vec![(placeholder, paste)]);
    assert_no_submit_op(&mut op_rx);
}

#[tokio::test]
async fn queued_goal_before_thread_preserves_large_paste() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Goals, /*enabled*/ true);
    chat.bottom_pane
        .set_composer_text("/goal ".to_string(), Vec::new(), Vec::new());
    let objective = "x".repeat(MAX_THREAD_GOAL_OBJECTIVE_CHARS + 1);
    chat.handle_paste(objective.clone());

    submit_current_composer(&mut chat);
    assert_eq!(chat.input_queue.queued_user_messages.len(), 1);
    assert_no_submit_op(&mut op_rx);

    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    chat.maybe_send_next_queued_input();

    let draft = next_goal_draft(&mut rx, thread_id);
    assert_eq!(draft.objective, objective);
    assert!(draft.pending_pastes.is_empty());
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

    let draft = next_goal_draft(&mut rx, thread_id);
    assert_eq!(draft.objective, objective);
    assert_eq!(chat.input_queue.queued_user_messages.len(), 1);
    assert_no_submit_op(&mut op_rx);
}
