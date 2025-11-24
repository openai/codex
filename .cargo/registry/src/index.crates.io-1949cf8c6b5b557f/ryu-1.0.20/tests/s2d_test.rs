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

#![cfg(not(feature = "small"))]
#![allow(dead_code)]
#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::excessive_precision,
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

#[path = "../src/s2d.rs"]
mod s2d;

#[path = "../src/parse.rs"]
mod parse;

use crate::parse::Error;
use crate::s2d::s2d;

impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        *self as u8 == *other as u8
    }
}

#[test]
fn test_bad_input() {
    assert_eq!(Error::MalformedInput, s2d(b"x").unwrap_err());
    assert_eq!(Error::MalformedInput, s2d(b"1..1").unwrap_err());
    assert_eq!(Error::MalformedInput, s2d(b"..").unwrap_err());
    assert_eq!(Error::MalformedInput, s2d(b"1..1").unwrap_err());
    assert_eq!(Error::MalformedInput, s2d(b"1ee1").unwrap_err());
    assert_eq!(Error::MalformedInput, s2d(b"1e.1").unwrap_err());
    assert_eq!(Error::InputTooShort, s2d(b"").unwrap_err());
    assert_eq!(Error::InputTooLong, s2d(b"123456789012345678").unwrap_err());
    assert_eq!(Error::InputTooLong, s2d(b"1e12345").unwrap_err());
}

#[test]
fn test_basic() {
    assert_eq!(0.0, s2d(b"0").unwrap());
    assert_eq!(-0.0, s2d(b"-0").unwrap());
    assert_eq!(1.0, s2d(b"1").unwrap());
    assert_eq!(2.0, s2d(b"2").unwrap());
    assert_eq!(123456789.0, s2d(b"123456789").unwrap());
    assert_eq!(123.456, s2d(b"123.456").unwrap());
    assert_eq!(123.456, s2d(b"123456e-3").unwrap());
    assert_eq!(123.456, s2d(b"1234.56e-1").unwrap());
    assert_eq!(1.453, s2d(b"1.453").unwrap());
    assert_eq!(1453.0, s2d(b"1.453e+3").unwrap());
    assert_eq!(0.0, s2d(b".0").unwrap());
    assert_eq!(1.0, s2d(b"1e0").unwrap());
    assert_eq!(1.0, s2d(b"1E0").unwrap());
    assert_eq!(1.0, s2d(b"000001.000000").unwrap());
    assert_eq!(0.2316419, s2d(b"0.2316419").unwrap());
}

#[test]
fn test_min_max() {
    assert_eq!(
        1.7976931348623157e308,
        s2d(b"1.7976931348623157e308").unwrap(),
    );
    assert_eq!(5E-324, s2d(b"5E-324").unwrap());
}

#[test]
fn test_mantissa_rounding_overflow() {
    // This results in binary mantissa that is all ones and requires rounding up
    // because it is closer to 1 than to the next smaller float. This is a
    // regression test that the mantissa overflow is handled correctly by
    // increasing the exponent.
    assert_eq!(1.0, s2d(b"0.99999999999999999").unwrap());
    // This number overflows the mantissa *and* the IEEE exponent.
    assert_eq!(f64::INFINITY, s2d(b"1.7976931348623159e308").unwrap());
}

#[test]
fn test_underflow() {
    assert_eq!(0.0, s2d(b"2.4e-324").unwrap());
    assert_eq!(0.0, s2d(b"1e-324").unwrap());
    assert_eq!(0.0, s2d(b"9.99999e-325").unwrap());
    // These are just about halfway between 0 and the smallest float.
    // The first is just below the halfway point, the second just above.
    assert_eq!(0.0, s2d(b"2.4703282292062327e-324").unwrap());
    assert_eq!(5e-324, s2d(b"2.4703282292062328e-324").unwrap());
}

#[test]
fn test_overflow() {
    assert_eq!(f64::INFINITY, s2d(b"2e308").unwrap());
    assert_eq!(f64::INFINITY, s2d(b"1e309").unwrap());
}

#[test]
fn test_table_size_denormal() {
    assert_eq!(5e-324, s2d(b"4.9406564584124654e-324").unwrap());
}

#[test]
fn test_issue157() {
    assert_eq!(
        1.2999999999999999E+154,
        s2d(b"1.2999999999999999E+154").unwrap(),
    );
}

#[test]
fn test_issue173() {
    // Denormal boundary
    assert_eq!(
        2.2250738585072012e-308,
        s2d(b"2.2250738585072012e-308").unwrap(),
    );
    assert_eq!(
        2.2250738585072013e-308,
        s2d(b"2.2250738585072013e-308").unwrap(),
    );
    assert_eq!(
        2.2250738585072014e-308,
        s2d(b"2.2250738585072014e-308").unwrap(),
    );
}
