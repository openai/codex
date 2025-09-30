#![allow(dead_code)]
//! Feature-gated bootstrap for the config-driven scheduler.

#[cfg(feature = "scheduler")]
pub fn start_if_enabled() {
    // Fire-and-forget task; the scheduler handles its own config gating.
    tokio::spawn(async move {
        if let Err(e) = codex_scheduler::start_scheduler_if_configured().await {
            tracing::warn!("scheduler bootstrap failed: {e:#}");
        }
    });
}

#[cfg(not(feature = "scheduler"))]
pub fn start_if_enabled() {
    // no-op when feature is disabled
}

