// Copyright 2014-2020 Optimal Computing (NZ) Ltd.
// Licensed under the MIT license.  See LICENSE for details.

use core::cmp::PartialOrd;
use core::ops::{Div, Neg, Sub};
use num_traits::Zero;

/// ApproxEqRatio is a trait for approximate equality comparisons bounding the ratio
/// of the difference to the larger.
pub trait ApproxEqRatio:
    Div<Output = Self> + Sub<Output = Self> + Neg<Output = Self> + PartialOrd + Zero + Sized + Copy
{
    /// This method tests if `self` and `other` are nearly equal by bounding the
    /// difference between them to some number much less than the larger of the two.
    /// This bound is set as the ratio of the difference to the larger.
    fn approx_eq_ratio(&self, other: &Self, ratio: Self) -> bool {
        // Not equal if signs are not equal
        if *self < Self::zero() && *other > Self::zero() {
            return false;
        }
        if *self > Self::zero() && *other < Self::zero() {
            return false;
        }

        // Handle all zero cases
        match (*self == Self::zero(), *other == Self::zero()) {
            (true, true) => return true,
            (true, false) => return false,
            (false, true) => return false,
            _ => {}
        }

        // abs
        let (s, o) = if *self < Self::zero() {
            (-*self, -*other)
        } else {
            (*self, *other)
        };

        let (smaller, larger) = if s < o { (s, o) } else { (o, s) };
        let difference: Self = larger.sub(smaller);
        let actual_ratio: Self = difference.div(larger);
        actual_ratio < ratio
    }

    /// This method tests if `self` and `other` are not nearly equal by bounding the
    /// difference between them to some number much less than the larger of the two.
    /// This bound is set as the ratio of the difference to the larger.
    #[inline]
    fn approx_ne_ratio(&self, other: &Self, ratio: Self) -> bool {
        !self.approx_eq_ratio(other, ratio)
    }
}

impl ApproxEqRatio for f32 {}

#[test]
fn f32_approx_eq_ratio_test1() {
    let x: f32 = 0.00004_f32;
    let y: f32 = 0.00004001_f32;
    assert!(x.approx_eq_ratio(&y, 0.00025));
    assert!(y.approx_eq_ratio(&x, 0.00025));
    assert!(x.approx_ne_ratio(&y, 0.00024));
    assert!(y.approx_ne_ratio(&x, 0.00024));
}

#[test]
fn f32_approx_eq_ratio_test2() {
    let x: f32 = 0.00000000001_f32;
    let y: f32 = 0.00000000005_f32;
    assert!(x.approx_eq_ratio(&y, 0.81));
    assert!(y.approx_ne_ratio(&x, 0.79));
}

#[test]
fn f32_approx_eq_ratio_test_zero_eq_zero_returns_true() {
    let x: f32 = 0.0_f32;
    assert!(x.approx_eq_ratio(&x, 0.1) == true);
}

#[test]
fn f32_approx_eq_ratio_test_zero_ne_zero_returns_false() {
    let x: f32 = 0.0_f32;
    assert!(x.approx_ne_ratio(&x, 0.1) == false);
}

#[test]
fn f32_approx_eq_ratio_test_against_a_zero_is_false() {
    let x: f32 = 0.0_f32;
    let y: f32 = 0.1_f32;
    assert!(x.approx_eq_ratio(&y, 0.1) == false);
    assert!(y.approx_eq_ratio(&x, 0.1) == false);
}

#[test]
fn f32_approx_eq_ratio_test_negative_numbers() {
    let x: f32 = -3.0_f32;
    let y: f32 = -4.0_f32;
    // -3 and -4 should not be equal at a ratio of 0.1
    assert!(x.approx_eq_ratio(&y, 0.1) == false);
}

impl ApproxEqRatio for f64 {}

#[test]
fn f64_approx_eq_ratio_test1() {
    let x: f64 = 0.000000004_f64;
    let y: f64 = 0.000000004001_f64;
    assert!(x.approx_eq_ratio(&y, 0.00025));
    assert!(y.approx_eq_ratio(&x, 0.00025));
    assert!(x.approx_ne_ratio(&y, 0.00024));
    assert!(y.approx_ne_ratio(&x, 0.00024));
}

#[test]
fn f64_approx_eq_ratio_test2() {
    let x: f64 = 0.0000000000000001_f64;
    let y: f64 = 0.0000000000000005_f64;
    assert!(x.approx_eq_ratio(&y, 0.81));
    assert!(y.approx_ne_ratio(&x, 0.79));
}

#[test]
fn f64_approx_eq_ratio_test_zero_eq_zero_returns_true() {
    let x: f64 = 0.0_f64;
    assert!(x.approx_eq_ratio(&x, 0.1) == true);
}

#[test]
fn f64_approx_eq_ratio_test_zero_ne_zero_returns_false() {
    let x: f64 = 0.0_f64;
    assert!(x.approx_ne_ratio(&x, 0.1) == false);
}

#[test]
fn f64_approx_eq_ratio_test_negative_numbers() {
    let x: f64 = -3.0_f64;
    let y: f64 = -4.0_f64;
    // -3 and -4 should not be equal at a ratio of 0.1
    assert!(x.approx_eq_ratio(&y, 0.1) == false);
}
