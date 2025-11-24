// We have a lot of c-types in here, stop warning about their names!
#![allow(non_camel_case_types)]
// fmt::Debug isn't helpful on FFI types
#![allow(missing_debug_implementations)]
// unreachable_pub warns `#[no_mangle] pub extern fn` in private mod.
#![allow(unreachable_pub)]

//! # hyper C API
//!
//! This part of the documentation describes the C API for hyper. That is, how
//! to *use* the hyper library in C code. This is **not** a regular Rust
//! module, and thus it is not accessible in Rust.
//!
//! ## Unstable
//!
//! The C API of hyper is currently **unstable**, which means it's not part of
//! the semver contract as the rest of the Rust API is. Because of that, it's
//! only accessible if `--cfg hyper_unstable_ffi` is passed to `rustc` when
//! compiling. The easiest way to do that is setting the `RUSTFLAGS`
//! environment variable.
//!
//! ## Building
//!
//! The C API is part of the Rust library, but isn't compiled by default. Using
//! `cargo`, staring with `1.64.0`, it can be compiled with the following command:
//!
//! ```notrust
//! RUSTFLAGS="--cfg hyper_unstable_ffi" cargo rustc --crate-type cdylib --features client,http1,http2,ffi
//! ```

// We may eventually allow the FFI to be enabled without `client` or `http1`,
// that is why we don't auto enable them as `ffi = ["client", "http1"]` in
// the `Cargo.toml`.
//
// But for now, give a clear message that this compile error is expected.
#[cfg(not(all(feature = "client", feature = "http1")))]
compile_error!("The `ffi` feature currently requires the `client` and `http1` features.");

#[cfg(not(hyper_unstable_ffi))]
compile_error!(
    "\
    The `ffi` feature is unstable, and requires the \
    `RUSTFLAGS='--cfg hyper_unstable_ffi'` environment variable to be set.\
"
);

#[macro_use]
mod macros;

mod body;
mod client;
mod error;
mod http_types;
mod io;
mod task;

pub use self::body::*;
pub use self::client::*;
pub use self::error::*;
pub use self::http_types::*;
pub use self::io::*;
pub use self::task::*;

/// Return in iter functions to continue iterating.
pub const HYPER_ITER_CONTINUE: std::ffi::c_int = 0;
/// Return in iter functions to stop iterating.
#[allow(unused)]
pub const HYPER_ITER_BREAK: std::ffi::c_int = 1;

/// An HTTP Version that is unspecified.
pub const HYPER_HTTP_VERSION_NONE: std::ffi::c_int = 0;
/// The HTTP/1.0 version.
pub const HYPER_HTTP_VERSION_1_0: std::ffi::c_int = 10;
/// The HTTP/1.1 version.
pub const HYPER_HTTP_VERSION_1_1: std::ffi::c_int = 11;
/// The HTTP/2 version.
pub const HYPER_HTTP_VERSION_2: std::ffi::c_int = 20;

#[derive(Clone)]
struct UserDataPointer(*mut std::ffi::c_void);

// We don't actually know anything about this pointer, it's up to the user
// to do the right thing.
unsafe impl Send for UserDataPointer {}
unsafe impl Sync for UserDataPointer {}

/// cbindgen:ignore
static VERSION_CSTR: &str = concat!(env!("CARGO_PKG_VERSION"), "\0");

// `core::ffi::c_size_t` is a nightly-only experimental API.
// https://github.com/rust-lang/rust/issues/88345
type size_t = usize;

ffi_fn! {
    /// Returns a static ASCII (null terminated) string of the hyper version.
    fn hyper_version() -> *const std::ffi::c_char {
        VERSION_CSTR.as_ptr() as _
    } ?= std::ptr::null()
}
