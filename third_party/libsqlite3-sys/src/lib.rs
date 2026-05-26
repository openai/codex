#![allow(non_snake_case, non_camel_case_types)]
#![cfg_attr(test, allow(deref_nullptr))] // https://github.com/rust-lang/rust-bindgen/issues/2066

// force linking to openssl
#[cfg(feature = "bundled-sqlcipher-vendored-openssl")]
extern crate openssl_sys;

pub use self::error::*;

use std::mem;

mod error;

#[must_use]
pub fn SQLITE_STATIC() -> sqlite3_destructor_type {
    None
}

#[must_use]
pub fn SQLITE_TRANSIENT() -> sqlite3_destructor_type {
    Some(unsafe { mem::transmute::<isize, unsafe extern "C" fn(*mut std::ffi::c_void)>(-1_isize) })
}

#[allow(dead_code, clippy::all)]
mod bindings {
    include!(concat!(env!("OUT_DIR"), "/bindgen.rs"));
}
pub use bindings::*;

impl Default for sqlite3_vtab {
    fn default() -> Self {
        unsafe { mem::zeroed() }
    }
}

impl Default for sqlite3_vtab_cursor {
    fn default() -> Self {
        unsafe { mem::zeroed() }
    }
}
