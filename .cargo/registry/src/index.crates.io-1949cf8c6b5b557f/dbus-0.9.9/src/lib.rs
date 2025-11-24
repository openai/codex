//! D-Bus bindings for Rust
//!
//! [D-Bus](http://dbus.freedesktop.org/) is a message bus, and is mainly used in Linux
//! for communication between processes. It is present by default on almost every
//! Linux distribution out there, and runs in two instances - one per session, and one
//! system-wide.
//!
//! In addition to the API documentation, which you're currently reading, you might want to
//! look in the examples directory, which contains many examples and some additional documents.
//! README.md also contains a few quick "getting started" examples (as well as information about
//! the `futures` and `no-string-validation` features).
//!
//! In addition to this crate, there are some companion crates:
//!  * dbus-tokio for integrating D-Bus with [Tokio](http://tokio.rs)
//!  * dbus-codegen for generating code from D-Bus introspection data
//!  * libdbus-sys contains the raw bindings to the C libdbus library.

#![warn(missing_docs)]

// We have io-lifetimes and native-channel which are not ready yet
// so for now allow them in the codebase and silence the warning
#![allow(unexpected_cfgs)]

extern crate libc;

#[allow(missing_docs)]
extern crate libdbus_sys as ffi;

pub use crate::message::{Message, MessageType};

pub mod message;

pub mod ffidisp;

mod error;
pub use error::{Error, MethodErr, Result};

pub mod channel;

mod filters;

pub mod blocking;

#[cfg(feature = "futures")]
pub mod nonblock;

pub mod strings;
pub use crate::strings::{Signature, Path};

pub mod arg;

// pub mod tree;

static INITDBUS: std::sync::Once = std::sync::Once::new();

use std::ffi::{CString, CStr};
use std::os::raw::c_char;

fn init_dbus() {
    INITDBUS.call_once(|| {
        if unsafe { ffi::dbus_threads_init_default() } == 0 {
            panic!("Out of memory when trying to initialize D-Bus library!");
        }
    });
}

fn c_str_to_slice(c: & *const c_char) -> Option<&str> {
    if c.is_null() { None }
    else { std::str::from_utf8( unsafe { CStr::from_ptr(*c).to_bytes() }).ok() }
}

fn to_c_str(n: &str) -> CString { CString::new(n.as_bytes()).unwrap() }
