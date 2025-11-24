// Copyright 2014-2020 Optimal Computing (NZ) Ltd.
// Licensed under the MIT license.  See LICENSE for details.

use super::Ulps;
#[cfg(feature = "num-traits")]
#[allow(unused_imports)]
use num_traits::float::FloatCore;

/// ApproxEqUlps is a trait for approximate equality comparisons.
/// The associated type Flt is a floating point type which implements Ulps, and is
/// required so that this trait can be implemented for compound types (e.g. vectors),
/// not just for the floats themselves.
pub trait ApproxEqUlps {
    type Flt: Ulps;

    /// This method tests for `self` and `other` values to be approximately equal
    /// within ULPs (Units of Least Precision) floating point representations.
    /// Differing signs are always unequal with this method, and zeroes are only
    /// equal to zeroes. Use approx_eq() from the ApproxEq trait if that is more
    /// appropriate.
    fn approx_eq_ulps(&self, other: &Self, ulps: <Self::Flt as Ulps>::U) -> bool;

    /// This method tests for `self` and `other` values to be not approximately
    /// equal within ULPs (Units of Least Precision) floating point representations.
    /// Differing signs are always unequal with this method, and zeroes are only
    /// equal to zeroes. Use approx_eq() from the ApproxEq trait if that is more
    /// appropriate.
    #[inline]
    fn approx_ne_ulps(&self, other: &Self, ulps: <Self::Flt as Ulps>::U) -> bool {
        !self.approx_eq_ulps(other, ulps)
    }
}

impl ApproxEqUlps for f32 {
    type Flt = f32;

    fn approx_eq_ulps(&self, other: &f32, ulps: i32) -> bool {
        // -0 and +0 are drastically far in ulps terms, so
        // we need a special case for that.
        if *self == *other {
            return true;
        }

        // Handle differing signs as a special case, even if
        // they are very close, most people consider them
        // unequal.
        if self.is_sign_positive() != other.is_sign_positive() {
            return false;
        }

        let diff: i32 = self.ulps(other);
        diff >= -ulps && diff <= ulps
    }
}

#[test]
fn f32_approx_eq_ulps_test1() {
    let f: f32 = 0.1_f32;
    let mut sum: f32 = 0.0_f32;
    for _ in 0_isize..10_isize {
        sum += f;
    }
    let product: f32 = f * 10.0_f32;
    assert!(sum != product); // Should not be directly equal:
    assert!(sum.approx_eq_ulps(&product, 1) == true); // But should be close
    assert!(sum.approx_eq_ulps(&product, 0) == false);
}
#[test]
fn f32_approx_eq_ulps_test2() {
    let x: f32 = 1000000_f32;
    let y: f32 = 1000000.1_f32;
    assert!(x != y); // Should not be directly equal
    assert!(x.approx_eq_ulps(&y, 2) == true);
    assert!(x.approx_eq_ulps(&y, 1) == false);
}
#[test]
fn f32_approx_eq_ulps_test_zeroes() {
    let x: f32 = 0.0_f32;
    let y: f32 = -0.0_f32;
    assert!(x.approx_eq_ulps(&y, 0) == true);
}

impl ApproxEqUlps for f64 {
    type Flt = f64;

    fn approx_eq_ulps(&self, other: &f64, ulps: i64) -> bool {
        // -0 and +0 are drastically far in ulps terms, so
        // we need a special case for that.
        if *self == *other {
            return true;
        }

        // Handle differing signs as a special case, even if
        // they are very close, most people consider them
        // unequal.
        if self.is_sign_positive() != other.is_sign_positive() {
            return false;
        }

        let diff: i64 = self.ulps(other);
        diff >= -ulps && diff <= ulps
    }
}

#[test]
fn f64_approx_eq_ulps_test1() {
    let f: f64 = 0.1_f64;
    let mut sum: f64 = 0.0_f64;
    for _ in 0_isize..10_isize {
        sum += f;
    }
    let product: f64 = f * 10.0_f64;
    assert!(sum != product); // Should not be directly equal:
    assert!(sum.approx_eq_ulps(&product, 1) == true); // But should be close
    assert!(sum.approx_eq_ulps(&product, 0) == false);
}
#[test]
fn f64_approx_eq_ulps_test2() {
    let x: f64 = 1000000_f64;
    let y: f64 = 1000000.0000000003_f64;
    assert!(x != y); // Should not be directly equal
    assert!(x.approx_eq_ulps(&y, 3) == true);
    assert!(x.approx_eq_ulps(&y, 2) == false);
}
#[test]
fn f64_approx_eq_ulps_test_zeroes() {
    let x: f64 = 0.0_f64;
    let y: f64 = -0.0_f64;
    assert!(x.approx_eq_ulps(&y, 0) == true);
}
