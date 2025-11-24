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
struct TS<T>(T, T);

pub fn main() {
    let ts1 = TS(1, 1);
    let ts2 = TS(1, 2);

    // in order for both PartialOrd and Ord
    let tss = [ts1, ts2];

    for (i, ts1) in tss.iter().enumerate() {
        for (j, ts2) in tss.iter().enumerate() {
            let ord = i.cmp(&j);

            let eq = i == j;
            let lt = i < j;
            let le = i <= j;
            let gt = i > j;
            let ge = i >= j;

            // PartialEq
            assert_eq!(*ts1 == *ts2, eq);
            assert_eq!(*ts1 != *ts2, !eq);

            // PartialOrd
            assert_eq!(*ts1 < *ts2, lt);
            assert_eq!(*ts1 > *ts2, gt);

            assert_eq!(*ts1 <= *ts2, le);
            assert_eq!(*ts1 >= *ts2, ge);

            // Ord
            assert_eq!(ts1.cmp(ts2), ord);
        }
    }
}
