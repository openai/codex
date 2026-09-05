//! Question orchestration retains input buffers and renders pending-question summaries.
use super::*;
use pretty_assertions::assert_eq;
use tokio::sync::mpsc::unbounded_channel;

#[test]
fn questions_flush_buffered_typing_and_record_activity() {
    let (tx, _rx) = unbounded_channel();
    let mut pane = super::tests::test_pane(AppEventSender::new(tx));
    let questions = [codex_protocol::items::AsyncUserInputQuestion {
        title: "Which way?".into(),
        options: None,
    }];
    pane.push_async_questions("message", &questions);
    pane.questions
        .as_mut()
        .unwrap()
        .set_expanded(/*expanded*/ true);
    pane.handle_paste("pasted ".into());
    assert!(pane.last_composer_activity_at.is_some());
    pane.last_composer_activity_at = None;
    for ch in "answer".chars() {
        pane.handle_key_event(KeyEvent::from(KeyCode::Char(ch)));
    }
    assert!(pane.last_composer_activity_at.is_some());
    std::thread::sleep(paste_burst::PasteBurst::recommended_active_flush_delay());
    assert!(pane.flush_paste_burst_if_due());
    assert_eq!(
        pane.questions.as_ref().unwrap().composer.current_text(),
        "pasted answer"
    );
    pane.questions
        .as_mut()
        .unwrap()
        .set_expanded(/*expanded*/ false);
    pane.push_async_questions("next", &questions);
    let now = Instant::now() + Duration::from_secs(15);
    insta::assert_snapshot!(
        "question_collapsed_countdown",
        pane.question_summary(now)
            .unwrap()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    );
    let editor = pane.questions.as_mut().unwrap();
    assert!(editor.countdown(now + Duration::from_secs(30)).is_none());
    editor.set_expanded(/*expanded*/ true);
    editor.append("while-open", &questions);
    assert!(editor.countdown(now).is_none());
}
