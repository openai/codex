// Copyright 2018-2020 Developers of the Rand project.
// Copyright 2017 The Rust Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! `UniformFloat` implementation

use super::{Error, SampleBorrow, SampleUniform, UniformSampler};
use crate::distr::float::IntoFloat;
use crate::distr::utils::{BoolAsSIMD, FloatAsSIMD, FloatSIMDUtils, IntAsSIMD};
use crate::Rng;

#[cfg(feature = "simd_support")]
use core::simd::prelude::*;
// #[cfg(feature = "simd_support")]
// use core::simd::{LaneCount, SupportedLaneCount};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// The back-end implementing [`UniformSampler`] for floating-point types.
///
/// Unless you are implementing [`UniformSampler`] for your own type, this type
/// should not be used directly, use [`Uniform`] instead.
///
/// # Implementation notes
///
/// `UniformFloat` implementations convert RNG output to a float in the range
/// `[1, 2)` via transmutation, map this to `[0, 1)`, then scale and translate
/// to the desired range. Values produced this way have what equals 23 bits of
/// random digits for an `f32` and 52 for an `f64`.
///
/// # Bias and range errors
///
/// Bias may be expected within the least-significant bit of the significand.
/// It is not guaranteed that exclusive limits of a range are respected; i.e.
/// when sampling the range `[a, b)` it is not guaranteed that `b` is never
/// sampled.
///
/// [`new`]: UniformSampler::new
/// [`new_inclusive`]: UniformSampler::new_inclusive
/// [`StandardUniform`]: crate::distr::StandardUniform
/// [`Uniform`]: super::Uniform
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct UniformFloat<X> {
    low: X,
    scale: X,
}

macro_rules! uniform_float_impl {
    ($($meta:meta)?, $ty:ty, $uty:ident, $f_scalar:ident, $u_scalar:ident, $bits_to_discard:expr) => {
        $(#[cfg($meta)])?
        impl UniformFloat<$ty> {
            /// Construct, reducing `scale` as required to ensure that rounding
            /// can never yield values greater than `high`.
            ///
            /// Note: though it may be tempting to use a variant of this method
            /// to ensure that samples from `[low, high)` are always strictly
            /// less than `high`, this approach may be very slow where
            /// `scale.abs()` is much smaller than `high.abs()`
            /// (example: `low=0.99999999997819644, high=1.`).
            fn new_bounded(low: $ty, high: $ty, mut scale: $ty) -> Self {
                let max_rand = <$ty>::splat(1.0 as $f_scalar - $f_scalar::EPSILON);

                loop {
                    let mask = (scale * max_rand + low).gt_mask(high);
                    if !mask.any() {
                        break;
                    }
                    scale = scale.decrease_masked(mask);
                }

                debug_assert!(<$ty>::splat(0.0).all_le(scale));

                UniformFloat { low, scale }
            }
        }

        $(#[cfg($meta)])?
        impl SampleUniform for $ty {
            type Sampler = UniformFloat<$ty>;
        }

        $(#[cfg($meta)])?
        impl UniformSampler for UniformFloat<$ty> {
            type X = $ty;

            fn new<B1, B2>(low_b: B1, high_b: B2) -> Result<Self, Error>
            where
                B1: SampleBorrow<Self::X> + Sized,
                B2: SampleBorrow<Self::X> + Sized,
            {
                let low = *low_b.borrow();
                let high = *high_b.borrow();
                #[cfg(debug_assertions)]
                if !(low.all_finite()) || !(high.all_finite()) {
                    return Err(Error::NonFinite);
                }
                if !(low.all_lt(high)) {
                    return Err(Error::EmptyRange);
                }

                let scale = high - low;
                if !(scale.all_finite()) {
                    return Err(Error::NonFinite);
                }

                Ok(Self::new_bounded(low, high, scale))
            }

            fn new_inclusive<B1, B2>(low_b: B1, high_b: B2) -> Result<Self, Error>
            where
                B1: SampleBorrow<Self::X> + Sized,
                B2: SampleBorrow<Self::X> + Sized,
            {
                let low = *low_b.borrow();
                let high = *high_b.borrow();
                #[cfg(debug_assertions)]
                if !(low.all_finite()) || !(high.all_finite()) {
                    return Err(Error::NonFinite);
                }
                if !low.all_le(high) {
                    return Err(Error::EmptyRange);
                }

                let max_rand = <$ty>::splat(1.0 as $f_scalar - $f_scalar::EPSILON);
                let scale = (high - low) / max_rand;
                if !scale.all_finite() {
                    return Err(Error::NonFinite);
                }

                Ok(Self::new_bounded(low, high, scale))
            }

            fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Self::X {
                // Generate a value in the range [1, 2)
                let value1_2 = (rng.random::<$uty>() >> $uty::splat($bits_to_discard)).into_float_with_exponent(0);

                // Get a value in the range [0, 1) to avoid overflow when multiplying by scale
                let value0_1 = value1_2 - <$ty>::splat(1.0);

                // We don't use `f64::mul_add`, because it is not available with
                // `no_std`. Furthermore, it is slower for some targets (but
                // faster for others). However, the order of multiplication and
                // addition is important, because on some platforms (e.g. ARM)
                // it will be optimized to a single (non-FMA) instruction.
                value0_1 * self.scale + self.low
            }

            #[inline]
            fn sample_single<R: Rng + ?Sized, B1, B2>(low_b: B1, high_b: B2, rng: &mut R) -> Result<Self::X, Error>
            where
                B1: SampleBorrow<Self::X> + Sized,
                B2: SampleBorrow<Self::X> + Sized,
            {
                Self::sample_single_inclusive(low_b, high_b, rng)
            }

            #[inline]
            fn sample_single_inclusive<R: Rng + ?Sized, B1, B2>(low_b: B1, high_b: B2, rng: &mut R) -> Result<Self::X, Error>
            where
                B1: SampleBorrow<Self::X> + Sized,
                B2: SampleBorrow<Self::X> + Sized,
            {
                let low = *low_b.borrow();
                let high = *high_b.borrow();
                #[cfg(debug_assertions)]
                if !low.all_finite() || !high.all_finite() {
                    return Err(Error::NonFinite);
                }
                if !low.all_le(high) {
                    return Err(Error::EmptyRange);
                }
                let scale = high - low;
                if !scale.all_finite() {
                    return Err(Error::NonFinite);
                }

                // Generate a value in the range [1, 2)
                let value1_2 =
                    (rng.random::<$uty>() >> $uty::splat($bits_to_discard)).into_float_with_exponent(0);

                // Get a value in the range [0, 1) to avoid overflow when multiplying by scale
                let value0_1 = value1_2 - <$ty>::splat(1.0);

                // Doing multiply before addition allows some architectures
                // to use a single instruction.
                Ok(value0_1 * scale + low)
            }
        }
    };
}

uniform_float_impl! { , f32, u32, f32, u32, 32 - 23 }
uniform_float_impl! { , f64, u64, f64, u64, 64 - 52 }

#[cfg(feature = "simd_support")]
uniform_float_impl! { feature = "simd_support", f32x2, u32x2, f32, u32, 32 - 23 }
#[cfg(feature = "simd_support")]
uniform_float_impl! { feature = "simd_support", f32x4, u32x4, f32, u32, 32 - 23 }
#[cfg(feature = "simd_support")]
uniform_float_impl! { feature = "simd_support", f32x8, u32x8, f32, u32, 32 - 23 }
#[cfg(feature = "simd_support")]
uniform_float_impl! { feature = "simd_support", f32x16, u32x16, f32, u32, 32 - 23 }

#[cfg(feature = "simd_support")]
uniform_float_impl! { feature = "simd_support", f64x2, u64x2, f64, u64, 64 - 52 }
#[cfg(feature = "simd_support")]
uniform_float_impl! { feature = "simd_support", f64x4, u64x4, f64, u64, 64 - 52 }
#[cfg(feature = "simd_support")]
uniform_float_impl! { feature = "simd_support", f64x8, u64x8, f64, u64, 64 - 52 }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::distr::{utils::FloatSIMDScalarUtils, Uniform};
    use crate::test::{const_rng, step_rng};

    #[test]
    #[cfg_attr(miri, ignore)] // Miri is too slow
    fn test_floats() {
        let mut rng = crate::test::rng(252);
        let mut zero_rng = const_rng(0);
        let mut max_rng = const_rng(0xffff_ffff_ffff_ffff);
        macro_rules! t {
            ($ty:ty, $f_scalar:ident, $bits_shifted:expr) => {{
                let v: &[($f_scalar, $f_scalar)] = &[
                    (0.0, 100.0),
                    (-1e35, -1e25),
                    (1e-35, 1e-25),
                    (-1e35, 1e35),
                    (<$f_scalar>::from_bits(0), <$f_scalar>::from_bits(3)),
                    (-<$f_scalar>::from_bits(10), -<$f_scalar>::from_bits(1)),
                    (-<$f_scalar>::from_bits(5), 0.0),
                    (-<$f_scalar>::from_bits(7), -0.0),
                    (0.1 * $f_scalar::MAX, $f_scalar::MAX),
                    (-$f_scalar::MAX * 0.2, $f_scalar::MAX * 0.7),
                ];
                for &(low_scalar, high_scalar) in v.iter() {
                    for lane in 0..<$ty>::LEN {
                        let low = <$ty>::splat(0.0 as $f_scalar).replace(lane, low_scalar);
                        let high = <$ty>::splat(1.0 as $f_scalar).replace(lane, high_scalar);
                        let my_uniform = Uniform::new(low, high).unwrap();
                        let my_incl_uniform = Uniform::new_inclusive(low, high).unwrap();
                        for _ in 0..100 {
                            let v = rng.sample(my_uniform).extract_lane(lane);
                            assert!(low_scalar <= v && v <= high_scalar);
                            let v = rng.sample(my_incl_uniform).extract_lane(lane);
                            assert!(low_scalar <= v && v <= high_scalar);
                            let v =
                                <$ty as SampleUniform>::Sampler::sample_single(low, high, &mut rng)
                                    .unwrap()
                                    .extract_lane(lane);
                            assert!(low_scalar <= v && v <= high_scalar);
                            let v = <$ty as SampleUniform>::Sampler::sample_single_inclusive(
                                low, high, &mut rng,
                            )
                            .unwrap()
                            .extract_lane(lane);
                            assert!(low_scalar <= v && v <= high_scalar);
                        }

                        assert_eq!(
                            rng.sample(Uniform::new_inclusive(low, low).unwrap())
                                .extract_lane(lane),
                            low_scalar
                        );

                        assert_eq!(zero_rng.sample(my_uniform).extract_lane(lane), low_scalar);
                        assert_eq!(
                            zero_rng.sample(my_incl_uniform).extract_lane(lane),
                            low_scalar
                        );
                        assert_eq!(
                            <$ty as SampleUniform>::Sampler::sample_single(
                                low,
                                high,
                                &mut zero_rng
                            )
                            .unwrap()
                            .extract_lane(lane),
                            low_scalar
                        );
                        assert_eq!(
                            <$ty as SampleUniform>::Sampler::sample_single_inclusive(
                                low,
                                high,
                                &mut zero_rng
                            )
                            .unwrap()
                            .extract_lane(lane),
                            low_scalar
                        );

                        assert!(max_rng.sample(my_uniform).extract_lane(lane) <= high_scalar);
                        assert!(max_rng.sample(my_incl_uniform).extract_lane(lane) <= high_scalar);
                        // sample_single cannot cope with max_rng:
                        // assert!(<$ty as SampleUniform>::Sampler
                        //     ::sample_single(low, high, &mut max_rng).unwrap()
                        //     .extract(lane) <= high_scalar);
                        assert!(
                            <$ty as SampleUniform>::Sampler::sample_single_inclusive(
                                low,
                                high,
                                &mut max_rng
                            )
                            .unwrap()
                            .extract_lane(lane)
                                <= high_scalar
                        );

                        // Don't run this test for really tiny differences between high and low
                        // since for those rounding might result in selecting high for a very
                        // long time.
                        if (high_scalar - low_scalar) > 0.0001 {
                            let mut lowering_max_rng =
                                step_rng(0xffff_ffff_ffff_ffff, (-1i64 << $bits_shifted) as u64);
                            assert!(
                                <$ty as SampleUniform>::Sampler::sample_single(
                                    low,
                                    high,
                                    &mut lowering_max_rng
                                )
                                .unwrap()
                                .extract_lane(lane)
                                    <= high_scalar
                            );
                        }
                    }
                }

                assert_eq!(
                    rng.sample(Uniform::new_inclusive($f_scalar::MAX, $f_scalar::MAX).unwrap()),
                    $f_scalar::MAX
                );
                assert_eq!(
                    rng.sample(Uniform::new_inclusive(-$f_scalar::MAX, -$f_scalar::MAX).unwrap()),
                    -$f_scalar::MAX
                );
            }};
        }

        t!(f32, f32, 32 - 23);
        t!(f64, f64, 64 - 52);
        #[cfg(feature = "simd_support")]
        {
            t!(f32x2, f32, 32 - 23);
            t!(f32x4, f32, 32 - 23);
            t!(f32x8, f32, 32 - 23);
            t!(f32x16, f32, 32 - 23);
            t!(f64x2, f64, 64 - 52);
            t!(f64x4, f64, 64 - 52);
            t!(f64x8, f64, 64 - 52);
        }
    }

    #[test]
    fn test_float_overflow() {
        assert_eq!(Uniform::try_from(f64::MIN..f64::MAX), Err(Error::NonFinite));
    }

    #[test]
    #[should_panic]
    fn test_float_overflow_single() {
        let mut rng = crate::test::rng(252);
        rng.random_range(f64::MIN..f64::MAX);
    }

    #[test]
    #[cfg(all(feature = "std", panic = "unwind"))]
    fn test_float_assertions() {
        use super::SampleUniform;
        fn range<T: SampleUniform>(low: T, high: T) -> Result<T, Error> {
            let mut rng = crate::test::rng(253);
            T::Sampler::sample_single(low, high, &mut rng)
        }

        macro_rules! t {
            ($ty:ident, $f_scalar:ident) => {{
                let v: &[($f_scalar, $f_scalar)] = &[
                    ($f_scalar::NAN, 0.0),
                    (1.0, $f_scalar::NAN),
                    ($f_scalar::NAN, $f_scalar::NAN),
                    (1.0, 0.5),
                    ($f_scalar::MAX, -$f_scalar::MAX),
                    ($f_scalar::INFINITY, $f_scalar::INFINITY),
                    ($f_scalar::NEG_INFINITY, $f_scalar::NEG_INFINITY),
                    ($f_scalar::NEG_INFINITY, 5.0),
                    (5.0, $f_scalar::INFINITY),
                    ($f_scalar::NAN, $f_scalar::INFINITY),
                    ($f_scalar::NEG_INFINITY, $f_scalar::NAN),
                    ($f_scalar::NEG_INFINITY, $f_scalar::INFINITY),
                ];
                for &(low_scalar, high_scalar) in v.iter() {
                    for lane in 0..<$ty>::LEN {
                        let low = <$ty>::splat(0.0 as $f_scalar).replace(lane, low_scalar);
                        let high = <$ty>::splat(1.0 as $f_scalar).replace(lane, high_scalar);
                        assert!(range(low, high).is_err());
                        assert!(Uniform::new(low, high).is_err());
                        assert!(Uniform::new_inclusive(low, high).is_err());
                        assert!(Uniform::new(low, low).is_err());
                    }
                }
            }};
        }

        t!(f32, f32);
        t!(f64, f64);
        #[cfg(feature = "simd_support")]
        {
            t!(f32x2, f32);
            t!(f32x4, f32);
            t!(f32x8, f32);
            t!(f32x16, f32);
            t!(f64x2, f64);
            t!(f64x4, f64);
            t!(f64x8, f64);
        }
    }

    #[test]
    fn test_uniform_from_std_range() {
        let r = Uniform::try_from(2.0f64..7.0).unwrap();
        assert_eq!(r.0.low, 2.0);
        assert_eq!(r.0.scale, 5.0);
    }

    #[test]
    fn test_uniform_from_std_range_bad_limits() {
        #![allow(clippy::reversed_empty_ranges)]
        assert!(Uniform::try_from(100.0..10.0).is_err());
        assert!(Uniform::try_from(100.0..100.0).is_err());
    }

    #[test]
    fn test_uniform_from_std_range_inclusive() {
        let r = Uniform::try_from(2.0f64..=7.0).unwrap();
        assert_eq!(r.0.low, 2.0);
        assert!(r.0.scale > 5.0);
        assert!(r.0.scale < 5.0 + 1e-14);
    }

    #[test]
    fn test_uniform_from_std_range_inclusive_bad_limits() {
        #![allow(clippy::reversed_empty_ranges)]
        assert!(Uniform::try_from(100.0..=10.0).is_err());
        assert!(Uniform::try_from(100.0..=99.0).is_err());
    }
}
