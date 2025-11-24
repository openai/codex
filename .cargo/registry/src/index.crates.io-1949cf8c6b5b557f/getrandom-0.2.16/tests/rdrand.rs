// We only test the RDRAND-based RNG source on supported architectures.
#![cfg(any(target_arch = "x86_64", target_arch = "x86"))]

// rdrand.rs expects to be part of the getrandom main crate, so we need these
// additional imports to get rdrand.rs to compile.
use getrandom::Error;
#[macro_use]
extern crate cfg_if;
#[path = "../src/lazy.rs"]
mod lazy;
#[path = "../src/rdrand.rs"]
mod rdrand;
#[path = "../src/util.rs"]
mod util;

// The rdrand implementation has the signature of getrandom_uninit(), but our
// tests expect getrandom_impl() to have the signature of getrandom().
fn getrandom_impl(dest: &mut [u8]) -> Result<(), Error> {
    rdrand::getrandom_inner(unsafe { util::slice_as_uninit_mut(dest) })?;
    Ok(())
}
mod common;
