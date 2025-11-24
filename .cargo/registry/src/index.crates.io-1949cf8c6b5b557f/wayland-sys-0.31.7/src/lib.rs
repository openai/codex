//! FFI bindings to the wayland system libraries.
//!
//! The names exported by this crate should *not* be used directly, but through
//! the `ffi_dispatch` macro, like this:
//!
//! ```ignore
//! ffi_dispatch!(HANDLE_NAME, func_name, arg1, arg2, arg3);
//! ```
//!
//! Where `HANDLE_NAME` is the name of the handle generated if the cargo feature `dlopen` is on.
//!
//! For this to work, you must ensure every needed symbol is in scope (aka the static handle
//! if `dlopen` is on, the extern function if not). The easiest way to do this is to glob import
//! the appropriate module. For example:
//!
//! ```ignore
//! #[macro_use] extern crate wayland_sys;
//!
//! use wayland_sys::client::*;
//!
//! let display_ptr = unsafe {
//!         ffi_dispatch!(wayland_client_handle(), wl_display_connect, ::std::ptr::null())
//! };
//! ```
//!
//! Each module except `common` corresponds to a system library. They all define a function named
//! `is_lib_available()` which returns whether the library could be loaded. They always return true
//! if the feature `dlopen` is absent, as we link against the library directly in that case.
#![allow(non_camel_case_types)]
#![forbid(improper_ctypes, unsafe_op_in_unsafe_fn)]
// Doc feature labels can be tested locally by running RUSTDOCFLAGS="--cfg=docsrs" cargo +nightly doc -p <crate>
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

// If compiling with neither the `client` or `server` feature (non-sensical but
// it's what happens when running `cargo test --all` from the workspace root),
// dlib isn't actually used. This is not an issue, so don't warn about it.
#[allow(unused_imports)]
#[cfg(any(feature = "client", feature = "server"))]
#[macro_use]
extern crate dlib;

pub mod common;

pub mod client;

pub mod server;

#[cfg(all(feature = "egl", feature = "client"))]
pub mod egl;

#[cfg(all(feature = "cursor", feature = "client"))]
pub mod cursor;

#[cfg(feature = "server")]
pub use libc::{gid_t, pid_t, uid_t};

// We cannot just reexport dlib::ffi_dispatch, because it'd then
// use the "dlopen" feature *on the crate invoking it* rather than
// the "dlopen" feature of wayland-sys.

#[cfg(feature = "dlopen")]
#[macro_export]
macro_rules! ffi_dispatch(
    ($handle: expr, $func: ident $(, $arg: expr)* $(,)?) => (
        ($handle.$func)($($arg),*)
    )
);

#[cfg(not(feature = "dlopen"))]
#[macro_export]
macro_rules! ffi_dispatch(
    ($handle: expr, $func: ident $(, $arg: expr)* $(,)?) => (
        $func($($arg),*)
    )
);
