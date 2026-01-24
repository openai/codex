//! Common error utilities for cocode-rs.

pub mod ext;
pub mod status_code;

// Re-export snafu and snafu-virtstack for convenience
pub use snafu;
pub use snafu::Location;
pub use snafu_virtstack::{VirtualStackTrace, stack_trace_debug};

pub use ext::ErrorExt;
pub use status_code::StatusCode;
