//! Pointer editing follows rendered rows and preserves atomic text boundaries.

use super::*;
use crossterm::event::MouseButton::Left;
use crossterm::event::MouseEventKind::Down;
use crossterm::event::MouseEventKind::Drag;
use crossterm::event::MouseEventKind::Up;
use pretty_assertions::assert_eq;
use ratatui::style::Modifier;

fn render(t: &TextArea, area: Rect, state: &mut TextAreaState) -> Buffer {
    let mut buffer = Buffer::empty(area);
    StatefulWidgetRef::render_ref(&t, area, &mut buffer, state);
    buffer
}

fn mouse(t: &mut TextArea, state: TextAreaState, kind: MouseEventKind, x: u16, y: u16) {
    assert!(t.handle_mouse(
        MouseEvent {
            kind,
            column: x,
            row: y,
            modifiers: KeyModifiers::NONE,
        },
        state
    ));
}

#[test]
fn mouse_click_maps_wrapping_unicode_tabs_and_empty_lines() {
    let area = Rect::new(
        /*x*/ 3, /*y*/ 2, /*width*/ 8, /*height*/ 8,
    );
    let mut t = TextArea::new();
    let text = "ab 界e\u{301}👩‍💻\n\nx\ty";
    t.insert_str(text);
    let mut state = TextAreaState::default();
    render(&t, area, &mut state);
    for (pos, _) in text
        .grapheme_indices(/*is_extended*/ true)
        .chain(std::iter::once((text.len(), "")))
    {
        t.set_cursor(pos);
        let (x, y) = t.cursor_pos_with_state(area, state).unwrap();
        mouse(&mut t, state, Down(Left), x, y);
        assert_eq!(t.cursor(), pos);
    }
    // Both cells of a wide grapheme resolve to its leading insertion boundary.
    mouse(&mut t, state, Down(Left), /*x*/ 7, /*y*/ 2);
    t.insert_str("!");
    assert_eq!(t.text(), "ab !界e\u{301}👩‍💻\n\nx\ty");
}

#[test]
fn mouse_drag_highlights_and_edits_wrapped_selection() {
    let area = Rect::new(
        /*x*/ 0, /*y*/ 1, /*width*/ 8, /*height*/ 1,
    );
    let backward_word = KeyEvent::new(KeyCode::Backspace, KeyModifiers::CONTROL);
    let forward_word = KeyEvent::new(KeyCode::Delete, KeyModifiers::CONTROL);
    let kill_line = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL);
    for (start, end, key, expected) in [
        ((1, 1), (3, 2), KeyCode::Char('X').into(), ("hXld!", 2)),
        ((3, 1), (1, 0), KeyCode::Backspace.into(), ("hld!", 1)),
        ((1, 1), (3, 2), KeyCode::Delete.into(), ("hld!", 1)),
        ((3, 1), (1, 0), backward_word, ("hld!", 1)),
        ((1, 1), (3, 2), forward_word, ("hld!", 1)),
        ((1, 1), (3, 2), kill_line, ("hld!", 1)),
    ] {
        let mut t = TextArea::new();
        t.insert_str("hello world");
        if key.modifiers.is_empty() {
            t.set_vim_enabled(/*enabled*/ true);
            t.input(KeyCode::Char('A').into());
            t.input(KeyCode::Char('!').into());
            t.input(KeyCode::Esc.into());
            t.input(KeyCode::Char('R').into());
        } else {
            t.insert_str("!");
        }
        if end.1 > start.1 {
            t.set_cursor(/*pos*/ 0);
        }
        let mut state = TextAreaState::default();
        render(&t, area, &mut state);
        mouse(&mut t, state, Down(Left), start.0, start.1);
        mouse(&mut t, state, Drag(Left), end.0, end.1);
        mouse(&mut t, state, Up(Left), end.0, end.1);
        render(&t, area, &mut state);
        assert_eq!(state.scroll, u16::from(end.1 > start.1));
        let buffer = render(&t, Rect { height: 2, ..area }, &mut state);
        let rows = buffer
            .content
            .chunks(usize::from(area.width))
            .map(|row| {
                row.iter()
                    .map(|cell| {
                        if cell.modifier.contains(Modifier::REVERSED) {
                            cell.symbol()
                        } else {
                            "·"
                        }
                    })
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        insta::assert_snapshot!("mouse_wrapped_selection", rows);
        t.input(key);
        assert_eq!((t.text(), t.cursor()), expected);
        if key.modifiers == KeyModifiers::CONTROL {
            t.input(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL));
            assert_eq!(t.text(), "hello world!");
        } else {
            t.input(KeyCode::Esc.into());
            assert!(t.vim_repeat_actions().is_none());
        }
    }
}

#[test]
fn mouse_multiclick_preserves_the_initial_word_or_wrapped_line_when_reversing() {
    for (clicks, selected, backward) in
        [(2, "two", "one two"), (3, "one two six\n", "one two six\n")]
    {
        let mut t = TextArea::new();
        t.insert_str("one two six\nnext");
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 8, /*height*/ 4,
        );
        let mut state = TextAreaState::default();
        render(&t, area, &mut state);
        // Click timing is shared and tested by the transcript; exercise the editor's unit mapping.
        mouse(&mut t, state, Down(Left), /*x*/ 5, /*y*/ 0);
        t.last_click = Some((std::time::Instant::now(), 5, 0, clicks - 1));
        mouse(&mut t, state, Down(Left), /*x*/ 5, /*y*/ 0);
        assert_eq!(&t.text()[t.mouse_selection_range().unwrap()], selected);
        mouse(&mut t, state, Drag(Left), /*x*/ 1, /*y*/ 0);
        mouse(&mut t, state, Up(Left), /*x*/ 1, /*y*/ 0);
        assert_eq!(&t.text()[t.mouse_selection_range().unwrap()], backward);
    }
}
