//! Autopilot preferences shared across TUI components.
//!
//! Keeps lightweight, process-local toggles that can be read by subsystems
//! (e.g., Reviewer integration) without threading state through many layers.

use std::sync::atomic::{AtomicBool, Ordering};

// Defaults are conservative (disabled).
static PATCHGATE_ENABLED: AtomicBool = AtomicBool::new(false);
static PATCHGATE_PERMISSIVE: AtomicBool = AtomicBool::new(false);

pub fn set_patchgate_enabled(on: bool) {
    PATCHGATE_ENABLED.store(on, Ordering::Relaxed);
}

pub fn patchgate_enabled() -> bool {
    PATCHGATE_ENABLED.load(Ordering::Relaxed)
}

pub fn set_patchgate_permissive(on: bool) {
    PATCHGATE_PERMISSIVE.store(on, Ordering::Relaxed);
}

pub fn patchgate_permissive() -> bool {
    PATCHGATE_PERMISSIVE.load(Ordering::Relaxed)
}

