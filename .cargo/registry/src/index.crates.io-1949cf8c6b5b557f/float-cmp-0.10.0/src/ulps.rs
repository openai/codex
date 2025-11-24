// Copyright 2014-2020 Optimal Computing (NZ) Ltd.
// Licensed under the MIT license.  See LICENSE for details.

#[cfg(feature = "num-traits")]
use num_traits::NumCast;

#[inline]
fn f32_ordered_bits(f: f32) -> u32 {
    const SIGN_BIT: u32 = 1 << 31;
    let bits = f.to_bits();
    if bits & SIGN_BIT != 0 {
        !bits
    } else {
        bits ^ SIGN_BIT
    }
}

#[inline]
fn f64_ordered_bits(f: f64) -> u64 {
    const SIGN_BIT: u64 = 1 << 63;
    let bits = f.to_bits();
    if bits & SIGN_BIT != 0 {
        !bits
    } else {
        bits ^ SIGN_BIT
    }
}

/// A trait for floating point numbers which computes the number of representable
/// values or ULPs (Units of Least Precision) that separate the two given values.
#[cfg(feature = "num-traits")]
pub trait Ulps {
    type U: Copy + NumCast;

    /// The number of representable values or ULPs (Units of Least Precision) that
    /// separate `self` and `other`.  The result `U` is an integral value, and will
    /// be zero if `self` and `other` are exactly equal.
    fn ulps(&self, other: &Self) -> <Self as Ulps>::U;

    /// The next representable number above this one
    fn next(&self) -> Self;

    /// The previous representable number below this one
    fn prev(&self) -> Self;
}

#[cfg(not(feature = "num-traits"))]
pub trait Ulps {
    type U: Copy;

    /// The number of representable values or ULPs (Units of Least Precision) that
    /// separate `self` and `other`.  The result `U` is an integral value, and will
    /// be zero if `self` and `other` are exactly equal.
    fn ulps(&self, other: &Self) -> <Self as Ulps>::U;

    /// The next representable number above this one
    fn next(&self) -> Self;

    /// The previous representable number below this one
    fn prev(&self) -> Self;
}

impl Ulps for f32 {
    type U = i32;

    fn ulps(&self, other: &f32) -> i32 {
        // IEEE754 defined floating point storage representation to
        // maintain their order when their bit patterns are interpreted as
        // integers.  This is a huge boon to the task at hand, as we can
        // reinterpret them as integers to find out how many ULPs apart any
        // two floats are

        // Setup integer representations of the input
        let ai32: i32 = f32_ordered_bits(*self) as i32;
        let bi32: i32 = f32_ordered_bits(*other) as i32;

        ai32.wrapping_sub(bi32)
    }

    fn next(&self) -> Self {
        if self.is_infinite() && *self > 0.0 {
            *self
        } else if *self == -0.0 && self.is_sign_negative() {
            0.0
        } else {
            let mut u = self.to_bits();
            if *self >= 0.0 {
                u += 1;
            } else {
                u -= 1;
            }
            f32::from_bits(u)
        }
    }

    fn prev(&self) -> Self {
        if self.is_infinite() && *self < 0.0 {
            *self
        } else if *self == 0.0 && self.is_sign_positive() {
            -0.0
        } else {
            let mut u = self.to_bits();
            if *self <= -0.0 {
                u += 1;
            } else {
                u -= 1;
            }
            f32::from_bits(u)
        }
    }
}

#[test]
fn f32_ulps_test1() {
    let x: f32 = 1000000_f32;
    let y: f32 = 1000000.1_f32;
    assert!(x.ulps(&y) == -2);
}

#[test]
fn f32_ulps_test2() {
    let pzero: f32 = f32::from_bits(0x00000000_u32);
    let nzero: f32 = f32::from_bits(0x80000000_u32);
    assert_eq!(pzero.ulps(&nzero), 1);
}
#[test]
fn f32_ulps_test3() {
    let pinf: f32 = f32::from_bits(0x7f800000_u32);
    let ninf: f32 = f32::from_bits(0xff800000_u32);
    assert_eq!(pinf.ulps(&ninf), -16777215);
}

#[test]
fn f32_ulps_test4() {
    let x: f32 = f32::from_bits(0x63a7f026_u32);
    let y: f32 = f32::from_bits(0x63a7f023_u32);
    assert!(x.ulps(&y) == 3);
}

#[test]
fn f32_ulps_test5() {
    let x: f32 = 2.0;
    let ulps: i32 = x.to_bits() as i32;
    let x2: f32 = <f32>::from_bits(ulps as u32);
    assert_eq!(x, x2);
}

#[test]
fn f32_ulps_test6() {
    let negzero: f32 = -0.;
    let zero: f32 = 0.;
    assert_eq!(negzero.next(), zero);
    assert_eq!(zero.prev(), negzero);
    assert!(negzero.prev() < 0.0);
    assert!(zero.next() > 0.0);
}

impl Ulps for f64 {
    type U = i64;

    fn ulps(&self, other: &f64) -> i64 {
        // IEEE754 defined floating point storage representation to
        // maintain their order when their bit patterns are interpreted as
        // integers.  This is a huge boon to the task at hand, as we can
        // reinterpret them as integers to find out how many ULPs apart any
        // two floats are

        // Setup integer representations of the input
        let ai64: i64 = f64_ordered_bits(*self) as i64;
        let bi64: i64 = f64_ordered_bits(*other) as i64;

        ai64.wrapping_sub(bi64)
    }

    fn next(&self) -> Self {
        if self.is_infinite() && *self > 0.0 {
            *self
        } else if *self == -0.0 && self.is_sign_negative() {
            0.0
        } else {
            let mut u = self.to_bits();
            if *self >= 0.0 {
                u += 1;
            } else {
                u -= 1;
            }
            f64::from_bits(u)
        }
    }

    fn prev(&self) -> Self {
        if self.is_infinite() && *self < 0.0 {
            *self
        } else if *self == 0.0 && self.is_sign_positive() {
            -0.0
        } else {
            let mut u = self.to_bits();
            if *self <= -0.0 {
                u += 1;
            } else {
                u -= 1;
            }
            f64::from_bits(u)
        }
    }
}

#[test]
fn f64_ulps_test1() {
    let x: f64 = 1000000_f64;
    let y: f64 = 1000000.00000001_f64;
    assert!(x.ulps(&y) == -86);
}

#[test]
fn f64_ulps_test2() {
    let pzero: f64 = f64::from_bits(0x0000000000000000_u64);
    let nzero: f64 = f64::from_bits(0x8000000000000000_u64);
    assert_eq!(pzero.ulps(&nzero), 1);
}
#[test]
fn f64_ulps_test3() {
    let pinf: f64 = f64::from_bits(0x7f80000000000000_u64);
    let ninf: f64 = f64::from_bits(0xff80000000000000_u64);
    assert_eq!(pinf.ulps(&ninf), -72057594037927935);
}

#[test]
fn f64_ulps_test4() {
    let x: f64 = f64::from_bits(0xd017f6cc63a7f026_u64);
    let y: f64 = f64::from_bits(0xd017f6cc63a7f023_u64);
    assert!(x.ulps(&y) == -3);
}

#[test]
fn f64_ulps_test5() {
    let x: f64 = 2.0;
    let ulps: i64 = x.to_bits() as i64;
    let x2: f64 = <f64>::from_bits(ulps as u64);
    assert_eq!(x, x2);
}

#[test]
fn f64_ulps_test6() {
    let negzero: f64 = -0.;
    let zero: f64 = 0.;
    assert_eq!(negzero.next(), zero);
    assert_eq!(zero.prev(), negzero);
    assert!(negzero.prev() < 0.0);
    assert!(zero.next() > 0.0);
}
