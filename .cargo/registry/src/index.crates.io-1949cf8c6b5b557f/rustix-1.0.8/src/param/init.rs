//! rustix's `init` function.
//!
//! # Safety
//!
//! When "use-explicitly-provided-auxv" is enabled, the `init` function must be
//! called before any other function in this module. It is unsafe because it
//! operates on raw pointers.
#![allow(unsafe_code)]

use crate::backend;

/// Initialize process-wide state.
///
/// # Safety
///
/// This must be passed a pointer to the original environment variable block
/// set up by the OS at process startup, and it must be called before any other
/// rustix functions are called.
#[inline]
#[doc(hidden)]
pub unsafe fn init(envp: *mut *mut u8) {
    backend::param::auxv::init(envp)
}
