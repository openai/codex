//! Aggregates the TUI integration test modules into a single binary.
//!
//! The submodules live under `tests/suite` and are wired here so the test
//! runner can build one integration test binary while still keeping tests
//! grouped by feature area.

/// Provides vt100-backed fixtures when the `vt100-tests` feature is enabled.
#[cfg(feature = "vt100-tests")]
mod test_backend;

#[allow(unused_imports)]
use codex_cli as _; // Keep dev-dep for cargo-shear; tests spawn the codex binary.

/// Collects the suite-style integration tests.
mod suite;
