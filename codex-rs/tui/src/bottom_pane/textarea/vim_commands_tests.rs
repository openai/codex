use super::super::TextArea;
use super::VimAction;
use crate::keymap::KeyChordMatch;
use crate::keymap::KeyChordMatcher;
use crate::keymap::RuntimeKeymap;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;

fn vim_textarea(text: &str, cursor: usize) -> TextArea {
    let mut textarea = TextArea::new();
    textarea.insert_str(text);
    textarea.set_cursor(cursor);
    textarea.set_vim_enabled(/*enabled*/ true);
    textarea.enable_vim_search();
    textarea
}

fn keys(textarea: &mut TextArea, keys: &str) {
    let keymap = RuntimeKeymap::defaults();
    let mut matcher = KeyChordMatcher::default();
    for key in keys.chars() {
        let code = match key {
            '\n' => KeyCode::Enter,
            '\x1b' => KeyCode::Esc,
            ch => KeyCode::Char(ch),
        };
        let event = KeyEvent::new(code, KeyModifiers::NONE);
        match matcher.advance(event, &keymap.chords, textarea.keymap_contexts()) {
            KeyChordMatch::PassThrough => textarea.input(event),
            KeyChordMatch::Completed(event) => textarea.input(event),
            KeyChordMatch::Pending(_) | KeyChordMatch::Cancelled | KeyChordMatch::Ignored => {}
        }
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
fn replace_mode_overwrites_graphemes_and_appends_at_line_end() {
    let mut textarea = vim_textarea("a👩‍💻c\nnext", /*cursor*/ 1);
    keys(&mut textarea, "RXY!");
    assert_eq!(textarea.text(), "aXY!\nnext");
    assert_eq!(textarea.vim_mode_label(), Some("Replace"));
    assert_eq!(
        textarea.keymap_context(),
        crate::keymap::KeymapContext::Editor
    );

    escape(&mut textarea);
    assert_eq!(
        (textarea.vim_mode_label(), textarea.cursor()),
        (Some("Normal"), 3)
    );

    let mut textarea = vim_textarea("abc", /*cursor*/ 0);
    keys(&mut textarea, "R\nX");
    assert_eq!(textarea.text(), "\nXbc");

    let mut textarea = vim_textarea("abc def", /*cursor*/ 0);
    keys(&mut textarea, "R/?nN");
    assert_eq!(textarea.text(), "/?nNdef");
    escape(&mut textarea);
    keys(&mut textarea, "/def\n");
    assert_eq!(textarea.cursor(), 4);
}

#[test]
fn replace_mode_uses_configured_binding_and_respects_unbinding() {
    let mut textarea = vim_textarea("abc", /*cursor*/ 0);
    let mut keymap = crate::keymap::RuntimeKeymap::defaults();
    keymap.vim_normal.enter_replace_mode = vec![crate::key_hint::plain(KeyCode::F(2))];
    textarea.set_keymap_bindings(&keymap);

    keys(&mut textarea, "R");
    assert_eq!(textarea.vim_mode_label(), Some("Normal"));

    textarea.input(KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE));
    keys(&mut textarea, "X");
    assert_eq!(textarea.text(), "Xbc");

    escape(&mut textarea);
    keymap.vim_normal.enter_replace_mode.clear();
    textarea.set_keymap_bindings(&keymap);
    textarea.input(KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE));
    assert_eq!(textarea.vim_mode_label(), Some("Normal"));
}

#[test]
fn replace_mode_backspace_restores_original_text() {
    let mut textarea = vim_textarea("a👩‍💻", /*cursor*/ 0);
    keys(&mut textarea, "RXYZ");
    assert_eq!(textarea.text(), "XYZ");

    let backspace = KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE);
    for expected in ["XY", "X👩‍💻", "a👩‍💻", "a👩‍💻"] {
        textarea.input(backspace);
        assert_eq!(textarea.text(), expected);
    }
    assert_eq!(textarea.cursor(), 0);

    let mut textarea = vim_textarea("abcd", /*cursor*/ 0);
    keys(&mut textarea, "RXY");
    textarea.input(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    textarea.input(backspace);
    assert_eq!(textarea.text(), "Ycd");

    let mut textarea = vim_textarea("abé", /*cursor*/ 0);
    keys(&mut textarea, "RXY");
    textarea.input(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL));
    textarea.input(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    textarea.input(backspace);
    assert_eq!(textarea.text(), "");
    assert_eq!(textarea.vim_mode_label(), Some("Replace"));
}

#[test]
fn replace_mode_preserves_atomic_attachments() {
    for (cursor, command, edited) in [
        (0, "RXY", "X<image><paste>Y"),
        (1, "RX", "a<image><paste>X"),
    ] {
        let mut textarea = vim_textarea("ab", /*cursor*/ 1);
        textarea.insert_element("<image>");
        textarea.insert_element("<paste>");
        textarea.set_cursor(cursor);
        let elements = textarea.text_element_snapshots();
        keys(&mut textarea, command);
        assert_eq!(textarea.text(), edited);
        assert_eq!(textarea.text_element_snapshots(), elements);
        assert!(!textarea.retract_paste_burst(/*start*/ 0));
        for _ in command.chars().skip(/*n*/ 1) {
            textarea.input(KeyEvent::from(KeyCode::Backspace));
        }
        assert_eq!(
            (
                textarea.text(),
                textarea.cursor(),
                textarea.text_element_snapshots(),
            ),
            ("a<image><paste>b", cursor, elements)
        );
        textarea.input(KeyEvent::from(KeyCode::Backspace));
        assert_eq!(
            textarea.text(),
            if cursor == 0 {
                "a<image><paste>b"
            } else {
                "<image><paste>b"
            }
        );
        assert_eq!(textarea.element_payloads(), vec!["<image>", "<paste>"]);
    }
}

#[test]
fn replace_mode_replays_with_dot() {
    let mut textarea = vim_textarea("abc def", /*cursor*/ 0);
    keys(&mut textarea, "Rx\u{301}");
    escape(&mut textarea);
    assert_eq!(textarea.text(), "x\u{301}c def");

    keys(&mut textarea, "w.");
    assert_eq!(textarea.text(), "x\u{301}c x\u{301}f");
    assert_eq!(textarea.vim_mode_label(), Some("Normal"));

    let mut textarea = vim_textarea("abc def", /*cursor*/ 0);
    keys(&mut textarea, "RXY");
    textarea.input(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
    escape(&mut textarea);
    assert_eq!(textarea.text(), "Xbc def");

    keys(&mut textarea, "w.");
    assert_eq!(textarea.text(), "Xbc Xef");
    assert_eq!(textarea.vim_mode_label(), Some("Normal"));
}

#[test]
fn replace_mode_overwrites_explicit_paste() {
    let mut textarea = vim_textarea("abcdef", /*cursor*/ 1);
    keys(&mut textarea, "R");
    textarea.insert_str("XYZ");
    assert_eq!(textarea.text(), "aXYZef");
    escape(&mut textarea);

    keys(&mut textarea, "0.");
    assert_eq!(textarea.text(), "XYZZef");
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
        textarea.apply_vim_insert_action(action.clone());
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
fn character_find_is_line_local_and_works_as_operator_motion() {
    let mut textarea = vim_textarea("alpha beta\nbeta", /*cursor*/ 0);
    keys(&mut textarea, "fb");
    assert_eq!(textarea.cursor(), "alpha ".len());
    keys(&mut textarea, "Fa");
    assert_eq!(textarea.cursor(), "alpha".len() - 1);
    keys(&mut textarea, "fz");
    assert_eq!(textarea.cursor(), "alpha".len() - 1);

    for (motion, cursor, destination, range) in [
        ("fr", 0, 3, 0..4),
        ("tr", 0, 2, 0..3),
        ("Fb", 3, 1, 1..3),
        ("Tb", 3, 2, 2..3),
    ] {
        let text = "abcrdef";
        let mut textarea = vim_textarea(text, cursor);
        keys(&mut textarea, motion);
        assert_eq!((textarea.text(), textarea.cursor()), (text, destination));
        for operator in ['c', 'd', 'y'] {
            let mut textarea = vim_textarea(text, cursor);
            keys(&mut textarea, &format!("{operator}{motion}"));
            let expected = if operator == 'y' {
                text.to_owned()
            } else {
                format!("{}{}", &text[..range.start], &text[range.end..])
            };
            let mode = if operator == 'c' { "Insert" } else { "Normal" };
            assert_eq!(
                (
                    textarea.text(),
                    textarea.cursor(),
                    textarea.vim_mode_label(),
                    textarea.kill_buffer.as_str()
                ),
                (
                    expected.as_str(),
                    range.start,
                    Some(mode),
                    &text[range.clone()]
                ),
                "{operator}{motion}",
            );
        }
    }

    let mut textarea = vim_textarea("\nabc", /*cursor*/ 0);
    keys(&mut textarea, "fx");
    assert_eq!(textarea.text(), "\nabc");
    assert_eq!(textarea.cursor(), 0);
}

#[test]
fn find_and_till_handle_missing_cancelled_and_adjacent_targets() {
    for motion in ['f', 'F', 't', 'T'] {
        for operator in ["", "c", "d", "y"] {
            let mut textarea = vim_textarea("abc\nz", /*cursor*/ 1);
            keys(&mut textarea, &format!("{operator}{motion}z"));
            assert_eq!(
                (
                    textarea.text(),
                    textarea.cursor(),
                    textarea.vim_mode_label()
                ),
                ("abc\nz", 1, Some("Normal"))
            );
            keys(&mut textarea, &format!("{operator}{motion}"));
            escape(&mut textarea);
            assert!(!textarea.is_vim_operator_pending());
        }
    }
    for (cursor, command, expected) in [(0, "ctb", "bc"), (2, "cTb", "abc")] {
        let mut textarea = vim_textarea("abc", cursor);
        keys(&mut textarea, command);
        assert_eq!(
            (textarea.text(), textarea.vim_mode_label()),
            (expected, Some("Insert"))
        );
    }
}

#[test]
fn dot_repeat_replays_character_find_and_buffer_jump_operators() {
    let mut textarea = vim_textarea("one:two:three", /*cursor*/ 0);
    keys(&mut textarea, "df:.");
    assert_eq!(textarea.text(), "three");

    for (cursor, command, repeat, expected) in [
        (0, "cfrX", "w.", "X X"),
        (0, "ctrX", "w.", "Xr Xr"),
        (3, "cFbX", concat!("w", "lll."), "aXr aXr"),
        (3, "cTbX", concat!("w", "lll."), "abXr abXr"),
    ] {
        let mut textarea = vim_textarea("abcr abcr", cursor);
        keys(&mut textarea, command);
        escape(&mut textarea);
        keys(&mut textarea, repeat);
        assert_eq!(
            (textarea.text(), textarea.vim_mode_label()),
            (expected, Some("Normal"))
        );
    }

    let mut textarea = vim_textarea("one\ntwo\nthree\nfour\nfive", "one\n".len());
    keys(&mut textarea, "dggj.");
    assert_eq!(textarea.text(), "five");
}

#[test]
fn character_find_never_splits_extended_graphemes() {
    let mut textarea = vim_textarea("a👩‍💻z", /*cursor*/ 0);
    keys(&mut textarea, "f💻");
    assert_eq!(textarea.cursor(), 0);
    keys(&mut textarea, "f👩rX");
    assert_eq!(textarea.text(), "aXz");
    for (text, command, expected, cursor) in [
        ("a👩‍💻r", "tr", "a👩‍💻r", 1),
        ("aहिr", "tr", "aहिr", 1),
        ("aहिr", "dfह", "r", 0),
    ] {
        let mut textarea = vim_textarea(text, /*cursor*/ 0);
        keys(&mut textarea, command);
        assert_eq!((textarea.text(), textarea.cursor()), (expected, cursor));
    }
    let mut textarea = vim_textarea("a", /*cursor*/ 0);
    textarea.set_cursor(/*pos*/ 1);
    textarea.insert_element("[image]");
    textarea.insert_str("r");
    textarea.set_cursor(/*pos*/ 0);
    keys(&mut textarea, "t]");
    assert_eq!(textarea.cursor(), 0);
    keys(&mut textarea, "tr");
    assert_eq!(textarea.cursor(), 1);
    textarea.set_cursor(/*pos*/ 0);
    keys(&mut textarea, "ctr");
    assert_eq!((textarea.text(), textarea.elements.len()), ("r", 0));

    let prefix = "earlier line\na";
    for (command, cursor) in [("Fa", prefix.len() - 1), ("Ta", prefix.len())] {
        let mut textarea = vim_textarea(prefix, /*cursor*/ 0);
        textarea.set_cursor(prefix.len());
        textarea.insert_element("[aaaa]");
        textarea.insert_str("z");
        textarea.set_cursor(textarea.text().len() - 1);
        keys(&mut textarea, command);
        assert_eq!(
            (textarea.text(), textarea.cursor()),
            ("earlier line\na[aaaa]z", cursor)
        );
    }
}

#[test]
fn character_find_and_operator_motion_use_configured_bindings() {
    let mut keymap = crate::keymap::RuntimeKeymap::defaults();
    keymap.vim_normal.find_forward = vec![crate::key_hint::plain(KeyCode::Char('z'))];
    keymap.vim_operator.motion_find_forward = vec![crate::key_hint::plain(KeyCode::Char('z'))];
    keymap.vim_normal.till_forward = vec![crate::key_hint::plain(KeyCode::Char('q'))];
    keymap.vim_operator.motion_till_forward = vec![crate::key_hint::plain(KeyCode::Char('q'))];
    for (motion, cursor, expected) in [("zb", 6, "eta gamma"), ("qb", 5, "beta gamma")] {
        let mut textarea = vim_textarea("alpha beta gamma", /*cursor*/ 0);
        textarea.set_keymap_bindings(&keymap);
        keys(&mut textarea, motion);
        assert_eq!(textarea.cursor(), cursor);
        textarea.set_cursor(/*pos*/ 0);
        keys(&mut textarea, &format!("d{motion}"));
        assert_eq!(textarea.text(), expected);
    }
}

#[test]
fn uppercase_commands_accept_shift_only_terminal_events() {
    let mut textarea = vim_textarea("alpha\nbeta", /*cursor*/ 0);
    textarea.input(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::SHIFT));
    assert_eq!(textarea.cursor(), "alpha\n".len());

    textarea.set_cursor("alpha\nbe".len());
    textarea.input(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::SHIFT));
    keys(&mut textarea, "b");
    assert_eq!(textarea.cursor(), "alpha\n".len());
    textarea.set_cursor("alpha\nbe".len());
    textarea.input(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::SHIFT));
    keys(&mut textarea, "b");
    assert_eq!(textarea.cursor(), "alpha\nb".len());
}

#[test]
fn buffer_jumps_target_first_non_blank_and_support_operators() {
    let mut textarea = vim_textarea("  first\n  second\n  third", /*cursor*/ 2);
    keys(&mut textarea, "G");
    assert_eq!(textarea.cursor(), "  first\n  second\n  ".len());
    keys(&mut textarea, "gg");
    assert_eq!(textarea.cursor(), 2);

    let mut textarea = vim_textarea("first\nsecond\nthird", "first\n".len());
    keys(&mut textarea, "dG");
    assert_eq!(textarea.text(), "first\n");

    let mut textarea = vim_textarea("first\nsecond\nthird", "first\n".len());
    keys(&mut textarea, "dgg");
    assert_eq!(textarea.text(), "third");

    let mut textarea = vim_textarea("one\ntwo\nthree\nfour", "one\n".len());
    keys(&mut textarea, "dggp");
    assert_eq!(textarea.text(), "three\none\ntwo\nfour");

    let mut textarea = vim_textarea("one\ntwo\nthree", "one\n".len());
    keys(&mut textarea, "yGp");
    assert_eq!(textarea.text(), "one\ntwo\ntwo\nthree\nthree");

    let mut textarea = vim_textarea("one\ntwo\nthree", "one\n".len());
    keys(&mut textarea, "cG");
    assert_eq!(textarea.vim_mode_label(), Some("Insert"));
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

#[test]
fn find_and_navigation_have_visual_snapshot_coverage() {
    let mut textarea = vim_textarea("alpha beta\ngamma delta", /*cursor*/ 0);
    let mut states = Vec::new();
    for command in ["tb", "fb", "Ta", "Fa", "G", "gg"] {
        keys(&mut textarea, command);
        states.push(format!(
            "{command}: {}\n{}^",
            textarea.text().replace('\n', "\\n"),
            " ".repeat(textarea.cursor())
        ));
    }
    insta::assert_snapshot!(states.join("\n\n"), @r###"
    tb: alpha beta\ngamma delta
         ^

    fb: alpha beta\ngamma delta
          ^

    Ta: alpha beta\ngamma delta
         ^

    Fa: alpha beta\ngamma delta
        ^

    G: alpha beta\ngamma delta
               ^

    gg: alpha beta\ngamma delta
    ^
    "###);
}

#[test]
fn search_motions_compose_with_operators_and_repeat() {
    for (commands, text, cursor, mode) in [
        ("/b\n", "a b c b d b", 2, "Normal"),
        ("/b\nn", "a b c b d b", 6, "Normal"),
        ("/b\nnN", "a b c b d b", 2, "Normal"),
        ("/b\nnnn", "a b c b d b", 2, "Normal"),
        ("?b\n", "a b c b d b", 10, "Normal"),
        ("?b\nn", "a b c b d b", 6, "Normal"),
        ("?b\nN", "a b c b d b", 2, "Normal"),
        ("d/b\n", "b c b d b", 0, "Normal"),
        ("c/b\n", "b c b d b", 0, "Insert"),
        ("y/b\np", "aa  b c b d b", 3, "Normal"),
        ("/b\ndn", "a b d b", 2, "Normal"),
        ("?b\ndN", "a b", 2, "Normal"),
        ("?b\nd?b\n", "a b c b", 6, "Normal"),
        ("?b\ny?b\n", "a b c b d b", 6, "Normal"),
        ("d/missing\n", "a b c b d b", 0, "Normal"),
        ("/b\nc?missing\x1bn", "a b c b d b", 6, "Normal"),
        ("c/b\nX\x1bl.", "XXb d b", 1, "Normal"),
    ] {
        let mut area = vim_textarea("a b c b d b", /*cursor*/ 0);
        keys(&mut area, commands);
        assert_eq!(
            (area.text(), area.cursor(), area.vim_mode_label()),
            (text, cursor, Some(mode)),
            "{commands:?}"
        );
    }
    for (cursor, commands, expected) in [
        (1, "d/xyz\n", "a\nxyz"),
        (0, "d/xyz\n", "xyz"),
        (0, "c/xyz\nQ\x1b", "Q\nxyz"),
        (1, "c/xyz\nQ\x1b", "aQ\nxyz"),
        (0, "y/xyz\np", "abc\nabc\nxyz"),
    ] {
        let mut area = vim_textarea("abc\nxyz", cursor);
        keys(&mut area, commands);
        assert_eq!(area.text(), expected, "{commands:?}");
    }
}

#[test]
fn search_skips_atomic_elements_and_partial_graphemes() {
    let mut area = vim_textarea("é e\u{301}\n", /*cursor*/ 0);
    area.insert_element("[photo.png]");
    keys(&mut area, "/photo\n");
    assert_eq!(area.cursor(), "[photo.png]".len());
    keys(&mut area, "/\u{301}\n");
    assert_eq!(area.cursor(), "[photo.png]".len());
    keys(&mut area, "/e\u{301}\n");
    assert_eq!(area.cursor(), "[photo.png]é ".len());
}
