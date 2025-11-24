// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Regression test for #24085. Errors were occurring in region
// inference due to the requirement that `'a:b'`, which was getting
// incorrectly translated in connection with the closure below.

#![allow(clippy::redundant_closure)]

#[cfg(feature = "use_core")]
extern crate core;

#[macro_use]
extern crate derivative;

#[derive(Derivative)]
#[derivative(Copy,Clone)]
#[allow(dead_code)]
struct Path<'a:'b, 'b> {
    x: &'a i32,
    tail: Option<&'b Path<'a, 'b>>
}

#[allow(dead_code, unconditional_recursion)]
fn foo<'a,'b,F>(p: Path<'a, 'b>, mut f: F)
                where F: for<'c> FnMut(Path<'a, 'c>) {
    foo(p, |x| f(x))
}

#[test]
fn main() { }
