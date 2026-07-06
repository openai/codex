use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;

use super::MAX_STARTUP_INPUT_CHARS;
use super::StartupInputBuffer;

#[test]
fn startup_input_keeps_text_without_replaying_actions() {
    let mut input = StartupInputBuffer::default();
    for event in [
        Event::Key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE)),
        Event::Key(KeyEvent::new(KeyCode::Char('I'), KeyModifiers::SHIFT)),
        Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        Event::Key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE)),
        Event::Paste("ello\r\n\tworld".to_string()),
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        Event::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
        Event::Key(KeyEvent::new_with_kind(
            KeyCode::Char('!'),
            KeyModifiers::NONE,
            KeyEventKind::Release,
        )),
    ] {
        input.handle_event(event);
    }

    assert_eq!(input.into_text(), Some("hello\n\tworld".to_string()));
}

#[test]
fn startup_input_preserves_internal_plain_whitespace_but_drops_trailing_actions() {
    let mut input = StartupInputBuffer::default();
    for code in [
        KeyCode::Char('a'),
        KeyCode::Enter,
        KeyCode::Char('b'),
        KeyCode::Tab,
        KeyCode::Char('c'),
        KeyCode::Enter,
    ] {
        input.handle_event(Event::Key(KeyEvent::new(code, KeyModifiers::NONE)));
    }

    assert_eq!(input.into_text(), Some("a\nb\tc".to_string()));
}

#[test]
fn startup_probe_input_preserves_internal_plain_whitespace_across_phases() {
    let mut input = StartupInputBuffer::default();
    input.handle_probe_input(b"a\r\n");
    input.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Char('b'),
        KeyModifiers::NONE,
    )));
    input.handle_probe_input(b"\t");

    assert_eq!(input.into_text(), Some("a\nb".to_string()));
}

#[test]
fn startup_input_is_bounded() {
    let mut input = StartupInputBuffer::default();
    input.handle_event(Event::Paste("x".repeat(MAX_STARTUP_INPUT_CHARS + 1)));

    assert_eq!(input.into_text(), Some("x".repeat(MAX_STARTUP_INPUT_CHARS)));
}

#[test]
fn startup_input_applies_edits_across_capture_phases() {
    let mut input = StartupInputBuffer::default();
    input.handle_event(Event::Paste("draft".to_string()));

    input.handle_probe_input(b"\x7f\x7fph");
    input.handle_event(Event::Key(KeyEvent::new(
        KeyCode::Backspace,
        KeyModifiers::NONE,
    )));

    assert_eq!(input.into_text(), Some("drap".to_string()));
}
