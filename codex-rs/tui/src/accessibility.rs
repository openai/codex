//! Accessibility configuration for the TUI.
//!
//! This module provides functionality to disable animations for accessibility purposes.
//! Animations can be disabled via the `--no-animations` CLI flag.

use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

static CLI_ANIMATIONS_DISABLED: AtomicBool = AtomicBool::new(false);

/// Set whether animations are disabled via CLI flag.
pub(crate) fn set_cli_animations_disabled(value: bool) {
    CLI_ANIMATIONS_DISABLED.store(value, Ordering::Relaxed);
}

/// Check if animations are disabled via CLI flag.
fn animations_disabled_by_cli() -> bool {
    CLI_ANIMATIONS_DISABLED.load(Ordering::Relaxed)
}

/// Check if animations are enabled (i.e., not disabled by CLI flag).
pub(crate) fn animations_enabled() -> bool {
    !animations_disabled_by_cli()
}

#[cfg(test)]
pub(crate) fn with_cli_animations_disabled_for_tests<F, R>(value: bool, f: F) -> R
where
    F: FnOnce() -> R,
{
    let previous = animations_disabled_by_cli();
    set_cli_animations_disabled(value);
    let result = f();
    set_cli_animations_disabled(previous);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn animations_enabled_by_default() {
        with_cli_animations_disabled_for_tests(false, || {
            assert_eq!(animations_enabled(), true);
        });
    }

    #[test]
    fn animations_can_be_disabled_via_cli() {
        with_cli_animations_disabled_for_tests(true, || {
            assert_eq!(animations_enabled(), false);
        });
    }
}
