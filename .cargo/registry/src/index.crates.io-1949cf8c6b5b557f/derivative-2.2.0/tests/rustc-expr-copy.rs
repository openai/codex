// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[cfg(feature = "use_core")]
extern crate core;

#[macro_use]
extern crate derivative;


fn f(arg: &mut A) {
    arg.a = 100;
}

#[derive(Derivative)]
#[derivative(Copy, Clone)]
struct A { a: isize }

#[test]
fn main() {
    let mut x = A {a: 10};
    f(&mut x);
    assert_eq!(x.a, 100);
    x.a = 20;
    let mut y = x;
    f(&mut y);
    assert_eq!(x.a, 20);
}
