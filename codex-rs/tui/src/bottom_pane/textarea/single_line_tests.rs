use super::*;
use pretty_assertions::assert_eq;

#[test]
fn single_line_vim_newline_commands_are_noops_and_yanks_stay_inline() {
    let mut input = TextArea::new_single_line();
    input.set_vim_enabled(/*enabled*/ true);
    input.set_text_clearing_elements("one\r\ntwo");
    assert_eq!(input.text(), "onetwo");
    input.set_text_clearing_elements("");
    input.enter_vim_insert_mode();
    input.insert_str("word");
    for event in [
        KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL),
        KeyEvent::new(KeyCode::Char('m'), KeyModifiers::CONTROL),
        KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT),
    ] {
        input.input(event);
    }
    assert_eq!(input.text(), "word");
    input.input(KeyCode::Esc.into());
    input.input(KeyCode::Char('0').into());
    input.input(KeyCode::Char('.').into());
    assert_eq!(input.text(), "wordword");
    let cursor = input.cursor_pos;
    for code in ['o', 'O'] {
        input.input(KeyCode::Char(code).into());
        assert_eq!(
            (input.text(), input.cursor_pos, input.is_vim_normal_mode()),
            ("wordword", cursor, true)
        );
    }
    input.input(KeyCode::Char('r').into());
    input.input(KeyCode::Enter.into());
    assert_eq!((input.text(), input.cursor_pos), ("wordword", cursor));
    for code in ['y', 'y', 'p'] {
        input.input(KeyCode::Char(code).into());
    }
    assert_eq!(input.text(), "wordwordwordword");
}
