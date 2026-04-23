//! Shared popup-related constants for bottom pane widgets.

use crossterm::event::KeyCode;
use ratatui::text::Line;

use crate::key_hint;
use crate::keymap::ListKeymap;
use crate::keymap::primary_binding;

/// Maximum number of rows any popup should attempt to display.
/// Keep this consistent across all popups for a uniform feel.
pub(crate) const MAX_POPUP_ROWS: usize = 8;

/// Standard footer hint text used by popups.
pub(crate) fn standard_popup_hint_line() -> Line<'static> {
    standard_popup_hint_line_for_keymap(&ListKeymap {
        move_up: Vec::new(),
        move_down: Vec::new(),
        accept: vec![key_hint::plain(KeyCode::Enter)],
        cancel: vec![key_hint::plain(KeyCode::Esc)],
    })
}

pub(crate) fn standard_popup_hint_line_for_keymap(keymap: &ListKeymap) -> Line<'static> {
    let accept = primary_binding(&keymap.accept).unwrap_or_else(|| key_hint::plain(KeyCode::Enter));
    let cancel = primary_binding(&keymap.cancel).unwrap_or_else(|| key_hint::plain(KeyCode::Esc));
    Line::from(vec![
        "Press ".into(),
        accept.into(),
        " to confirm or ".into(),
        cancel.into(),
        " to go back".into(),
    ])
}
