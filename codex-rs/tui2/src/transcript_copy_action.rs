//! Performs "copy selection" and manages transient UI feedback.
//!
//! `transcript_copy` is intentionally pure: it reconstructs clipboard text from a
//! [`TranscriptSelection`], preserving wrapping, indentation, and Markdown markers.
//!
//! This module is the side-effecting layer on top of that pure logic:
//! - writes the reconstructed text to the system clipboard
//! - stores short-lived state so the footer can show `"Copied"` / `"Copy failed"`
//! - schedules redraws so feedback appears promptly and then clears itself
//!
//! Keeping these responsibilities separate reduces cognitive load:
//! - `transcript_copy` answers *what text should be copied?*
//! - `transcript_copy_action` answers *do the copy and tell the user it happened*

use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use crate::history_cell::HistoryCell;
use crate::transcript_scrollbar::split_transcript_area;
use crate::transcript_selection::TranscriptSelection;
use crate::tui;
use ratatui::layout::Rect;

/// User-visible feedback shown briefly after a copy attempt.
///
/// The footer renders this value when present, and it expires automatically.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TranscriptCopyFeedback {
    /// Copy succeeded and the clipboard was updated.
    Copied,
    /// Copy failed (typically due to OS clipboard integration issues).
    Failed,
}

/// The outcome of attempting to copy the current selection.
///
/// This is a compact signal for UI code:
/// - `NoSelection` means the action is a no-op (nothing to dismiss).
/// - `Copied`/`Failed` mean the action was triggered and the selection should be dismissed.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum CopySelectionOutcome {
    /// No active selection exists (or the terminal is too small to compute one).
    NoSelection,
    /// Clipboard write succeeded.
    Copied,
    /// Clipboard write failed.
    Failed,
}

const TRANSCRIPT_COPY_FEEDBACK_DURATION: Duration = Duration::from_millis(1500);

#[derive(Debug, Clone, Copy)]
struct TranscriptCopyFeedbackState {
    kind: TranscriptCopyFeedback,
    expires_at: Instant,
}

/// Performs the copy action and tracks transient footer feedback.
///
/// `App` owns one instance and calls [`Self::copy_and_handle`] when the user triggers "copy
/// selection" (either via the on-screen copy pill or the keyboard shortcut).
#[derive(Debug, Default)]
pub(crate) struct TranscriptCopyAction {
    feedback: Option<TranscriptCopyFeedbackState>,
}

impl TranscriptCopyAction {
    /// Attempt to copy the current selection and record feedback.
    ///
    /// Returns `true` when a copy attempt was made (success or failure). Callers should treat that
    /// as a signal to dismiss the selection highlight.
    pub(crate) fn copy_and_handle(
        &mut self,
        tui: &mut tui::Tui,
        chat_height: u16,
        transcript_cells: &[Arc<dyn HistoryCell>],
        transcript_selection: TranscriptSelection,
    ) -> bool {
        let outcome =
            copy_transcript_selection(tui, chat_height, transcript_cells, transcript_selection);
        self.handle_copy_outcome(tui, outcome)
    }

    /// Return footer feedback to render for the current frame, if any.
    ///
    /// This is called from `App`'s render loop. It clears expired feedback lazily so callers do
    /// not need separate timer plumbing.
    pub(crate) fn footer_feedback(&mut self) -> Option<TranscriptCopyFeedback> {
        let state = self.feedback?;

        if Instant::now() >= state.expires_at {
            self.feedback = None;
            return None;
        }

        Some(state.kind)
    }

    /// Record the outcome of a copy attempt and schedule redraws.
    ///
    /// Returns `true` when a copy attempt happened (success or failure). This is the signal to
    /// dismiss the selection highlight.
    pub(crate) fn handle_copy_outcome(
        &mut self,
        tui: &mut tui::Tui,
        outcome: CopySelectionOutcome,
    ) -> bool {
        match outcome {
            CopySelectionOutcome::NoSelection => false,
            CopySelectionOutcome::Copied => {
                self.set_feedback(tui, TranscriptCopyFeedback::Copied);
                true
            }
            CopySelectionOutcome::Failed => {
                self.set_feedback(tui, TranscriptCopyFeedback::Failed);
                true
            }
        }
    }

    /// Store feedback state and schedule a redraw for its appearance + expiration.
    fn set_feedback(&mut self, tui: &mut tui::Tui, kind: TranscriptCopyFeedback) {
        let expires_at = Instant::now()
            .checked_add(TRANSCRIPT_COPY_FEEDBACK_DURATION)
            .unwrap_or_else(Instant::now);
        self.feedback = Some(TranscriptCopyFeedbackState { kind, expires_at });

        tui.frame_requester().schedule_frame();
        tui.frame_requester()
            .schedule_frame_in(TRANSCRIPT_COPY_FEEDBACK_DURATION);
    }
}

/// Copy the current transcript selection to the system clipboard.
///
/// This function ties together layout validation, selection-to-text reconstruction via
/// `transcript_copy`, and the actual clipboard write.
pub(crate) fn copy_transcript_selection(
    tui: &tui::Tui,
    chat_height: u16,
    transcript_cells: &[Arc<dyn HistoryCell>],
    transcript_selection: TranscriptSelection,
) -> CopySelectionOutcome {
    // This function is intentionally "dumb plumbing":
    // - validate layout prerequisites
    // - reconstruct clipboard text (`transcript_copy`)
    // - write to clipboard
    //
    // UI state management (feedback + redraw scheduling) lives in `TranscriptCopyAction`.
    let size = tui.terminal.last_known_screen_size;
    let width = size.width;
    let height = size.height;
    if width == 0 || height == 0 {
        return CopySelectionOutcome::NoSelection;
    }

    if chat_height >= height {
        return CopySelectionOutcome::NoSelection;
    }

    let transcript_height = height.saturating_sub(chat_height);
    if transcript_height == 0 {
        return CopySelectionOutcome::NoSelection;
    }

    let transcript_full_area = Rect {
        x: 0,
        y: 0,
        width,
        height: transcript_height,
    };
    let (transcript_area, _) = split_transcript_area(transcript_full_area);

    let Some(text) = crate::transcript_copy::selection_to_copy_text_for_cells(
        transcript_cells,
        transcript_selection,
        transcript_area.width,
    ) else {
        return CopySelectionOutcome::NoSelection;
    };

    if let Err(err) = crate::clipboard_copy::copy_text(text) {
        tracing::error!(error = %err, "failed to copy selection to clipboard");
        return CopySelectionOutcome::Failed;
    }

    CopySelectionOutcome::Copied
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn footer_feedback_returns_value_before_expiration() {
        let mut action = TranscriptCopyAction {
            feedback: Some(TranscriptCopyFeedbackState {
                kind: TranscriptCopyFeedback::Copied,
                expires_at: Instant::now() + Duration::from_secs(10),
            }),
        };

        assert_eq!(
            action.footer_feedback(),
            Some(TranscriptCopyFeedback::Copied)
        );
        assert_eq!(
            action.footer_feedback(),
            Some(TranscriptCopyFeedback::Copied)
        );
    }

    #[test]
    fn footer_feedback_clears_after_expiration() {
        let mut action = TranscriptCopyAction {
            feedback: Some(TranscriptCopyFeedbackState {
                kind: TranscriptCopyFeedback::Copied,
                expires_at: Instant::now() - Duration::from_secs(1),
            }),
        };

        assert_eq!(action.footer_feedback(), None);
        assert!(action.feedback.is_none());
        assert_eq!(action.footer_feedback(), None);
    }
}
