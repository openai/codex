// Copyright 2018 Developers of the Rand project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Basic floating-point number distributions

use crate::distr::utils::{FloatAsSIMD, FloatSIMDUtils, IntAsSIMD};
use crate::distr::{Distribution, StandardUniform};
use crate::Rng;
use core::mem;
#[cfg(feature = "simd_support")]
use core::simd::prelude::*;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A distribution to sample floating point numbers uniformly in the half-open
/// interval `(0, 1]`, i.e. including 1 but not 0.
///
/// All values that can be generated are of the form `n * ε/2`. For `f32`
/// the 24 most significant random bits of a `u32` are used and for `f64` the
/// 53 most significant bits of a `u64` are used. The conversion uses the
/// multiplicative method.
///
/// See also: [`StandardUniform`] which samples from `[0, 1)`, [`Open01`]
/// which samples from `(0, 1)` and [`Uniform`] which samples from arbitrary
/// ranges.
///
/// # Example
/// ```
/// use rand::Rng;
/// use rand::distr::OpenClosed01;
///
/// let val: f32 = rand::rng().sample(OpenClosed01);
/// println!("f32 from (0, 1): {}", val);
/// ```
///
/// [`StandardUniform`]: crate::distr::StandardUniform
/// [`Open01`]: crate::distr::Open01
/// [`Uniform`]: crate::distr::uniform::Uniform
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct OpenClosed01;

/// A distribution to sample floating point numbers uniformly in the open
/// interval `(0, 1)`, i.e. not including either endpoint.
///
/// All values that can be generated are of the form `n * ε + ε/2`. For `f32`
/// the 23 most significant random bits of an `u32` are used, for `f64` 52 from
/// an `u64`. The conversion uses a transmute-based method.
///
/// See also: [`StandardUniform`] which samples from `[0, 1)`, [`OpenClosed01`]
/// which samples from `(0, 1]` and [`Uniform`] which samples from arbitrary
/// ranges.
///
/// # Example
/// ```
/// use rand::Rng;
/// use rand::distr::Open01;
///
/// let val: f32 = rand::rng().sample(Open01);
/// println!("f32 from (0, 1): {}", val);
/// ```
///
/// [`StandardUniform`]: crate::distr::StandardUniform
/// [`OpenClosed01`]: crate::distr::OpenClosed01
/// [`Uniform`]: crate::distr::uniform::Uniform
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Open01;

// This trait is needed by both this lib and rand_distr hence is a hidden export
#[doc(hidden)]
pub trait IntoFloat {
    type F;

    /// Helper method to combine the fraction and a constant exponent into a
    /// float.
    ///
    /// Only the least significant bits of `self` may be set, 23 for `f32` and
    /// 52 for `f64`.
    /// The resulting value will fall in a range that depends on the exponent.
    /// As an example the range with exponent 0 will be
    /// [2<sup>0</sup>..2<sup>1</sup>), which is [1..2).
    fn into_float_with_exponent(self, exponent: i32) -> Self::F;
}

macro_rules! float_impls {
    ($($meta:meta)?, $ty:ident, $uty:ident, $f_scalar:ident, $u_scalar:ty,
     $fraction_bits:expr, $exponent_bias:expr) => {
        $(#[cfg($meta)])?
        impl IntoFloat for $uty {
            type F = $ty;
            #[inline(always)]
            fn into_float_with_exponent(self, exponent: i32) -> $ty {
                // The exponent is encoded using an offset-binary representation
                let exponent_bits: $u_scalar =
                    (($exponent_bias + exponent) as $u_scalar) << $fraction_bits;
                $ty::from_bits(self | $uty::splat(exponent_bits))
            }
        }

        $(#[cfg($meta)])?
        impl Distribution<$ty> for StandardUniform {
            fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> $ty {
                // Multiply-based method; 24/53 random bits; [0, 1) interval.
                // We use the most significant bits because for simple RNGs
                // those are usually more random.
                let float_size = mem::size_of::<$f_scalar>() as $u_scalar * 8;
                let precision = $fraction_bits + 1;
                let scale = 1.0 / ((1 as $u_scalar << precision) as $f_scalar);

                let value: $uty = rng.random();
                let value = value >> $uty::splat(float_size - precision);
                $ty::splat(scale) * $ty::cast_from_int(value)
            }
        }

        $(#[cfg($meta)])?
        impl Distribution<$ty> for OpenClosed01 {
            fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> $ty {
                // Multiply-based method; 24/53 random bits; (0, 1] interval.
                // We use the most significant bits because for simple RNGs
                // those are usually more random.
                let float_size = mem::size_of::<$f_scalar>() as $u_scalar * 8;
                let precision = $fraction_bits + 1;
                let scale = 1.0 / ((1 as $u_scalar << precision) as $f_scalar);

                let value: $uty = rng.random();
                let value = value >> $uty::splat(float_size - precision);
                // Add 1 to shift up; will not overflow because of right-shift:
                $ty::splat(scale) * $ty::cast_from_int(value + $uty::splat(1))
            }
        }

        $(#[cfg($meta)])?
        impl Distribution<$ty> for Open01 {
            fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> $ty {
                // Transmute-based method; 23/52 random bits; (0, 1) interval.
                // We use the most significant bits because for simple RNGs
                // those are usually more random.
                let float_size = mem::size_of::<$f_scalar>() as $u_scalar * 8;

                let value: $uty = rng.random();
                let fraction = value >> $uty::splat(float_size - $fraction_bits);
                fraction.into_float_with_exponent(0) - $ty::splat(1.0 - $f_scalar::EPSILON / 2.0)
            }
        }
    }
}

float_impls! { , f32, u32, f32, u32, 23, 127 }
float_impls! { , f64, u64, f64, u64, 52, 1023 }

#[cfg(feature = "simd_support")]
float_impls! { feature = "simd_support", f32x2, u32x2, f32, u32, 23, 127 }
#[cfg(feature = "simd_support")]
float_impls! { feature = "simd_support", f32x4, u32x4, f32, u32, 23, 127 }
#[cfg(feature = "simd_support")]
float_impls! { feature = "simd_support", f32x8, u32x8, f32, u32, 23, 127 }
#[cfg(feature = "simd_support")]
float_impls! { feature = "simd_support", f32x16, u32x16, f32, u32, 23, 127 }

#[cfg(feature = "simd_support")]
float_impls! { feature = "simd_support", f64x2, u64x2, f64, u64, 52, 1023 }
#[cfg(feature = "simd_support")]
float_impls! { feature = "simd_support", f64x4, u64x4, f64, u64, 52, 1023 }
#[cfg(feature = "simd_support")]
float_impls! { feature = "simd_support", f64x8, u64x8, f64, u64, 52, 1023 }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::const_rng;

    const EPSILON32: f32 = f32::EPSILON;
    const EPSILON64: f64 = f64::EPSILON;

    macro_rules! test_f32 {
        ($fnn:ident, $ty:ident, $ZERO:expr, $EPSILON:expr) => {
            #[test]
            fn $fnn() {
                let two = $ty::splat(2.0);

                // StandardUniform
                let mut zeros = const_rng(0);
                assert_eq!(zeros.random::<$ty>(), $ZERO);
                let mut one = const_rng(1 << 8 | 1 << (8 + 32));
                assert_eq!(one.random::<$ty>(), $EPSILON / two);
                let mut max = const_rng(!0);
                assert_eq!(max.random::<$ty>(), $ty::splat(1.0) - $EPSILON / two);

                // OpenClosed01
                let mut zeros = const_rng(0);
                assert_eq!(zeros.sample::<$ty, _>(OpenClosed01), $ZERO + $EPSILON / two);
                let mut one = const_rng(1 << 8 | 1 << (8 + 32));
                assert_eq!(one.sample::<$ty, _>(OpenClosed01), $EPSILON);
                let mut max = const_rng(!0);
                assert_eq!(max.sample::<$ty, _>(OpenClosed01), $ZERO + $ty::splat(1.0));

                // Open01
                let mut zeros = const_rng(0);
                assert_eq!(zeros.sample::<$ty, _>(Open01), $ZERO + $EPSILON / two);
                let mut one = const_rng(1 << 9 | 1 << (9 + 32));
                assert_eq!(
                    one.sample::<$ty, _>(Open01),
                    $EPSILON / two * $ty::splat(3.0)
                );
                let mut max = const_rng(!0);
                assert_eq!(
                    max.sample::<$ty, _>(Open01),
                    $ty::splat(1.0) - $EPSILON / two
                );
            }
        };
    }
    test_f32! { f32_edge_cases, f32, 0.0, EPSILON32 }
    #[cfg(feature = "simd_support")]
    test_f32! { f32x2_edge_cases, f32x2, f32x2::splat(0.0), f32x2::splat(EPSILON32) }
    #[cfg(feature = "simd_support")]
    test_f32! { f32x4_edge_cases, f32x4, f32x4::splat(0.0), f32x4::splat(EPSILON32) }
    #[cfg(feature = "simd_support")]
    test_f32! { f32x8_edge_cases, f32x8, f32x8::splat(0.0), f32x8::splat(EPSILON32) }
    #[cfg(feature = "simd_support")]
    test_f32! { f32x16_edge_cases, f32x16, f32x16::splat(0.0), f32x16::splat(EPSILON32) }

    macro_rules! test_f64 {
        ($fnn:ident, $ty:ident, $ZERO:expr, $EPSILON:expr) => {
            #[test]
            fn $fnn() {
                let two = $ty::splat(2.0);

                // StandardUniform
                let mut zeros = const_rng(0);
                assert_eq!(zeros.random::<$ty>(), $ZERO);
                let mut one = const_rng(1 << 11);
                assert_eq!(one.random::<$ty>(), $EPSILON / two);
                let mut max = const_rng(!0);
                assert_eq!(max.random::<$ty>(), $ty::splat(1.0) - $EPSILON / two);

                // OpenClosed01
                let mut zeros = const_rng(0);
                assert_eq!(zeros.sample::<$ty, _>(OpenClosed01), $ZERO + $EPSILON / two);
                let mut one = const_rng(1 << 11);
                assert_eq!(one.sample::<$ty, _>(OpenClosed01), $EPSILON);
                let mut max = const_rng(!0);
                assert_eq!(max.sample::<$ty, _>(OpenClosed01), $ZERO + $ty::splat(1.0));

                // Open01
                let mut zeros = const_rng(0);
                assert_eq!(zeros.sample::<$ty, _>(Open01), $ZERO + $EPSILON / two);
                let mut one = const_rng(1 << 12);
                assert_eq!(
                    one.sample::<$ty, _>(Open01),
                    $EPSILON / two * $ty::splat(3.0)
                );
                let mut max = const_rng(!0);
                assert_eq!(
                    max.sample::<$ty, _>(Open01),
                    $ty::splat(1.0) - $EPSILON / two
                );
            }
        };
    }
    test_f64! { f64_edge_cases, f64, 0.0, EPSILON64 }
    #[cfg(feature = "simd_support")]
    test_f64! { f64x2_edge_cases, f64x2, f64x2::splat(0.0), f64x2::splat(EPSILON64) }
    #[cfg(feature = "simd_support")]
    test_f64! { f64x4_edge_cases, f64x4, f64x4::splat(0.0), f64x4::splat(EPSILON64) }
    #[cfg(feature = "simd_support")]
    test_f64! { f64x8_edge_cases, f64x8, f64x8::splat(0.0), f64x8::splat(EPSILON64) }

    #[test]
    fn value_stability() {
        fn test_samples<T: Copy + core::fmt::Debug + PartialEq, D: Distribution<T>>(
            distr: &D,
            zero: T,
            expected: &[T],
        ) {
            let mut rng = crate::test::rng(0x6f44f5646c2a7334);
            let mut buf = [zero; 3];
            for x in &mut buf {
                *x = rng.sample(distr);
            }
            assert_eq!(&buf, expected);
        }

        test_samples(
            &StandardUniform,
            0f32,
            &[0.0035963655, 0.7346052, 0.09778172],
        );
        test_samples(
            &StandardUniform,
            0f64,
            &[0.7346051961657583, 0.20298547462974248, 0.8166436635290655],
        );

        test_samples(&OpenClosed01, 0f32, &[0.003596425, 0.73460525, 0.09778178]);
        test_samples(
            &OpenClosed01,
            0f64,
            &[0.7346051961657584, 0.2029854746297426, 0.8166436635290656],
        );

        test_samples(&Open01, 0f32, &[0.0035963655, 0.73460525, 0.09778172]);
        test_samples(
            &Open01,
            0f64,
            &[0.7346051961657584, 0.20298547462974248, 0.8166436635290656],
        );

        #[cfg(feature = "simd_support")]
        {
            // We only test a sub-set of types here. Values are identical to
            // non-SIMD types; we assume this pattern continues across all
            // SIMD types.

            test_samples(
                &StandardUniform,
                f32x2::from([0.0, 0.0]),
                &[
                    f32x2::from([0.0035963655, 0.7346052]),
                    f32x2::from([0.09778172, 0.20298547]),
                    f32x2::from([0.34296435, 0.81664366]),
                ],
            );

            test_samples(
                &StandardUniform,
                f64x2::from([0.0, 0.0]),
                &[
                    f64x2::from([0.7346051961657583, 0.20298547462974248]),
                    f64x2::from([0.8166436635290655, 0.7423708925400552]),
                    f64x2::from([0.16387782224016323, 0.9087068770169618]),
                ],
            );
        }
    }
}
