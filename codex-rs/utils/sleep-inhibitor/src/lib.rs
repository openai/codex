//! Cross-platform helper for preventing idle sleep while a turn is running.
//!
//! On macOS this uses native IOKit power assertions instead of spawning
//! `caffeinate`, so assertion lifecycle is tied directly to Rust object lifetime.

#[cfg(target_os = "macos")]
mod macos_inhibitor;

#[cfg(target_os = "macos")]
use macos_inhibitor::MacSleepAssertion;
#[cfg(target_os = "macos")]
use macos_inhibitor::MacSleepAssertionError;
#[cfg(target_os = "macos")]
use tracing::warn;

#[cfg(target_os = "macos")]
const ASSERTION_REASON: &str = "Codex is running an active turn";

/// Keeps the machine awake while a turn is in progress when enabled.
#[derive(Debug)]
pub struct SleepInhibitor {
    enabled: bool,
    #[cfg(target_os = "macos")]
    assertion: Option<MacSleepAssertion>,
}

impl SleepInhibitor {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            #[cfg(target_os = "macos")]
            assertion: None,
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
        #[cfg(target_os = "macos")]
        {
            if self.assertion.is_some() {
                return;
            }
            match MacSleepAssertion::create(ASSERTION_REASON) {
                Ok(assertion) => {
                    self.assertion = Some(assertion);
                }
                Err(error) => match error {
                    MacSleepAssertionError::ApiUnavailable(reason) => {
                        warn!(reason, "Failed to create macOS sleep-prevention assertion");
                    }
                    MacSleepAssertionError::Iokit(code) => {
                        warn!(
                            iokit_error = code,
                            "Failed to create macOS sleep-prevention assertion"
                        );
                    }
                },
            }
        }
    }

    fn release(&mut self) {
        #[cfg(target_os = "macos")]
        {
            // Dropping the assertion releases it via `MacSleepAssertion::drop`.
            self.assertion = None;
        }
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
