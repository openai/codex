use super::*;
use crossterm::event::MouseButton;
use crossterm::event::MouseEvent;
use crossterm::event::MouseEventKind;
use pretty_assertions::assert_eq;

#[test]
fn target_replaces_forward_and_backward_selections() {
    for (anchor, cursor) in [(1, 4), (4, 1)] {
        let mut textarea = TextArea::new();
        textarea.insert_str("abcdef");
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 20, /*height*/ 2,
        );
        let mut state = TextAreaState::default();
        StatefulWidgetRef::render_ref(&&textarea, area, &mut Buffer::empty(area), &mut state);
        for (kind, column) in [
            (MouseEventKind::Down(MouseButton::Left), anchor),
            (MouseEventKind::Drag(MouseButton::Left), cursor),
        ] {
            assert!(textarea.handle_mouse(
                MouseEvent {
                    kind,
                    column,
                    row: 0,
                    modifiers: KeyModifiers::NONE
                },
                state,
            ));
        }
        let target = textarea.edit_target();
        assert_eq!(&textarea.text()[..target.start()], "a");
        textarea.insert_str_at_target(target, "界");
        assert_eq!((textarea.text(), textarea.cursor()), ("a界ef", "a界".len()));
        assert_eq!(textarea.mouse_selection_range(), None);
    }
}

#[test]
fn target_element_insertion_preserves_adjacent_elements() {
    let mut textarea = TextArea::new();
    textarea.insert_str("before ");
    textarea.insert_element("[old]");
    textarea.insert_str(" after");
    textarea.set_cursor("before ".len());
    let target = textarea.edit_target();
    textarea.insert_element_at_target(target, "[new]");
    assert_eq!(
        (
            textarea.text(),
            textarea.cursor(),
            textarea.element_payloads()
        ),
        (
            "before [new][old] after",
            "before [new]".len(),
            vec!["[new]".to_string(), "[old]".to_string()]
        ),
    );
}

#[test]
fn target_text_preserves_vim_replace_and_backspace_recovery() {
    let mut textarea = TextArea::new();
    textarea.insert_str("abc");
    textarea.set_cursor(/*pos*/ 0);
    textarea.set_vim_enabled(/*enabled*/ true);
    textarea.input(KeyCode::Char('R').into());
    let target = textarea.edit_target();
    textarea.insert_str_at_target(target, "界");
    assert_eq!((textarea.text(), textarea.cursor()), ("界bc", "界".len()));
    textarea.input(KeyCode::Backspace.into());
    assert_eq!((textarea.text(), textarea.cursor()), ("abc", 0));
}
