//! Minimum supported Rust version: 1.28

use std::num::NonZeroUsize;

#[cfg_attr(any(target_os = "linux", target_os = "android"), path = "linux.rs")]
#[cfg_attr(target_os = "freebsd", path = "freebsd.rs")]
#[cfg_attr(any(target_os = "macos", target_os = "ios"), path = "apple.rs")]
#[cfg_attr(target_os = "aix", path = "aix.rs")]
mod imp;

/// Obtain the number of threads currently part of the active process. Returns `None` if the number
/// of threads cannot be determined.
pub fn num_threads() -> Option<NonZeroUsize> {
    imp::num_threads()
}

/// Determine if the current process is single-threaded. Returns `None` if the number of threads
/// cannot be determined.
pub fn is_single_threaded() -> Option<bool> {
    num_threads().map(|n| n.get() == 1)
}
