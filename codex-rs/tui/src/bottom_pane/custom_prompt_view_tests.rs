use super::*;
use pretty_assertions::assert_eq;
use std::sync::mpsc::Receiver;

#[test]
fn paste_burst_newline_does_not_submit_short_first_line() {
    let now = Instant::now();

    for (first_line, second_line) in [("x", "rest"), ("id", "body"), ("foo", "bar")] {
        let (mut view, submitted_rx) = custom_prompt_view("");
        let mut ms = 0;

        for ch in first_line.chars() {
            view.handle_key_event_at(KeyEvent::from(KeyCode::Char(ch)), now + elapsed(ms));
            ms += 1;
        }
        view.handle_key_event_at(KeyEvent::from(KeyCode::Enter), now + elapsed(ms));
        ms += 1;
        for ch in second_line.chars() {
            view.handle_key_event_at(KeyEvent::from(KeyCode::Char(ch)), now + elapsed(ms));
            ms += 1;
        }

        assert!(submitted_rx.try_recv().is_err());
        assert!(!view.is_complete());

        view.handle_key_event_at(KeyEvent::from(KeyCode::Enter), now + elapsed(200));

        assert_eq!(
            submitted_rx.try_recv(),
            Ok(format!("{first_line}\n{second_line}"))
        );
        assert!(view.is_complete());
    }
}

#[test]
fn delayed_enter_after_typing_submits() {
    let (mut view, submitted_rx) = custom_prompt_view("");
    let now = Instant::now();

    for (idx, ch) in "foo".chars().enumerate() {
        view.handle_key_event_at(KeyEvent::from(KeyCode::Char(ch)), now + elapsed(idx * 20));
    }
    view.handle_key_event_at(KeyEvent::from(KeyCode::Enter), now + elapsed(80));

    assert_eq!(submitted_rx.try_recv(), Ok("foo".to_string()));
    assert!(view.is_complete());
}

fn custom_prompt_view(initial_text: &str) -> (CustomPromptView, Receiver<String>) {
    let (submitted, submitted_rx) = std::sync::mpsc::channel();
    let view = CustomPromptView::new(
        "Edit goal".to_string(),
        "Type a goal objective and press Enter".to_string(),
        initial_text.to_string(),
        /*context_label*/ None,
        Box::new(move |text| {
            submitted.send(text).expect("send submitted text");
        }),
    );
    (view, submitted_rx)
}

fn elapsed(ms: usize) -> std::time::Duration {
    std::time::Duration::from_millis(ms as u64)
}
