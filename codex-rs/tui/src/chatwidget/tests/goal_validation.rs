use super::*;
use codex_protocol::protocol::MAX_THREAD_GOAL_OBJECTIVE_CHARS;
use pretty_assertions::assert_eq;
use std::path::PathBuf;

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

fn goal_file_path(objective: &str) -> PathBuf {
    crate::goal_files::objective_file_path(objective)
        .unwrap_or_else(|| panic!("expected goal file objective, got {objective:?}"))
}

fn path_after_prefix(text: &str, prefix: &str) -> PathBuf {
    let rest = text
        .split_once(prefix)
        .unwrap_or_else(|| panic!("expected {prefix:?} in {text:?}"))
        .1;
    PathBuf::from(rest.lines().next().expect("path line").trim())
}

fn next_goal_objective(
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
    expected_thread_id: ThreadId,
) -> String {
    let event = rx.try_recv().expect("expected goal objective event");
    let AppEvent::SetThreadGoalObjective {
        thread_id,
        objective,
        ..
    } = event
    else {
        panic!("expected SetThreadGoalObjective, got {event:?}");
    };
    assert_eq!(thread_id, expected_thread_id);
    objective
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
async fn goal_slash_command_materializes_oversized_objective() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Goals, /*enabled*/ true);
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    let objective = "x".repeat(MAX_THREAD_GOAL_OBJECTIVE_CHARS + 1);

    submit_composer_text(&mut chat, &format!("/goal {objective}"));

    let actual_objective = next_goal_objective(&mut rx, thread_id);
    let path = goal_file_path(&actual_objective);
    assert_eq!(
        std::fs::read_to_string(path).expect("read goal file"),
        objective
    );
    assert_no_submit_op(&mut op_rx);
}

#[tokio::test]
async fn goal_slash_command_materializes_large_paste() {
    let (mut chat, mut rx, mut op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.set_feature_enabled(Feature::Goals, /*enabled*/ true);
    let thread_id = ThreadId::new();
    chat.thread_id = Some(thread_id);
    chat.bottom_pane
        .set_composer_text("/goal ".to_string(), Vec::new(), Vec::new());
    let objective = "x".repeat(MAX_THREAD_GOAL_OBJECTIVE_CHARS + 1);
    chat.handle_paste(objective.clone());

    assert!(
        chat.bottom_pane.composer_text().contains("[Pasted Content"),
        "expected large paste placeholder in composer"
    );
    submit_current_composer(&mut chat);

    let actual_objective = next_goal_objective(&mut rx, thread_id);
    let path = path_after_prefix(&actual_objective, "pasted text file: ");
    assert_eq!(
        std::fs::read_to_string(path).expect("read paste file"),
        objective
    );
    assert_no_submit_op(&mut op_rx);
}

#[tokio::test]
async fn queued_goal_slash_command_materializes_oversized_objective_and_stops_queue() {
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

    let (actual_thread_id, actual_objective) = loop {
        match rx.try_recv().expect("expected goal objective event") {
            AppEvent::SetThreadGoalObjective {
                thread_id,
                objective,
                ..
            } => break (thread_id, objective),
            _ => continue,
        }
    };
    assert_eq!(actual_thread_id, thread_id);
    let path = goal_file_path(&actual_objective);
    assert_eq!(
        std::fs::read_to_string(path).expect("read goal file"),
        objective
    );
    assert_eq!(chat.input_queue.queued_user_messages.len(), 1);
    assert_no_submit_op(&mut op_rx);
}
