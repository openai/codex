//! Polyfill of parts of the standard library for `no_std` builds.
//!
//! All code in this subtree is derived from the standard library and licensed
//! MIT or Apache 2.0 at your option.
//!
//! This implementation is used when `std` is not available and polyfills the
//! necessary items from `std`. When the `std` feature is specified (so the
//! standard library is available), the file `src/polyfill/std` is used
//! instead, which just imports the respective items from `std`.

#[cfg(not(windows))]
pub mod io;
#[cfg(not(any(target_os = "redox", target_os = "wasi")))]
#[cfg(feature = "net")]
pub mod net;
pub mod os;
