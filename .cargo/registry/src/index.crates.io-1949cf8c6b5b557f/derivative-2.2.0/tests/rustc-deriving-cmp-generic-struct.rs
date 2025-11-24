// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// no-pretty-expanded FIXME #15189

#[cfg(feature = "use_core")]
extern crate core;

#[macro_use]
extern crate derivative;

#[derive(Derivative)]
#[derivative(PartialEq, Eq, PartialOrd, Ord)]
struct S<T> {
    x: T,
    y: T,
}

pub fn main() {
    let s1 = S { x: 1, y: 1 };
    let s2 = S { x: 1, y: 2 };

    // in order for both PartialOrd and Ord
    let ss = [s1, s2];

    for (i, s1) in ss.iter().enumerate() {
        for (j, s2) in ss.iter().enumerate() {
            let ord = i.cmp(&j);

            let eq = i == j;
            let lt = i < j;
            let le = i <= j;
            let gt = i > j;
            let ge = i >= j;

            // PartialEq
            assert_eq!(*s1 == *s2, eq);
            assert_eq!(*s1 != *s2, !eq);

            // PartialOrd
            assert_eq!(*s1 < *s2, lt);
            assert_eq!(*s1 > *s2, gt);

            assert_eq!(*s1 <= *s2, le);
            assert_eq!(*s1 >= *s2, ge);

            // Ord
            assert_eq!(s1.cmp(s2), ord);
        }
    }
}
