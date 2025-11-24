// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Test that #[derive(Copy, Clone)] produces a shallow copy
//! even when a member violates RFC 1521

#![allow(clippy::clone_on_copy)]

#[cfg(feature = "use_core")]
extern crate core;

#[macro_use]
extern crate derivative;

use std::sync::atomic::{AtomicBool, Ordering};

/// A struct that pretends to be Copy, but actually does something
/// in its Clone impl
#[derive(Copy)]
struct Liar;

/// Static cooperating with the rogue Clone impl
static CLONED: AtomicBool = AtomicBool::new(false);

impl Clone for Liar {
    fn clone(&self) -> Self {
        // this makes Clone vs Copy observable
        CLONED.store(true, Ordering::SeqCst);

        *self
    }
}

/// This struct is actually Copy... at least, it thinks it is!
#[derive(Copy, Clone)]
struct TheirTheir(Liar);

#[derive(Derivative)]
#[derivative(Copy, Clone)]
struct OurOur1(Liar);
#[derive(Derivative)]
#[derivative(Clone, Copy)]
struct OurOur2(Liar);

#[test]
fn main() {
    let _ = TheirTheir(Liar).clone();
    assert!(!CLONED.load(Ordering::SeqCst), "TheirTheir");

    let _ = OurOur1(Liar).clone();
    assert!(!CLONED.load(Ordering::SeqCst), "OurOur1");
    let _ = OurOur2(Liar).clone();
    assert!(!CLONED.load(Ordering::SeqCst), "OurOur2");
}
