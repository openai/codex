//! Re-export of the VT100 test backend for integration tests.
//!
//! Integration tests live under `tests/` and cannot reach crate-private modules directly, so we
//! re-export the shared `VT100Backend` from `src/test_backend.rs` here.

/// Import the shared VT100 backend implementation from the crate sources.
#[path = "../src/test_backend.rs"]
mod inner;

/// VT100 backend used by integration tests.
pub use inner::VT100Backend;
