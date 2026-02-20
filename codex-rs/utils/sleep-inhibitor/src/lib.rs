//! Cross-platform helper for preventing idle sleep while a turn is running.
//!
//! On macOS this uses native IOKit power assertions instead of spawning
//! `caffeinate`, so assertion lifecycle is tied directly to Rust object lifetime.

// Bazel's macOS CI toolchain uses a minimal SDK that omits IOKit, so Bazel
// builds use the no-op backend on macOS while Cargo builds use the native one.
#[cfg(any(not(target_os = "macos"), codex_bazel))]
mod dummy;
#[cfg(all(target_os = "macos", not(codex_bazel)))]
mod macos;

#[cfg(any(not(target_os = "macos"), codex_bazel))]
use dummy as imp;
#[cfg(all(target_os = "macos", not(codex_bazel)))]
use macos as imp;

/// Keeps the machine awake while a turn is in progress when enabled.
#[derive(Debug)]
pub struct SleepInhibitor {
    enabled: bool,
    platform: imp::SleepInhibitor,
}

impl SleepInhibitor {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            platform: imp::SleepInhibitor::new(),
        }
    }

    /// Update the active turn state; turns sleep prevention on/off as needed.
    pub fn set_turn_running(&mut self, turn_running: bool) {
        if !self.enabled {
            self.release();
            return;
        }

        if turn_running {
            self.acquire();
        } else {
            self.release();
        }
    }

    fn acquire(&mut self) {
        self.platform.acquire();
    }

    fn release(&mut self) {
        self.platform.release();
    }
}

#[cfg(test)]
mod tests {
    use super::SleepInhibitor;

    #[test]
    fn sleep_inhibitor_toggles_without_panicking() {
        let mut inhibitor = SleepInhibitor::new(true);
        inhibitor.set_turn_running(true);
        inhibitor.set_turn_running(false);
    }

    #[test]
    fn sleep_inhibitor_disabled_does_not_panic() {
        let mut inhibitor = SleepInhibitor::new(false);
        inhibitor.set_turn_running(true);
        inhibitor.set_turn_running(false);
    }

    #[test]
    fn sleep_inhibitor_multiple_true_calls_are_idempotent() {
        let mut inhibitor = SleepInhibitor::new(true);
        inhibitor.set_turn_running(true);
        inhibitor.set_turn_running(true);
        inhibitor.set_turn_running(true);
        inhibitor.set_turn_running(false);
    }

    #[test]
    fn sleep_inhibitor_can_toggle_multiple_times() {
        let mut inhibitor = SleepInhibitor::new(true);
        inhibitor.set_turn_running(true);
        inhibitor.set_turn_running(false);
        inhibitor.set_turn_running(true);
        inhibitor.set_turn_running(false);
    }
}
