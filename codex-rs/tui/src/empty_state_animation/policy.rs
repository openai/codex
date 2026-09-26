//! Positive eligibility for the fresh-conversation decoration.
//! Unknown history cell types are activity, even when they render no visible text.

use crate::history_cell;
use crate::history_cell::HistoryCell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Presentation {
    Hidden,
    Animated,
    Faded,
}

/// Startup metadata is the entire allowlist; new content types opt out by default.
pub(crate) fn is_startup_cell(cell: &dyn HistoryCell) -> bool {
    cell.as_any().is::<history_cell::SessionHeaderHistoryCell>()
        || cell.as_any().is::<history_cell::SessionInfoCell>()
        || cell.as_any().is::<history_cell::StartupWarningsCell>()
        || cell.as_any().is::<history_cell::DeprecationNoticeCell>()
        || cell
            .as_any()
            .is::<history_cell::UpdateAvailableHistoryCell>()
        || cell.as_any().is::<history_cell::SessionNoticeCell>()
        || cell
            .as_any()
            .downcast_ref::<history_cell::WarningHistoryCell>()
            .is_some_and(|warning| warning.server_version_notice)
}
