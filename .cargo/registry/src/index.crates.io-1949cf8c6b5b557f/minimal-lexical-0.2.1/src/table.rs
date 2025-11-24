//! Pre-computed tables for parsing float strings.

#![doc(hidden)]

// Re-export all the feature-specific files.
#[cfg(feature = "compact")]
pub use crate::table_bellerophon::*;
#[cfg(not(feature = "compact"))]
pub use crate::table_lemire::*;
#[cfg(not(feature = "compact"))]
pub use crate::table_small::*;
