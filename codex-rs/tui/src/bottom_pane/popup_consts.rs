//! Shared popup constants for bottom-pane overlays.
//!
//! The bottom pane has multiple popups (selection lists, approvals, file search) that should feel
//! consistent in height and in their footer hints. This module centralizes those shared values so
//! individual widgets can avoid hard-coding layout numbers or rephrasing the same hint text.
//!
//! These constants are UI-facing rather than behavioral: they define how popups present
//! themselves, but the owning widget is still responsible for deciding when to show a popup and
//! how to handle its input.
//!
//! The shared footer hint returned here standardizes the `Enter`/`Esc` wording and styling via
//! [`crate::key_hint`], so every popup uses the same key glyph treatment.

use crossterm::event::KeyCode;
use ratatui::text::Line;

use crate::key_hint;

/// Maximum number of rows any popup should attempt to display.
///
/// Keeping this consistent across popups produces a stable layout as users switch between
/// overlays, even when the underlying list is longer.
pub(crate) const MAX_POPUP_ROWS: usize = 8;

/// Returns the shared footer hint used by bottom-pane popups.
///
/// The `Enter`/`Esc` hint text is intentionally centralized so all popups render the same wording
/// and key glyph styling via [`crate::key_hint`].
pub(crate) fn standard_popup_hint_line() -> Line<'static> {
    Line::from(vec![
        "Press ".into(),
        key_hint::plain(KeyCode::Enter).into(),
        " to confirm or ".into(),
        key_hint::plain(KeyCode::Esc).into(),
        " to go back".into(),
    ])
}
