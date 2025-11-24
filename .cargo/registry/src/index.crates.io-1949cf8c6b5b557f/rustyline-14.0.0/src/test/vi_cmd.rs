//! Vi command mode specific key bindings
use super::{assert_cursor, assert_history};
use crate::config::EditMode;
use crate::keys::KeyEvent as E;

#[test]
fn dollar() {
    assert_cursor(
        EditMode::Vi,
        ("", "Hi"),
        &[E::ESC, E::from('$'), E::ENTER],
        ("Hi", ""), // FIXME
    );
}

#[test]
fn dot() {
    assert_cursor(
        EditMode::Vi,
        ("", ""),
        &[E::ESC, E::from('.'), E::ENTER],
        ("", ""),
    );
}

#[test]
fn semi_colon() {
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[E::ESC, E::from('f'), E::from('o'), E::from(';'), E::ENTER],
        ("Hello, w", "orld!"),
    );
}

#[test]
fn comma() {
    assert_cursor(
        EditMode::Vi,
        ("Hello, w", "orld!"),
        &[E::ESC, E::from('f'), E::from('l'), E::from(','), E::ENTER],
        ("Hel", "lo, world!"),
    );
}

#[test]
fn zero() {
    assert_cursor(
        EditMode::Vi,
        ("Hi", ""),
        &[E::ESC, E::from('0'), E::ENTER],
        ("", "Hi"),
    );
}

#[test]
fn caret() {
    assert_cursor(
        EditMode::Vi,
        (" Hi", ""),
        &[E::ESC, E::from('^'), E::ENTER],
        (" ", "Hi"),
    );
}

#[test]
fn caret_no_whitespace() {
    assert_cursor(
        EditMode::Vi,
        ("Hi", ""),
        &[E::ESC, E::from('^'), E::ENTER],
        ("", "Hi"),
    );
}

#[test]
fn a() {
    assert_cursor(
        EditMode::Vi,
        ("B", "e"),
        &[E::ESC, E::from('a'), E::from('y'), E::ENTER],
        ("By", "e"),
    );
}

#[test]
fn uppercase_a() {
    assert_cursor(
        EditMode::Vi,
        ("", "By"),
        &[E::ESC, E::from('A'), E::from('e'), E::ENTER],
        ("Bye", ""),
    );
}

#[test]
fn b() {
    assert_cursor(
        EditMode::Vi,
        ("Hello, world!", ""),
        &[E::ESC, E::from('b'), E::ENTER],
        ("Hello, ", "world!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("Hello, world!", ""),
        &[E::ESC, E::from('2'), E::from('b'), E::ENTER],
        ("Hello", ", world!"),
    );
}

#[test]
fn uppercase_b() {
    assert_cursor(
        EditMode::Vi,
        ("Hello, world!", ""),
        &[E::ESC, E::from('B'), E::ENTER],
        ("Hello, ", "world!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("Hello, world!", ""),
        &[E::ESC, E::from('2'), E::from('B'), E::ENTER],
        ("", "Hello, world!"),
    );
}

#[test]
fn uppercase_c() {
    assert_cursor(
        EditMode::Vi,
        ("Hello, w", "orld!"),
        &[E::ESC, E::from('C'), E::from('i'), E::ENTER],
        ("Hello, i", ""),
    );
}

#[test]
fn ctrl_k() {
    for key in &[E::from('D'), E::ctrl('K')] {
        assert_cursor(
            EditMode::Vi,
            ("Hi", ""),
            &[E::ESC, *key, E::ENTER],
            ("H", ""),
        );
        assert_cursor(
            EditMode::Vi,
            ("", "Hi"),
            &[E::ESC, *key, E::ENTER],
            ("", ""),
        );
        assert_cursor(
            EditMode::Vi,
            ("By", "e"),
            &[E::ESC, *key, E::ENTER],
            ("B", ""),
        );
    }
}

#[test]
fn e() {
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[E::ESC, E::from('e'), E::ENTER],
        ("Hell", "o, world!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[E::ESC, E::from('2'), E::from('e'), E::ENTER],
        ("Hello, worl", "d!"),
    );
}

#[test]
fn uppercase_e() {
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[E::ESC, E::from('E'), E::ENTER],
        ("Hello", ", world!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[E::ESC, E::from('2'), E::from('E'), E::ENTER],
        ("Hello, world", "!"),
    );
}

#[test]
fn f() {
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[E::ESC, E::from('f'), E::from('r'), E::ENTER],
        ("Hello, wo", "rld!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[E::ESC, E::from('3'), E::from('f'), E::from('l'), E::ENTER],
        ("Hello, wor", "ld!"),
    );
}

#[test]
fn uppercase_f() {
    assert_cursor(
        EditMode::Vi,
        ("Hello, world!", ""),
        &[E::ESC, E::from('F'), E::from('r'), E::ENTER],
        ("Hello, wo", "rld!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("Hello, world!", ""),
        &[E::ESC, E::from('3'), E::from('F'), E::from('l'), E::ENTER],
        ("He", "llo, world!"),
    );
}

#[test]
fn i() {
    assert_cursor(
        EditMode::Vi,
        ("Be", ""),
        &[E::ESC, E::from('i'), E::from('y'), E::ENTER],
        ("By", "e"),
    );
}

#[test]
fn uppercase_i() {
    assert_cursor(
        EditMode::Vi,
        ("Be", ""),
        &[E::ESC, E::from('I'), E::from('y'), E::ENTER],
        ("y", "Be"),
    );
}

#[test]
fn u() {
    assert_cursor(
        EditMode::Vi,
        ("Hello, ", "world"),
        &[E::ESC, E::ctrl('W'), E::from('u'), E::ENTER],
        ("Hello,", " world"),
    );
}

#[test]
fn w() {
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[E::ESC, E::from('w'), E::ENTER],
        ("Hello", ", world!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[E::ESC, E::from('2'), E::from('w'), E::ENTER],
        ("Hello, ", "world!"),
    );
}

#[test]
fn uppercase_w() {
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[E::ESC, E::from('W'), E::ENTER],
        ("Hello, ", "world!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[E::ESC, E::from('2'), E::from('W'), E::ENTER],
        ("Hello, world", "!"),
    );
}

#[test]
fn x() {
    assert_cursor(
        EditMode::Vi,
        ("", "a"),
        &[E::ESC, E::from('x'), E::ENTER],
        ("", ""),
    );
}

#[test]
fn uppercase_x() {
    assert_cursor(
        EditMode::Vi,
        ("Hi", ""),
        &[E::ESC, E::from('X'), E::ENTER],
        ("", "i"),
    );
}

#[test]
fn h() {
    for key in &[E::from('h'), E::ctrl('H'), E::BACKSPACE] {
        assert_cursor(
            EditMode::Vi,
            ("Bye", ""),
            &[E::ESC, *key, E::ENTER],
            ("B", "ye"),
        );
        assert_cursor(
            EditMode::Vi,
            ("Bye", ""),
            &[E::ESC, E::from('2'), *key, E::ENTER],
            ("", "Bye"),
        );
    }
}

#[test]
fn l() {
    for key in &[E::from('l'), E::from(' ')] {
        assert_cursor(
            EditMode::Vi,
            ("", "Hi"),
            &[E::ESC, *key, E::ENTER],
            ("H", "i"),
        );
        assert_cursor(
            EditMode::Vi,
            ("", "Hi"),
            &[E::ESC, E::from('2'), *key, E::ENTER],
            ("Hi", ""),
        );
    }
}

#[test]
fn j() {
    for key in &[E::from('j'), E::from('+')] {
        assert_cursor(
            EditMode::Vi,
            ("Hel", "lo,\nworld!"),
            // NOTE: escape moves backwards on char
            &[E::ESC, *key, E::ENTER],
            ("Hello,\nwo", "rld!"),
        );
        assert_cursor(
            EditMode::Vi,
            ("", "One\nTwo\nThree"),
            &[E::ESC, E::from('2'), *key, E::ENTER],
            ("One\nTwo\n", "Three"),
        );
        assert_cursor(
            EditMode::Vi,
            ("Hel", "lo,\nworld!"),
            // NOTE: escape moves backwards on char
            &[E::ESC, E::from('7'), *key, E::ENTER],
            ("Hello,\nwo", "rld!"),
        );
    }
}

#[test]
fn k() {
    for key in &[E::from('k'), E::from('-')] {
        assert_cursor(
            EditMode::Vi,
            ("Hello,\nworl", "d!"),
            // NOTE: escape moves backwards on char
            &[E::ESC, *key, E::ENTER],
            ("Hel", "lo,\nworld!"),
        );
        assert_cursor(
            EditMode::Vi,
            ("One\nTwo\nT", "hree"),
            // NOTE: escape moves backwards on char
            &[E::ESC, E::from('2'), *key, E::ENTER],
            ("", "One\nTwo\nThree"),
        );
        assert_cursor(
            EditMode::Vi,
            ("Hello,\nworl", "d!"),
            // NOTE: escape moves backwards on char
            &[E::ESC, E::from('5'), *key, E::ENTER],
            ("Hel", "lo,\nworld!"),
        );
        assert_cursor(
            EditMode::Vi,
            ("first line\nshort\nlong line", ""),
            &[E::ESC, *key, E::ENTER],
            ("first line\nshort", "\nlong line"),
        );
    }
}

#[test]
fn ctrl_n() {
    for key in &[E::ctrl('N')] {
        assert_history(
            EditMode::Vi,
            &["line1", "line2"],
            &[E::ESC, E::ctrl('P'), E::ctrl('P'), *key, E::ENTER],
            "",
            ("line2", ""),
        );
    }
}

#[test]
fn ctrl_p() {
    for key in &[E::ctrl('P')] {
        assert_history(
            EditMode::Vi,
            &["line1"],
            &[E::ESC, *key, E::ENTER],
            "",
            ("line1", ""),
        );
    }
}

#[test]
fn p() {
    assert_cursor(
        EditMode::Vi,
        ("Hello, ", "world"),
        &[E::ESC, E::ctrl('W'), E::from('p'), E::ENTER],
        (" Hello", ",world"),
    );
}

#[test]
fn uppercase_p() {
    assert_cursor(
        EditMode::Vi,
        ("Hello, ", "world"),
        &[E::ESC, E::ctrl('W'), E::from('P'), E::ENTER],
        ("Hello", ", world"),
    );
}

#[test]
fn r() {
    assert_cursor(
        EditMode::Vi,
        ("Hi", ", world!"),
        &[E::ESC, E::from('r'), E::from('o'), E::ENTER],
        ("H", "o, world!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("He", "llo, world!"),
        &[E::ESC, E::from('4'), E::from('r'), E::from('i'), E::ENTER],
        ("Hiii", "i, world!"),
    );
}

#[test]
fn s() {
    assert_cursor(
        EditMode::Vi,
        ("Hi", ", world!"),
        &[E::ESC, E::from('s'), E::from('o'), E::ENTER],
        ("Ho", ", world!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("He", "llo, world!"),
        &[E::ESC, E::from('4'), E::from('s'), E::from('i'), E::ENTER],
        ("Hi", ", world!"),
    );
}

#[test]
fn uppercase_s() {
    assert_cursor(
        EditMode::Vi,
        ("Hello, ", "world"),
        &[E::ESC, E::from('S'), E::ENTER],
        ("", ""),
    );
}

#[test]
fn t() {
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[E::ESC, E::from('t'), E::from('r'), E::ENTER],
        ("Hello, w", "orld!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("", "Hello, world!"),
        &[E::ESC, E::from('3'), E::from('t'), E::from('l'), E::ENTER],
        ("Hello, wo", "rld!"),
    );
}

#[test]
fn uppercase_t() {
    assert_cursor(
        EditMode::Vi,
        ("Hello, world!", ""),
        &[E::ESC, E::from('T'), E::from('r'), E::ENTER],
        ("Hello, wor", "ld!"),
    );
    assert_cursor(
        EditMode::Vi,
        ("Hello, world!", ""),
        &[E::ESC, E::from('3'), E::from('T'), E::from('l'), E::ENTER],
        ("Hel", "lo, world!"),
    );
}

#[test]
fn indent() {
    assert_cursor(
        EditMode::Vi,
        ("Hello, world!", ""),
        &[E::ESC, E::from('>'), E::from('>'), E::ENTER],
        ("  Hello, world", "!"), // Esc moves to the left
    );
    assert_cursor(
        EditMode::Vi,
        ("line1\nline2", ""),
        &[E::ESC, E::from('>'), E::from('>'), E::ENTER],
        ("line1\n  line", "2"), // Esc moves to the left
    );
    assert_cursor(
        EditMode::Vi,
        ("line1\nline2", ""),
        &[E::ESC, E::from('>'), E::from('k'), E::ENTER],
        ("  line1\n  line", "2"), // Esc moves to the left
    );
    assert_cursor(
        EditMode::Vi,
        ("  li", "ne1\n  line2"),
        &[E::ESC, E::from('>'), E::from('j'), E::ENTER],
        ("    l", "ine1\n    line2"), // Esc moves to the left
    );
    assert_cursor(
        EditMode::Vi,
        ("  ", "line1\n  line2"),
        &[E::ESC, E::from('>'), E::from('j'), E::ENTER],
        ("   ", " line1\n    line2"), // Esc moves to the left
    );
    assert_cursor(
        EditMode::Vi,
        ("  ", "line1\n  line2"),
        &[E::ESC, E::from('>'), E::from('j'), E::ENTER],
        ("   ", " line1\n    line2"), // Esc moves to the left
    );
}

#[test]
fn dedent() {
    assert_cursor(
        EditMode::Vi,
        ("  line1\n  line2", ""),
        &[E::ESC, E::from('<'), E::from('<'), E::ENTER],
        ("  line1\nline", "2"),
    );

    assert_cursor(
        EditMode::Vi,
        ("  line1\n  line2", ""),
        &[E::ESC, E::from('<'), E::from('k'), E::ENTER],
        ("line1\nline", "2"),
    );

    assert_cursor(
        EditMode::Vi,
        ("  li", "ne1\n  line2"),
        &[E::ESC, E::from('<'), E::from('j'), E::ENTER],
        ("l", "ine1\nline2"),
    );

    assert_cursor(
        EditMode::Vi,
        ("  ", "line1\n  line2"),
        &[E::ESC, E::from('<'), E::from('j'), E::ENTER],
        ("", "line1\nline2"),
    );
    assert_cursor(
        EditMode::Vi,
        ("line", "1\n  line2"),
        &[E::ESC, E::from('<'), E::from('j'), E::ENTER],
        ("lin", "e1\nline2"),
    );
}
