//! Basic commands tests.
use super::{assert_cursor, assert_line, assert_line_with_initial, init_editor};
use crate::config::EditMode;
use crate::error::ReadlineError;
use crate::keys::{KeyCode as K, KeyEvent as E, Modifiers as M};

#[test]
fn home_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(*mode, ("", ""), &[E(K::Home, M::NONE), E::ENTER], ("", ""));
        assert_cursor(
            *mode,
            ("Hi", ""),
            &[E(K::Home, M::NONE), E::ENTER],
            ("", "Hi"),
        );
        if *mode == EditMode::Vi {
            // vi command mode
            assert_cursor(
                *mode,
                ("Hi", ""),
                &[E::ESC, E(K::Home, M::NONE), E::ENTER],
                ("", "Hi"),
            );
        }
    }
}

#[test]
fn end_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(*mode, ("", ""), &[E(K::End, M::NONE), E::ENTER], ("", ""));
        assert_cursor(
            *mode,
            ("H", "i"),
            &[E(K::End, M::NONE), E::ENTER],
            ("Hi", ""),
        );
        assert_cursor(
            *mode,
            ("", "Hi"),
            &[E(K::End, M::NONE), E::ENTER],
            ("Hi", ""),
        );
        if *mode == EditMode::Vi {
            // vi command mode
            assert_cursor(
                *mode,
                ("", "Hi"),
                &[E::ESC, E(K::End, M::NONE), E::ENTER],
                ("Hi", ""),
            );
        }
    }
}

#[test]
fn left_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(
            *mode,
            ("Hi", ""),
            &[E(K::Left, M::NONE), E::ENTER],
            ("H", "i"),
        );
        assert_cursor(
            *mode,
            ("H", "i"),
            &[E(K::Left, M::NONE), E::ENTER],
            ("", "Hi"),
        );
        assert_cursor(
            *mode,
            ("", "Hi"),
            &[E(K::Left, M::NONE), E::ENTER],
            ("", "Hi"),
        );
        if *mode == EditMode::Vi {
            // vi command mode
            assert_cursor(
                *mode,
                ("Bye", ""),
                &[E::ESC, E(K::Left, M::NONE), E::ENTER],
                ("B", "ye"),
            );
        }
    }
}

#[test]
fn right_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(*mode, ("", ""), &[E(K::Right, M::NONE), E::ENTER], ("", ""));
        assert_cursor(
            *mode,
            ("", "Hi"),
            &[E(K::Right, M::NONE), E::ENTER],
            ("H", "i"),
        );
        assert_cursor(
            *mode,
            ("B", "ye"),
            &[E(K::Right, M::NONE), E::ENTER],
            ("By", "e"),
        );
        assert_cursor(
            *mode,
            ("H", "i"),
            &[E(K::Right, M::NONE), E::ENTER],
            ("Hi", ""),
        );
        if *mode == EditMode::Vi {
            // vi command mode
            assert_cursor(
                *mode,
                ("", "Hi"),
                &[E::ESC, E(K::Right, M::NONE), E::ENTER],
                ("H", "i"),
            );
        }
    }
}

#[test]
fn enter_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_line(*mode, &[E::ENTER], "");
        assert_line(*mode, &[E::from('a'), E::ENTER], "a");
        assert_line_with_initial(*mode, ("Hi", ""), &[E::ENTER], "Hi");
        assert_line_with_initial(*mode, ("", "Hi"), &[E::ENTER], "Hi");
        assert_line_with_initial(*mode, ("H", "i"), &[E::ENTER], "Hi");
        if *mode == EditMode::Vi {
            // vi command mode
            assert_line(*mode, &[E::ESC, E::ENTER], "");
            assert_line(*mode, &[E::from('a'), E::ESC, E::ENTER], "a");
            assert_line_with_initial(*mode, ("Hi", ""), &[E::ESC, E::ENTER], "Hi");
            assert_line_with_initial(*mode, ("", "Hi"), &[E::ESC, E::ENTER], "Hi");
            assert_line_with_initial(*mode, ("H", "i"), &[E::ESC, E::ENTER], "Hi");
        }
    }
}

#[test]
fn newline_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_line(*mode, &[E::ctrl('J')], "");
        assert_line(*mode, &[E::from('a'), E::ctrl('J')], "a");
        if *mode == EditMode::Vi {
            // vi command mode
            assert_line(*mode, &[E::ESC, E::ctrl('J')], "");
            assert_line(*mode, &[E::from('a'), E::ESC, E::ctrl('J')], "a");
        }
    }
}

#[test]
fn eof_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        let mut editor = init_editor(*mode, &[E::ctrl('D')]);
        let err = editor.readline(">>");
        assert_matches!(err, Err(ReadlineError::Eof));
    }
    assert_line(
        EditMode::Emacs,
        &[E::from('a'), E::ctrl('D'), E::ENTER],
        "a",
    );
    assert_line(EditMode::Vi, &[E::from('a'), E::ctrl('D')], "a");
    assert_line(EditMode::Vi, &[E::from('a'), E::ESC, E::ctrl('D')], "a");
    assert_line_with_initial(EditMode::Emacs, ("", "Hi"), &[E::ctrl('D'), E::ENTER], "i");
    assert_line_with_initial(EditMode::Vi, ("", "Hi"), &[E::ctrl('D')], "Hi");
    assert_line_with_initial(EditMode::Vi, ("", "Hi"), &[E::ESC, E::ctrl('D')], "Hi");
}

#[test]
fn interrupt_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        let mut editor = init_editor(*mode, &[E::ctrl('C')]);
        let err = editor.readline(">>");
        assert_matches!(err, Err(ReadlineError::Interrupted));

        let mut editor = init_editor(*mode, &[E::ctrl('C')]);
        let err = editor.readline_with_initial(">>", ("Hi", ""));
        assert_matches!(err, Err(ReadlineError::Interrupted));
        if *mode == EditMode::Vi {
            // vi command mode
            let mut editor = init_editor(*mode, &[E::ESC, E::ctrl('C')]);
            let err = editor.readline_with_initial(">>", ("Hi", ""));
            assert_matches!(err, Err(ReadlineError::Interrupted));
        }
    }
}

#[test]
fn delete_key() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(
            *mode,
            ("a", ""),
            &[E(K::Delete, M::NONE), E::ENTER],
            ("a", ""),
        );
        assert_cursor(
            *mode,
            ("", "a"),
            &[E(K::Delete, M::NONE), E::ENTER],
            ("", ""),
        );
        if *mode == EditMode::Vi {
            // vi command mode
            assert_cursor(
                *mode,
                ("", "a"),
                &[E::ESC, E(K::Delete, M::NONE), E::ENTER],
                ("", ""),
            );
        }
    }
}

#[test]
fn ctrl_t() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(*mode, ("a", "b"), &[E::ctrl('T'), E::ENTER], ("ba", ""));
        assert_cursor(*mode, ("ab", "cd"), &[E::ctrl('T'), E::ENTER], ("acb", "d"));
        if *mode == EditMode::Vi {
            // vi command mode
            assert_cursor(
                *mode,
                ("ab", ""),
                &[E::ESC, E::ctrl('T'), E::ENTER],
                ("ba", ""),
            );
        }
    }
}

#[test]
fn ctrl_u() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(
            *mode,
            ("start of line ", "end"),
            &[E::ctrl('U'), E::ENTER],
            ("", "end"),
        );
        assert_cursor(*mode, ("", "end"), &[E::ctrl('U'), E::ENTER], ("", "end"));
        if *mode == EditMode::Vi {
            // vi command mode
            assert_cursor(
                *mode,
                ("start of line ", "end"),
                &[E::ESC, E::ctrl('U'), E::ENTER],
                ("", " end"),
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn ctrl_v() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(
            *mode,
            ("", ""),
            &[E::ctrl('V'), E(K::Char('\t'), M::NONE), E::ENTER],
            ("\t", ""),
        );
        if *mode == EditMode::Vi {
            // vi command mode
            assert_cursor(
                *mode,
                ("", ""),
                &[E::ESC, E::ctrl('V'), E(K::Char('\t'), M::NONE), E::ENTER],
                ("\t", ""),
            );
        }
    }
}

#[test]
fn ctrl_w() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(
            *mode,
            ("Hello, ", "world"),
            &[E::ctrl('W'), E::ENTER],
            ("", "world"),
        );
        assert_cursor(
            *mode,
            ("Hello, world.", ""),
            &[E::ctrl('W'), E::ENTER],
            ("Hello, ", ""),
        );
        if *mode == EditMode::Vi {
            // vi command mode
            assert_cursor(
                *mode,
                ("Hello, world.", ""),
                &[E::ESC, E::ctrl('W'), E::ENTER],
                ("Hello, ", "."),
            );
        }
    }
}

#[test]
fn ctrl_y() {
    for mode in &[EditMode::Emacs /* FIXME, EditMode::Vi */] {
        assert_cursor(
            *mode,
            ("Hello, ", "world"),
            &[E::ctrl('W'), E::ctrl('Y'), E::ENTER],
            ("Hello, ", "world"),
        );
    }
}

#[test]
fn ctrl__() {
    for mode in &[EditMode::Emacs, EditMode::Vi] {
        assert_cursor(
            *mode,
            ("Hello, ", "world"),
            &[E::ctrl('W'), E::ctrl('_'), E::ENTER],
            ("Hello, ", "world"),
        );
        if *mode == EditMode::Vi {
            // vi command mode
            assert_cursor(
                *mode,
                ("Hello, ", "world"),
                &[E::ESC, E::ctrl('W'), E::ctrl('_'), E::ENTER],
                ("Hello,", " world"),
            );
        }
    }
}
