//! Alternate-screen nesting guard.
//!
//! The main `codex-tui2` UI typically runs inside the terminal’s alternate screen buffer so the
//! full viewport can be used without polluting normal scrollback. Some sub-flows (e.g. pager-style
//! overlays) also call `enter_alt_screen()`/`leave_alt_screen()` for historical reasons.
//!
//! Those calls are conceptually “idempotent” (the UI is already on the alt screen), but the
//! underlying terminal commands are *not*: issuing a real `LeaveAlternateScreen` while the rest of
//! the app still thinks it is drawing on the alternate buffer desynchronizes rendering and can
//! leave stale characters behind when returning to the normal view.
//!
//! `AltScreenNesting` tracks a small nesting depth so only the outermost enter/leave actually
//! toggles the terminal mode.

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AltScreenNesting {
    depth: u16,
}

impl AltScreenNesting {
    pub(crate) fn is_active(self) -> bool {
        self.depth > 0
    }

    /// Record an enter-alt-screen request.
    ///
    /// Returns `true` when the caller should actually enter the alternate screen.
    pub(crate) fn enter(&mut self) -> bool {
        if self.depth == 0 {
            self.depth = 1;
            true
        } else {
            self.depth = self.depth.saturating_add(1);
            false
        }
    }

    /// Record a leave-alt-screen request.
    ///
    /// Returns `true` when the caller should actually leave the alternate screen.
    pub(crate) fn leave(&mut self) -> bool {
        match self.depth {
            0 => false,
            1 => {
                self.depth = 0;
                true
            }
            _ => {
                self.depth = self.depth.saturating_sub(1);
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AltScreenNesting;
    use pretty_assertions::assert_eq;

    #[test]
    fn alt_screen_nesting_tracks_outermost_transitions() {
        let mut nesting = AltScreenNesting::default();
        assert_eq!(false, nesting.is_active());

        assert_eq!(true, nesting.enter());
        assert_eq!(true, nesting.is_active());

        assert_eq!(false, nesting.enter());
        assert_eq!(true, nesting.is_active());

        assert_eq!(false, nesting.leave());
        assert_eq!(true, nesting.is_active());

        assert_eq!(true, nesting.leave());
        assert_eq!(false, nesting.is_active());

        assert_eq!(false, nesting.leave());
        assert_eq!(false, nesting.is_active());
    }
}
