// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(clippy::zero_prefixed_literal, clippy::type_complexity)]

#[cfg(feature = "use_core")]
extern crate core;

#[macro_use]
extern crate derivative;

#[derive(Derivative)]
#[derivative(Debug, Default, Eq, Hash, PartialEq)]
// TODO: Ord, PartialOrd
struct Array<T> {
    f00: [T; 00],
    f01: [T; 01],
    f02: [T; 02],
    f03: [T; 03],
    f04: [T; 04],
    f05: [T; 05],
    f06: [T; 06],
    f07: [T; 07],
    f08: [T; 08],
    f09: [T; 09],
    f10: [T; 10],
    f11: [T; 11],
    f12: [T; 12],
    f13: [T; 13],
    f14: [T; 14],
    f15: [T; 15],
    f16: [T; 16],
    f17: [T; 17],
    f18: [T; 18],
    f19: [T; 19],
    f20: [T; 20],
    f21: [T; 21],
    f22: [T; 22],
    f23: [T; 23],
    f24: [T; 24],
    f25: [T; 25],
    f26: [T; 26],
    f27: [T; 27],
    f28: [T; 28],
    f29: [T; 29],
    f30: [T; 30],
    f31: [T; 31],
    f32: [T; 32],
}

// FIXME(#7622): merge with `Array` once `[T; N]: Clone` where `T: Clone`
#[derive(Derivative)]
#[derivative(Clone, Copy)]
struct CopyArray<T: Copy> {
    f00: [T; 00],
    f01: [T; 01],
    f02: [T; 02],
    f03: [T; 03],
    f04: [T; 04],
    f05: [T; 05],
    f06: [T; 06],
    f07: [T; 07],
    f08: [T; 08],
    f09: [T; 09],
    f10: [T; 10],
    f11: [T; 11],
    f12: [T; 12],
    f13: [T; 13],
    f14: [T; 14],
    f15: [T; 15],
    f16: [T; 16],
    f17: [T; 17],
    f18: [T; 18],
    f19: [T; 19],
    f20: [T; 20],
    f21: [T; 21],
    f22: [T; 22],
    f23: [T; 23],
    f24: [T; 24],
    f25: [T; 25],
    f26: [T; 26],
    f27: [T; 27],
    f28: [T; 28],
    f29: [T; 29],
    f30: [T; 30],
    f31: [T; 31],
    f32: [T; 32],
}

#[derive(Derivative)]
#[derivative(Clone, Copy, Debug, Eq, Hash, PartialEq)]
// TODO: Ord, PartialOrd
struct Fn<A, B, C, D, E, F, G, H, I, J, K, L> {
    f00: fn(),
    f01: fn(A),
    f02: fn(A, B),
    f03: fn(A, B, C),
    f04: fn(A, B, C, D),
    f05: fn(A, B, C, D, E),
    f06: fn(A, B, C, D, E, F),
    f07: fn(A, B, C, D, E, F, G),
    f08: fn(A, B, C, D, E, F, G, H),
    f09: fn(A, B, C, D, E, F, G, H, I),
    f10: fn(A, B, C, D, E, F, G, H, I, J),
    f11: fn(A, B, C, D, E, F, G, H, I, J, K),
    f12: fn(A, B, C, D, E, F, G, H, I, J, K, L),
}

#[derive(Derivative)]
#[derivative(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
// TODO: Ord, PartialOrd
struct Tuple<A, B, C, D, E, F, G, H, I, J, K, L> {
    f00: (),
    f01: (A,),
    f02: (A, B),
    f03: (A, B, C),
    f04: (A, B, C, D),
    f05: (A, B, C, D, E),
    f06: (A, B, C, D, E, F),
    f07: (A, B, C, D, E, F, G),
    f08: (A, B, C, D, E, F, G, H),
    f09: (A, B, C, D, E, F, G, H, I),
    f10: (A, B, C, D, E, F, G, H, I, J),
    f11: (A, B, C, D, E, F, G, H, I, J, K),
    f12: (A, B, C, D, E, F, G, H, I, J, K, L),
}

#[test]
fn main() {}
