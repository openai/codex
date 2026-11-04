//! Keep a held activation key from triggering another action.
//!
//! Terminals with enhanced keyboard reporting distinguish a held `Repeat` from a fresh `Press`.
//! A view transition on a press-only terminal can use a bounded window to block ambiguous presses.

use crate::key_hint::KeyBinding;
use crossterm::event::KeyEventKind;
use std::time::Duration;
use std::time::Instant;

const INITIAL_GUARD: Duration = Duration::from_secs(/*secs*/ 1);
const LEGACY_REPEAT_GUARD: Duration = Duration::from_secs(/*secs*/ 1);

pub(crate) enum ActivationKeySuppression {
    ViewTransition,
    ReportedRepeats,
}

#[derive(Clone, Copy)]
pub(crate) struct ActivationKeyGuard {
    key: KeyBinding,
    block_until: Instant,
    legacy_repeat_fallback: bool,
    active: bool,
}

impl ActivationKeyGuard {
    pub(crate) fn new_at(key: KeyBinding, now: Instant, reported_repeats_only: bool) -> Self {
        Self {
            key,
            block_until: now.checked_add(INITIAL_GUARD).unwrap_or(now),
            legacy_repeat_fallback: !reported_repeats_only,
            active: true,
        }
    }

    pub(crate) fn consume(&mut self, key: KeyBinding, kind: KeyEventKind) -> bool {
        self.consume_at(key, kind, Instant::now())
    }

    fn consume_at(&mut self, key: KeyBinding, kind: KeyEventKind, now: Instant) -> bool {
        if !self.active {
            return false;
        }

        if self.key.is_same_physical_key(key) {
            if kind == KeyEventKind::Release {
                self.active = false;
                return true;
            }
            // Without the legacy fallback, only a reported Repeat is treated as held.
            if !self.legacy_repeat_fallback && kind == KeyEventKind::Press {
                self.active = false;
                return false;
            }
            if kind == KeyEventKind::Repeat || now <= self.block_until {
                let repeat_until = now.checked_add(LEGACY_REPEAT_GUARD).unwrap_or(now);
                self.block_until = self.block_until.max(repeat_until);
                return true;
            }
            self.active = false;
        } else if matches!(kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            self.active = false;
        }

        false
    }

    pub(crate) fn is_active(self) -> bool {
        self.active
    }
}

#[cfg(test)]
#[path = "activation_key_guard_tests.rs"]
mod tests;
