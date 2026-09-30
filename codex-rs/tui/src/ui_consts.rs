//! Shared layout constants and transcript navigation text for the TUI.

/// Width (in terminal columns) reserved for the left gutter/prefix used by
/// live cells and aligned widgets.
///
/// Semantics:
/// - Chat composer reserves this many columns for the left border + padding.
/// - Status indicator lines begin with this many spaces for alignment.
/// - User history lines account for this many columns (e.g., "▌ ") when wrapping.
pub(crate) const LIVE_PREFIX_COLS: u16 = 2;
pub(crate) const FOOTER_INDENT_COLS: usize = LIVE_PREFIX_COLS as usize;
pub(crate) fn transcript_hint() -> String {
    format!(
        "{} to view transcript",
        crate::key_hint::ctrl(crossterm::event::KeyCode::Char('t')).display_label()
    )
}
