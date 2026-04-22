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
    last_observed_width: Option<u16>,
    last_reflow_width: Option<u16>,
    pending_full_reflow_width: Option<u16>,
    pending_until: Option<Instant>,
    pending_kind: Option<TranscriptReflowKind>,
    ran_during_stream: bool,
    resize_requested_during_stream: bool,
    runtime_disabled: Option<ResizeReflowDisableReason>,
    runtime_disable_warning_shown: bool,
    row_cap_trim_warning_shown: bool,
}

/// Describes how much terminal history repair is needed for a pending resize.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TranscriptReflowKind {
    /// Repaint only the visible transcript rows above the inline viewport.
    VisibleRows,
    /// Purge and rebuild Codex-owned scrollback from source-backed transcript cells.
    Full,
}

/// Describes why resize reflow was disabled for the current transcript lifetime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResizeReflowDisableReason {
    /// Re-rendering transcript cells exceeded the slow-reflow threshold.
    RenderSlow,
    /// Writing reflowed rows back into the terminal exceeded the slow-reflow threshold.
    FlushSlow,
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
        let previous_width = self.last_observed_width.replace(width);
        if previous_width.is_none() {
            self.last_reflow_width = Some(width);
        }
        TranscriptWidthChange {
            changed: previous_width.is_some_and(|previous| previous != width),
            initialized: previous_width.is_none(),
        }
    }

    /// Return whether scrollback still needs to be rebuilt at `width`.
    ///
    /// This compares against the width that actually rebuilt scrollback, not just the most recently
    /// observed terminal width. A terminal can report the final size after the reflow that handled
    /// the resize event, so the follow-up draw must be able to request one more full reflow even if
    /// the observed-width tracker already saw that value.
    pub(crate) fn full_reflow_needed_for_width(&self, width: u16) -> bool {
        self.last_reflow_width != Some(width) && self.pending_full_reflow_width != Some(width)
    }

    /// Schedule a trailing-debounced reflow. Returns false because resize events wait for a quiet
    /// period before rebuilding terminal scrollback.
    ///
    /// Repeated resize events push the deadline out so dragging a terminal edge rebuilds scrollback
    /// at the final observed width rather than at intermediate widths.
    pub(crate) fn schedule_debounced(
        &mut self,
        kind: TranscriptReflowKind,
        target_width: Option<u16>,
    ) -> bool {
        debug_assert!(!self.is_runtime_disabled());
        let now = Instant::now();
        if matches!(kind, TranscriptReflowKind::Full) {
            self.pending_full_reflow_width = target_width;
        }
        self.record_pending_kind(kind);
        self.pending_until = Some(now + TRANSCRIPT_REFLOW_DEBOUNCE);
        false
    }

    /// Schedule an immediate reflow for the next draw opportunity.
    ///
    /// This is used after stream consolidation when waiting for the debounce interval would leave
    /// visible terminal-wrapped stream rows in the finalized transcript.
    pub(crate) fn schedule_immediate(&mut self, kind: TranscriptReflowKind) {
        debug_assert!(!self.is_runtime_disabled());
        if matches!(kind, TranscriptReflowKind::Full) {
            self.pending_full_reflow_width = None;
        }
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
        self.pending_full_reflow_width = None;
    }

    /// Remember the terminal width that actually rebuilt transcript scrollback.
    ///
    /// Resize scheduling is driven by observed widths, but debounced redraws may run before a
    /// terminal emulator has settled on its final size. Keeping the rendered width separate avoids
    /// confusing "seen during a draw" with "scrollback has been repaired at this width".
    pub(crate) fn mark_reflowed_width(&mut self, width: u16) -> bool {
        self.last_reflow_width.replace(width) != Some(width)
    }

    pub(crate) fn clear_runtime_disable(&mut self) {
        self.runtime_disabled = None;
        self.runtime_disable_warning_shown = false;
        self.row_cap_trim_warning_shown = false;
    }

    pub(crate) fn is_runtime_disabled(&self) -> bool {
        self.runtime_disabled.is_some()
    }

    pub(crate) fn runtime_disabled_reason(&self) -> Option<ResizeReflowDisableReason> {
        self.runtime_disabled
    }

    pub(crate) fn take_runtime_disable_warning_needed(&mut self) -> bool {
        if self.runtime_disabled.is_none() || self.runtime_disable_warning_shown {
            return false;
        }
        self.runtime_disable_warning_shown = true;
        true
    }

    pub(crate) fn take_row_cap_trim_warning_needed(&mut self) -> bool {
        if self.row_cap_trim_warning_shown {
            return false;
        }
        self.row_cap_trim_warning_shown = true;
        true
    }

    pub(crate) fn record_elapsed(
        &mut self,
        reason: ResizeReflowDisableReason,
        elapsed: Duration,
        threshold: Duration,
    ) -> bool {
        if elapsed <= threshold || self.runtime_disabled.is_some() {
            return false;
        }

        self.runtime_disabled = Some(reason);
        self.clear_pending_reflow();
        self.clear_stream_flags();
        true
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

/// Describes how the latest draw width relates to the previous observed draw width.
///
/// `initialized` means this was the first width observed by the state machine. `changed` means a
/// previously observed transcript width exists and differs from the new width.
pub(crate) struct TranscriptWidthChange {
    pub(crate) changed: bool,
    pub(crate) initialized: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_debounced_postpones_existing_reflow() {
        let mut state = TranscriptReflowState::default();

        assert!(!state.schedule_debounced(
            TranscriptReflowKind::VisibleRows,
            /*target_width*/ None
        ));
        let first_deadline = state.pending_until().expect("pending reflow");

        std::thread::sleep(Duration::from_millis(1));
        assert!(!state.schedule_debounced(
            TranscriptReflowKind::VisibleRows,
            /*target_width*/ None
        ));

        assert!(
            state.pending_until().expect("pending reflow") > first_deadline,
            "a later resize should push the debounce deadline out"
        );
    }

    #[test]
    fn schedule_debounced_postpones_due_existing_reflow() {
        let mut state = TranscriptReflowState::default();
        state.set_due_for_test();
        let before_reschedule = Instant::now();

        assert!(!state.schedule_debounced(
            TranscriptReflowKind::VisibleRows,
            /*target_width*/ None
        ));
        assert!(
            state.pending_until().expect("pending reflow") > before_reschedule,
            "a resize after the old deadline should start a fresh quiet period"
        );
    }

    #[test]
    fn full_reflow_request_promotes_visible_rows_request() {
        let mut state = TranscriptReflowState::default();

        state.schedule_debounced(
            TranscriptReflowKind::VisibleRows,
            /*target_width*/ None,
        );
        state.schedule_debounced(TranscriptReflowKind::Full, /*target_width*/ Some(100));

        assert_eq!(state.pending_kind(), Some(TranscriptReflowKind::Full));
    }

    #[test]
    fn first_observed_width_marks_reflow_baseline() {
        let mut state = TranscriptReflowState::default();

        let width = state.note_width(/*width*/ 80);

        assert!(width.initialized);
        assert_eq!(state.last_observed_width, Some(80));
        assert_eq!(state.last_reflow_width, Some(80));
        assert!(!state.full_reflow_needed_for_width(/*width*/ 80));
    }

    #[test]
    fn mark_reflowed_width_records_actual_rebuild_width() {
        let mut state = TranscriptReflowState::default();
        state.note_width(/*width*/ 80);

        assert!(state.mark_reflowed_width(/*width*/ 100));

        assert_eq!(state.last_observed_width, Some(80));
        assert_eq!(state.last_reflow_width, Some(100));
    }

    #[test]
    fn full_reflow_needed_compares_against_actual_rebuild_width() {
        let mut state = TranscriptReflowState::default();
        state.note_width(/*width*/ 80);
        state.mark_reflowed_width(/*width*/ 90);
        state.note_width(/*width*/ 100);

        assert!(state.full_reflow_needed_for_width(/*width*/ 100));
    }

    #[test]
    fn pending_full_reflow_target_prevents_repeated_reschedule() {
        let mut state = TranscriptReflowState::default();
        state.note_width(/*width*/ 80);

        assert!(state.full_reflow_needed_for_width(/*width*/ 100));
        state.schedule_debounced(TranscriptReflowKind::Full, /*target_width*/ Some(100));

        assert!(!state.full_reflow_needed_for_width(/*width*/ 100));
    }

    #[test]
    fn clear_pending_reflow_allows_same_width_to_be_rescheduled() {
        let mut state = TranscriptReflowState::default();
        state.note_width(/*width*/ 80);
        state.schedule_debounced(TranscriptReflowKind::Full, /*target_width*/ Some(100));

        state.clear_pending_reflow();

        assert!(state.full_reflow_needed_for_width(/*width*/ 100));
    }

    #[test]
    fn mark_reflowed_width_reports_unchanged_width() {
        let mut state = TranscriptReflowState::default();
        assert!(state.mark_reflowed_width(/*width*/ 100));

        assert!(!state.mark_reflowed_width(/*width*/ 100));
        assert_eq!(state.last_reflow_width, Some(100));
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

    #[test]
    fn below_threshold_measurement_keeps_reflow_active() {
        let mut state = TranscriptReflowState::default();

        assert!(!state.record_elapsed(
            ResizeReflowDisableReason::RenderSlow,
            Duration::from_millis(/*millis*/ 249),
            Duration::from_millis(/*millis*/ 250),
        ));

        assert!(!state.is_runtime_disabled());
        assert_eq!(state.runtime_disabled_reason(), None);
    }

    #[test]
    fn slow_render_measurement_disables_reflow() {
        let mut state = TranscriptReflowState::default();

        assert!(state.record_elapsed(
            ResizeReflowDisableReason::RenderSlow,
            Duration::from_millis(/*millis*/ 251),
            Duration::from_millis(/*millis*/ 250),
        ));

        assert!(state.is_runtime_disabled());
        assert_eq!(
            state.runtime_disabled_reason(),
            Some(ResizeReflowDisableReason::RenderSlow)
        );
    }

    #[test]
    fn runtime_disable_warning_is_reported_once() {
        let mut state = TranscriptReflowState::default();

        assert!(!state.take_runtime_disable_warning_needed());
        assert!(state.record_elapsed(
            ResizeReflowDisableReason::RenderSlow,
            Duration::from_millis(/*millis*/ 251),
            Duration::from_millis(/*millis*/ 250),
        ));

        assert!(state.take_runtime_disable_warning_needed());
        assert!(!state.take_runtime_disable_warning_needed());
    }

    #[test]
    fn slow_flush_measurement_disables_reflow() {
        let mut state = TranscriptReflowState::default();

        assert!(state.record_elapsed(
            ResizeReflowDisableReason::FlushSlow,
            Duration::from_millis(/*millis*/ 251),
            Duration::from_millis(/*millis*/ 250),
        ));

        assert!(state.is_runtime_disabled());
        assert_eq!(
            state.runtime_disabled_reason(),
            Some(ResizeReflowDisableReason::FlushSlow)
        );
    }

    #[test]
    fn row_cap_trim_warning_is_reported_once() {
        let mut state = TranscriptReflowState::default();

        assert!(state.take_row_cap_trim_warning_needed());
        assert!(!state.take_row_cap_trim_warning_needed());
    }

    #[test]
    fn clear_resets_runtime_disable() {
        let mut state = TranscriptReflowState::default();
        state.record_elapsed(
            ResizeReflowDisableReason::RenderSlow,
            Duration::from_millis(/*millis*/ 251),
            Duration::from_millis(/*millis*/ 250),
        );

        state.clear();

        assert!(!state.is_runtime_disabled());
        assert_eq!(state.runtime_disabled_reason(), None);
    }

    #[test]
    fn clear_runtime_disable_preserves_pending_reflow() {
        let mut state = TranscriptReflowState::default();
        state.record_elapsed(
            ResizeReflowDisableReason::RenderSlow,
            Duration::from_millis(/*millis*/ 251),
            Duration::from_millis(/*millis*/ 250),
        );
        state.pending_until = Some(Instant::now());
        state.pending_kind = Some(TranscriptReflowKind::Full);

        state.clear_runtime_disable();

        assert!(!state.is_runtime_disabled());
        assert!(state.has_pending_reflow());
    }

    #[test]
    fn clear_pending_reflow_preserves_runtime_disable() {
        let mut state = TranscriptReflowState::default();
        state.record_elapsed(
            ResizeReflowDisableReason::RenderSlow,
            Duration::from_millis(/*millis*/ 251),
            Duration::from_millis(/*millis*/ 250),
        );

        state.clear_pending_reflow();

        assert!(state.is_runtime_disabled());
        assert_eq!(
            state.runtime_disabled_reason(),
            Some(ResizeReflowDisableReason::RenderSlow)
        );
    }
}
