//! Vi insert mode specific key bindings
use super::assert_cursor;
use crate::config::EditMode;
use crate::keys::KeyEvent as E;

#[test]
fn insert_mode_by_default() {
    assert_cursor(EditMode::Vi, ("", ""), &[E::from('a'), E::ENTER], ("a", ""));
}

#[test]
fn ctrl_h() {
    assert_cursor(
        EditMode::Vi,
        ("Hi", ""),
        &[E::ctrl('H'), E::ENTER],
        ("H", ""),
    );
}

#[test]
fn backspace() {
    assert_cursor(EditMode::Vi, ("", ""), &[E::BACKSPACE, E::ENTER], ("", ""));
    assert_cursor(
        EditMode::Vi,
        ("Hi", ""),
        &[E::BACKSPACE, E::ENTER],
        ("H", ""),
    );
    assert_cursor(
        EditMode::Vi,
        ("", "Hi"),
        &[E::BACKSPACE, E::ENTER],
        ("", "Hi"),
    );
}

#[test]
fn esc() {
    assert_cursor(
        EditMode::Vi,
        ("", ""),
        &[E::from('a'), E::ESC, E::ENTER],
        ("", "a"),
    );
}
