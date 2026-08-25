use super::super::TextArea;
use super::VimAction;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;

fn vim_textarea(text: &str, cursor: usize) -> TextArea {
    let mut textarea = TextArea::new();
    textarea.insert_str(text);
    textarea.set_cursor(cursor);
    textarea.set_vim_enabled(/*enabled*/ true);
    textarea
}

fn keys(textarea: &mut TextArea, keys: &str) {
    for key in keys.chars() {
        let code = if key == '\n' {
            KeyCode::Enter
        } else {
            KeyCode::Char(key)
        };
        textarea.input(KeyEvent::new(code, KeyModifiers::NONE));
    }
}

fn escape(textarea: &mut TextArea) {
    textarea.input(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
}

#[test]
fn replace_character_preserves_normal_mode_and_grapheme_boundaries() {
    let mut textarea = vim_textarea("a👩‍💻c", /*cursor*/ 1);
    keys(&mut textarea, "rZ");
    assert_eq!(textarea.text(), "aZc");
    assert_eq!(textarea.cursor(), 1);
    assert_eq!(textarea.vim_mode_label(), Some("Normal"));

    keys(&mut textarea, "r");
    assert_eq!(
        textarea.keymap_context(),
        crate::keymap::KeymapContext::Editor
    );
    escape(&mut textarea);
    assert_eq!(textarea.text(), "aZc");

    keys(&mut textarea, "r\n");
    assert_eq!(textarea.text(), "a\nc");
    assert_eq!(textarea.cursor(), 2);

    #[cfg(windows)]
    {
        keys(&mut textarea, "r");
        textarea.input(KeyEvent::new(
            KeyCode::Char('@'),
            KeyModifiers::CONTROL | KeyModifiers::ALT,
        ));
        assert_eq!(textarea.text(), "a\n@");
    }
}

#[test]
fn repeat_replays_delete_replace_and_complete_change() {
    let mut textarea = vim_textarea("alpha beta gamma", /*cursor*/ 0);
    keys(&mut textarea, "dw.");
    assert_eq!(textarea.text(), "gamma");

    let mut textarea = vim_textarea("abc", /*cursor*/ 0);
    keys(&mut textarea, "rXl.");
    assert_eq!(textarea.text(), "XXc");

    let mut textarea = vim_textarea("one two three", /*cursor*/ 0);
    keys(&mut textarea, "cwX");
    escape(&mut textarea);
    keys(&mut textarea, "w.");
    assert_eq!(textarea.text(), "X X three");
    assert_eq!(textarea.vim_mode_label(), Some("Normal"));
}

#[test]
fn repeat_records_pasted_insertions_and_survives_keymap_changes() {
    let mut textarea = vim_textarea("", /*cursor*/ 0);
    keys(&mut textarea, "i");
    textarea.insert_str("foo");
    escape(&mut textarea);
    keys(&mut textarea, ".");
    assert_eq!(textarea.text(), "fofooo");

    let mut textarea = vim_textarea("one two three", /*cursor*/ 0);
    keys(&mut textarea, "dw");
    let mut keymap = crate::keymap::RuntimeKeymap::defaults();
    keymap.vim_normal.start_delete_operator = vec![crate::key_hint::plain(KeyCode::Char('z'))];
    textarea.set_keymap_bindings(&keymap);
    keys(&mut textarea, ".");
    assert_eq!(textarea.text(), "three");
}

#[test]
fn repeat_aborts_when_change_motion_cannot_start() {
    let mut textarea = vim_textarea("one\ntwo\nthree", /*cursor*/ 0);
    keys(&mut textarea, "cjfoo");
    escape(&mut textarea);
    keys(&mut textarea, "j");
    let original = (textarea.text().to_owned(), textarea.cursor());

    keys(&mut textarea, ".");

    assert_eq!(
        (textarea.text(), textarea.cursor()),
        (original.0.as_str(), original.1)
    );
    assert_eq!(textarea.vim_mode_label(), Some("Normal"));
}

#[test]
fn repeat_distinguishes_insert_mode_deletions_from_noops() {
    let mut textarea = vim_textarea("abcd", /*cursor*/ 3);
    keys(&mut textarea, "i");
    textarea.input(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
    escape(&mut textarea);
    keys(&mut textarea, "l.");
    assert_eq!(textarea.text(), "ad");

    let mut textarea = vim_textarea("abc", /*cursor*/ 0);
    keys(&mut textarea, "xi");
    textarea.input(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
    escape(&mut textarea);
    keys(&mut textarea, ".");
    assert_eq!(textarea.text(), "c");

    let mut textarea = vim_textarea("one two", /*cursor*/ 0);
    keys(&mut textarea, "i");
    textarea.input(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
    keys(&mut textarea, "X");
    escape(&mut textarea);
    keys(&mut textarea, "w.");
    assert_eq!(textarea.text(), "Xone Xtwo");
}

#[test]
fn repeat_omits_ineffective_deletions_before_inserted_text() {
    for (action, cursor, repeat_cursor, expected) in [
        (VimAction::DeleteBackward, 0, 5, "Xone Xtwo\n"),
        (VimAction::DeleteBackwardWord, 0, 5, "Xone Xtwo\n"),
        (VimAction::KillLineStart, 0, 5, "Xone Xtwo\n"),
        (VimAction::DeleteForward, 8, 4, "one Xtwo\nX"),
        (VimAction::DeleteForwardWord, 8, 4, "one Xtwo\nX"),
        (VimAction::KillLineEnd, 8, 4, "one Xtwo\nX"),
        (VimAction::KillLine, 8, 4, "one Xtwo\nX"),
    ] {
        let mut textarea = vim_textarea("one two\n", cursor);
        keys(&mut textarea, "i");
        textarea.apply_vim_insert_action(action);
        keys(&mut textarea, "X");
        escape(&mut textarea);
        textarea.set_cursor(repeat_cursor);

        keys(&mut textarea, ".");

        assert_eq!(textarea.text(), expected, "{action:?}");
    }
}

#[test]
fn repeat_replays_resolved_insert_actions_after_editor_keymap_changes() {
    let mut textarea = vim_textarea("abc", /*cursor*/ 0);
    keys(&mut textarea, "ix");
    textarea.input(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    keys(&mut textarea, "y");
    escape(&mut textarea);

    let mut keymap = crate::keymap::RuntimeKeymap::defaults();
    std::sync::Arc::make_mut(&mut keymap.editor).move_left =
        vec![crate::key_hint::plain(KeyCode::Char('z'))];
    textarea.set_keymap_bindings(&keymap);
    keys(&mut textarea, ".");

    assert_eq!(textarea.text(), "yxyxabc");
}

#[test]
fn repeat_replays_literal_tab_insertions() {
    let mut textarea = vim_textarea("", /*cursor*/ 0);
    keys(&mut textarea, "i");
    textarea.insert_str("\t");
    escape(&mut textarea);

    keys(&mut textarea, ".");

    assert_eq!(textarea.text(), "\t\t");
}

#[test]
fn repeat_replays_literal_newlines_with_custom_editor_bindings() {
    let mut textarea = vim_textarea("", /*cursor*/ 0);
    std::sync::Arc::make_mut(&mut textarea.editor_keymap).insert_newline =
        vec![crate::key_hint::plain(KeyCode::Char('n'))];
    keys(&mut textarea, "i");
    textarea.insert_str("\n");
    escape(&mut textarea);

    keys(&mut textarea, ".");

    assert_eq!(textarea.text(), "\n\n");
}

#[test]
fn repeat_uses_configured_binding_and_supports_unbinding() {
    let mut textarea = vim_textarea("one two three", /*cursor*/ 0);
    let mut keymap = crate::keymap::RuntimeKeymap::defaults();
    keymap.vim_normal.repeat_last_change = vec![crate::key_hint::plain(KeyCode::Char('z'))];
    textarea.set_keymap_bindings(&keymap);

    keys(&mut textarea, "dwz");
    assert_eq!(textarea.text(), "three");

    keymap.vim_normal.repeat_last_change.clear();
    textarea.set_keymap_bindings(&keymap);
    keys(&mut textarea, "z");
    assert_eq!(textarea.text(), "three");
}

#[test]
fn replacing_the_buffer_discards_incomplete_dot_repeat_history() {
    let mut textarea = vim_textarea("", /*cursor*/ 0);
    keys(&mut textarea, "iold");
    textarea.set_text_clearing_elements("");
    textarea.enter_vim_normal_mode();

    keys(&mut textarea, "iX");
    escape(&mut textarea);
    keys(&mut textarea, ".");

    assert_eq!(textarea.text(), "XX");
}

#[test]
fn replace_character_uses_configured_binding_and_supports_unbinding() {
    let mut textarea = vim_textarea("abc", /*cursor*/ 0);
    let mut keymap = crate::keymap::RuntimeKeymap::defaults();
    keymap.vim_normal.replace_char = vec![crate::key_hint::plain(KeyCode::Char('z'))];
    textarea.set_keymap_bindings(&keymap);

    keys(&mut textarea, "zQ");
    assert_eq!(textarea.text(), "Qbc");

    keymap.vim_normal.replace_char.clear();
    textarea.set_keymap_bindings(&keymap);
    keys(&mut textarea, "zR");
    assert_eq!(textarea.text(), "Qbc");
}

#[test]
fn change_accepts_word_line_and_repeated_operator_motions() {
    let mut textarea = vim_textarea("hello world\nnext", /*cursor*/ 0);
    keys(&mut textarea, "cw");
    assert_eq!(textarea.text(), " world\nnext");
    assert_eq!(textarea.vim_mode_label(), Some("Insert"));

    let mut textarea = vim_textarea("hello world\nnext", /*cursor*/ 1);
    keys(&mut textarea, "c$");
    assert_eq!(textarea.text(), "h\nnext");
    assert_eq!(textarea.vim_mode_label(), Some("Insert"));

    let mut textarea = vim_textarea("first\nsecond\nthird", /*cursor*/ 8);
    keys(&mut textarea, "cc");
    assert_eq!(textarea.text(), "first\n\nthird");
    assert_eq!(textarea.cursor(), "first\n".len());
    keys(&mut textarea, "X");
    escape(&mut textarea);
    keys(&mut textarea, "p");
    assert_eq!(textarea.text(), "first\nX\nsecond\nthird");

    for (text, cursor, command, expected) in [
        ("a b", 0, "cw", " b"),
        ("\nnext", 0, "cw", "\nnext"),
        ("one\ntwo\nthree", 0, "cj", "\nthree"),
        ("one\ntwo\nthree", 8, "ck", "one\n"),
        ("one\ntwo\n", 8, "ck", "one\n"),
        ("\nnext", 0, "c$", "\nnext"),
    ] {
        let mut textarea = vim_textarea(text, cursor);
        keys(&mut textarea, command);
        assert_eq!(textarea.text(), expected);
        assert_eq!(textarea.vim_mode_label(), Some("Insert"));
    }
    for (cursor, command) in [(0, "ck"), (4, "cj")] {
        let mut textarea = vim_textarea("one\ntwo", cursor);
        keys(&mut textarea, command);
        assert_eq!(
            (textarea.text(), textarea.vim_mode_label()),
            ("one\ntwo", Some("Normal"))
        );
    }

    let mut textarea = vim_textarea("hello world", /*cursor*/ 0);
    textarea.vim_operator_keymap.motion_word_forward =
        vec![crate::key_hint::plain(KeyCode::Char('c'))];
    keys(&mut textarea, "cc");
    assert_eq!(textarea.text(), " world");
}

#[test]
fn pending_replacement_owns_escape_before_turn_interruption() {
    let mut textarea = vim_textarea("alpha", /*cursor*/ 0);
    let escape = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    assert!(!textarea.should_handle_vim_insert_escape(escape));

    keys(&mut textarea, "r");
    assert!(textarea.should_handle_vim_insert_escape(escape));
    textarea.input(escape);
    assert!(!textarea.should_handle_vim_insert_escape(escape));
    assert_eq!(textarea.text(), "alpha");
}

#[test]
fn editor_commands_have_visual_snapshot_coverage() {
    let mut textarea = vim_textarea("alpha beta\ngamma delta", /*cursor*/ 0);
    let mut states = Vec::new();
    for command in ["rZ", "cw"] {
        keys(&mut textarea, command);
        states.push(format!(
            "{command}: {}\n{}^",
            textarea.text().replace('\n', "\\n"),
            " ".repeat(textarea.cursor())
        ));
    }
    insta::assert_snapshot!(states.join("\n\n"), @r###"
    rZ: Zlpha beta\ngamma delta
    ^

    cw:  beta\ngamma delta
    ^
    "###);
}

#[test]
fn dot_repeat_has_visual_snapshot_coverage() {
    let mut textarea = vim_textarea("alpha beta\ngamma delta", /*cursor*/ 0);
    let mut states = Vec::new();
    for command in ["dw", "."] {
        keys(&mut textarea, command);
        states.push(format!(
            "{command}: {}\n{}^",
            textarea.text().replace('\n', "\\n"),
            " ".repeat(textarea.cursor())
        ));
    }
    insta::assert_snapshot!(states.join("\n\n"), @r###"
    dw: beta\ngamma delta
    ^

    .: gamma delta
    ^
    "###);
}
