//! In-Memory exporters for testing purpose.

/// Structs used for testing
#[cfg(all(feature = "testing", feature = "trace"))]
pub mod trace;

#[cfg(all(feature = "testing", feature = "metrics"))]
pub mod metrics;
