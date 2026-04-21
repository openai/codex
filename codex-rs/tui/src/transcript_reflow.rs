//! Tracks when Codex-owned transcript scrollback must be repaired after terminal resize.
//!
//! Terminal scrollback is not a retained widget tree: once Codex writes wrapped lines into the
//! terminal, the terminal owns those rows. Width resize reflow treats the in-memory transcript cells
//! as the source of truth, clears Codex-owned history, and re-emits the cells at the current width.
//! Height-only growth uses a narrower repaint that fills rows exposed above the inline viewport
//! without purging scrollback, because wrapping did not change.
//!
//! This module owns only scheduling and stream-time repair state. It does not know how to render
//! cells or clear terminal output; `app::resize_reflow` consumes this state and performs the
//! rebuild. The key invariant is that a full reflow request which happens while streaming output is
//! active, or while transient stream cells are still waiting for consolidation, must trigger one
//! final reflow after the stream becomes source-backed history.

use std::time::Duration;
use std::time::Instant;

pub(crate) const TRANSCRIPT_REFLOW_DEBOUNCE: Duration = Duration::from_millis(75);

#[derive(Debug, Default)]
pub(crate) struct TranscriptReflowState {
    last_render_width: Option<u16>,
    pending_until: Option<Instant>,
    pending_kind: Option<TranscriptReflowKind>,
    ran_during_stream: bool,
    resize_requested_during_stream: bool,
}

/// Describes how much terminal history repair is needed for a pending resize.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TranscriptReflowKind {
    /// Repaint only the visible transcript rows above the inline viewport.
    VisibleRows,
    /// Purge and rebuild Codex-owned scrollback from source-backed transcript cells.
    Full,
}

impl TranscriptReflowState {
    /// Reset all width, pending deadline, and stream repair state.
    ///
    /// Call this when resize reflow is disabled or when the app discards the transcript state that
    /// pending reflow work would have rebuilt. Leaving stale deadlines behind would make a later
    /// draw attempt to rebuild history from unrelated cells.
    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }

    /// Record the width observed during a draw and report whether it is new or changed.
    ///
    /// The first observed width initializes the state without scheduling a rebuild because no
    /// old-width transcript has been emitted yet. Treating initialization as a real resize would
    /// make the first draw do redundant scrollback work.
    pub(crate) fn note_width(&mut self, width: u16) -> TranscriptWidthChange {
        let previous_width = self.last_render_width.replace(width);
        TranscriptWidthChange {
            changed: previous_width.is_some_and(|previous| previous != width),
            initialized: previous_width.is_none(),
        }
    }

    /// Schedule a coalesced reflow. Returns true if the pending reflow is already due.
    ///
    /// Repeated resize events keep the existing deadline instead of pushing it out, so continuous
    /// resizing cannot postpone scrollback repair indefinitely.
    pub(crate) fn schedule_debounced(&mut self, kind: TranscriptReflowKind) -> bool {
        let now = Instant::now();
        let due_now = self.pending_is_due(now);
        self.record_pending_kind(kind);
        if self.pending_until.is_none() {
            self.pending_until = Some(now + TRANSCRIPT_REFLOW_DEBOUNCE);
        }
        due_now
    }

    /// Schedule an immediate reflow for the next draw opportunity.
    ///
    /// This is used after stream consolidation when waiting for the debounce interval would leave
    /// visible terminal-wrapped stream rows in the finalized transcript.
    pub(crate) fn schedule_immediate(&mut self, kind: TranscriptReflowKind) {
        self.record_pending_kind(kind);
        self.pending_until = Some(Instant::now());
    }

    fn record_pending_kind(&mut self, kind: TranscriptReflowKind) {
        self.pending_kind = Some(match (self.pending_kind, kind) {
            (Some(TranscriptReflowKind::Full), _) | (_, TranscriptReflowKind::Full) => {
                TranscriptReflowKind::Full
            }
            _ => TranscriptReflowKind::VisibleRows,
        });
    }

    #[cfg(test)]
    pub(crate) fn set_due_for_test(&mut self) {
        self.pending_until = Some(Instant::now() - Duration::from_millis(1));
    }

    pub(crate) fn pending_is_due(&self, now: Instant) -> bool {
        self.pending_until.is_some_and(|deadline| now >= deadline)
    }

    pub(crate) fn pending_until(&self) -> Option<Instant> {
        self.pending_until
    }

    pub(crate) fn pending_kind(&self) -> Option<TranscriptReflowKind> {
        self.pending_kind
    }

    pub(crate) fn has_pending_reflow(&self) -> bool {
        self.pending_until.is_some()
    }

    pub(crate) fn clear_pending_reflow(&mut self) {
        self.pending_until = None;
        self.pending_kind = None;
    }

    /// Remember that a reflow actually rebuilt history before stream consolidation completed.
    ///
    /// A mid-stream rebuild can only render the transient stream cells that exist at that moment.
    /// The consolidation handler must later rebuild again from the finalized source-backed cell or
    /// the transcript can keep old stream wrapping.
    pub(crate) fn mark_ran_during_stream(&mut self) {
        self.ran_during_stream = true;
    }

    /// Remember that the terminal width changed while streaming or pre-consolidation cells existed.
    ///
    /// This captures the case where the debounce did not fire before the stream finished. Without
    /// this flag, consolidation could complete without the final source-backed resize repair.
    pub(crate) fn mark_resize_requested_during_stream(&mut self) {
        self.resize_requested_during_stream = true;
    }

    /// Return whether stream finalization needs a source-backed reflow and clear the request.
    ///
    /// This is a draining read because each resize-during-stream episode should force at most one
    /// post-consolidation repair. Calling it before consolidation would drop the repair request and
    /// leave finalized scrollback shaped by transient stream rows.
    pub(crate) fn take_stream_finish_reflow_needed(&mut self) -> bool {
        let needed = self.ran_during_stream || self.resize_requested_during_stream;
        self.ran_during_stream = false;
        self.resize_requested_during_stream = false;
        needed
    }

    /// Clear only the stream repair flags while preserving width and pending-deadline state.
    ///
    /// Use this after a required final stream reflow has completed. Calling `clear()` here would
    /// also forget the last observed width and make the next draw look like first initialization.
    pub(crate) fn clear_stream_flags(&mut self) {
        self.ran_during_stream = false;
        self.resize_requested_during_stream = false;
    }
}

/// Describes how the latest draw width relates to the previous draw width.
///
/// `initialized` means this was the first width observed by the state machine. `changed` means a
/// previously rendered transcript width exists and differs from the new width.
pub(crate) struct TranscriptWidthChange {
    pub(crate) changed: bool,
    pub(crate) initialized: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_debounced_does_not_postpone_existing_reflow() {
        let mut state = TranscriptReflowState::default();

        assert!(!state.schedule_debounced(TranscriptReflowKind::VisibleRows));
        let first_deadline = state.pending_until().expect("pending reflow");

        std::thread::sleep(Duration::from_millis(1));
        assert!(!state.schedule_debounced(TranscriptReflowKind::VisibleRows));

        assert_eq!(state.pending_until(), Some(first_deadline));
    }

    #[test]
    fn schedule_debounced_reports_due_existing_reflow() {
        let mut state = TranscriptReflowState::default();
        state.set_due_for_test();

        assert!(state.schedule_debounced(TranscriptReflowKind::VisibleRows));
    }

    #[test]
    fn full_reflow_request_promotes_visible_rows_request() {
        let mut state = TranscriptReflowState::default();

        state.schedule_debounced(TranscriptReflowKind::VisibleRows);
        state.schedule_debounced(TranscriptReflowKind::Full);

        assert_eq!(state.pending_kind(), Some(TranscriptReflowKind::Full));
    }

    #[test]
    fn take_stream_finish_reflow_needed_drains_resize_request() {
        let mut state = TranscriptReflowState::default();
        state.mark_resize_requested_during_stream();

        assert!(state.take_stream_finish_reflow_needed());
        assert!(!state.take_stream_finish_reflow_needed());
    }

    #[test]
    fn take_stream_finish_reflow_needed_drains_ran_during_stream() {
        let mut state = TranscriptReflowState::default();
        state.mark_ran_during_stream();

        assert!(state.take_stream_finish_reflow_needed());
        assert!(!state.take_stream_finish_reflow_needed());
    }

    #[test]
    fn clear_resets_stream_reflow_flags() {
        let mut state = TranscriptReflowState::default();
        state.mark_ran_during_stream();
        state.mark_resize_requested_during_stream();

        state.clear();

        assert!(!state.take_stream_finish_reflow_needed());
    }
}
