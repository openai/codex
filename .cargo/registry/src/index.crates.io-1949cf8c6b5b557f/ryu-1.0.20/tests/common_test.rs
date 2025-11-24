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
    clippy::approx_constant,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::excessive_precision,
    clippy::unreadable_literal,
    clippy::wildcard_imports
)]

#[path = "../src/common.rs"]
mod common;

use common::{ceil_log2_pow5, decimal_length9, log10_pow2, log10_pow5};

#[test]
fn test_decimal_length9() {
    assert_eq!(1, decimal_length9(0));
    assert_eq!(1, decimal_length9(1));
    assert_eq!(1, decimal_length9(9));
    assert_eq!(2, decimal_length9(10));
    assert_eq!(2, decimal_length9(99));
    assert_eq!(3, decimal_length9(100));
    assert_eq!(3, decimal_length9(999));
    assert_eq!(9, decimal_length9(999999999));
}

#[test]
fn test_ceil_log2_pow5() {
    assert_eq!(1, ceil_log2_pow5(0));
    assert_eq!(3, ceil_log2_pow5(1));
    assert_eq!(5, ceil_log2_pow5(2));
    assert_eq!(7, ceil_log2_pow5(3));
    assert_eq!(10, ceil_log2_pow5(4));
    assert_eq!(8192, ceil_log2_pow5(3528));
}

#[test]
fn test_log10_pow2() {
    assert_eq!(0, log10_pow2(0));
    assert_eq!(0, log10_pow2(1));
    assert_eq!(0, log10_pow2(2));
    assert_eq!(0, log10_pow2(3));
    assert_eq!(1, log10_pow2(4));
    assert_eq!(496, log10_pow2(1650));
}

#[test]
fn test_log10_pow5() {
    assert_eq!(0, log10_pow5(0));
    assert_eq!(0, log10_pow5(1));
    assert_eq!(1, log10_pow5(2));
    assert_eq!(2, log10_pow5(3));
    assert_eq!(2, log10_pow5(4));
    assert_eq!(1831, log10_pow5(2620));
}

#[test]
fn test_float_to_bits() {
    assert_eq!(0, 0.0_f32.to_bits());
    assert_eq!(0x40490fda, 3.1415926_f32.to_bits());
}

#[test]
fn test_double_to_bits() {
    assert_eq!(0, 0.0_f64.to_bits());
    assert_eq!(
        0x400921FB54442D18,
        3.1415926535897932384626433_f64.to_bits(),
    );
}
