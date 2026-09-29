use super::super::tests::new_test_composer;
use super::super::tests::snapshot_composer_state_with_width;
use super::super::*;
use crossterm::event::MouseEvent;
use crossterm::event::MouseEventKind;
use pretty_assertions::assert_eq;

#[test]
fn blockquote_paste_continues_current_line_prefix() {
    for (draft, pasted, expected) in [
        ("> ", "first\nsecond", "> first\n> second\n\n"),
        ("intro\n> ", "α\r\n\r\nβ\r", "intro\n> α\n> \n> β\n> \n\n"),
        (
            "> existing ",
            "first\nsecond",
            "> existing first\n> second\n\n",
        ),
        ("text > ", "first\nsecond", "text > first\nsecond"),
        (">", "first\nsecond", ">first\nsecond"),
        (
            "> quote\nplain ",
            "first\nsecond",
            "> quote\nplain first\nsecond",
        ),
        ("> ", "single", "> single"),
        ("!> ", "out\npwd", "!> out\npwd"),
        (" !echo hi\n> ", "out\npwd", " !echo hi\n> out\npwd"),
    ] {
        let (mut composer, _rx) = new_test_composer();
        composer.insert_str(draft);
        composer.handle_paste(pasted.to_string());
        assert_eq!(composer.current_text(), expected);
    }
}

#[test]
fn blockquote_paste_uses_cursor_line_and_preserves_suffix() {
    let (mut composer, _rx) = new_test_composer();
    composer.insert_str("intro\n> suffix\nend");
    composer.draft.textarea.set_cursor("intro\n> ".len());
    composer.handle_paste("first\nsecond".to_string());
    assert_eq!(
        composer.draft.textarea.text(),
        "intro\n> first\n> second\n\nsuffix\nend"
    );
    composer.insert_str("next ");
    assert_eq!(
        composer.current_text(),
        "intro\n> first\n> second\n\nnext suffix\nend"
    );
}

#[test]
fn blockquote_paste_large_expands_quoted_text_on_submit() {
    let (mut composer, _rx) = new_test_composer();
    composer.insert_str("> ");
    let first = "x".repeat(LARGE_PASTE_CHAR_THRESHOLD);
    composer.handle_paste(format!("{first}\nsecond"));
    let quoted = format!("{first}\n> second");
    let placeholder = format!("[Pasted Content {} chars]", quoted.chars().count());
    assert_eq!(
        composer.draft.textarea.text(),
        format!("> {placeholder}\n\n")
    );
    assert_eq!(
        composer.draft.pending_pastes,
        vec![(placeholder, quoted.clone())]
    );
    composer.insert_str("Next block");
    let (result, _) = composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    let InputResult::Submitted { text, .. } = result else {
        panic!("expected Submitted");
    };
    assert_eq!(text, format!("> {quoted}\n\nNext block"));
}

#[test]
fn blockquote_paste_rendered() {
    snapshot_composer_state_with_width(
        "blockquote_paste",
        /*width*/ 60,
        /*enhanced_keys_supported*/ false,
        |composer| {
            composer.insert_str("Please explain:\n> ");
            composer.handle_paste("first line\n\nlast line".to_string());
            composer.insert_str("Next block");
        },
    );
}

#[test]
fn blockquote_paste_replacing_selected_prefix_stays_literal() {
    let (mut composer, _rx) = new_test_composer();
    composer.insert_str("> selected");
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 10,
    );
    composer.render(area, &mut Buffer::empty(area));
    let (x, y) = composer.cursor_pos(area).unwrap();
    for (kind, column) in [
        (
            MouseEventKind::Down(crossterm::event::MouseButton::Left),
            x - 10,
        ),
        (MouseEventKind::Drag(crossterm::event::MouseButton::Left), x),
    ] {
        let event = MouseEvent {
            kind,
            column,
            row: y,
            modifiers: KeyModifiers::NONE,
        };
        assert!(composer.prepare_mouse(event));
        composer.render(area, &mut Buffer::empty(area));
        assert!(composer.handle_mouse(event));
    }
    composer.handle_paste("first\nsecond".to_string());
    assert_eq!(composer.current_text(), "first\nsecond");
}

#[test]
fn blockquote_paste_embedded_answer_starts_next_block() {
    let (mut composer, _rx) = new_test_composer();
    composer.config = ChatComposerConfig::plain_text();
    composer.insert_str("> ");
    composer.handle_paste("first\nsecond".to_string());
    composer.insert_str("Next block");
    assert_eq!(composer.current_text(), "> first\n> second\n\nNext block");
}

#[test]
fn blockquote_paste_embedded_answer_rendered() {
    snapshot_composer_state_with_width(
        "blockquote_paste_embedded_answer",
        /*width*/ 60,
        /*enhanced_keys_supported*/ false,
        |composer| {
            composer.config = ChatComposerConfig::plain_text();
            composer.insert_str("> ");
            composer.handle_paste("first line\n\nlast line".to_string());
            composer.insert_str("Next block");
        },
    );
}
