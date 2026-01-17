//! Aggregates the legacy integration test modules for the TUI crate.
//!
//! Each module contains tests that used to live as standalone integration
//! binaries; keeping them under a single suite module keeps shared setup in
//! one place while preserving the original test grouping.
mod no_panic_on_startup;
mod status_indicator;
mod vt100_history;
mod vt100_live_commit;
