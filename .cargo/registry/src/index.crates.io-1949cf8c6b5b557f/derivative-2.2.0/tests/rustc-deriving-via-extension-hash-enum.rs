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

#[derive(Derivative)]
#[derivative(Hash)]
#[allow(dead_code)]
enum Foo {
    Bar(isize, char),
    Baz(char, isize)
}

#[derive(Derivative)]
#[derivative(Hash)]
#[allow(dead_code)]
enum A {
    B,
    C,
    D,
    E
}

#[test]
fn main(){}
