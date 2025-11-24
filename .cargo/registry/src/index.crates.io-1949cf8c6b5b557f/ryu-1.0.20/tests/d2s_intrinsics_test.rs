// Translated from C to Rust. The original C code can be found at
// https://github.com/ulfjack/ryu and carries the following license:
//
// Copyright 2018 Ulf Adams
//
// The contents of this file may be used under the terms of the Apache License,
// Version 2.0.
//
//    (See accompanying file LICENSE-Apache or copy at
//     http://www.apache.org/licenses/LICENSE-2.0)
//
// Alternatively, the contents of this file may be used under the terms of
// the Boost Software License, Version 1.0.
//    (See accompanying file LICENSE-Boost or copy at
//     https://www.boost.org/LICENSE_1_0.txt)
//
// Unless required by applicable law or agreed to in writing, this software
// is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.

#![allow(dead_code)]
#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::unreadable_literal
)]

#[path = "../src/d2s_intrinsics.rs"]
mod d2s_intrinsics;

use d2s_intrinsics::pow5_factor;

#[test]
fn test_pow5_factor() {
    assert_eq!(0, pow5_factor(1));
    assert_eq!(0, pow5_factor(2));
    assert_eq!(0, pow5_factor(3));
    assert_eq!(0, pow5_factor(4));
    assert_eq!(1, pow5_factor(5));
    assert_eq!(0, pow5_factor(6));
    assert_eq!(0, pow5_factor(7));
    assert_eq!(0, pow5_factor(8));
    assert_eq!(0, pow5_factor(9));
    assert_eq!(1, pow5_factor(10));

    assert_eq!(0, pow5_factor(12));
    assert_eq!(0, pow5_factor(14));
    assert_eq!(0, pow5_factor(16));
    assert_eq!(0, pow5_factor(18));
    assert_eq!(1, pow5_factor(20));

    assert_eq!(2, pow5_factor(5 * 5));
    assert_eq!(3, pow5_factor(5 * 5 * 5));
    assert_eq!(4, pow5_factor(5 * 5 * 5 * 5));
    assert_eq!(5, pow5_factor(5 * 5 * 5 * 5 * 5));
    assert_eq!(6, pow5_factor(5 * 5 * 5 * 5 * 5 * 5));
    assert_eq!(7, pow5_factor(5 * 5 * 5 * 5 * 5 * 5 * 5));
    assert_eq!(8, pow5_factor(5 * 5 * 5 * 5 * 5 * 5 * 5 * 5));
    assert_eq!(9, pow5_factor(5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5));
    assert_eq!(10, pow5_factor(5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5 * 5));

    assert_eq!(0, pow5_factor(42));
    assert_eq!(1, pow5_factor(42 * 5));
    assert_eq!(2, pow5_factor(42 * 5 * 5));
    assert_eq!(3, pow5_factor(42 * 5 * 5 * 5));
    assert_eq!(4, pow5_factor(42 * 5 * 5 * 5 * 5));
    assert_eq!(5, pow5_factor(42 * 5 * 5 * 5 * 5 * 5));

    assert_eq!(27, pow5_factor(7450580596923828125)); // 5^27, largest power of 5 < 2^64.
    assert_eq!(1, pow5_factor(18446744073709551615)); // 2^64 - 1, largest multiple of 5 < 2^64.
    assert_eq!(0, pow5_factor(18446744073709551614)); // 2^64 - 2, largest non-multiple of 5 < 2^64.
}
