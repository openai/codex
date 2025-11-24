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
    clippy::cast_possible_wrap,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::checked_conversions,
    clippy::float_cmp,
    clippy::manual_range_contains,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::unreadable_literal,
    clippy::unseparated_literal_suffix,
    clippy::wildcard_imports
)]

#[path = "../src/common.rs"]
mod common;

#[cfg(not(feature = "small"))]
#[path = "../src/d2s_full_table.rs"]
mod d2s_full_table;

#[path = "../src/d2s_intrinsics.rs"]
mod d2s_intrinsics;

#[cfg(feature = "small")]
#[path = "../src/d2s_small_table.rs"]
mod d2s_small_table;

#[path = "../src/d2s.rs"]
mod d2s;

#[path = "../src/f2s_intrinsics.rs"]
mod f2s_intrinsics;

#[path = "../src/f2s.rs"]
mod f2s;

#[path = "../src/s2f.rs"]
mod s2f;

#[path = "../src/parse.rs"]
mod parse;

use crate::parse::Error;
use crate::s2f::s2f;

impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        *self as u8 == *other as u8
    }
}

#[test]
fn test_basic() {
    assert_eq!(0.0, s2f(b"0").unwrap());
    assert_eq!(-0.0, s2f(b"-0").unwrap());
    assert_eq!(1.0, s2f(b"1").unwrap());
    assert_eq!(-1.0, s2f(b"-1").unwrap());
    assert_eq!(123456792.0, s2f(b"123456789").unwrap());
    assert_eq!(299792448.0, s2f(b"299792458").unwrap());
}

#[test]
fn test_min_max() {
    assert_eq!(1e-45, s2f(b"1e-45").unwrap());
    assert_eq!(f32::MIN_POSITIVE, s2f(b"1.1754944e-38").unwrap());
    assert_eq!(f32::MAX, s2f(b"3.4028235e+38").unwrap());
}

#[test]
fn test_mantissa_rounding_overflow() {
    assert_eq!(1.0, s2f(b"0.999999999").unwrap());
    assert_eq!(f32::INFINITY, s2f(b"3.4028236e+38").unwrap());
    assert_eq!(1.1754944e-38, s2f(b"1.17549430e-38").unwrap()); // FLT_MIN
    assert_eq!(1.1754944e-38, s2f(b"1.17549431e-38").unwrap());
    assert_eq!(1.1754944e-38, s2f(b"1.17549432e-38").unwrap());
    assert_eq!(1.1754944e-38, s2f(b"1.17549433e-38").unwrap());
    assert_eq!(1.1754944e-38, s2f(b"1.17549434e-38").unwrap());
    assert_eq!(1.1754944e-38, s2f(b"1.17549435e-38").unwrap());
}

#[test]
fn test_trailing_zeros() {
    assert_eq!(26843550.0, s2f(b"26843549.5").unwrap());
    assert_eq!(50000004.0, s2f(b"50000002.5").unwrap());
    assert_eq!(99999992.0, s2f(b"99999989.5").unwrap());
}
