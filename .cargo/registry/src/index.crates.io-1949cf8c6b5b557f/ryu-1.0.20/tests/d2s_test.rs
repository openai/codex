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

#![allow(
    clippy::approx_constant,
    clippy::cast_lossless,
    clippy::float_cmp,
    clippy::int_plus_one,
    clippy::non_ascii_literal,
    clippy::unreadable_literal,
    clippy::unseparated_literal_suffix
)]

#[macro_use]
mod macros;

use std::f64;

fn pretty(f: f64) -> String {
    ryu::Buffer::new().format(f).to_owned()
}

fn ieee_parts_to_double(sign: bool, ieee_exponent: u32, ieee_mantissa: u64) -> f64 {
    assert!(ieee_exponent <= 2047);
    assert!(ieee_mantissa <= (1u64 << 53) - 1);
    f64::from_bits(((sign as u64) << 63) | ((ieee_exponent as u64) << 52) | ieee_mantissa)
}

#[test]
fn test_ryu() {
    check!(0.3);
    check!(1234000000000000.0);
    check!(1.234e16);
    check!(2.71828);
    check!(1.1e128);
    check!(1.1e-64);
    check!(2.718281828459045);
    check!(5e-324);
    check!(1.7976931348623157e308);
}

#[test]
fn test_random() {
    let n = if cfg!(miri) { 100 } else { 1000000 };
    let mut buffer = ryu::Buffer::new();
    for _ in 0..n {
        let f: f64 = rand::random();
        assert_eq!(f, buffer.format_finite(f).parse().unwrap());
    }
}

#[test]
#[cfg_attr(miri, ignore = "too slow for miri")]
fn test_non_finite() {
    for i in 0u64..1 << 23 {
        let f = f64::from_bits((((1 << 11) - 1) << 52) + (i << 29));
        assert!(!f.is_finite(), "f={}", f);
        ryu::Buffer::new().format_finite(f);
    }
}

#[test]
fn test_basic() {
    check!(0.0);
    check!(-0.0);
    check!(1.0);
    check!(-1.0);
    assert_eq!(pretty(f64::NAN.copysign(1.0)), "NaN");
    assert_eq!(pretty(f64::NAN.copysign(-1.0)), "NaN");
    assert_eq!(pretty(f64::INFINITY), "inf");
    assert_eq!(pretty(f64::NEG_INFINITY), "-inf");
}

#[test]
fn test_switch_to_subnormal() {
    check!(2.2250738585072014e-308);
}

#[test]
fn test_min_and_max() {
    assert_eq!(f64::from_bits(0x7fefffffffffffff), 1.7976931348623157e308);
    check!(1.7976931348623157e308);
    assert_eq!(f64::from_bits(1), 5e-324);
    check!(5e-324);
}

#[test]
fn test_lots_of_trailing_zeros() {
    check!(2.9802322387695312e-8);
}

#[test]
fn test_regression() {
    check!(-2.109808898695963e16);
    check!(4.940656e-318);
    check!(1.18575755e-316);
    check!(2.989102097996e-312);
    check!(9060801153433600.0);
    check!(4.708356024711512e18);
    check!(9.409340012568248e18);
    check!(1.2345678);
}

#[test]
fn test_looks_like_pow5() {
    // These numbers have a mantissa that is a multiple of the largest power of
    // 5 that fits, and an exponent that causes the computation for q to result
    // in 22, which is a corner case for Ryū.
    assert_eq!(f64::from_bits(0x4830F0CF064DD592), 5.764607523034235e39);
    check!(5.764607523034235e39);
    assert_eq!(f64::from_bits(0x4840F0CF064DD592), 1.152921504606847e40);
    check!(1.152921504606847e40);
    assert_eq!(f64::from_bits(0x4850F0CF064DD592), 2.305843009213694e40);
    check!(2.305843009213694e40);
}

#[test]
fn test_output_length() {
    check!(1.0); // already tested in Basic
    check!(1.2);
    check!(1.23);
    check!(1.234);
    check!(1.2345);
    check!(1.23456);
    check!(1.234567);
    check!(1.2345678); // already tested in Regression
    check!(1.23456789);
    check!(1.234567895); // 1.234567890 would be trimmed
    check!(1.2345678901);
    check!(1.23456789012);
    check!(1.234567890123);
    check!(1.2345678901234);
    check!(1.23456789012345);
    check!(1.234567890123456);
    check!(1.2345678901234567);

    // Test 32-bit chunking
    check!(4.294967294); // 2^32 - 2
    check!(4.294967295); // 2^32 - 1
    check!(4.294967296); // 2^32
    check!(4.294967297); // 2^32 + 1
    check!(4.294967298); // 2^32 + 2
}

// Test min, max shift values in shiftright128
#[test]
fn test_min_max_shift() {
    let max_mantissa = (1u64 << 53) - 1;

    // 32-bit opt-size=0:  49 <= dist <= 50
    // 32-bit opt-size=1:  30 <= dist <= 50
    // 64-bit opt-size=0:  50 <= dist <= 50
    // 64-bit opt-size=1:  30 <= dist <= 50
    assert_eq!(1.7800590868057611E-307, ieee_parts_to_double(false, 4, 0));
    check!(1.7800590868057611e-307);
    // 32-bit opt-size=0:  49 <= dist <= 49
    // 32-bit opt-size=1:  28 <= dist <= 49
    // 64-bit opt-size=0:  50 <= dist <= 50
    // 64-bit opt-size=1:  28 <= dist <= 50
    assert_eq!(
        2.8480945388892175E-306,
        ieee_parts_to_double(false, 6, max_mantissa)
    );
    check!(2.8480945388892175e-306);
    // 32-bit opt-size=0:  52 <= dist <= 53
    // 32-bit opt-size=1:   2 <= dist <= 53
    // 64-bit opt-size=0:  53 <= dist <= 53
    // 64-bit opt-size=1:   2 <= dist <= 53
    assert_eq!(2.446494580089078E-296, ieee_parts_to_double(false, 41, 0));
    check!(2.446494580089078e-296);
    // 32-bit opt-size=0:  52 <= dist <= 52
    // 32-bit opt-size=1:   2 <= dist <= 52
    // 64-bit opt-size=0:  53 <= dist <= 53
    // 64-bit opt-size=1:   2 <= dist <= 53
    assert_eq!(
        4.8929891601781557E-296,
        ieee_parts_to_double(false, 40, max_mantissa)
    );
    check!(4.8929891601781557e-296);

    // 32-bit opt-size=0:  57 <= dist <= 58
    // 32-bit opt-size=1:  57 <= dist <= 58
    // 64-bit opt-size=0:  58 <= dist <= 58
    // 64-bit opt-size=1:  58 <= dist <= 58
    assert_eq!(1.8014398509481984E16, ieee_parts_to_double(false, 1077, 0));
    check!(1.8014398509481984e16);
    // 32-bit opt-size=0:  57 <= dist <= 57
    // 32-bit opt-size=1:  57 <= dist <= 57
    // 64-bit opt-size=0:  58 <= dist <= 58
    // 64-bit opt-size=1:  58 <= dist <= 58
    assert_eq!(
        3.6028797018963964E16,
        ieee_parts_to_double(false, 1076, max_mantissa)
    );
    check!(3.6028797018963964e16);
    // 32-bit opt-size=0:  51 <= dist <= 52
    // 32-bit opt-size=1:  51 <= dist <= 59
    // 64-bit opt-size=0:  52 <= dist <= 52
    // 64-bit opt-size=1:  52 <= dist <= 59
    assert_eq!(2.900835519859558E-216, ieee_parts_to_double(false, 307, 0));
    check!(2.900835519859558e-216);
    // 32-bit opt-size=0:  51 <= dist <= 51
    // 32-bit opt-size=1:  51 <= dist <= 59
    // 64-bit opt-size=0:  52 <= dist <= 52
    // 64-bit opt-size=1:  52 <= dist <= 59
    assert_eq!(
        5.801671039719115E-216,
        ieee_parts_to_double(false, 306, max_mantissa)
    );
    check!(5.801671039719115e-216);

    // https://github.com/ulfjack/ryu/commit/19e44d16d80236f5de25800f56d82606d1be00b9#commitcomment-30146483
    // 32-bit opt-size=0:  49 <= dist <= 49
    // 32-bit opt-size=1:  44 <= dist <= 49
    // 64-bit opt-size=0:  50 <= dist <= 50
    // 64-bit opt-size=1:  44 <= dist <= 50
    assert_eq!(
        3.196104012172126E-27,
        ieee_parts_to_double(false, 934, 0x000FA7161A4D6E0C)
    );
    check!(3.196104012172126e-27);
}

#[test]
fn test_small_integers() {
    check!(9007199254740991.0); // 2^53-1
    check!(9007199254740992.0); // 2^53

    check!(1.0);
    check!(12.0);
    check!(123.0);
    check!(1234.0);
    check!(12345.0);
    check!(123456.0);
    check!(1234567.0);
    check!(12345678.0);
    check!(123456789.0);
    check!(1234567890.0);
    check!(1234567895.0);
    check!(12345678901.0);
    check!(123456789012.0);
    check!(1234567890123.0);
    check!(12345678901234.0);
    check!(123456789012345.0);
    check!(1234567890123456.0);

    // 10^i
    check!(1.0);
    check!(10.0);
    check!(100.0);
    check!(1000.0);
    check!(10000.0);
    check!(100000.0);
    check!(1000000.0);
    check!(10000000.0);
    check!(100000000.0);
    check!(1000000000.0);
    check!(10000000000.0);
    check!(100000000000.0);
    check!(1000000000000.0);
    check!(10000000000000.0);
    check!(100000000000000.0);
    check!(1000000000000000.0);

    // 10^15 + 10^i
    check!(1000000000000001.0);
    check!(1000000000000010.0);
    check!(1000000000000100.0);
    check!(1000000000001000.0);
    check!(1000000000010000.0);
    check!(1000000000100000.0);
    check!(1000000001000000.0);
    check!(1000000010000000.0);
    check!(1000000100000000.0);
    check!(1000001000000000.0);
    check!(1000010000000000.0);
    check!(1000100000000000.0);
    check!(1001000000000000.0);
    check!(1010000000000000.0);
    check!(1100000000000000.0);

    // Largest power of 2 <= 10^(i+1)
    check!(8.0);
    check!(64.0);
    check!(512.0);
    check!(8192.0);
    check!(65536.0);
    check!(524288.0);
    check!(8388608.0);
    check!(67108864.0);
    check!(536870912.0);
    check!(8589934592.0);
    check!(68719476736.0);
    check!(549755813888.0);
    check!(8796093022208.0);
    check!(70368744177664.0);
    check!(562949953421312.0);
    check!(9007199254740992.0);

    // 1000 * (Largest power of 2 <= 10^(i+1))
    check!(8000.0);
    check!(64000.0);
    check!(512000.0);
    check!(8192000.0);
    check!(65536000.0);
    check!(524288000.0);
    check!(8388608000.0);
    check!(67108864000.0);
    check!(536870912000.0);
    check!(8589934592000.0);
    check!(68719476736000.0);
    check!(549755813888000.0);
    check!(8796093022208000.0);
}
