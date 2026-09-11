//! Platform-specific app actions and small global shortcuts.
//!
//! This module owns platform state used by `App`, the side-conversation return shortcut predicate,
//! and Windows sandbox helper actions that are compiled only on Windows.

use super::*;

#[derive(Default)]
pub(super) struct WindowsSandboxState {
    pub(super) setup_started_at: Option<Instant>,
}

pub(super) fn side_return_shortcut_matches(key_event: KeyEvent) -> bool {
    matches!(
        key_event,
        KeyEvent {
            code: KeyCode::Char(c),
            modifiers,
            kind: KeyEventKind::Press,
            ..
        } if modifiers.contains(KeyModifiers::CONTROL)
            && (c.eq_ignore_ascii_case(&'c') || c.eq_ignore_ascii_case(&'d'))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn side_return_shortcuts_match_ctrl_c_and_ctrl_d() {
        assert!(side_return_shortcut_matches(KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
        )));
        assert!(side_return_shortcut_matches(KeyEvent::new(
            KeyCode::Char('C'),
            KeyModifiers::CONTROL,
        )));
        assert!(side_return_shortcut_matches(KeyEvent::new(
            KeyCode::Char('d'),
            KeyModifiers::CONTROL,
        )));
        assert!(side_return_shortcut_matches(KeyEvent::new(
            KeyCode::Char('D'),
            KeyModifiers::CONTROL,
        )));
        assert!(!side_return_shortcut_matches(KeyEvent::new_with_kind(
            KeyCode::Esc,
            KeyModifiers::NONE,
            KeyEventKind::Press,
        )));
        assert!(!side_return_shortcut_matches(KeyEvent::new_with_kind(
            KeyCode::Esc,
            KeyModifiers::NONE,
            KeyEventKind::Release,
        )));
    }
}
