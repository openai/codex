//! The following is derived from Rust's
//! library/std/src/os/fd/mod.rs at revision
//! fa68e73e9947be8ffc5b3b46d899e4953a44e7e9.
//!
//! All code in this file is licensed MIT or Apache 2.0 at your option.
//!
//! Owned and borrowed Unix-like file descriptors.
//!
//! This module is supported on Unix platforms and WASI, which both use a
//! similar file descriptor system for referencing OS resources.

#![cfg_attr(staged_api, stable(feature = "os_fd", since = "1.66.0"))]
#![deny(unsafe_op_in_unsafe_fn)]

// `RawFd`, `AsRawFd`, etc.
mod raw;

// `OwnedFd`, `AsFd`, etc.
mod owned;

// Export the types and traits for the public API.
#[cfg_attr(staged_api, stable(feature = "os_fd", since = "1.66.0"))]
pub use owned::*;
#[cfg_attr(staged_api, stable(feature = "os_fd", since = "1.66.0"))]
pub use raw::*;
