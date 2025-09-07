use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

// Minimal TUI metrics surface (P2-02). Centralizes counters to avoid ad-hoc statics.

fn enforcement_rejections_cell() -> &'static AtomicU64 {
    static C: OnceLock<AtomicU64> = OnceLock::new();
    C.get_or_init(|| AtomicU64::new(0))
}

/// Increment the enforcement rejection counter (rejections_total{reason="enforcement"}).
pub fn inc_rejections_enforcement() {
    enforcement_rejections_cell().fetch_add(1, Ordering::Relaxed);
}

/// Snapshot the enforcement rejection counter (for debugging/tests if needed).
pub fn get_rejections_enforcement() -> u64 {
    enforcement_rejections_cell().load(Ordering::Relaxed)
}

fn ci_pre_cell() -> &'static AtomicU64 {
    static C: OnceLock<AtomicU64> = OnceLock::new();
    C.get_or_init(|| AtomicU64::new(0))
}

fn ci_post_cell() -> &'static AtomicU64 {
    static C: OnceLock<AtomicU64> = OnceLock::new();
    C.get_or_init(|| AtomicU64::new(0))
}

fn apply_millis_cell() -> &'static AtomicU64 {
    static C: OnceLock<AtomicU64> = OnceLock::new();
    C.get_or_init(|| AtomicU64::new(0))
}

/// Increment CI runs counter for phase=pre.
#[derive(Copy, Clone)]
pub enum Phase { Pre, Post }

#[derive(Copy, Clone)]
pub enum Reason { Enforcement }

/// Increment CI runs counter for given phase.
pub fn inc_ci_runs(phase: Phase) {
    match phase {
        Phase::Pre => { ci_pre_cell().fetch_add(1, Ordering::Relaxed); }
        Phase::Post => { ci_post_cell().fetch_add(1, Ordering::Relaxed); }
    }
}
/// Set last apply duration in milliseconds.
pub fn set_apply_millis(ms: u64) { apply_millis_cell().store(ms, Ordering::Relaxed); }

pub fn get_ci_runs(phase: Phase) -> u64 {
    match phase {
        Phase::Pre => ci_pre_cell().load(Ordering::Relaxed),
        Phase::Post => ci_post_cell().load(Ordering::Relaxed),
    }
}

/// Increment the rejection counter with a fixed-label reason.
pub fn inc_rejections(reason: Reason) {
    match reason {
        Reason::Enforcement => { enforcement_rejections_cell().fetch_add(1, Ordering::Relaxed); }
    }
}

/// Get current rejection counts for the reason label.
pub fn get_rejections(reason: Reason) -> u64 {
    match reason {
        Reason::Enforcement => enforcement_rejections_cell().load(Ordering::Relaxed),
    }
}
pub fn get_apply_millis() -> u64 { apply_millis_cell().load(Ordering::Relaxed) }
