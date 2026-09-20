//! Positive eligibility for the fresh-conversation decoration.
//! Unknown history cell types are activity, even when they render no visible text.

use super::ComposerState;
use crate::history_cell;
use crate::history_cell::HistoryCell;
use crate::motion::MotionMode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Presentation {
    Hidden,
    Animated,
    Faded,
}

impl Presentation {
    /// Only an explicitly available ordinary composer may display a fresh-thread logo.
    pub(crate) fn for_composer(
        composer: Option<ComposerState>,
        motion: MotionMode,
        focused: bool,
    ) -> Self {
        match (composer, motion, focused) {
            (None, _, _) | (_, MotionMode::Reduced, _) => Self::Hidden,
            (Some(ComposerState::Empty), MotionMode::Animated, true) => Self::Animated,
            (Some(ComposerState::Empty), MotionMode::Animated, false)
            | (Some(ComposerState::Draft), MotionMode::Animated, _) => Self::Faded,
        }
    }
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
