//! This example demonstrates the various types of ctor/dtor in an executable
//! context.

#![cfg_attr(feature = "used_linker", feature(used_with_arg))]

use libc_print::*;
use std::collections::HashMap;
use ctor::{ctor, dtor};

#[dtor]
#[allow(unsafe_code)]
unsafe fn dtor() {
    libc_eprintln!("dtor");
}

#[dtor]
#[allow(unsafe_code)]
unsafe fn dtor_unsafe() {
    libc_eprintln!("dtor_unsafe");
}

/// Executable main which demonstrates the various types of ctor/dtor.
pub fn main() {
    use libc_print::*;
    libc_eprintln!("main!");
}
