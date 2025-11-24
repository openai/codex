//! Utilities related to FFI bindings.

// If we have std, use it.
#[cfg(windows)]
#[cfg(feature = "std")]
pub use std::os::raw::{c_char, c_int, c_long, c_short, c_uint, c_ulong, c_ushort};
#[cfg(not(windows))]
#[cfg(feature = "std")]
pub use {
    std::ffi::{CStr, CString, FromBytesWithNulError, NulError},
    std::os::raw::c_char,
};

// If we don't have std, we can depend on core and alloc having these features
// in Rust 1.64+.
#[cfg(not(windows))]
#[cfg(all(feature = "alloc", not(feature = "std")))]
pub use alloc::ffi::{CString, NulError};
#[cfg(windows)]
#[cfg(not(feature = "std"))]
pub use core::ffi::{c_char, c_int, c_long, c_short, c_uint, c_ulong, c_ushort, c_void};
#[cfg(not(windows))]
#[cfg(not(feature = "std"))]
pub use core::ffi::{c_char, CStr, FromBytesWithNulError};
