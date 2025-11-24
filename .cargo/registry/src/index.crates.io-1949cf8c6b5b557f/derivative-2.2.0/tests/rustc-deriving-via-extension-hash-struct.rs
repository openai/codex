// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// pretty-expanded FIXME #23616

#[cfg(feature = "use_core")]
extern crate core;

#[macro_use]
extern crate derivative;

use std::collections::hash_map::DefaultHasher;

#[derive(Derivative)]
#[derivative(Hash)]
struct Foo {
    x: isize,
    y: isize,
    z: isize
}

#[test]
fn main() {
    use std::hash::Hash;
    let mut hasher = DefaultHasher::new();
    Foo { x: 0, y: 0, z: 0 }.hash(&mut hasher);
}
