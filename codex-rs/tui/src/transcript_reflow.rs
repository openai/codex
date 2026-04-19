//! State machine for transcript scrollback reflow.

use std::time::Duration;
use std::time::Instant;

pub(crate) const TRANSCRIPT_REFLOW_DEBOUNCE: Duration = Duration::from_millis(75);

#[derive(Debug, Default)]
pub(crate) struct TranscriptReflowState {
    last_render_width: Option<u16>,
    pending_until: Option<Instant>,
    ran_during_stream: bool,
}

impl TranscriptReflowState {
    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn note_width(&mut self, width: u16) -> TranscriptWidthChange {
        let previous_width = self.last_render_width.replace(width);
        TranscriptWidthChange {
            changed: previous_width.is_some_and(|previous| previous != width),
            initialized: previous_width.is_none(),
        }
    }

    #[cfg(test)]
    pub(crate) fn set_last_render_width_for_test(&mut self, width: u16) {
        self.last_render_width = Some(width);
    }

    /// Schedule a coalesced reflow. Returns true if the pending reflow is already due.
    ///
    /// Repeated resize events keep the existing deadline instead of pushing it out, so continuous
    /// resizing cannot postpone scrollback repair indefinitely.
    pub(crate) fn schedule_debounced(&mut self) -> bool {
        let now = Instant::now();
        let due_now = self.pending_is_due(now);
        if self.pending_until.is_none() {
            self.pending_until = Some(now + TRANSCRIPT_REFLOW_DEBOUNCE);
        }
        due_now
    }

    pub(crate) fn schedule_immediate(&mut self) {
        self.pending_until = Some(Instant::now());
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

    pub(crate) fn has_pending_reflow(&self) -> bool {
        self.pending_until.is_some()
    }

    pub(crate) fn clear_pending_reflow(&mut self) {
        self.pending_until = None;
    }

    pub(crate) fn mark_ran_during_stream(&mut self) {
        self.ran_during_stream = true;
    }

    pub(crate) fn take_ran_during_stream(&mut self) -> bool {
        let ran = self.ran_during_stream;
        self.ran_during_stream = false;
        ran
    }

    pub(crate) fn clear_ran_during_stream(&mut self) {
        self.ran_during_stream = false;
    }
}

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

        assert!(!state.schedule_debounced());
        let first_deadline = state.pending_until().expect("pending reflow");

        std::thread::sleep(Duration::from_millis(1));
        assert!(!state.schedule_debounced());

        assert_eq!(state.pending_until(), Some(first_deadline));
    }

    #[test]
    fn schedule_debounced_reports_due_existing_reflow() {
        let mut state = TranscriptReflowState::default();
        state.set_due_for_test();

        assert!(state.schedule_debounced());
    }
}
