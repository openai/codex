// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
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

use std::cell::Cell;

#[macro_use]
extern crate derivative;

#[derive(Derivative)]
#[derivative(Copy, Clone)]
struct Point {x: isize, y: isize, z: isize}

fn f(p: &Cell<Point>) {
    assert_eq!(p.get().z, 12);
    p.set(Point {x: 10, y: 11, z: 13});
    assert_eq!(p.get().z, 13);
}

#[test]
fn main() {
    let a: Point = Point {x: 10, y: 11, z: 12};
    let b: &Cell<Point> = &Cell::new(a);
    assert_eq!(b.get().z, 12);
    f(b);
    assert_eq!(a.x, 10);
    assert_eq!(a.y, 11);
    assert_eq!(a.z, 12);
    assert_eq!(b.get().x, 10);
    assert_eq!(b.get().y, 11);
    assert_eq!(b.get().z, 13);
}
