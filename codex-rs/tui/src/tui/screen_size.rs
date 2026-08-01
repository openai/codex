//! Terminal geometry policy shared by the main app and standalone screens.

use std::io;
use std::time::Duration;
use std::time::Instant;

use ratatui::layout::Size;

use super::Tui;
use super::TuiEvent;
use crate::transcript_reflow::TRANSCRIPT_REFLOW_DEBOUNCE;

#[derive(Default)]
pub(super) struct ScreenSizePolicy {
    pending_recheck_at: Option<Instant>,
    pending_draw_size: Option<Size>,
    deferred_size: Option<Size>,
}

impl Tui {
    /// Resolve event geometry while avoiding backend queries on ordinary repaint frames.
    pub(crate) fn screen_size_for_event(&mut self, event: &TuiEvent) -> io::Result<Size> {
        if matches!(event, TuiEvent::Resize(_)) {
            self.schedule_screen_size_recheck(TRANSCRIPT_REFLOW_DEBOUNCE);
        } else if matches!(event, TuiEvent::Draw)
            && let Some(deadline) = self.screen_size.pending_recheck_at
        {
            let now = Instant::now();
            if now < deadline {
                self.frame_requester.schedule_frame_in(deadline - now);
            } else {
                return self.screen_size_for_event(&TuiEvent::Resume);
            }
        }

        let cached = self.terminal.last_known_screen_size;
        let size = match event {
            TuiEvent::Resize(size) => {
                self.screen_size.deferred_size = None;
                *size
            }
            TuiEvent::Resume => {
                self.screen_size.pending_recheck_at = None;
                self.screen_size.deferred_size = None;
                self.terminal.size()?
            }
            TuiEvent::Draw => self.screen_size.deferred_size.take().unwrap_or(cached),
            TuiEvent::Key(_) | TuiEvent::Paste(_) => cached,
        };
        self.screen_size.pending_draw_size =
            (!matches!(event, TuiEvent::Key(_) | TuiEvent::Paste(_))).then_some(size);
        Ok(size)
    }

    /// Schedule one backend sample after a resize, reflow, or external program.
    pub(crate) fn schedule_screen_size_recheck(&mut self, delay: Duration) {
        self.screen_size.pending_recheck_at = Some(Instant::now() + delay);
        self.frame_requester.schedule_frame_in(delay);
    }

    pub(crate) fn defer_screen_size(&mut self, size: Size) {
        self.screen_size.deferred_size = Some(size);
    }

    pub(super) fn take_event_screen_size(&mut self) -> io::Result<Size> {
        if let Some(size) = self.screen_size.pending_draw_size.take() {
            return Ok(size);
        }
        let size = self.terminal.size()?;
        self.screen_size.pending_recheck_at = None;
        self.screen_size.deferred_size = None;
        Ok(size)
    }
}

#[cfg(test)]
#[path = "screen_size_tests.rs"]
mod tests;
